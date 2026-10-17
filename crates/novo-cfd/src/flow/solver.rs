use super::mesh::Mesh;
use super::par::{for_planes, max_over_planes};
use super::pressure::{PressureSolver, Smoother};
use super::scheme::{Sample, Scheme, face_value};
use super::sheet::{PlateSheet, SheetGeometry};
use crate::openings::{OpenPatch, OpenSet};
use crate::solids::Medium;

mod steady;
pub use steady::{
    RunStatus, SteadyConfig, SteadyInfo, SteadyLimits, SteadyReport, SteadyRun, SteadyState,
};

/// Thermal condition on a wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalBc {
    Adiabatic,
    /// Fixed wall temperature, degrees C.
    Fixed(f64),
    /// Heat leaves through a film to air at `t_out` (degrees C):
    /// `h` is the film coefficient, W/m2K (an outside wall, IEC 60890).
    Convective {
        h: f64,
        t_out: f64,
    },
    /// Heat leaves the air next to the wall as a whole: `u` (W/m2K) is the
    /// overall coefficient from the air to air at `t_out`, inner film, sheet
    /// and outer film together (IEC 60890 `k`). It replaces the half cell of
    /// conduction in `Convective`, so the coarse inner boundary layer is not
    /// counted on top of a coefficient that already contains it.
    Overall {
        u: f64,
        t_out: f64,
    },
}

/// Condition on one of the six domain faces (all walls).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceBc {
    /// Wall velocity, m/s (only the tangential components count).
    pub wall_velocity: [f64; 3],
    /// Free slip instead of no slip (used for the unused depth of 2D cases).
    pub slip: bool,
    pub thermal: ThermalBc,
}

impl FaceBc {
    pub const WALL: FaceBc = FaceBc {
        wall_velocity: [0.0; 3],
        slip: false,
        thermal: ThermalBc::Adiabatic,
    };
    /// Symmetry plane: free slip, adiabatic.
    pub const SYMMETRY: FaceBc = FaceBc {
        wall_velocity: [0.0; 3],
        slip: true,
        thermal: ThermalBc::Adiabatic,
    };
}

/// Face names; the index is `2 * axis + (1 if the max side)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    XMin = 0,
    XMax = 1,
    YMin = 2,
    YMax = 3,
    ZMin = 4,
    ZMax = 5,
}

#[derive(Debug, Clone)]
pub struct FlowConfig {
    /// Kinematic viscosity, m2/s.
    pub nu: f64,
    /// Thermal diffusivity, m2/s.
    pub alpha: f64,
    /// Thermal conductivity of the fluid, W/mK (with `alpha` it gives
    /// rho cp, which turns film coefficients into the cell equation).
    pub conductivity: f64,
    /// Thermal expansion coefficient, 1/K.
    pub beta: f64,
    /// Gravity vector, m/s2, in the frame of the mesh (`y` down: `[0, 9.81, 0]`).
    pub gravity: [f64; 3],
    pub scheme: Scheme,
    pub faces: [FaceBc; 6],
    /// Volumetric heating per cell, K/s (heat / (rho cp)), or none.
    pub heat_source: Option<Vec<f64>>,
    /// Turbulent Prandtl number: the eddy diffusivity is `nu_t / prandtl_t`.
    pub prandtl_t: f64,
    /// Openings to the outside air. Empty = closed box.
    pub openings: Vec<OpenPatch>,
    /// Temperature of the outside air and buoyancy reference, degrees C.
    /// `None` = the volume mean (right for a closed box); with openings set
    /// it to the outside temperature so the stack effect comes out.
    pub t_ref: Option<f64>,
    /// Density, kg/m3: turns fan curves in Pa into the kinematic pressure.
    pub rho: f64,
    /// Solids in the grid (conjugate heat transfer). `None` = all air.
    pub medium: Option<Medium>,
    /// Relative residual the pressure projection solves to each step. The
    /// default is tight (benchmarks); a march to a steady state does not need
    /// the last digits and may loosen it.
    pub pressure_tol: f64,
    /// The mounting plate as a thin sheet on the rear face (`ZMin`), if any.
    /// Where it covers a column, the rear thermal condition acts only on the
    /// bare part and the sheet carries the heat to the outside instead.
    pub plate: Option<PlateSheet>,
    /// Surface film coefficient of each device (by the slot of
    /// [`Medium::dev_slot`]), W/m2K. A face between a device cell and an air
    /// cell then conducts as the half cell of the device in series with this
    /// film; empty keeps plain cell-to-cell conduction.
    pub device_film_h: Vec<f64>,
    /// True exposed surface of each device (to air, without the foot), m2.
    /// The film conducts as if the faces the grid found had this area (the
    /// grid cuts a device in steps), so the result does not depend on how
    /// the mesh meets the device. Missing or 0 = the grid's own area.
    pub device_exposed_m2: Vec<f64>,
    /// True contact area of each device on the plate sheet, m2; same idea.
    pub device_contact_m2: Vec<f64>,
    /// Height of each device (the 2D engine's characteristic length), m, and
    /// the radiation constants: with them the solver refreshes the films
    /// itself every [`FILM_EVERY`] steps or iterations.
    pub device_length_m: Vec<f64>,
    pub device_emissivity: f64,
    pub device_view: f64,
    /// Surface-to-surface radiation between devices, plate and walls. When
    /// set, the device films are convection only (set `device_emissivity`
    /// to 0) and the solver refreshes the radiative heat every
    /// [`RAD_EVERY`] steps or iterations.
    pub radiation: Option<std::sync::Arc<crate::rad::RadPlan>>,
}

/// Steps (or steady iterations) between refreshes of the device films.
pub const FILM_EVERY: usize = 10;
/// Steps (or steady iterations) between refreshes of the radiative heat.
pub const RAD_EVERY: usize = 10;

impl FlowConfig {
    /// Air near 25 degrees C with closed no-slip adiabatic walls.
    pub fn air() -> FlowConfig {
        FlowConfig {
            nu: 1.55e-5,
            alpha: 2.2e-5,
            conductivity: 0.026,
            beta: 1.0 / 298.15,
            gravity: [0.0, 9.81, 0.0],
            scheme: Scheme::VanLeer,
            faces: [FaceBc::WALL; 6],
            heat_source: None,
            prandtl_t: 0.85,
            openings: Vec::new(),
            t_ref: None,
            rho: 1.18,
            medium: None,
            pressure_tol: 1e-9,
            plate: None,
            device_film_h: Vec::new(),
            device_exposed_m2: Vec::new(),
            device_contact_m2: Vec::new(),
            device_length_m: Vec::new(),
            device_emissivity: 0.85,
            device_view: 0.5,
            radiation: None,
        }
    }
}

/// What one step did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepInfo {
    pub dt: f64,
    /// Largest velocity change per second over the step (steadiness).
    pub max_accel: f64,
    /// Largest temperature change per second.
    pub max_dtdt: f64,
    pub pressure_iterations: usize,
}

