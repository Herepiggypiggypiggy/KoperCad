//! Steady-state iteration of the flow core (lane CFD-FAST).
//!
//! The time marching of [`FlowSolver::step`] has to resolve the convective
//! time scale (`dt` about `0.5 h / u`) all the way to the end of the thermal
//! transient: thousands of steps. A steady answer needs neither. This path
//! iterates the same equations as a pseudo-transient:
//!
//! - convection and diffusion implicit (first order upwind in the matrix, the
//!   configured scheme restored by a deferred correction), so the pseudo step
//!   may be many cell crossings long;
//! - the pressure gradient in the momentum equation and a pressure
//!   correction `phi` (`p += phi`, velocity `-= dt grad phi`), so the answer
//!   does not depend on the step (the plain projection of `step` leaves an
//!   error of order `nu dt / h^2`);
//! - the energy equation implicit with its own, longer, step;
//! - a few Gauss-Seidel sweeps per iteration instead of solving each system.
//!
//! At convergence the velocity, pressure and temperature satisfy the same
//! discrete equations as the converged time march. `FlowSolver::step` is
//! untouched and stays the reference (and the way to transients).

mod assemble;
mod sweep;

use super::FlowSolver;
use super::with;
use crate::flow::par::for_planes;
use sweep::{Coef, apply_defer, residual, sweep};

/// Tuning of the steady iteration.
#[derive(Debug, Clone, PartialEq)]
pub struct SteadyConfig {
    /// Courant number of the momentum pseudo step (`dt = cfl / max(|u|/h)`).
    pub cfl: f64,
    /// Largest pseudo step, s.
    pub dt_max: f64,
    /// The pseudo step may grow by this factor per iteration at most.
    pub dt_growth: f64,
    /// Energy pseudo step as a multiple of the momentum one.
    pub energy_factor: f64,
    /// Largest energy pseudo step, s.
    pub energy_dt_max: f64,
    /// Gauss-Seidel sweeps per momentum component and iteration.
    pub momentum_sweeps: usize,
    /// Most sweeps for the energy equation per iteration.
    pub energy_sweeps: usize,
    /// The energy sweeps stop when a sweep changes `T` by less than this
    /// fraction of the change of the first sweep.
    pub energy_drop: f64,
    /// Relative tolerance of the pressure correction solve.
    pub pressure_tol: f64,
    pub pressure_max_iter: usize,
    /// Share of the new deferred correction (1 = no damping).
    pub defer_relax: f64,
}

impl Default for SteadyConfig {
    fn default() -> Self {
        SteadyConfig {
            cfl: 10.0,
            dt_max: 100.0,
            dt_growth: 1.5,
            energy_factor: 10.0,
            energy_dt_max: 1000.0,
            momentum_sweeps: 3,
            energy_sweeps: 20,
            energy_drop: 0.1,
            pressure_tol: 0.05,
            pressure_max_iter: 50,
            defer_relax: 1.0,
        }
    }
}

/// What one iteration did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteadyInfo {
    /// Momentum pseudo step, s.
    pub dt: f64,
    /// Largest velocity change over the iteration, m/s.
    pub max_du: f64,
    /// Largest temperature change over the iteration, K.
    pub max_dt_k: f64,
    /// Net flux of the worst cell before the pressure correction over the
    /// largest face flux (the continuity residual).
    pub continuity: f64,
    pub pressure_iterations: usize,
    pub energy_sweeps: usize,
}

/// Work arrays and the state that lives between iterations.
pub struct SteadyState {
    pub cfg: SteadyConfig,
    pub iteration: usize,
    dt: f64,
    coef_u: [Vec<Coef>; 3],
    coef_t: Vec<Coef>,
    phi: Vec<f64>,
    defer_u: [Vec<f64>; 3],
    defer_t: Vec<f64>,
    /// Seconds spent so far in: energy assembly, energy sweeps, momentum
    /// assembly, momentum sweeps, pressure correction.
    pub phase_s: [f64; 5],
}

impl SteadyState {
    pub fn new(flow: &FlowSolver, cfg: SteadyConfig) -> SteadyState {
        let m = &flow.mesh;
        SteadyState {
            cfg,
            iteration: 0,
            dt: 0.0,
            coef_u: std::array::from_fn(|c| vec![Coef::default(); m.len(c)]),
            coef_t: vec![Coef::default(); m.cells()],
            phi: vec![0.0; m.cells()],
            defer_u: std::array::from_fn(|c| vec![0.0; m.len(c)]),
            defer_t: vec![0.0; m.cells()],
            phase_s: [0.0; 5],
        }
    }

    /// The momentum pseudo step of the last iteration, s.
    pub fn dt(&self) -> f64 {
        self.dt
    }
}

