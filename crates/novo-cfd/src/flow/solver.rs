use super::mesh::Mesh;
use super::par::{for_planes, max_over_planes};
use super::pressure::{PressureSolver, Smoother};
use super::scheme::{Sample, Scheme, face_value};
use crate::openings::{OpenPatch, OpenSet};
use crate::solids::Medium;

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
}

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
            vel,
            p: vec![0.0; cells],
            t: vec![t_init; cells],
            time: 0.0,
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
        let iters = self.pressure.solve(&b, &mut self.p, 1e-9, 200);
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
                let closed = 1.0 - self.open.afrac[a][m.idx(a, with(ix, a, edge))];
                let id = m.cell(ix);
                let (t, k) = (self.t[id], self.k_cell[id]);
                let half = 0.5 * m.dx[a][ix[a]];
                let g = match bc {
                    ThermalBc::Adiabatic => 0.0,
                    ThermalBc::Fixed(tw) => k / half * (t - tw),
                    ThermalBc::Convective { h, t_out } => (t - t_out) / (half / k + 1.0 / h),
                };
                sum += closed * g * m.area(a, ix);
            }
        }
        sum
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
                    rhs[id] = -conv * self.conv_scale[id] + self.q_cell[id];
                }
            }
        }
        let old = self.t.clone();
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
                                    let closed = 1.0
                                        - self.open.afrac[a]
                                            [m.idx(a, with(ix, a, if upper { n[a] } else { 0 }))];
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
                        let new = (old[id] / dt + rhs[id] + a_nb) / (1.0 / dt + a_sum);
                        change = change.max((new - self.t[id]).abs());
                        scale = scale.max(new.abs());
                        self.t[id] = new;
                    }
                }
            }
            if change <= 1e-12 * scale {
                break;
            }
        }
    }
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