pub struct FlowSolver {
    pub mesh: Mesh,
    pub cfg: FlowConfig,
    /// Velocity components on their faces, m/s.
    pub vel: [Vec<f64>; 3],
    /// Kinematic pressure (pressure / rho), m2/s2, zero mean.
    pub p: Vec<f64>,
    pub t: Vec<f64>,
    pub time: f64,
    /// Multiplies every heat source (config and medium): the load share of a
    /// transient run. 1 = rated.
    pub heat_scale: f64,
    /// Eddy viscosity per cell, m2/s, set by a turbulence model each step.
    /// Momentum diffuses with `nu + nu_t`, heat with `k + rho cp nu_t / prandtl_t`,
    /// both as the harmonic mean over a face. The cross terms of the stress
    /// tensor (gradients of `nu_t`) are neglected. `None` is laminar flow and
    /// gives bit-identical results.
    pub nu_t: Option<Vec<f64>>,
    /// Wall function values, per cell and face (index `2 * axis + high`, the
    /// order of [`Face`]): the viscosity / diffusivity that replaces `nu` /
    /// `alpha` between the cell centre and the wall (a domain wall or a
    /// solid). `NaN` keeps the molecular value.
    pub wall_nu: Option<Vec<[f64; 6]>>,
    pub wall_alpha: Option<Vec<[f64; 6]>>,
    t_buoyancy_ref: f64,
    pressure: PressureSolver,
    /// The openings resolved on the mesh.
    pub open: OpenSet,
    /// Smoothed speed at each open face, m/s (lags the quadratic losses).
    open_speed: Vec<f64>,
    open_alpha: Vec<f64>,
    /// Inflow temperature of each patch, degrees C.
    t_in: Vec<f64>,
    /// Volumetric heat capacity per cell, J/m3K (air where there is no solid).
    c_cell: Vec<f64>,
    /// Conductivity per cell, W/mK.
    k_cell: Vec<f64>,
    /// Conductivity of the face below each cell along each axis (harmonic
    /// mean of the two cells), W/mK; 0 on the domain boundary.
    k_face: [Vec<f64>; 3],
    /// Convection scale per cell: heat capacity of air over the cell's.
    conv_scale: Vec<f64>,
    /// Heating per cell, K/s (from the config and the medium).
    q_cell: Vec<f64>,
    /// Quadratic drag per cell and axis, 1/m (porous cells), if any.
    drag_cell: Option<[Vec<f64>; 3]>,
    /// Drag rate `F |u|` on each velocity face, 1/s (lagged one step).
    drag_rate: [Vec<f64>; 3],
    /// Plate sheet state, if the configuration has one.
    sheet: Option<Sheet>,
    /// Faces between a device cell and an air cell that carry a film.
    film_faces: Vec<FilmFace>,
    /// Factor on each device's film coefficient (true area over grid area).
    film_scale: Vec<f64>,
    film_clock: usize,
    /// Radiation part of each device's film, W/m2K: it does not warm the air,
    /// so only the rest of the film is a boundary layer (see
    /// [`film_conductance`]).
    film_rad: Vec<f64>,
    rad_state: Option<crate::rad::RadState>,
    /// Radiative heat of the last update (see [`crate::rad::RadLoad`]).
    pub rad_load: Option<crate::rad::RadLoad>,
    rad_clock: usize,
}

/// Temperatures and constants of the plate sheet.
struct Sheet {
    geo: SheetGeometry,
    t: Vec<f64>,
    /// Exchange with the cell in front, W/K: contact where a solid stands on
    /// the column, film where it is bare.
    g_col: Vec<f64>,
    /// The contact part of `g_col`, W/K.
    g_contact: Vec<f64>,
}

/// A face between a device cell and an air cell.
struct FilmFace {
    axis: usize,
    /// Cell on the high side (key of `k_face`).
    hi: usize,
    /// Distance between the two cell centres, m.
    dist: f64,
    /// Half cell of the device in series, m2K/W.
    r_dev: f64,
    /// Device slot.
    slot: usize,
    /// The air cell of the face.
    air: usize,
    /// Distance from the device surface to the centre of the air cell, m.
    y_air: f64,
}

/// Conductance per area from a device surface to the centre of an air cell
/// `y` away, W/m2K, for a surface film `h` that is defined against the air
/// outside the boundary layer.
///
/// A cell centre outside the layer (`y` over the layer depth `2 k / h`, the
/// quadratic temperature profile of natural convection) sees the film as it
/// is. A cell centre inside it already sits partly in the layer the film
/// stands for, so the film acts against a temperature that is further out:
/// `h / (1 - (1 - y / delta)^2)`, which runs to plain conduction `k / y` as
/// the mesh resolves the layer. Without it a fine mesh counts the layer twice.
fn film_conductance(h: f64, h_rad: f64, y: f64, k: f64) -> f64 {
    let h_c = (h - h_rad).max(1e-3);
    let delta = 2.0 * k / h_c;
    let conv = if y >= delta {
        h_c
    } else {
        h_c / (1.0 - (1.0 - y / delta).powi(2))
    };
    conv + h_rad
}

impl FlowSolver {
    pub fn new(mesh: Mesh, cfg: FlowConfig, t_init: f64) -> FlowSolver {
        let vel = std::array::from_fn(|c| vec![0.0; mesh.len(c)]);
        let cells = mesh.cells();
        let mut open = OpenSet::resolve(&mesh, &cfg.openings);
        let rho_cp_air = cfg.conductivity / cfg.alpha;
        let (c_cell, k_cell, conv_scale, mut q_cell) = match &cfg.medium {
            Some(md) => {
                assert_eq!(md.cells(), cells, "medium and mesh differ in size");
                md.block_faces(&mesh, &mut open.afrac);
                let c: Vec<f64> = (0..cells).map(|i| md.cell_rho_cp(i, rho_cp_air)).collect();
                let k: Vec<f64> = (0..cells).map(|i| md.cell_k(i, cfg.conductivity)).collect();
                let sc = c.iter().map(|c| rho_cp_air / c).collect();
                let q = (0..cells).map(|i| md.heat_w_m3(i) / c[i]).collect();
                (c, k, sc, q)
            }
            None => (
                vec![rho_cp_air; cells],
                vec![cfg.conductivity; cells],
                vec![1.0; cells],
                vec![0.0; cells],
            ),
        };
        if let Some(h) = &cfg.heat_source {
            for (q, h) in q_cell.iter_mut().zip(h) {
                *q += h;
            }
        }
        let k_face = std::array::from_fn(|a| {
            let mut kf = vec![0.0; cells];
            for k in 0..mesh.n[2] {
                for j in 0..mesh.n[1] {
                    for i in 0..mesh.n[0] {
                        let ix = [i, j, k];
                        if ix[a] == 0 {
                            continue;
                        }
                        let (dl, du) = (0.5 * mesh.dx[a][ix[a] - 1], 0.5 * mesh.dx[a][ix[a]]);
                        let kl = k_cell[mesh.cell(with(ix, a, ix[a] - 1))];
                        let ku = k_cell[mesh.cell(ix)];
                        kf[mesh.cell(ix)] = (dl + du) / (dl / kl + du / ku);
                    }
                }
            }
            kf
        });
        let mut k_face = k_face;
        let cfg_film_len = cfg.device_film_h.len();
        let mut film_faces = Vec::new();
        let mut film_scale = Vec::new();
        if let Some(md) = cfg
            .medium
            .as_ref()
            .filter(|_| !cfg.device_film_h.is_empty())
        {
            for a in 0..3 {
                for k in 0..mesh.n[2] {
                    for j in 0..mesh.n[1] {
                        for i in 0..mesh.n[0] {
                            let hi_ix = [i, j, k];
                            if hi_ix[a] == 0 {
                                continue;
                            }
                            let (lo, hi) =
                                (mesh.cell(with(hi_ix, a, hi_ix[a] - 1)), mesh.cell(hi_ix));
                            let dev_is_lo = md.dev_frac[lo] >= 0.5 && md.fluid_frac[hi] >= 0.5;
                            let dev_is_hi = md.dev_frac[hi] >= 0.5 && md.fluid_frac[lo] >= 0.5;
                            if !dev_is_lo && !dev_is_hi {
                                continue;
                            }
                            let (dc, dix) = if dev_is_lo {
                                (lo, with(hi_ix, a, hi_ix[a] - 1))
                            } else {
                                (hi, hi_ix)
                            };
                            let slot = md.dev_slot[dc] as usize;
                            if slot >= cfg.device_film_h.len() {
                                continue;
                            }
                            film_faces.push(FilmFace {
                                axis: a,
                                hi,
                                dist: mesh.centre[a][hi_ix[a]] - mesh.centre[a][hi_ix[a] - 1],
                                r_dev: 0.5 * mesh.dx[a][dix[a]] / k_cell[dc],
                                slot,
                                air: if dev_is_lo { hi } else { lo },
                                y_air: 0.5
                                    * mesh.dx[a][if dev_is_lo { hi_ix[a] } else { hi_ix[a] - 1 }],
                            });
                        }
                    }
                }
            }
            let mut stair = vec![0.0; cfg.device_film_h.len()];
            for f in &film_faces {
                stair[f.slot] += mesh.area(f.axis, mesh.unindex(f.hi));
            }
            film_scale = stair
                .iter()
                .enumerate()
                .map(|(i, a)| match cfg.device_exposed_m2.get(i) {
                    Some(t) if *t > 0.0 && *a > 0.0 => t / a,
                    _ => 1.0,
                })
                .collect();
            for f in &film_faces {
                let h = cfg.device_film_h[f.slot];
                let g = film_conductance(h, 0.0, f.y_air, cfg.conductivity);
                k_face[f.axis][f.hi] = f.dist / (f.r_dev + 1.0 / (g * film_scale[f.slot]));
            }
        }
        let sheet = cfg.plate.as_ref().map(|pl| {
            let geo = SheetGeometry::new(
                pl,
                &mesh.dx[0],
                &mesh.dx[1],
                &mesh.centre[0],
                &mesh.centre[1],
            );
            let cols = geo.area.len();
            let mut g_col = vec![0.0; cols];
            let mut g_contact = vec![0.0; cols];
            // contact area of each device: grid steps against the true foot
            let mut base = vec![0.0; cfg.device_contact_m2.len()];
            if let Some(md) = cfg.medium.as_ref() {
                for c in 0..cols {
                    let slot = md.dev_slot[mesh.cell([c % geo.nx, c / geo.nx, 0])] as usize;
                    if slot < base.len() {
                        base[slot] += geo.area[c];
                    }
                }
            }
            for c in 0..cols {
                let id = mesh.cell([c % geo.nx, c / geo.nx, 0]);
                let fluid = cfg.medium.as_ref().map_or(1.0, |md| md.fluid_frac[id]);
                let slot = cfg
                    .medium
                    .as_ref()
                    .map_or(usize::MAX, |md| md.dev_slot[id] as usize);
                let scale = match (cfg.device_contact_m2.get(slot), base.get(slot)) {
                    (Some(t), Some(b)) if *t > 0.0 && *b > 0.0 => t / b,
                    _ => 1.0,
                };
                g_contact[c] = geo.area[c] * pl.h_contact * (1.0 - fluid) * scale;
                g_col[c] = g_contact[c] + geo.area[c] * pl.h_air * fluid;
            }
            Sheet {
                geo,
                t: vec![t_init; cols],
                g_col,
                g_contact,
            }
        });
        let drag_cell = cfg
            .medium
            .as_ref()
            .filter(|md| md.has_drag())
            .map(|md| md.drag.clone());
        let drag_rate =
            std::array::from_fn(|c| vec![0.0; if drag_cell.is_some() { mesh.len(c) } else { 0 }]);
        let pressure = PressureSolver::with_openness(&mesh, Some(&open.afrac));
        let t_in = cfg
            .openings
            .iter()
            .map(|p| p.inflow_c.unwrap_or(cfg.t_ref.unwrap_or(t_init)))
            .collect();
        let nf = open.faces.len();
        FlowSolver {
            open,
            open_speed: vec![0.0; nf],
            open_alpha: vec![1.0; nf],
            t_in,
            c_cell,
            k_cell,
            k_face,
            conv_scale,
            q_cell,
            drag_cell,
            drag_rate,
            sheet,
            film_faces,
            film_scale,
            film_clock: 0,
            film_rad: vec![0.0; cfg_film_len],
            rad_state: cfg.radiation.as_ref().map(|p| p.state()),
            rad_load: None,
            rad_clock: 0,
            vel,
            p: vec![0.0; cells],
            t: vec![t_init; cells],
            time: 0.0,
            heat_scale: 1.0,
            nu_t: None,
            wall_nu: None,
            wall_alpha: None,
            t_buoyancy_ref: t_init,
            pressure,
            mesh,
            cfg,
        }
    }

