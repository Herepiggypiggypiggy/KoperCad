//! Coupling of the radiosity solution with the conduction in the solids.
//!
//! The flow core owns a [`RadPlan`] (patches, view factors, which patch is a
//! device, the plate or a wall) and a [`RadState`]. Every few iterations it
//! calls [`RadPlan::update`] with the cell temperatures and gets a
//! [`RadLoad`] back: per cell and per plate column the radiative heat as a
//! linear function of the own temperature, `heat = s - g T`, so that the
//! energy equation takes the slope implicitly and stays stable, and the part
//! of the wall absorption that leaves the cabinet.
//!
//! What the surfaces are:
//! - a **device** face takes the temperature of its (lumped, nearly
//!   isothermal) solid cell;
//! - the **plate** takes the temperature of its sheet column;
//! - an **enclosure wall** face is a surface node between the air and the
//!   outside: inner film `h_in = r U`, the rest `U' = U r / (r - 1)` in
//!   series with it, so that the two give the overall coefficient `U` the
//!   flow solver already applies air to air. Absorbed radiation then splits
//!   by the film ratio: `(r - 1) / r` returns to the air, the rest leaves
//!   through the wall. The ratio `r` is a planning value.

use super::radiosity::RadResult;
use super::{RadiationModel, patches::PatchKind};
use crate::result::Grid3;

/// `h_in / U` of an enclosure wall (see the module docs).
pub const WALL_INNER_RATIO: f64 = 1.5;
/// Inner film of a wall with no coefficient to the outside, W/m2K.
const WALL_H_MIN: f64 = 3.0;
/// Share of a new flux that replaces the old one per update.
const RELAX: f64 = 0.7;

/// What the energy equation knows of a domain wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WallBc {
    /// Overall air-to-air coefficient to the outside, W/m2K of the interior
    /// face, and the outside temperature, degrees C. `u = 0` is adiabatic.
    Overall { u: f64, t_out: f64 },
    /// Fixed wall surface temperature, degrees C.
    Fixed(f64),
}

/// The fixed part: built once per grid.
#[derive(Debug, Clone)]
pub struct RadPlan {
    pub model: RadiationModel,
    pub grid: Grid3,
}

/// The part that changes: the last flux of every patch, W/m2.
#[derive(Debug, Clone)]
pub struct RadState {
    q_w_m2: Vec<f64>,
    /// Temperature (degrees C) and slope `4 e sigma T^3` of every patch at
    /// the last update: a wall node takes one Newton step from them, which
    /// keeps it stable (an explicit lag diverges when the radiative slope
    /// exceeds the inner film).
    t_c: Vec<f64>,
    h_w_m2k: Vec<f64>,
}

/// What an update needs from the flow solver.
pub struct RadInputs<'a> {
    /// Cell temperatures, degrees C.
    pub t_c: &'a [f64],
    /// Plate sheet temperatures per column, degrees C, if the plate is a sheet.
    pub sheet_t_c: Option<&'a [f64]>,
    /// The six domain faces in the order of `Face`.
    pub walls: [WallBc; 6],
}

/// Radiative heat in the form the energy equation takes it.
#[derive(Debug, Clone, PartialEq)]
pub struct RadLoad {
    /// Heat into each cell is `cell_s - cell_g * T`, W (T in degrees C).
    pub cell_s: Vec<f64>,
    pub cell_g: Vec<f64>,
    /// Same for the plate sheet columns.
    pub sheet_s: Vec<f64>,
    pub sheet_g: Vec<f64>,
    /// Absorbed by a wall and carried out through it, W per domain face.
    pub wall_out_w: [f64; 6],
    /// Net radiative power given off per material id (0 = wall), W.
    pub power_by_material: Vec<(u16, f64)>,
    /// Solver residual of the radiosity system and its energy imbalance, W.
    pub residual: f64,
    pub imbalance_w: f64,
}

impl RadLoad {
    pub fn zero(cells: usize, columns: usize) -> RadLoad {
        RadLoad {
            cell_s: vec![0.0; cells],
            cell_g: vec![0.0; cells],
            sheet_s: vec![0.0; columns],
            sheet_g: vec![0.0; columns],
            wall_out_w: [0.0; 6],
            power_by_material: Vec::new(),
            residual: 0.0,
            imbalance_w: 0.0,
        }
    }

    /// Net radiative power of a material, W (positive = gives off heat).
    pub fn power_w(&self, material: u16) -> f64 {
        self.power_by_material
            .iter()
            .find(|p| p.0 == material)
            .map_or(0.0, |p| p.1)
    }

    /// Heat the load puts into cells and sheet plus what the walls carry out, at the
    /// temperatures `t_c` and `sheet_t_c`, W. Zero up to the linearisation
    /// error of the last update: the radiation only moves heat around.
    pub fn net_heat_w(&self, t_c: &[f64], sheet_t_c: &[f64]) -> f64 {
        let cells: f64 = (0..t_c.len())
            .map(|i| self.cell_s[i] - self.cell_g[i] * t_c[i])
            .sum();
        let sheet: f64 = (0..sheet_t_c.len())
            .map(|c| self.sheet_s[c] - self.sheet_g[c] * sheet_t_c[c])
            .sum();
        cells + sheet + self.wall_out_w.iter().sum::<f64>()
    }
}

