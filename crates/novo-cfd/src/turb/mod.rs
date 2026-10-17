//! Turbulence for the flow core (lane CFD-B): k-omega SST (default) and a
//! k-epsilon cross check, both with wall functions on every wall of the
//! fluid (domain faces and solid cells).
//!
//! The models work on the cell grid and read the face velocities of the flow
//! core. They produce the turbulent viscosity `nu_t` per cell (for the
//! momentum diffusion `nu + nu_t` and the energy conductivity
//! `k + cp mu_t / Pr_t`) and the wall data (`WallCell`) for the shear and
//! heat flux at wall-adjacent cells. Laminar runs never create a state, so
//! the laminar benchmarks cannot change.
//!
//! Honest limits: wall functions assume attached, near-equilibrium
//! boundary layers; SST and k-epsilon are steady RANS closures and do not
//! resolve transition, strong separation or unsteady plumes; convection of
//! `k` and `omega` is first-order upwind. In natural convection the wall
//! functions (made for forced flow) only work when the wall cells resolve the
//! boundary layer: with a first cell thicker than the layer both models
//! under-predict the wall heat flux by about half (measured, see
//! `tests/turb_plate.rs`). Use resolved wall cells there.
//! See `docs/thermal-validation.md`.
//!
//! [`TurbulentFlow`] couples a model to the flow core through its hooks.
//!
//! Where the user finds it: Cabinet view, Heat layer, Simulate, turbulence
//! model in the solver options (lane CFD-F wires it); `turb_visc_ratio` is
//! the "Turbulence" field of the snapshot.

// Per-cell kernels index several parallel arrays by cell; iterators would
// obscure them.
#![allow(clippy::needless_range_loop)]

mod coupling;
mod mesh;
mod transport;
mod wall;

pub use coupling::{TurbulentFlow, grid_of};
pub use mesh::{FaceKind, TurbMesh, WallFace};
pub use transport::Equation;
pub use wall::{WallCell, friction_velocity, thermal_t_plus, wall_quantities};

/// Von Karman constant.
pub const KAPPA: f64 = 0.41;
/// `beta*` of the omega models and `C_mu` of k-epsilon.
const BETA_STAR: f64 = 0.09;
const C_MU: f64 = 0.09;
const NU_T_RATIO_MAX: f64 = 1e5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TurbModel {
    /// No turbulence model (the laminar benchmarks).
    #[default]
    Laminar,
    /// Menter k-omega SST (2003), automatic wall treatment.
    KOmegaSst,
    /// Standard k-epsilon with equilibrium wall functions (cross check).
    KEpsilon,
}

/// Buoyancy for the `k` equation: `G_b = beta (nu_t / Pr_t) g . grad T`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Buoyancy {
    /// Gravity in the mesh frame (`y` down: `[0, 9.81, 0]`).
    pub gravity: [f64; 3],
    pub beta: f64,
}

#[derive(Debug, Clone)]
pub struct TurbConfig {
    pub model: TurbModel,
    /// Kinematic viscosity, m2/s.
    pub nu: f64,
    /// Molecular Prandtl number.
    pub pr: f64,
    /// Turbulent Prandtl number (0.85).
    pub pr_t: f64,
    pub faces: [FaceKind; 6],
    pub buoyancy: Option<Buoyancy>,
    /// Initial turbulence intensity (fraction of the reference speed).
    pub intensity: f64,
    /// Initial turbulent length scale, m.
    pub length_scale: f64,
    /// Second-order (limited) convection of k and omega/epsilon; the
    /// default first-order upwind is more diffusive but never overshoots.
    pub second_order: bool,
}

impl TurbConfig {
    /// Air near 25 degrees C, closed walls, SST.
    pub fn air(model: TurbModel) -> TurbConfig {
        TurbConfig {
            model,
            nu: 1.55e-5,
            pr: 0.71,
            pr_t: 0.85,
            faces: [FaceKind::Wall; 6],
            buoyancy: None,
            intensity: 0.05,
            length_scale: 0.01,
            second_order: false,
        }
    }
}