    /// Chooses the ordering of the pressure smoother (see [`Smoother`]);
    /// red-black runs on all threads of the current rayon pool.
    pub fn set_pressure_smoother(&mut self, smoother: Smoother) {
        self.pressure.set_smoother(smoother);
    }

    /// Counts a step or iteration and refreshes the films every
    /// [`FILM_EVERY`].
    pub(crate) fn tick_films(&mut self) {
        self.tick_radiation();
        if self.cfg.device_length_m.is_empty() {
            return;
        }
        self.film_clock += 1;
        if self.film_clock.is_multiple_of(FILM_EVERY) {
            self.update_device_films();
        }
    }

    /// Counts a step or iteration and refreshes the radiative heat on the
    /// first and then every [`RAD_EVERY`].
    fn tick_radiation(&mut self) {
        let (Some(plan), Some(state)) = (self.cfg.radiation.clone(), self.rad_state.as_mut())
        else {
            return;
        };
        self.rad_clock += 1;
        if !(self.rad_clock - 1).is_multiple_of(RAD_EVERY) {
            return;
        }
        let walls = std::array::from_fn(|f| match self.cfg.faces[f].thermal {
            ThermalBc::Adiabatic => crate::rad::WallBc::Overall { u: 0.0, t_out: 0.0 },
            ThermalBc::Fixed(tw) => crate::rad::WallBc::Fixed(tw),
            ThermalBc::Convective { h, t_out } => crate::rad::WallBc::Overall { u: h, t_out },
            ThermalBc::Overall { u, t_out } => crate::rad::WallBc::Overall { u, t_out },
        });
        let inp = crate::rad::RadInputs {
            t_c: &self.t,
            sheet_t_c: self.sheet.as_ref().map(|s| s.t.as_slice()),
            walls,
        };
        self.rad_load = Some(plan.update(state, &inp));
    }

    /// Net radiative power of a mesh material (0 = enclosure wall), W;
    /// positive = gives off heat. Zero before the first update.
    pub fn radiation_power_w(&self, material: u16) -> f64 {
        self.rad_load.as_ref().map_or(0.0, |l| l.power_w(material))
    }

    /// Sets the surface film of every device from the present state: the
    /// 2D engine's correlation (natural plus forced convection and
    /// radiation), relaxed halfway as it does.
    pub fn update_device_films(&mut self) {
        for slot in 0..self.cfg.device_length_m.len() {
            let Some((t_dev, t_air, speed)) = self.device_film_state(slot) else {
                continue;
            };
            let h = novo_thermal::device_h(
                self.cfg.device_length_m[slot],
                t_dev,
                t_air,
                speed,
                self.cfg.device_emissivity,
                self.cfg.device_view,
            );
            let old = self.cfg.device_film_h.get(slot).copied().unwrap_or(h);
            // radiation share of the film (the 2D engine's linearisation)
            let tf = 0.5 * (t_dev + t_air) + 273.15;
            let h_rad =
                4.0 * self.cfg.device_emissivity * 5.670_374e-8 * tf.powi(3) * self.cfg.device_view;
            self.set_device_film_split(slot, 0.5 * old + 0.5 * h, h_rad);
        }
    }

    /// Sets the film coefficient of device `slot`, W/m2K, on every face it
    /// shares with air (see [`FlowConfig::device_film_h`]).
    pub fn set_device_film(&mut self, slot: usize, h: f64) {
        self.set_device_film_split(slot, h, 0.0);
    }

    /// As [`FlowSolver::set_device_film`], with `h_rad` of `h` being radiation.
    pub fn set_device_film_split(&mut self, slot: usize, h: f64, h_rad: f64) {
        if let Some(v) = self.cfg.device_film_h.get_mut(slot) {
            *v = h.max(1e-3);
        }
        if let Some(v) = self.film_rad.get_mut(slot) {
            *v = h_rad.clamp(0.0, 0.9 * h.max(1e-3));
        }
        let h_rad = self.film_rad.get(slot).copied().unwrap_or(0.0);
        for f in self.film_faces.iter().filter(|f| f.slot == slot) {
            let g = film_conductance(h.max(1e-3), h_rad, f.y_air, self.cfg.conductivity)
                * self.film_scale[slot];
            self.k_face[f.axis][f.hi] = f.dist / (f.r_dev + 1.0 / g);
        }
    }