impl RadPlan {
    pub fn new(model: RadiationModel, grid: Grid3) -> RadPlan {
        RadPlan { model, grid }
    }

    pub fn state(&self) -> RadState {
        RadState {
            q_w_m2: vec![0.0; self.model.patches.len()],
            t_c: vec![20.0; self.model.patches.len()],
            h_w_m2k: vec![0.0; self.model.patches.len()],
        }
    }

    fn face_area_m2(&self, f: super::patches::FaceRef) -> f64 {
        super::patches::face_area_mm2(&self.grid, f) * 1e-6
    }

    /// Surface temperature of every patch, kelvin, and for wall patches the
    /// share of the absorbed heat that returns to the air.
    fn patch_temperatures(&self, st: &RadState, inp: &RadInputs) -> (Vec<f64>, Vec<f64>) {
        let ps = &self.model.patches;
        let g = &self.grid;
        let cols = g.nx * g.ny;
        let mut t_k = Vec::with_capacity(ps.len());
        let mut f_air = vec![0.0; ps.len()];
        for (pi, p) in ps.patches.iter().enumerate() {
            let mean = |f: &dyn Fn(super::patches::FaceRef) -> f64| -> f64 {
                let (mut num, mut den) = (0.0, 0.0);
                for i in p.faces.clone() {
                    let fr = ps.faces[i];
                    let a = self.face_area_m2(fr);
                    num += f(fr) * a;
                    den += a;
                }
                num / den
            };
            let t_c = match p.kind {
                PatchKind::Solid => mean(&|f| inp.t_c[f.cell as usize]),
                PatchKind::Plate => match inp.sheet_t_c {
                    Some(s) => mean(&|f| s[f.cell as usize % cols]),
                    None => mean(&|f| inp.t_c[f.cell as usize]),
                },
                PatchKind::Wall => {
                    let face = ps.faces[p.faces.start].dir as usize;
                    let air = mean(&|f| inp.t_c[f.cell as usize]);
                    // Newton step of the node balance: the absorbed heat
                    // `-q` follows the surface temperature with slope `-h`.
                    let (q0, t0, h0) = (st.q_w_m2[pi], st.t_c[pi], st.h_w_m2k[pi]);
                    match inp.walls[face] {
                        WallBc::Fixed(tw) => tw,
                        WallBc::Overall { u, t_out } if u > 1e-9 => {
                            let r = WALL_INNER_RATIO;
                            let h_in = r * u;
                            let u_out = u * r / (r - 1.0);
                            f_air[pi] = (r - 1.0) / r;
                            (-q0 + h0 * t0 + h_in * air + u_out * t_out) / (h_in + u_out + h0)
                        }
                        WallBc::Overall { .. } => {
                            f_air[pi] = 1.0;
                            (-q0 + h0 * t0 + WALL_H_MIN * air) / (WALL_H_MIN + h0)
                        }
                    }
                }
            };
            t_k.push(t_c + 273.15);
        }
        (t_k, f_air)
    }

    /// One update from the present temperatures.
    pub fn update(&self, st: &mut RadState, inp: &RadInputs) -> RadLoad {
        let ps = &self.model.patches;
        let g = &self.grid;
        let (t_k, f_air) = self.patch_temperatures(st, inp);
        let res: RadResult = self.model.solve(&t_k);
        st.t_c = t_k.iter().map(|t| t - 273.15).collect();
        st.h_w_m2k = res.h_lin_w_m2k.clone();
        for (q, new) in st.q_w_m2.iter_mut().zip(&res.net_flux_w_m2) {
            *q = RELAX * new + (1.0 - RELAX) * *q;
        }
        let mut load = RadLoad::zero(g.cells(), g.nx * g.ny);
        let cols = g.nx * g.ny;
        for (pi, p) in ps.patches.iter().enumerate() {
            let q = st.q_w_m2[pi];
            let h = res.h_lin_w_m2k[pi];
            let t_c = t_k[pi] - 273.15;
            for i in p.faces.clone() {
                let fr = ps.faces[i];
                let a = self.face_area_m2(fr);
                let cell = fr.cell as usize;
                match p.kind {
                    PatchKind::Solid => {
                        load.cell_s[cell] -= a * (q - h * t_c);
                        load.cell_g[cell] += a * h;
                    }
                    PatchKind::Plate => {
                        let col = cell % cols;
                        load.sheet_s[col] -= a * (q - h * t_c);
                        load.sheet_g[col] += a * h;
                    }
                    PatchKind::Wall => {
                        let absorbed = -q * a;
                        load.cell_s[cell] += f_air[pi] * absorbed;
                        load.wall_out_w[fr.dir as usize] += (1.0 - f_air[pi]) * absorbed;
                    }
                }
            }
        }
        let mut by: Vec<(u16, f64)> = Vec::new();
        for (p, q) in ps.patches.iter().zip(&st.q_w_m2) {
            let w = q * p.area_m2;
            match by.iter_mut().find(|e| e.0 == p.material) {
                Some(e) => e.1 += w,
                None => by.push((p.material, w)),
            }
        }
        by.sort_by_key(|e| e.0);
        load.power_by_material = by;
        load.residual = res.residual;
        load.imbalance_w = st
            .q_w_m2
            .iter()
            .zip(&ps.patches)
            .map(|(q, p)| q * p.area_m2)
            .sum();
        load
    }
}