/// What one `advance` did.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TurbStep {
    /// Largest relative change of `k` over the step.
    pub k_change: f64,
    /// Largest relative change of `omega` or `epsilon`.
    pub w_change: f64,
    pub max_nu_t_ratio: f64,
}

pub struct TurbState {
    pub mesh: TurbMesh,
    pub cfg: TurbConfig,
    /// Turbulent kinetic energy, m2/s2.
    pub k: Vec<f64>,
    /// Specific dissipation `omega` (1/s) for SST, dissipation `epsilon`
    /// (m2/s3) for k-epsilon.
    pub w: Vec<f64>,
    /// Turbulent kinematic viscosity, m2/s.
    pub nu_t: Vec<f64>,
    walls: Vec<WallCell>,
    /// Per cell: friction velocity and wall distance of the nearest wall
    /// face (`u_tau` zero away from walls).
    wall_cell: Vec<Option<(f64, f64)>>,
    /// Cells that hold part solid, part air; the flow core keeps its own
    /// conductance there (empty: none).
    cut: Vec<bool>,
    /// Device cells whose air faces the film model of the flow core owns
    /// (empty: none); the wall function gives no heat transfer there.
    film: Vec<bool>,
}

impl TurbState {
    /// `ref_speed` (m/s) sets the initial `k = 1.5 (I U)^2`.
    pub fn new(
        grid: &crate::result::Grid3,
        solid: Option<&[bool]>,
        cfg: TurbConfig,
        ref_speed: f64,
    ) -> TurbState {
        let mesh = TurbMesh::new(grid, solid, cfg.faces);
        let n = mesh.cells();
        let k0 = (1.5 * (cfg.intensity * ref_speed).powi(2)).max(1e-10);
        let l = cfg.length_scale.max(1e-6);
        let w0 = match cfg.model {
            TurbModel::KEpsilon => C_MU.powf(0.75) * k0.powf(1.5) / l,
            _ => k0.sqrt() / (C_MU.powf(0.25) * l),
        };
        let mut s = TurbState {
            k: vec![k0; n],
            w: vec![w0.max(1e-6); n],
            nu_t: vec![0.0; n],
            walls: Vec::new(),
            wall_cell: vec![None; n],
            cut: Vec::new(),
            film: Vec::new(),
            mesh,
            cfg,
        };
        s.initialise_wall_layer(0.05 * ref_speed);
        s.update_nu_t_simple();
        s
    }

    /// Start `omega` (or `epsilon`) from the wall layer: the larger of the
    /// free value and the log-layer value for friction velocity `u_tau`
    /// at the wall distance of the cell. Avoids starting on the
    /// laminar-like branch next to walls.
    pub fn initialise_wall_layer(&mut self, u_tau: f64) {
        let nu = self.cfg.nu;
        for c in 0..self.w.len() {
            if !self.mesh.fluid[c] || u_tau <= 0.0 {
                continue;
            }
            let d = self.mesh.wall_dist[c].max(1e-9);
            let guess = match self.cfg.model {
                TurbModel::KEpsilon => C_MU.powf(0.75) * self.k[c].powf(1.5) / (KAPPA * d),
                _ => {
                    let w_vis = 6.0 * nu / (SST_BETA1 * d * d);
                    let w_log = u_tau / (BETA_STAR.sqrt() * KAPPA * d);
                    (w_vis * w_vis + w_log * w_log).sqrt()
                }
            };
            self.w[c] = self.w[c].max(guess);
        }
    }

    /// Boundary faces `(cell, axis, high)` that are open (vents, fans): they
    /// get no wall treatment.
    pub fn mark_open(&mut self, faces: &[(usize, usize, bool)]) {
        self.mesh.mark_open(faces);
    }