    /// One Gauss-Seidel sweep of the plate sheet against the present cell
    /// temperatures, implicit over `dt` from the temperatures `po` at its
    /// start; returns the largest change, K.
    pub(crate) fn sheet_sweep(&mut self, dt: f64, po: &[f64]) -> f64 {
        match (self.sheet.as_mut(), self.cfg.plate.as_ref()) {
            (Some(sh), Some(pl)) => {
                sheet_sweep(&self.mesh, sh, pl, &self.t, dt, po, self.rad_load.as_ref())
            }
            _ => 0.0,
        }
    }

    /// Steady path: the sheet follows the cells over the pseudo step `dt`
    /// (a few sweeps; its capacity is small, so it is nearly algebraic).
    pub(crate) fn advance_sheet_pseudo(&mut self, dt: f64) {
        let Some(po) = self.sheet.as_ref().map(|s| s.t.clone()) else {
            return;
        };
        for _ in 0..20 {
            if self.sheet_sweep(dt, &po) < 1e-9 {
                break;
            }
        }
    }

    /// Plate sheet constants for the steady assembly: exchange with the cell
    /// in front of column (`i`, `j`), W/K, and the plate temperature there.
    pub(crate) fn sheet_coupling(&self, i: usize, j: usize) -> Option<(f64, f64)> {
        let sh = self.sheet.as_ref()?;
        let c = i + self.mesh.n[0] * j;
        Some((sh.g_col[c], sh.t[c]))
    }

    /// Mean temperature of device `slot` (weighted by its volume share),
    /// the mean of the air cells touching it and their mean speed, m/s; for
    /// the surface film. `None` without films or when it touches no air.
    pub fn device_film_state(&self, slot: usize) -> Option<(f64, f64, f64)> {
        let md = self.cfg.medium.as_ref()?;
        let m = &self.mesh;
        let (mut t_air, mut speed, mut w_air) = (0.0, 0.0, 0.0);
        let mut seen = std::collections::HashSet::new();
        for f in self.film_faces.iter().filter(|f| f.slot == slot) {
            if seen.insert(f.air) {
                t_air += self.t[f.air];
                speed += self.cell_speed(f.air);
                w_air += 1.0;
            }
        }
        if w_air == 0.0 {
            return None;
        }
        let (mut tv, mut v) = (0.0, 0.0);
        for (id, s) in md.dev_slot.iter().enumerate() {
            if *s as usize == slot {
                let ix = m.unindex(id);
                let w = md.dev_frac[id] * m.volume(ix);
                tv += w * self.t[id];
                v += w;
            }
        }
        (v > 0.0).then(|| (tv / v, t_air / w_air, speed / w_air))
    }

    /// Area of the faces between device `slot` and air that carry its film, m2.
    pub fn device_film_area_m2(&self, slot: usize) -> f64 {
        self.film_faces
            .iter()
            .filter(|f| f.slot == slot)
            .map(|f| self.mesh.area(f.axis, self.mesh.unindex(f.hi)))
            .sum()
    }

    /// Heat from device `slot` into the air through its film faces, W.
    pub fn device_film_w(&self, slot: usize) -> f64 {
        let m = &self.mesh;
        self.film_faces
            .iter()
            .filter(|f| f.slot == slot)
            .map(|f| {
                let hi_ix = m.unindex(f.hi);
                let dev = if f.air == f.hi {
                    m.cell(with(hi_ix, f.axis, hi_ix[f.axis] - 1))
                } else {
                    f.hi
                };
                self.k_face[f.axis][f.hi] / f.dist
                    * m.area(f.axis, hi_ix)
                    * (self.t[dev] - self.t[f.air])
            })
            .sum()
    }

