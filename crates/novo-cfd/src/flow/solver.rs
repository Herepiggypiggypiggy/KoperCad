use super::mesh::Mesh;
use super::pressure::PressureSolver;
use super::scheme::{Sample, Scheme, face_value};

/// Thermal condition on a wall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalBc {
    Adiabatic,
    /// Fixed wall temperature, degrees C.
    Fixed(f64),
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
    /// Thermal expansion coefficient, 1/K.
    pub beta: f64,
    /// Gravity vector, m/s2, in the frame of the mesh (`y` down: `[0, 9.81, 0]`).
    pub gravity: [f64; 3],
    pub scheme: Scheme,
    pub faces: [FaceBc; 6],
    /// Volumetric heating per cell, K/s (heat / (rho cp)), or none.
    pub heat_source: Option<Vec<f64>>,
}

impl FlowConfig {
    /// Air near 25 degrees C with closed no-slip adiabatic walls.
    pub fn air() -> FlowConfig {
        FlowConfig {
            nu: 1.55e-5,
            alpha: 2.2e-5,
            beta: 1.0 / 298.15,
            gravity: [0.0, 9.81, 0.0],
            scheme: Scheme::VanLeer,
            faces: [FaceBc::WALL; 6],
            heat_source: None,
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
    t_buoyancy_ref: f64,
    pressure: PressureSolver,
}

impl FlowSolver {
    pub fn new(mesh: Mesh, cfg: FlowConfig, t_init: f64) -> FlowSolver {
        let vel = std::array::from_fn(|c| vec![0.0; mesh.len(c)]);
        let cells = mesh.cells();
        let pressure = PressureSolver::new(&mesh);
        FlowSolver {
            vel,
            p: vec![0.0; cells],
            t: vec![t_init; cells],
            time: 0.0,
            t_buoyancy_ref: t_init,
            pressure,
            mesh,
            cfg,
        }
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
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        let s = ix[c].min(m.n[c] - 1);
                        let h = m.dx[c][s];
                        rate = rate.max(self.vel[c][m.idx(c, ix)].abs() / h);
                    }
                }
            }
        }
        cfl / rate
    }

    /// Largest cell divergence `|div u|` times the cell volume over the
    /// smallest face area scale: the net outflow of a cell relative to the
    /// largest face flux (0 = exactly divergence free).
    pub fn divergence_max(&self) -> f64 {
        let m = &self.mesh;
        let mut worst: f64 = 0.0;
        let mut scale: f64 = 1e-30;
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let ix = [i, j, k];
                    let mut net = 0.0;
                    for a in 0..3 {
                        let mut hi = ix;
                        hi[a] += 1;
                        let (fl, fh) = (
                            self.vel[a][m.idx(a, ix)] * m.area(a, ix),
                            self.vel[a][m.idx(a, hi)] * m.area(a, ix),
                        );
                        net += fh - fl;
                        scale = scale.max(fl.abs()).max(fh.abs());
                    }
                    worst = worst.max(net.abs());
                }
            }
        }
        worst / scale
    }

    /// Advances one step of `dt` seconds.
    pub fn step(&mut self, dt: f64) -> StepInfo {
        let old_t = self.t.clone();
        let old_vel = self.vel.clone();
        self.advance_temperature(dt);
        // In a closed box a uniform temperature offset is a pure pressure
        // gradient, so the Boussinesq term uses the volume mean as reference.
        self.t_buoyancy_ref = self.mean_temperature();
        let rhs: [Vec<f64>; 3] = std::array::from_fn(|c| self.momentum_rhs(c));
        for (c, r) in rhs.iter().enumerate() {
            self.diffuse_velocity(c, r, dt);
        }
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
        for k in 0..d[2] {
            for j in 0..d[1] {
                for i in 0..d[0] {
                    let ix = [i, j, k];
                    if ix[c] == 0 || ix[c] == m.n[c] {
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
                                    return 0.0;
                                }
                                // advecting velocity u_b at b-edge e, c-edge ix[c]
                                let (lo, hi) = (ix[c] - 1, ix[c]);
                                let w_lo = m.dx[c][hi] / (m.dx[c][lo] + m.dx[c][hi]);
                                let mut ia = with(ix, b, e);
                                ia[c] = lo;
                                let mut ib = ia;
                                ib[c] = hi;
                                let ub =
                                    w_lo * u[b][m.idx(b, ia)] + (1.0 - w_lo) * u[b][m.idx(b, ib)];
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
                    out[m.idx(c, ix)] = -conv + buoy;
                }
            }
        }
        out
    }

    /// Solves `u/dt - nu L u = u_old/dt + rhs` by Gauss-Seidel.
    fn diffuse_velocity(&mut self, c: usize, rhs: &[f64], dt: f64) {
        let m = &self.mesh;
        let d = m.dims(c);
        let nu = self.cfg.nu;
        let old = self.vel[c].clone();
        let rhs_total: Vec<f64> = old.iter().zip(rhs).map(|(o, r)| o / dt + r).collect();
        for _ in 0..200 {
            let mut change: f64 = 0.0;
            let mut scale: f64 = 1e-30;
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        if ix[c] == 0 || ix[c] == m.n[c] {
                            continue;
                        }
                        let id = m.idx(c, ix);
                        let (mut a_sum, mut a_nb) = (0.0, 0.0);
                        for b in 0..3 {
                            let s = ix[b];
                            if b == c {
                                let w = 0.5 * (m.dx[c][s - 1] + m.dx[c][s]);
                                let (cl, cu) = (nu / (m.dx[c][s - 1] * w), nu / (m.dx[c][s] * w));
                                a_sum += cl + cu;
                                a_nb += cl * self.vel[c][m.idx(c, with(ix, c, s - 1))]
                                    + cu * self.vel[c][m.idx(c, with(ix, c, s + 1))];
                            } else {
                                let w = m.dx[b][s];
                                for upper in [false, true] {
                                    if (!upper && s == 0) || (upper && s + 1 == m.n[b]) {
                                        let bc = self.cfg.faces[2 * b + upper as usize];
                                        if !bc.slip {
                                            let cw = nu / (0.5 * w * w);
                                            a_sum += cw;
                                            a_nb += cw * bc.wall_velocity[c];
                                        }
                                    } else {
                                        let q = if upper { s + 1 } else { s - 1 };
                                        let dist = (m.centre[b][q] - m.centre[b][s]).abs();
                                        let cn = nu / (dist * w);
                                        a_sum += cn;
                                        a_nb += cn * self.vel[c][m.idx(c, with(ix, b, q))];
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
        let m = &self.mesh;
        let mut b = vec![0.0; m.cells()];
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let ix = [i, j, k];
                    let mut net = 0.0;
                    for a in 0..3 {
                        net += (self.vel[a][m.idx(a, with(ix, a, ix[a] + 1))]
                            - self.vel[a][m.idx(a, ix)])
                            * m.area(a, ix);
                    }
                    b[m.cell(ix)] = -net / dt;
                }
            }
        }
        let iters = self.pressure.solve(&b, &mut self.p, 1e-9, 200);
        for a in 0..3 {
            let d = m.dims(a);
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        if ix[a] == 0 || ix[a] == m.n[a] {
                            continue;
                        }
                        let lo = with(ix, a, ix[a] - 1);
                        let dist = m.centre[a][ix[a]] - m.centre[a][ix[a] - 1];
                        self.vel[a][m.idx(a, ix)] -=
                            dt * (self.p[m.cell(ix)] - self.p[m.cell(lo)]) / dist;
                    }
                }
            }
        }
        iters
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
                                return 0.0;
                            }
                            let ua = self.vel[a][m.idx(a, with(ix, a, e))];
                            let at = |q: usize| -> Sample {
                                (m.centre[a][q], self.t[m.cell(with(ix, a, q))])
                            };
                            let ll = (e >= 2).then(|| at(e - 2));
                            let uu = (e + 1 < n[a]).then(|| at(e + 1));
                            ua * face_value(sch, m.edges[a][e], ll, at(e - 1), at(e), uu, ua > 0.0)
                        };
                        conv += (face(ix[a] + 1) - face(ix[a])) / m.dx[a][ix[a]];
                    }
                    let q = self.cfg.heat_source.as_ref().map_or(0.0, |s| s[m.cell(ix)]);
                    rhs[m.cell(ix)] = -conv + q;
                }
            }
        }
        let alpha = self.cfg.alpha;
        let old = self.t.clone();
        for _ in 0..200 {
            let mut change: f64 = 0.0;
            let mut scale: f64 = 1e-30;
            for k in 0..n[2] {
                for j in 0..n[1] {
                    for i in 0..n[0] {
                        let ix = [i, j, k];
                        let id = m.cell(ix);
                        let (mut a_sum, mut a_nb) = (0.0, 0.0);
                        for a in 0..3 {
                            let s = ix[a];
                            let w = m.dx[a][s];
                            for upper in [false, true] {
                                if (!upper && s == 0) || (upper && s + 1 == n[a]) {
                                    if let ThermalBc::Fixed(tw) =
                                        self.cfg.faces[2 * a + upper as usize].thermal
                                    {
                                        let cw = alpha / (0.5 * w * w);
                                        a_sum += cw;
                                        a_nb += cw * tw;
                                    }
                                } else {
                                    let q = if upper { s + 1 } else { s - 1 };
                                    let dist = (m.centre[a][q] - m.centre[a][s]).abs();
                                    let cn = alpha / (dist * w);
                                    a_sum += cn;
                                    a_nb += cn * self.t[m.cell(with(ix, a, q))];
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