    /// Wall data from the last `advance`.
    pub fn walls(&self) -> &[WallCell] {
        &self.walls
    }

    /// Cell values for a snapshot: `nu_t / nu`.
    pub fn viscosity_ratio(&self) -> Vec<f32> {
        self.nu_t.iter().map(|v| (v / self.cfg.nu) as f32).collect()
    }

    /// Turbulent heat diffusivity `nu_t / Pr_t`, m2/s, per cell.
    pub fn alpha_t(&self) -> Vec<f64> {
        self.nu_t.iter().map(|v| v / self.cfg.pr_t).collect()
    }

    fn update_nu_t_simple(&mut self) {
        if self.cfg.model == TurbModel::Laminar {
            self.nu_t.iter_mut().for_each(|v| *v = 0.0);
            return;
        }
        if self.cfg.model == TurbModel::KEpsilon {
            for c in 0..self.nu_t.len() {
                self.nu_t[c] = if self.mesh.fluid[c] {
                    C_MU * self.k[c] * self.k[c] / self.w[c].max(1e-30)
                } else {
                    0.0
                };
            }
        } else {
            for c in 0..self.nu_t.len() {
                self.nu_t[c] = if self.mesh.fluid[c] {
                    self.k[c] / self.w[c].max(1e-30)
                } else {
                    0.0
                };
            }
        }
        self.limit_nu_t();
    }

    fn limit_nu_t(&mut self) {
        let cap = NU_T_RATIO_MAX * self.cfg.nu;
        for v in &mut self.nu_t {
            *v = v.clamp(0.0, cap);
        }
    }

    /// Advance `k` and `omega` (or `epsilon`) by `dt` with the given face
    /// velocities and, for buoyancy, the temperature field. Several calls
    /// converge a steady state (pseudo time); `dt` may be large.
    pub fn advance(
        &mut self,
        vel: &[Vec<f64>; 3],
        temperature: Option<&[f64]>,
        dt: f64,
        sweeps: usize,
    ) -> TurbStep {
        if self.cfg.model == TurbModel::Laminar {
            return TurbStep::default();
        }
        let uc = self.mesh.cell_velocity(vel);
        let grad_u = self.velocity_gradient(&uc, vel);
        let mut strain: Vec<f64> = grad_u.iter().map(strain_magnitude).collect();
        self.update_walls(&uc);
        // A wall cell sits in the law of the wall, where the no-slip finite
        // difference over half a cell badly overestimates the strain.
        for (c, w) in self.wall_cell.iter().enumerate() {
            if let Some((u_tau, d)) = *w {
                strain[c] = u_tau * u_tau / (self.cfg.nu + KAPPA * u_tau * d);
            }
        }
        let grad_t = temperature.map(|t| self.scalar_gradient(t));
        match self.cfg.model {
            TurbModel::KOmegaSst => self.advance_sst(vel, &strain, grad_t.as_deref(), dt, sweeps),
            TurbModel::KEpsilon => self.advance_ke(vel, &strain, grad_t.as_deref(), dt, sweeps),
            TurbModel::Laminar => TurbStep::default(),
        }
    }

    // ---- walls ---------------------------------------------------------

    fn update_walls(&mut self, uc: &[[f64; 3]]) {
        let (nu, pr) = (self.cfg.nu, self.cfg.pr);
        self.walls.clear();
        self.wall_cell.iter_mut().for_each(|w| *w = None);
        for wf in &self.mesh.walls {
            let u = uc[wf.cell];
            let t2: f64 = (0..3).filter(|a| *a != wf.axis).map(|a| u[a] * u[a]).sum();
            let (u_tau, y_plus, tau_w, nu_wall, alpha_wall) =
                wall_quantities(t2.sqrt(), wf.dist, nu, pr);
            self.walls.push(WallCell {
                cell: wf.cell,
                axis: wf.axis,
                high: wf.high,
                dist: wf.dist,
                area: wf.area,
                u_tau,
                y_plus,
                tau_w,
                nu_wall,
                alpha_wall,
            });
            let e = &mut self.wall_cell[wf.cell];
            match e {
                Some((ut, d)) => {
                    *ut = ut.max(u_tau);
                    *d = d.min(wf.dist);
                }
                None => *e = Some((u_tau, wf.dist)),
            }
        }
    }