    /// Speed at the centre of cell `id` from its face velocities, m/s.
    fn cell_speed(&self, id: usize) -> f64 {
        let m = &self.mesh;
        let ix = m.unindex(id);
        let c: Vec<f64> = (0..3)
            .map(|a| {
                0.5 * (self.vel[a][m.idx(a, ix)] + self.vel[a][m.idx(a, with(ix, a, ix[a] + 1))])
            })
            .collect();
        (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt()
    }

    /// Plate temperature of each rear column (`i + nx * j`), degrees C.
    pub fn plate_temps(&self) -> Option<&[f64]> {
        self.sheet.as_ref().map(|s| s.t.as_slice())
    }

    /// Heat from the cell in front of rear column `col` into the plate, W:
    /// `(through contact with solids standing on it, through the air film)`.
    pub fn plate_exchange_w(&self, col: usize) -> (f64, f64) {
        let Some(sh) = &self.sheet else {
            return (0.0, 0.0);
        };
        let id = self.mesh.cell([col % sh.geo.nx, col / sh.geo.nx, 0]);
        let dt = self.t[id] - sh.t[col];
        (
            sh.g_contact[col] * dt,
            (sh.g_col[col] - sh.g_contact[col]) * dt,
        )
    }

    pub fn pressure_levels(&self) -> usize {
        self.pressure.level_count()
    }

    /// Volume-weighted mean temperature, degrees C.
    pub fn mean_temperature(&self) -> f64 {
        let m = &self.mesh;
        let (mut sum, mut vol) = (0.0, 0.0);
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let v = m.volume([i, j, k]);
                    sum += v * self.t[m.cell([i, j, k])];
                    vol += v;
                }
            }
        }
        sum / vol
    }

    /// Largest stable step for the explicit convection, `cfl` about 0.5.
    pub fn max_dt(&self, cfl: f64) -> f64 {
        let m = &self.mesh;
        let mut rate: f64 = 1e-12;
        for c in 0..3 {
            let d = m.dims(c);
            let r = max_over_planes(d[2], m.len(c), |k| {
                let mut r: f64 = 0.0;
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        let s = ix[c].min(m.n[c] - 1);
                        let h = m.dx[c][s];
                        r = r.max(self.vel[c][m.idx(c, ix)].abs() / h);
                    }
                }
                r
            });
            rate = rate.max(r);
        }
        cfl / rate
    }

    /// Largest cell divergence `|div u|` times the cell volume over the
    /// smallest face area scale: the net outflow of a cell relative to the
    /// largest face flux (0 = exactly divergence free).
    pub fn divergence_max(&self) -> f64 {
        let m = &self.mesh;
        let plane = |k: usize| -> (f64, f64) {
            let (mut worst, mut scale): (f64, f64) = (0.0, 1e-30);
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let ix = [i, j, k];
                    let mut net = 0.0;
                    for a in 0..3 {
                        let mut hi = ix;
                        hi[a] += 1;
                        let (fl, fh) = (
                            self.vel[a][m.idx(a, ix)]
                                * m.area(a, ix)
                                * self.open.afrac[a][m.idx(a, ix)],
                            self.vel[a][m.idx(a, hi)]
                                * m.area(a, ix)
                                * self.open.afrac[a][m.idx(a, hi)],
                        );
                        net += fh - fl;
                        scale = scale.max(fl.abs()).max(fh.abs());
                    }
                    worst = worst.max(net.abs());
                }
            }
            (worst, scale)
        };
        // Both are maxima, so the plane order does not matter.
        let worst = max_over_planes(m.n[2], m.cells(), |k| plane(k).0).max(0.0);
        let scale = max_over_planes(m.n[2], m.cells(), |k| plane(k).1).max(1e-30);
        worst / scale
    }

    /// Advances one step of `dt` seconds.
    pub fn step(&mut self, dt: f64) -> StepInfo {
        self.tick_films();
        let old_t = self.t.clone();
        let old_vel = self.vel.clone();
        self.update_drag_rate();
        self.advance_temperature(dt);
        // In a closed box a uniform temperature offset is a pure pressure
        // gradient, so the Boussinesq term uses the volume mean as reference.
        self.t_buoyancy_ref = self.cfg.t_ref.unwrap_or_else(|| self.mean_temperature());
        let rhs: [Vec<f64>; 3] = std::array::from_fn(|c| self.momentum_rhs(c));
        for (c, r) in rhs.iter().enumerate() {
            self.diffuse_velocity(c, r, dt);
        }
        self.predict_open_faces(dt);
        let iters = self.project(dt);
        self.time += dt;
        let max_accel = (0..3)
            .flat_map(|c| {
                self.vel[c]
                    .iter()
                    .zip(&old_vel[c])
                    .map(|(a, b)| (a - b).abs() / dt)
            })
            .fold(0.0, f64::max);
        let max_dtdt = self
            .t
            .iter()
            .zip(&old_t)
            .map(|(a, b)| (a - b).abs() / dt)
            .fold(0.0, f64::max);
        StepInfo {
            dt,
            max_accel,
            max_dtdt,
            pressure_iterations: iters,
        }
    }

    // ---- momentum ----------------------------------------------------

    /// Convection (negative) plus buoyancy of velocity component `c`, at the
    /// interior faces. Zero on the faces of the walls.
    fn momentum_rhs(&self, c: usize) -> Vec<f64> {
        let m = &self.mesh;
        let mut out = vec![0.0; m.len(c)];
        let d = m.dims(c);
        let u = &self.vel;
        let sch = self.cfg.scheme;
        for_planes(&mut out, d[0] * d[1], |k, plane| {
            for j in 0..d[1] {
                for i in 0..d[0] {
                    let ix = [i, j, k];
                    if ix[c] == 0 || ix[c] == m.n[c] || self.open.afrac[c][m.idx(c, ix)] == 0.0 {
                        continue;
                    }
                    let mut conv = 0.0;
                    for b in 0..3 {
                        let (flux_lo, flux_hi, width);
                        if b == c {
                            let s = ix[c];
                            width = 0.5 * (m.dx[c][s - 1] + m.dx[c][s]);
                            let at = |q: usize| -> Sample {
                                (m.edges[c][q], u[c][m.idx(c, with(ix, c, q))])
                            };
                            let nb = |q: isize| -> Option<Sample> {
                                (q >= 0 && q <= m.n[c] as isize).then(|| at(q as usize))
                            };
                            let u_lo = 0.5 * (at(s - 1).1 + at(s).1);
                            let u_hi = 0.5 * (at(s).1 + at(s + 1).1);
                            let s = s as isize;
                            let phi_lo = face_value(
                                sch,
                                m.centre[c][(s - 1) as usize],
                                nb(s - 2),
                                nb(s - 1).unwrap(),
                                nb(s).unwrap(),
                                nb(s + 1),
                                u_lo > 0.0,
                            );
                            let phi_hi = face_value(
                                sch,
                                m.centre[c][s as usize],
                                nb(s - 1),
                                nb(s).unwrap(),
                                nb(s + 1).unwrap(),
                                nb(s + 2),
                                u_hi > 0.0,
                            );
                            flux_lo = u_lo * phi_lo;
                            flux_hi = u_hi * phi_hi;
                        } else {
                            let s = ix[b];
                            width = m.dx[b][s];
                            let flux_at = |e: usize| -> f64 {
                                if e == 0 || e == m.n[b] {
                                    return self.open_transverse_flux(c, b, e, ix);
                                }
                                // advecting velocity u_b at b-edge e, c-edge ix[c]
                                let (lo, hi) = (ix[c] - 1, ix[c]);
                                let w_lo = m.dx[c][hi] / (m.dx[c][lo] + m.dx[c][hi]);
                                let mut ia = with(ix, b, e);
                                ia[c] = lo;
                                let mut ib = ia;
                                ib[c] = hi;
                                let (va, vb) = (m.idx(b, ia), m.idx(b, ib));
                                let ub = w_lo * u[b][va] * self.open.afrac[b][va]
                                    + (1.0 - w_lo) * u[b][vb] * self.open.afrac[b][vb];
                                let at = |q: usize| -> Sample {
                                    (m.centre[b][q], u[c][m.idx(c, with(ix, b, q))])
                                };
                                let ll = (e >= 2).then(|| at(e - 2));
                                let uu = (e + 1 < m.n[b]).then(|| at(e + 1));
                                let phi = face_value(
                                    sch,
                                    m.edges[b][e],
                                    ll,
                                    at(e - 1),
                                    at(e),
                                    uu,
                                    ub > 0.0,
                                );
                                ub * phi
                            };
                            flux_lo = flux_at(s);
                            flux_hi = flux_at(s + 1);
                        }
                        conv += (flux_hi - flux_lo) / width;
                    }
                    // buoyancy: -g beta (T - Tref), T averaged to the face
                    let (lo, hi) = (with(ix, c, ix[c] - 1), ix);
                    let w_lo = m.dx[c][ix[c]] / (m.dx[c][ix[c] - 1] + m.dx[c][ix[c]]);
                    let tf = w_lo * self.t[m.cell(lo)] + (1.0 - w_lo) * self.t[m.cell(hi)];
                    let buoy = -self.cfg.gravity[c] * self.cfg.beta * (tf - self.t_buoyancy_ref);
                    plane[j * d[0] + i] = -conv + buoy;
                }
            }
        });
        out
    }

    /// Solves `u/dt - nu L u = u_old/dt + rhs` by Gauss-Seidel.
    fn diffuse_velocity(&mut self, c: usize, rhs: &[f64], dt: f64) {
        let m = &self.mesh;
        let d = m.dims(c);
        let nu = self.cfg.nu;
        let (nu_t, wall_nu) = (self.nu_t.as_deref(), self.wall_nu.as_deref());
        let nu_cell = |cell: [usize; 3]| nu + nu_t.map_or(0.0, |v| v[m.cell(cell)]);
        let old = self.vel[c].clone();
        let rhs_total: Vec<f64> = old.iter().zip(rhs).map(|(o, r)| o / dt + r).collect();
        for _ in 0..200 {
            let mut change: f64 = 0.0;
            let mut scale: f64 = 1e-30;
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        if ix[c] == 0 || ix[c] == m.n[c] || self.open.afrac[c][m.idx(c, ix)] == 0.0
                        {
                            continue;
                        }
                        let id = m.idx(c, ix);
                        let (mut a_sum, mut a_nb) = (self.drag_rate_at(c, id), 0.0);
                        for b in 0..3 {
                            let s = ix[b];
                            if b == c {
                                let w = 0.5 * (m.dx[c][s - 1] + m.dx[c][s]);
                                let nu_lo = nu_cell(with(ix, c, s - 1));
                                let nu_hi = nu_cell(ix);
                                let (cl, cu) =
                                    (nu_lo / (m.dx[c][s - 1] * w), nu_hi / (m.dx[c][s] * w));
                                a_sum += cl + cu;
                                a_nb += cl * self.vel[c][m.idx(c, with(ix, c, s - 1))]
                                    + cu * self.vel[c][m.idx(c, with(ix, c, s + 1))];
                            } else {
                                let w = m.dx[b][s];
                                // the two cells beside this sample along c
                                let side = |t: usize| {
                                    [
                                        with(with(ix, b, t), c, ix[c] - 1),
                                        with(with(ix, b, t), c, ix[c]),
                                    ]
                                };
                                let nu_here = {
                                    let [a, a2] = side(s);
                                    0.5 * (nu_cell(a) + nu_cell(a2))
                                };
                                for upper in [false, true] {
                                    let face = 2 * b + upper as usize;
                                    if (!upper && s == 0) || (upper && s + 1 == m.n[b]) {
                                        let bc = self.cfg.faces[face];
                                        if !bc.slip {
                                            let closed = 1.0 - self.open_share(b, upper, ix, c);
                                            let nu_w = wall_value(wall_nu, m, &side(s), face)
                                                .unwrap_or(nu);
                                            let cw = closed * nu_w / (0.5 * w * w);
                                            a_sum += cw;
                                            a_nb += cw * bc.wall_velocity[c];
                                        }
                                    } else {
                                        let q = if upper { s + 1 } else { s - 1 };
                                        let nbr = m.idx(c, with(ix, b, q));
                                        if self.open.afrac[c][nbr] == 0.0 {
                                            // a solid next door: no-slip wall half a cell away
                                            let nu_w = wall_value(wall_nu, m, &side(s), face)
                                                .unwrap_or(nu);
                                            a_sum += nu_w / (0.5 * w * w);
                                        } else {
                                            let dist = (m.centre[b][q] - m.centre[b][s]).abs();
                                            let nu_there = {
                                                let [a, a2] = side(q);
                                                0.5 * (nu_cell(a) + nu_cell(a2))
                                            };
                                            let cn = harmonic(nu_here, nu_there) / (dist * w);
                                            a_sum += cn;
                                            a_nb += cn * self.vel[c][nbr];
                                        }
                                    }
                                }
                            }
                        }
                        let new = (rhs_total[id] + a_nb) / (1.0 / dt + a_sum);
                        change = change.max((new - self.vel[c][id]).abs());
                        scale = scale.max(new.abs());
                        self.vel[c][id] = new;
                    }
                }
            }
            if change <= 1e-12 * scale {
                break;
            }
        }
    }

    // ---- pressure projection -------------------------------------------

    fn project(&mut self, dt: f64) -> usize {
        // Porous drag acts inside the projection: the velocity correction of
        // a face is dt / (1 + dt d) times the pressure gradient, so a steady
        // state balances the drag exactly, however large dt d is.
        let porous = self.drag_cell.is_some();
        if porous {
            let m = &self.mesh;
            let coupling: [Vec<f64>; 3] = std::array::from_fn(|a| {
                self.open.afrac[a]
                    .iter()
                    .zip(&self.drag_rate[a])
                    .map(|(af, d)| af / (1.0 + dt * d))
                    .collect()
            });
            self.pressure.set_coupling(m, &coupling);
        }
        self.set_open_faces(dt);
        let m = &self.mesh;
        let af = &self.open.afrac;
        let mut b = vec![0.0; m.cells()];
        let vel = &self.vel;
        for_planes(&mut b, m.n[0] * m.n[1], |k, plane| {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let ix = [i, j, k];
                    let mut net = 0.0;
                    for a in 0..3 {
                        let (hi, lo) = (m.idx(a, with(ix, a, ix[a] + 1)), m.idx(a, ix));
                        net += (vel[a][hi] * af[a][hi] - vel[a][lo] * af[a][lo]) * m.area(a, ix);
                    }
                    plane[j * m.n[0] + i] = -net / dt;
                }
            }
        });
        let iters = self
            .pressure
            .solve(&b, &mut self.p, self.cfg.pressure_tol, 200);
        let (p, drag) = (&self.p, &self.drag_rate);
        for a in 0..3 {
            let d = m.dims(a);
            for_planes(&mut self.vel[a], d[0] * d[1], |k, plane| {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        if ix[a] == 0 || ix[a] == m.n[a] || af[a][m.idx(a, ix)] == 0.0 {
                            continue;
                        }
                        let lo = with(ix, a, ix[a] - 1);
                        let dist = m.centre[a][ix[a]] - m.centre[a][ix[a] - 1];
                        let g = if porous {
                            1.0 / (1.0 + dt * drag[a][m.idx(a, ix)])
                        } else {
                            1.0
                        };
                        plane[j * d[0] + i] -= g * dt * (p[m.cell(ix)] - p[m.cell(lo)]) / dist;
                    }
                }
            });
        }
        self.finish_open_faces(dt);
        iters
    }

    // ---- porous drag -------------------------------------------------

    fn drag_rate_at(&self, c: usize, id: usize) -> f64 {
        self.drag_rate[c].get(id).copied().unwrap_or(0.0)
    }

    /// `F |u|` on every interior face from the cells either side.
    fn update_drag_rate(&mut self) {
        let Some(f) = &self.drag_cell else {
            return;
        };
        let m = &self.mesh;
        for c in 0..3 {
            let d = m.dims(c);
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        let id = m.idx(c, ix);
                        self.drag_rate[c][id] = if ix[c] == 0 || ix[c] == m.n[c] {
                            0.0
                        } else {
                            let lo = m.cell(with(ix, c, ix[c] - 1));
                            let hi = m.cell(ix);
                            0.5 * (f[c][lo] + f[c][hi]) * self.vel[c][id].abs()
                        };
                    }
                }
            }
        }
    }

    // ---- openings --------------------------------------------------------

    /// Share of the boundary face on side `upper` of axis `b` that is open,
    /// averaged over the two cells next to the velocity face `ix` of
    /// component `c`.
    fn open_share(&self, b: usize, upper: bool, ix: [usize; 3], c: usize) -> f64 {
        if self.open.is_empty() {
            return 0.0;
        }
        let m = &self.mesh;
        let e = if upper { m.n[b] } else { 0 };
        let at = |q: usize| -> f64 {
            let mut f = with(ix, b, e);
            f[c] = q;
            self.open.afrac[b][m.idx(b, f)]
        };
        0.5 * (at(ix[c] - 1) + at(ix[c].min(m.n[c] - 1)))
    }

    /// Tangential momentum carried through an open face: the face velocity
    /// `u_b` times the component `c` of the air that crosses (zero for air
    /// coming in from rest, the adjacent value for air going out).
    fn open_transverse_flux(&self, c: usize, b: usize, e: usize, ix: [usize; 3]) -> f64 {
        if self.open.is_empty() {
            return 0.0;
        }
        let m = &self.mesh;
        let (lo, hi) = (ix[c] - 1, ix[c]);
        let w_lo = m.dx[c][hi] / (m.dx[c][lo] + m.dx[c][hi]);
        let mut ia = with(ix, b, e);
        ia[c] = lo;
        let mut ib = ia;
        ib[c] = hi;
        let (va, vb) = (m.idx(b, ia), m.idx(b, ib));
        let ub = w_lo * self.vel[b][va] * self.open.afrac[b][va]
            + (1.0 - w_lo) * self.vel[b][vb] * self.open.afrac[b][vb];
        let inflow = if e == 0 { ub > 0.0 } else { ub < 0.0 };
        if inflow {
            return 0.0;
        }
        let edge = if e == 0 { 0 } else { m.n[b] - 1 };
        ub * self.vel[c][m.idx(c, with(ix, b, edge))]
    }

    /// Enthalpy flux `phi u T` through boundary face `e` (0 or n) of the
    /// cell `ix` along axis `a`, in the +axis direction.
    fn open_enthalpy_flux(&self, a: usize, e: usize, ix: [usize; 3]) -> f64 {
        if self.open.is_empty() {
            return 0.0;
        }
        let m = &self.mesh;
        let v = m.idx(a, with(ix, a, e));
        let phi = self.open.afrac[a][v];
        if phi == 0.0 {
            return 0.0;
        }
        let u = self.vel[a][v];
        let inflow = if e == 0 { u > 0.0 } else { u < 0.0 };
        let tb = if inflow {
            self.t_in[self.open.patch_of[a][v]]
        } else {
            self.t[m.cell(ix)]
        };
        phi * u * tb
    }

    /// Open faces get the buoyancy of the air behind them as their
    /// predicted velocity (no convection or viscosity on the half cell).
    fn predict_open_faces(&mut self, dt: f64) {
        for f in &self.open.faces {
            let g = self.cfg.gravity[f.axis];
            let acc = -g * self.cfg.beta * (self.t[f.cell] - self.t_buoyancy_ref);
            self.vel[f.axis][f.vel] += dt * acc;
        }
    }

    /// Before the pressure solve: the open faces' implicit law becomes an
    /// effective predicted velocity and a conductance to zero pressure.
    fn set_open_faces(&mut self, dt: f64) {
        if self.open.is_empty() {
            return;
        }
        let mut g = vec![0.0; self.mesh.cells()];
        for (n, f) in self.open.faces.iter().enumerate() {
            let sgn = f.outward();
            let u_star = sgn * self.vel[f.axis][f.vel];
            let (r, pj) = self.open.law(f, self.open_speed[n], u_star, self.cfg.rho);
            let alpha = 1.0 / (1.0 + dt * r / f.dh);
            self.open_alpha[n] = alpha;
            g[f.cell] += f.area * alpha / f.dh;
            self.vel[f.axis][f.vel] = sgn * alpha * (u_star - dt * pj / f.dh);
        }
        self.pressure.set_open_conductance(&g);
    }

    /// After the pressure solve: the open faces' velocity from the cell
    /// pressure behind them.
    fn finish_open_faces(&mut self, dt: f64) {
        for (n, f) in self.open.faces.iter().enumerate() {
            let sgn = f.outward();
            let u_out =
                sgn * self.vel[f.axis][f.vel] + self.open_alpha[n] * dt * self.p[f.cell] / f.dh;
            self.vel[f.axis][f.vel] = sgn * u_out;
            self.open_speed[n] = 0.5 * (self.open_speed[n] + u_out.abs());
        }
    }

    /// Heat leaving through the thermal wall conditions of domain face
    /// `face` (index of [`Face`]), W; positive = out of the domain.
    pub fn wall_heat_w(&self, face: usize) -> f64 {
        let m = &self.mesh;
        let (a, upper) = (face / 2, face % 2 == 1);
        let (b, c) = super::mesh::other_axes(a);
        let bc = self.cfg.faces[face].thermal;
        let mut sum = 0.0;
        for kc in 0..m.n[c] {
            for jb in 0..m.n[b] {
                let mut ix = [0; 3];
                ix[a] = if upper { m.n[a] - 1 } else { 0 };
                ix[b] = jb;
                ix[c] = kc;
                let edge = if upper { m.n[a] } else { 0 };
                let mut closed = 1.0 - self.open.afrac[a][m.idx(a, with(ix, a, edge))];
                if face == 4 {
                    closed *= self.rear_bare(ix[0], ix[1]);
                }
                let id = m.cell(ix);
                let (t, k) = (self.t[id], self.k_cell[id]);
                let half = 0.5 * m.dx[a][ix[a]];
                let g = match bc {
                    ThermalBc::Adiabatic => 0.0,
                    ThermalBc::Fixed(tw) => k / half * (t - tw),
                    ThermalBc::Convective { h, t_out } => (t - t_out) / (half / k + 1.0 / h),
                    ThermalBc::Overall { u, t_out } => u * (t - t_out),
                };
                sum += closed * g * m.area(a, ix);
            }
        }
        if let Some(r) = &self.rad_load {
            sum += r.wall_out_w[face];
        }
        if face == 4
            && let (Some(sh), Some(pl)) = (&self.sheet, &self.cfg.plate)
        {
            sum +=
                sh.t.iter()
                    .zip(&sh.geo.area)
                    .map(|(t, a)| pl.u_out * a * (t - pl.t_out))
                    .sum::<f64>();
        }
        sum
    }

    /// Share of the rear face above column (`i`, `j`) that the plate sheet
    /// does not cover (1 without a sheet).
    fn rear_bare(&self, i: usize, j: usize) -> f64 {
        self.cfg
            .plate
            .as_ref()
            .map_or(1.0, |p| 1.0 - p.cover[i + self.mesh.n[0] * j])
    }

    /// Heat carried out through the openings relative to `t_ref`, W.
    pub fn open_heat_out_w(&self, t_ref: f64) -> f64 {
        let rho_cp = self.cfg.conductivity / self.cfg.alpha;
        self.open
            .faces
            .iter()
            .map(|f| {
                let u = f.outward() * self.vel[f.axis][f.vel];
                let tb = if u < 0.0 {
                    self.t_in[f.patch]
                } else {
                    self.t[f.cell]
                };
                rho_cp * u * f.area * (tb - t_ref)
            })
            .sum()
    }

    /// Heat stored in the grid relative to `t_ref`, J.
    pub fn stored_heat_j(&self, t_ref: f64) -> f64 {
        let m = &self.mesh;
        let mut sum = 0.0;
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let ix = [i, j, k];
                    let id = m.cell(ix);
                    sum += self.c_cell[id] * m.volume(ix) * (self.t[id] - t_ref);
                }
            }
        }
        if let (Some(sh), Some(pl)) = (&self.sheet, &self.cfg.plate) {
            sum +=
                sh.t.iter()
                    .zip(&sh.geo.area)
                    .map(|(t, a)| pl.cap * a * (t - t_ref))
                    .sum::<f64>();
        }
        sum
    }

    /// Volume flow out through patch `patch`, m3/s (negative = in).
    pub fn patch_flow_m3s(&self, patch: usize) -> f64 {
        self.open
            .faces
            .iter()
            .filter(|f| f.patch == patch)
            .map(|f| f.outward() * self.vel[f.axis][f.vel] * f.area)
            .sum()
    }

    /// Enthalpy carried out through all openings, W, relative to 0 degrees C
    /// (the net mass flow is zero at steady state, so the zero is
    /// immaterial there). Air going in carries its inflow temperature.
    pub fn open_enthalpy_out_w(&self) -> f64 {
        let rho_cp = self.cfg.conductivity / self.cfg.alpha;
        self.open
            .faces
            .iter()
            .map(|f| {
                let u = f.outward() * self.vel[f.axis][f.vel];
                let tb = if u < 0.0 {
                    self.t_in[f.patch]
                } else {
                    self.t[f.cell]
                };
                rho_cp * u * f.area * tb
            })
            .sum()
    }

    // ---- energy --------------------------------------------------------

    fn advance_temperature(&mut self, dt: f64) {
        let m = &self.mesh;
        let sch = self.cfg.scheme;
        let n = m.n;
        let mut rhs = vec![0.0; m.cells()];
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let ix = [i, j, k];
                    let mut conv = 0.0;
                    for a in 0..3 {
                        let face = |e: usize| -> f64 {
                            if e == 0 || e == n[a] {
                                return self.open_enthalpy_flux(a, e, ix);
                            }
                            let v = m.idx(a, with(ix, a, e));
                            let ua = self.vel[a][v];
                            if ua == 0.0 {
                                return 0.0;
                            }
                            let at = |q: usize| -> Sample {
                                (m.centre[a][q], self.t[m.cell(with(ix, a, q))])
                            };
                            let ll = (e >= 2).then(|| at(e - 2));
                            let uu = (e + 1 < n[a]).then(|| at(e + 1));
                            self.open.afrac[a][v]
                                * ua
                                * face_value(sch, m.edges[a][e], ll, at(e - 1), at(e), uu, ua > 0.0)
                        };
                        conv += (face(ix[a] + 1) - face(ix[a])) / m.dx[a][ix[a]];
                    }
                    let id = m.cell(ix);
                    rhs[id] = -conv * self.conv_scale[id] + self.heat_scale * self.q_cell[id];
                }
            }
        }
        let old = self.t.clone();
        let plate_old = self.sheet.as_ref().map(|s| s.t.clone());
        let (nu_t, wall_alpha) = (self.nu_t.as_deref(), self.wall_alpha.as_deref());
        let pr_t = self.cfg.prandtl_t;
        // conductivity of a cell with the eddy part added
        let k_eff =
            |id: usize| self.k_cell[id] + nu_t.map_or(0.0, |v| self.c_cell[id] * v[id] / pr_t);
        for _ in 0..200 {
            let mut change: f64 = 0.0;
            let mut scale: f64 = 1e-30;
            for k in 0..n[2] {
                for j in 0..n[1] {
                    for i in 0..n[0] {
                        let ix = [i, j, k];
                        let id = m.cell(ix);
                        let (kc, cc) = (self.k_cell[id], self.c_cell[id]);
                        let (mut a_sum, mut a_nb) = (0.0, 0.0);
                        for a in 0..3 {
                            let s = ix[a];
                            let w = m.dx[a][s];
                            for upper in [false, true] {
                                let face = 2 * a + upper as usize;
                                if (!upper && s == 0) || (upper && s + 1 == n[a]) {
                                    // wall function: replaces kc / cc (alpha) on this face
                                    let a_wall = wall_value(wall_alpha, m, &[ix], face);
                                    let kc = a_wall.map_or(kc, |aw| aw * cc);
                                    // an opening takes its share of the face off the wall
                                    let mut closed = 1.0
                                        - self.open.afrac[a]
                                            [m.idx(a, with(ix, a, if upper { n[a] } else { 0 }))];
                                    if a == 2 && !upper {
                                        closed *= self.rear_bare(i, j);
                                    }
                                    match self.cfg.faces[2 * a + upper as usize].thermal {
                                        ThermalBc::Adiabatic => {}
                                        ThermalBc::Fixed(tw) => {
                                            let cw = closed * kc / (cc * 0.5 * w * w);
                                            a_sum += cw;
                                            a_nb += cw * tw;
                                        }
                                        ThermalBc::Convective { h, t_out } => {
                                            // half cell in series with the film
                                            let r = 0.5 * w / kc + 1.0 / h;
                                            let cw = closed / (cc * r * w);
                                            a_sum += cw;
                                            a_nb += cw * t_out;
                                        }
                                        ThermalBc::Overall { u, t_out } => {
                                            let cw = closed * u / (cc * w);
                                            a_sum += cw;
                                            a_nb += cw * t_out;
                                        }
                                    }
                                } else {
                                    let q = if upper { s + 1 } else { s - 1 };
                                    let dist = (m.centre[a][q] - m.centre[a][s]).abs();
                                    let nb = m.cell(with(ix, a, q));
                                    // one conductance for the face, the same from both
                                    // cells so the heat leaving one enters the other
                                    let (lo, hi) = if upper { (id, nb) } else { (nb, id) };
                                    let (lo_ix, hi_ix) = if upper {
                                        (ix, with(ix, a, q))
                                    } else {
                                        (with(ix, a, q), ix)
                                    };
                                    let mut kf = self.k_face[a][hi];
                                    let (c_lo, c_hi) = (self.c_cell[lo], self.c_cell[hi]);
                                    let (w_lo, w_hi) = (m.dx[a][lo_ix[a]], m.dx[a][hi_ix[a]]);
                                    let wall_lo = wall_value(wall_alpha, m, &[lo_ix], 2 * a + 1);
                                    let wall_hi = wall_value(wall_alpha, m, &[hi_ix], 2 * a);
                                    if let Some(aw) = wall_lo {
                                        kf = dist
                                            / (0.5 * w_lo / (aw * c_lo)
                                                + 0.5 * w_hi / self.k_cell[hi]);
                                    } else if let Some(aw) = wall_hi {
                                        kf = dist
                                            / (0.5 * w_hi / (aw * c_hi)
                                                + 0.5 * w_lo / self.k_cell[lo]);
                                    } else if nu_t.is_some() {
                                        let eff = harmonic(k_eff(lo), k_eff(hi));
                                        let mol = harmonic(self.k_cell[lo], self.k_cell[hi]);
                                        kf *= eff / mol;
                                    }
                                    let cn = kf / (cc * dist * w);
                                    a_sum += cn;
                                    a_nb += cn * self.t[nb];
                                }
                            }
                        }
                        if k == 0
                            && let Some(sh) = &self.sheet
                        {
                            let col = i + n[0] * j;
                            let cw = sh.g_col[col] / (cc * m.volume(ix));
                            a_sum += cw;
                            a_nb += cw * sh.t[col];
                        }
                        if let Some(r) = &self.rad_load {
                            let cv = 1.0 / (cc * m.volume(ix));
                            a_sum += r.cell_g[id] * cv;
                            a_nb += r.cell_s[id] * cv;
                        }
                        let new = (old[id] / dt + rhs[id] + a_nb) / (1.0 / dt + a_sum);
                        change = change.max((new - self.t[id]).abs());
                        scale = scale.max(new.abs());
                        self.t[id] = new;
                    }
                }
            }
            if let (Some(sh), Some(pl), Some(po)) = (
                self.sheet.as_mut(),
                self.cfg.plate.as_ref(),
                plate_old.as_ref(),
            ) {
                change = change.max(sheet_sweep(
                    m,
                    sh,
                    pl,
                    &self.t,
                    dt,
                    po,
                    self.rad_load.as_ref(),
                ));
            }
            if change <= 1e-12 * scale {
                break;
            }
        }
    }
}