impl FlowSolver {
    /// One iteration of the steady path; see the module documentation.
    pub fn steady_iterate(&mut self, st: &mut SteadyState) -> SteadyInfo {
        let cfg = st.cfg.clone();
        // pseudo step
        let mut dt = self.max_dt(cfg.cfl).min(cfg.dt_max);
        if st.dt > 0.0 {
            dt = dt.min(st.dt * cfg.dt_growth);
        } else {
            dt = dt.min(cfg.cfl * self.mesh_h_min() / 0.1);
        }
        st.dt = dt;
        let dt_t = (dt * cfg.energy_factor).clamp(dt, cfg.energy_dt_max.max(dt));

        let clock = std::time::Instant::now();
        let mut lap = {
            let mut last = clock;
            move |slot: &mut f64| {
                let now = std::time::Instant::now();
                *slot += (now - last).as_secs_f64();
                last = now;
            }
        };
        self.update_drag_rate();
        // ---- energy
        let old_t = self.t.clone();
        self.assemble_energy(dt_t, &mut st.coef_t);
        apply_defer(&mut st.coef_t, &mut st.defer_t, cfg.defer_relax);
        lap(&mut st.phase_s[0]);
        let n = self.mesh.n;
        let mut first = 0.0;
        let mut sweeps = 0;
        for s in 0..cfg.energy_sweeps.max(1) {
            let ch = sweep(&st.coef_t, &mut self.t, n, s % 2 == 0);
            sweeps += 1;
            if s == 0 {
                first = ch;
            } else if ch <= cfg.energy_drop * first {
                break;
            }
        }
        lap(&mut st.phase_s[1]);
        let max_dt_k = self
            .t
            .iter()
            .zip(&old_t)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        self.t_buoyancy_ref = self.cfg.t_ref.unwrap_or_else(|| self.mean_temperature());

        // ---- momentum
        let old_vel = self.vel.clone();
        for c in 0..3 {
            self.assemble_momentum(c, dt, &mut st.coef_u[c]);
            apply_defer(&mut st.coef_u[c], &mut st.defer_u[c], cfg.defer_relax);
        }
        lap(&mut st.phase_s[2]);
        for c in 0..3 {
            let d = self.mesh.dims(c);
            for s in 0..cfg.momentum_sweeps.max(1) {
                sweep(&st.coef_u[c], &mut self.vel[c], d, s % 2 == 0);
            }
        }

        lap(&mut st.phase_s[3]);
        // ---- pressure correction
        self.predict_open_faces(dt);
        let (continuity, iters) = self.project_incremental(dt, &cfg, &mut st.phi);
        lap(&mut st.phase_s[4]);
        let max_du = (0..3)
            .flat_map(|c| {
                self.vel[c]
                    .iter()
                    .zip(&old_vel[c])
                    .map(|(a, b)| (a - b).abs())
            })
            .fold(0.0, f64::max);
        st.iteration += 1;
        SteadyInfo {
            dt,
            max_du,
            max_dt_k,
            continuity,
            pressure_iterations: iters,
            energy_sweeps: sweeps,
        }
    }

    fn mesh_h_min(&self) -> f64 {
        self.mesh
            .dx
            .iter()
            .flat_map(|v| v.iter())
            .cloned()
            .fold(f64::MAX, f64::min)
    }

    /// Like `project`, but for the correction `phi` to the pressure that is
    /// already in the momentum equation: `p += phi`.
    fn project_incremental(
        &mut self,
        dt: f64,
        cfg: &SteadyConfig,
        phi: &mut [f64],
    ) -> (f64, usize) {
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
        self.set_open_faces_incremental(dt);
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
        let mut flux_scale: f64 = 1e-30;
        for a in 0..3 {
            let d = m.dims(a);
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        let id = m.idx(a, ix);
                        let area = m.area(a, with(ix, a, ix[a].min(m.n[a] - 1)));
                        flux_scale = flux_scale.max((vel[a][id] * af[a][id] * area).abs());
                    }
                }
            }
        }
        let continuity = b.iter().fold(0.0_f64, |a, v| a.max(v.abs())) * dt / flux_scale;
        phi.iter_mut().for_each(|v| *v = 0.0);
        let iters = self
            .pressure
            .solve(&b, phi, cfg.pressure_tol, cfg.pressure_max_iter);
        let drag = &self.drag_rate;
        let phi_ref: &[f64] = phi;
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
                        plane[j * d[0] + i] -=
                            g * dt * (phi_ref[m.cell(ix)] - phi_ref[m.cell(lo)]) / dist;
                    }
                }
            });
        }
        // open faces: the correction through the cell's pressure change
        for (n, f) in self.open.faces.iter().enumerate() {
            let sgn = f.outward();
            let u_out =
                sgn * self.vel[f.axis][f.vel] + self.open_alpha[n] * dt * phi_ref[f.cell] / f.dh;
            self.vel[f.axis][f.vel] = sgn * u_out;
            self.open_speed[n] = 0.5 * (self.open_speed[n] + u_out.abs());
        }
        for (p, d) in self.p.iter_mut().zip(phi_ref) {
            *p += d;
        }
        (continuity, iters)
    }

    /// `set_open_faces` for the incremental form: the open face law sees the
    /// pressure already in the cell, the correction adds the change.
    fn set_open_faces_incremental(&mut self, dt: f64) {
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
            self.vel[f.axis][f.vel] =
                sgn * alpha * (u_star - dt * pj / f.dh + dt * self.p[f.cell] / f.dh);
        }
        self.pressure.set_open_conductance(&g);
    }

    /// Equation residual and scale of the energy equation at the current
    /// state (diagnostic; assembles with the given step).
    pub fn steady_energy_residual(&self, st: &mut SteadyState, dt: f64) -> (f64, f64) {
        self.assemble_energy(dt, &mut st.coef_t);
        residual(&st.coef_t, &self.t, self.mesh.n)
    }
}