    /// `(production, omega or epsilon)` of a wall cell under the wall
    /// treatment, from friction velocity `u_tau` at distance `d`.
    fn wall_production(&self, u_tau: f64, d: f64) -> f64 {
        // strain of the wall cell: u_tau^2 / (nu + kappa u_tau d) goes to
        // u_tau^2/nu in the sublayer and to u_tau/(kappa d) in the log layer
        let s = u_tau * u_tau / (self.cfg.nu + KAPPA * u_tau * d);
        u_tau * u_tau * s
    }

    // ---- gradients -----------------------------------------------------

    /// `g[a][b] = d u_a / d x_b` at cell centres. Walls and solids impose no
    /// slip; other domain faces use the face velocity for the normal
    /// component and zero gradient for the tangential ones.
    fn velocity_gradient(&self, uc: &[[f64; 3]], vel: &[Vec<f64>; 3]) -> Vec<[[f64; 3]; 3]> {
        let m = &self.mesh;
        let mut out = vec![[[0.0; 3]; 3]; m.cells()];
        for c in 0..m.cells() {
            if !m.fluid[c] {
                continue;
            }
            let ix = m.ijk(c);
            for b in 0..3 {
                for a in 0..3 {
                    let u0 = uc[c][a];
                    // (distance, value) on each side
                    let side = |high: bool| -> Option<(f64, f64)> {
                        let hw = 0.5 * m.dx[b][ix[b]];
                        match m.neighbour(ix, b, high) {
                            Some(nb) => {
                                let cn = m.cell(nb);
                                if m.fluid[cn] {
                                    Some((
                                        (m.centre[b][nb[b]] - m.centre[b][ix[b]]).abs(),
                                        uc[cn][a],
                                    ))
                                } else {
                                    Some((hw, 0.0))
                                }
                            }
                            None => match m.boundary_kind(c, b, high) {
                                FaceKind::Wall => Some((hw, 0.0)),
                                FaceKind::Symmetry => {
                                    if a == b {
                                        let mut e = ix;
                                        e[b] += usize::from(high);
                                        Some((hw, vel[a][m.vel_idx(a, e)]))
                                    } else {
                                        None
                                    }
                                }
                            },
                        }
                    };
                    out[c][a][b] = one_d_gradient(u0, side(false), side(true));
                }
            }
        }
        out
    }

    /// Gradient of a cell scalar, zero-gradient at boundaries.
    fn scalar_gradient(&self, phi: &[f64]) -> Vec<[f64; 3]> {
        let m = &self.mesh;
        let mut out = vec![[0.0; 3]; m.cells()];
        for c in 0..m.cells() {
            if !m.fluid[c] {
                continue;
            }
            let ix = m.ijk(c);
            for b in 0..3 {
                let side = |high: bool| -> Option<(f64, f64)> {
                    let nb = m.neighbour(ix, b, high)?;
                    let cn = m.cell(nb);
                    m.fluid[cn].then(|| ((m.centre[b][nb[b]] - m.centre[b][ix[b]]).abs(), phi[cn]))
                };
                out[c][b] = one_d_gradient(phi[c], side(false), side(true));
            }
        }
        out
    }

    fn buoyancy_production(&self, gt: Option<&[[f64; 3]]>, c: usize) -> f64 {
        match (&self.cfg.buoyancy, gt) {
            (Some(b), Some(g)) => {
                let gdt: f64 = (0..3).map(|a| b.gravity[a] * g[c][a]).sum();
                b.beta * self.nu_t[c] / self.cfg.pr_t * gdt
            }
            _ => 0.0,
        }
    }