/// One Gauss-Seidel sweep of the plate sheet `sh` against cell temperatures
/// `t`; see [`FlowSolver::sheet_sweep`].
fn sheet_sweep(
    m: &Mesh,
    sh: &mut Sheet,
    pl: &PlateSheet,
    t: &[f64],
    dt: f64,
    po: &[f64],
    rad: Option<&crate::rad::RadLoad>,
) -> f64 {
    let mut change: f64 = 0.0;
    let nx = sh.geo.nx;
    #[allow(clippy::needless_range_loop)]
    for c in 0..sh.t.len() {
        let area = sh.geo.area[c];
        if area <= 0.0 {
            continue;
        }
        let (i, j) = (c % nx, c / nx);
        let cap = pl.cap * area / dt;
        let g_out = pl.u_out * area;
        let (mut num, mut den) = (cap * po[c] + g_out * pl.t_out, cap + g_out);
        if let Some(r) = rad {
            num += r.sheet_s[c];
            den += r.sheet_g[c];
        }
        let mut link = |g: f64, other: usize| {
            num += g * sh.t[other];
            den += g;
        };
        if i + 1 < nx {
            link(sh.geo.g_x[c], c + 1);
        }
        if i > 0 {
            link(sh.geo.g_x[c - 1], c - 1);
        }
        if c + nx < sh.t.len() {
            link(sh.geo.g_y[c], c + nx);
        }
        if c >= nx {
            link(sh.geo.g_y[c - nx], c - nx);
        }
        num += sh.g_col[c] * t[m.cell([i, j, 0])];
        den += sh.g_col[c];
        let new = num / den;
        change = change.max((new - sh.t[c]).abs());
        sh.t[c] = new;
    }
    change
}