    // ---- k-omega SST ---------------------------------------------------

    fn advance_sst(
        &mut self,
        vel: &[Vec<f64>; 3],
        strain: &[f64],
        grad_t: Option<&[[f64; 3]]>,
        dt: f64,
        sweeps: usize,
    ) -> TurbStep {
        let n = self.mesh.cells();
        let nu = self.cfg.nu;
        let gk = self.scalar_gradient(&self.k);
        let gw = self.scalar_gradient(&self.w);
        let mut f1 = vec![0.0; n];
        let mut nu_t = vec![0.0; n];
        for c in 0..n {
            if !self.mesh.fluid[c] {
                continue;
            }
            let d = self.mesh.wall_dist[c].max(1e-12);
            let (k, w) = (self.k[c].max(1e-12), self.w[c].max(1e-9));
            let cd = (2.0 * SST_SIGMA_W2 / w * dot(&gk[c], &gw[c])).max(1e-10);
            let sqk = k.sqrt();
            let arg1 = (sqk / (BETA_STAR * w * d))
                .max(500.0 * nu / (d * d * w))
                .min(4.0 * SST_SIGMA_W2 * k / (cd * d * d));
            f1[c] = arg1.powi(4).tanh();
            let arg2 = (2.0 * sqk / (BETA_STAR * w * d)).max(500.0 * nu / (d * d * w));
            let f2 = (arg2 * arg2).tanh();
            nu_t[c] = SST_A1 * k / (SST_A1 * w).max(strain[c] * f2);
        }
        self.nu_t = nu_t;
        self.limit_nu_t();

        // omega in wall cells: automatic wall treatment
        let mut fixed = vec![false; n];
        for c in 0..n {
            if let Some((u_tau, d)) = self.wall_cell[c] {
                let w_vis = 6.0 * nu / (SST_BETA1 * d * d);
                let w_log = u_tau / (BETA_STAR.sqrt() * KAPPA * d);
                self.w[c] = (w_vis * w_vis + w_log * w_log).sqrt();
                fixed[c] = true;
            }
        }
        let none = vec![false; n];

        // k equation
        let (mut gamma, mut su, mut sp) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for c in 0..n {
            if !self.mesh.fluid[c] {
                continue;
            }
            let s1 = f1[c];
            let sig_k = s1 * SST_SIGMA_K1 + (1.0 - s1) * SST_SIGMA_K2;
            gamma[c] = nu + sig_k * self.nu_t[c];
            let mut pk = (self.nu_t[c] * strain[c] * strain[c])
                .min(10.0 * BETA_STAR * self.k[c] * self.w[c]);
            if let Some((u_tau, d)) = self.wall_cell[c] {
                pk = self.wall_production(u_tau, d);
            }
            let gb = self.buoyancy_production(grad_t, c);
            su[c] = pk + gb.max(0.0);
            sp[c] = BETA_STAR * self.w[c] + (-gb).max(0.0) / self.k[c].max(1e-12);
        }
        let k_old = self.k.clone();
        let k_change = {
            let eq = Equation {
                mesh: &self.mesh,
                vel,
                gamma: &gamma,
                su: &su,
                sp: &sp,
                fixed: &none,
                tvd: self.cfg.second_order,
            };
            eq.solve(&mut self.k, &k_old, dt, sweeps)
        };
        for v in &mut self.k {
            *v = v.max(1e-12);
        }

        // omega equation
        let (mut gamma, mut su, mut sp) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for c in 0..n {
            if !self.mesh.fluid[c] {
                continue;
            }
            let s1 = f1[c];
            let sig_w = s1 * SST_SIGMA_W1 + (1.0 - s1) * SST_SIGMA_W2;
            let beta = s1 * SST_BETA1 + (1.0 - s1) * SST_BETA2;
            let gam = s1 * SST_GAMMA1 + (1.0 - s1) * SST_GAMMA2;
            gamma[c] = nu + sig_w * self.nu_t[c];
            let pk_over_nut = strain[c] * strain[c];
            // gamma / nu_t * Pk with Pk = min(nu_t S^2, 10 beta* k w)
            let prod = gam
                * pk_over_nut
                    .min(10.0 * BETA_STAR * self.w[c] * self.k[c] / self.nu_t[c].max(1e-30));
            let cross = 2.0 * (1.0 - s1) * SST_SIGMA_W2 / self.w[c].max(1e-9) * dot(&gk[c], &gw[c]);
            su[c] = prod + cross.max(0.0);
            sp[c] = beta * self.w[c] + (-cross).max(0.0) / self.w[c].max(1e-9);
        }
        let w_old = self.w.clone();
        let w_change = {
            let eq = Equation {
                mesh: &self.mesh,
                vel,
                gamma: &gamma,
                su: &su,
                sp: &sp,
                fixed: &fixed,
                tvd: self.cfg.second_order,
            };
            eq.solve(&mut self.w, &w_old, dt, sweeps)
        };
        for v in &mut self.w {
            *v = v.max(1e-9);
        }
        self.update_nu_t_sst(&f1, strain);
        self.step_report(k_change, w_change)
    }

    fn update_nu_t_sst(&mut self, _f1: &[f64], strain: &[f64]) {
        let n = self.mesh.cells();
        let nu = self.cfg.nu;
        for c in 0..n {
            if !self.mesh.fluid[c] {
                self.nu_t[c] = 0.0;
                continue;
            }
            let d = self.mesh.wall_dist[c].max(1e-12);
            let (k, w) = (self.k[c], self.w[c]);
            let arg2 = (2.0 * k.sqrt() / (BETA_STAR * w * d)).max(500.0 * nu / (d * d * w));
            let f2 = (arg2 * arg2).tanh();
            self.nu_t[c] = SST_A1 * k / (SST_A1 * w).max(strain[c] * f2);
        }
        self.limit_nu_t();
    }

    // ---- k-epsilon -----------------------------------------------------

    fn advance_ke(
        &mut self,
        vel: &[Vec<f64>; 3],
        strain: &[f64],
        grad_t: Option<&[[f64; 3]]>,
        dt: f64,
        sweeps: usize,
    ) -> TurbStep {
        let n = self.mesh.cells();
        let nu = self.cfg.nu;
        // epsilon in wall cells: equilibrium
        let mut fixed = vec![false; n];
        for c in 0..n {
            if let Some((_, d)) = self.wall_cell[c] {
                // y+ based on k so the value is sound at low y+ too
                let kc = self.k[c].max(1e-12);
                let y_star = C_MU.powf(0.25) * kc.sqrt() * d / nu;
                let eps_log = C_MU.powf(0.75) * kc.powf(1.5) / (KAPPA * d);
                let eps_vis = 2.0 * nu * kc / (d * d);
                self.w[c] = if y_star > 11.0 {
                    eps_log
                } else {
                    eps_vis.max(eps_log)
                };
                fixed[c] = true;
            }
        }
        let none = vec![false; n];
        let (mut gamma, mut su, mut sp) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        let mut gb_all = vec![0.0; n];
        for c in 0..n {
            if !self.mesh.fluid[c] {
                continue;
            }
            gamma[c] = nu + self.nu_t[c] / KE_SIGMA_K;
            let mut pk = self.nu_t[c] * strain[c] * strain[c];
            if let Some((u_tau, d)) = self.wall_cell[c] {
                pk = self.wall_production(u_tau, d);
            }
            let gb = self.buoyancy_production(grad_t, c);
            gb_all[c] = gb;
            su[c] = pk + gb.max(0.0);
            sp[c] = self.w[c] / self.k[c].max(1e-12) + (-gb).max(0.0) / self.k[c].max(1e-12);
        }
        let k_old = self.k.clone();
        let k_change = {
            let eq = Equation {
                mesh: &self.mesh,
                vel,
                gamma: &gamma,
                su: &su,
                sp: &sp,
                fixed: &none,
                tvd: self.cfg.second_order,
            };
            eq.solve(&mut self.k, &k_old, dt, sweeps)
        };
        for v in &mut self.k {
            *v = v.max(1e-12);
        }
        let (mut gamma, mut su, mut sp) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for c in 0..n {
            if !self.mesh.fluid[c] {
                continue;
            }
            gamma[c] = nu + self.nu_t[c] / KE_SIGMA_E;
            let kk = self.k[c].max(1e-12);
            let pk = self.nu_t[c] * strain[c] * strain[c] + gb_all[c].max(0.0);
            su[c] = KE_C1 * pk * self.w[c] / kk;
            sp[c] = KE_C2 * self.w[c] / kk;
        }
        let e_old = self.w.clone();
        let w_change = {
            let eq = Equation {
                mesh: &self.mesh,
                vel,
                gamma: &gamma,
                su: &su,
                sp: &sp,
                fixed: &fixed,
                tvd: self.cfg.second_order,
            };
            eq.solve(&mut self.w, &e_old, dt, sweeps)
        };
        for v in &mut self.w {
            *v = v.max(1e-12);
        }
        self.update_nu_t_simple();
        self.step_report(k_change, w_change)
    }