fn with(mut ix: [usize; 3], axis: usize, v: usize) -> [usize; 3] {
    ix[axis] = v;
    ix
}

/// Harmonic mean of two diffusivities (exactly `a` when equal).
fn harmonic(a: f64, b: f64) -> f64 {
    if a == b { a } else { 2.0 * a * b / (a + b) }
}

/// Mean of the wall-function values of `cells` on `face`, if any is set.
fn wall_value(
    table: Option<&[[f64; 6]]>,
    m: &Mesh,
    cells: &[[usize; 3]],
    face: usize,
) -> Option<f64> {
    let table = table?;
    let (mut sum, mut count) = (0.0, 0);
    for c in cells {
        let v = table[m.cell(*c)][face];
        if !v.is_nan() {
            sum += v;
            count += 1;
        }
    }
    (count > 0).then(|| sum / count as f64)
}

#[cfg(test)]
mod film_tests {
    use super::film_conductance;

    #[test]
    fn the_film_runs_from_the_correlation_to_plain_conduction() {
        let (h, k) = (6.0, 0.026);
        let delta = 2.0 * k / h;
        // outside the layer: the film as given, radiation included
        assert_eq!(film_conductance(h + 3.0, 3.0, 2.0 * delta, k), h + 3.0);
        // at the edge of the layer the two branches meet
        let edge = film_conductance(h, 0.0, delta, k);
        let inside = film_conductance(h, 0.0, delta * 0.999, k);
        assert!((edge - inside).abs() < 0.01 * h, "{edge} {inside}");
        // far inside: conduction to the cell centre, k / y
        let y = delta * 1e-3;
        let g = film_conductance(h, 0.0, y, k);
        assert!((g * y / k - 1.0).abs() < 0.01, "{g}");
        // never below the film
        for i in 1..100 {
            assert!(film_conductance(h, 0.0, delta * i as f64 / 50.0, k) >= h - 1e-12);
        }
    }
}