    fn step_report(&self, k_change: f64, w_change: f64) -> TurbStep {
        let max_ratio = self
            .nu_t
            .iter()
            .fold(0.0_f64, |a, v| a.max(*v / self.cfg.nu));
        TurbStep {
            k_change,
            w_change,
            max_nu_t_ratio: max_ratio,
        }
    }
}

// Menter SST (2003) constants.
const SST_SIGMA_K1: f64 = 0.85;
const SST_SIGMA_W1: f64 = 0.5;
const SST_BETA1: f64 = 0.075;
const SST_SIGMA_K2: f64 = 1.0;
const SST_SIGMA_W2: f64 = 0.856;
const SST_BETA2: f64 = 0.0828;
const SST_A1: f64 = 0.31;
const SST_GAMMA1: f64 = SST_BETA1 / BETA_STAR - SST_SIGMA_W1 * KAPPA * KAPPA / 0.3;
const SST_GAMMA2: f64 = SST_BETA2 / BETA_STAR - SST_SIGMA_W2 * KAPPA * KAPPA / 0.3;
// sqrt(0.09) = 0.3 exactly; the constants above use it as a literal.

// Standard k-epsilon.
const KE_C1: f64 = 1.44;
const KE_C2: f64 = 1.92;
const KE_SIGMA_K: f64 = 1.0;
const KE_SIGMA_E: f64 = 1.3;

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `sqrt(2 S_ij S_ij)` of a velocity gradient `g[a][b] = du_a/dx_b`.
fn strain_magnitude(g: &[[f64; 3]; 3]) -> f64 {
    let mut s2 = 0.0;
    for a in 0..3 {
        for b in 0..3 {
            let s = 0.5 * (g[a][b] + g[b][a]);
            s2 += s * s;
        }
    }
    (2.0 * s2).sqrt()
}

/// Second-order derivative at a cell value `v0` from the neighbours as
/// `(distance, value)`; one-sided when a side is missing.
fn one_d_gradient(v0: f64, lo: Option<(f64, f64)>, hi: Option<(f64, f64)>) -> f64 {
    match (lo, hi) {
        (Some((hl, vl)), Some((hh, vh))) => {
            (hl * hl * (vh - v0) + hh * hh * (v0 - vl)) / (hl * hh * (hl + hh))
        }
        (Some((hl, vl)), None) => (v0 - vl) / hl,
        (None, Some((hh, vh))) => (vh - v0) / hh,
        (None, None) => 0.0,
    }
}
