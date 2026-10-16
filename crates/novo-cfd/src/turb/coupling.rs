//! The flow core with a turbulence model: one object that keeps the eddy
//! viscosity and the wall-function values of [`TurbState`] in the hooks of
//! [`FlowSolver`] and steps both together.

use crate::flow::FlowSolver;
use crate::result::Grid3;

use super::{Buoyancy, FaceKind, TurbConfig, TurbModel, TurbState, WallCell};

pub struct TurbulentFlow {
    pub flow: FlowSolver,
    pub turb: TurbState,
    /// Gauss-Seidel sweeps of the turbulence equations per step.
    pub sweeps: usize,
}

/// The mesh of a flow solver as a result grid (millimetres).
pub fn grid_of(flow: &FlowSolver) -> Grid3 {
    let mm = |e: &Vec<f64>| e.iter().map(|v| v * 1e3).collect::<Vec<f64>>();
    let m = &flow.mesh;
    Grid3 {
        nx: m.n[0],
        ny: m.n[1],
        nz: m.n[2],
        x_edges_mm: mm(&m.edges[0]),
        y_edges_mm: mm(&m.edges[1]),
        z_edges_mm: mm(&m.edges[2]),
    }
}

impl TurbulentFlow {
    /// `ref_speed` (m/s) sets the initial turbulence level. Domain faces with
    /// free slip count as symmetry, all others as walls; `Laminar` leaves the
    /// flow solver untouched.
    pub fn new(flow: FlowSolver, model: TurbModel, ref_speed: f64, length_scale: f64) -> Self {
        let mut cfg = TurbConfig::air(model);
        cfg.nu = flow.cfg.nu;
        cfg.pr = flow.cfg.nu / flow.cfg.alpha;
        cfg.pr_t = flow.cfg.prandtl_t;
        cfg.length_scale = length_scale;
        cfg.faces = std::array::from_fn(|f| {
            if flow.cfg.faces[f].slip {
                FaceKind::Symmetry
            } else {
                FaceKind::Wall
            }
        });
        cfg.buoyancy = Some(Buoyancy {
            gravity: flow.cfg.gravity,
            beta: flow.cfg.beta,
        });
        let grid = grid_of(&flow);
        let turb = TurbState::new(&grid, None, cfg, ref_speed);
        TurbulentFlow {
            flow,
            turb,
            sweeps: 3,
        }
    }

    fn push_to_flow(&mut self) {
        if self.turb.cfg.model == TurbModel::Laminar {
            return;
        }
        let cells = self.turb.mesh.cells();
        let mut nu = vec![[f64::NAN; 6]; cells];
        let mut alpha = vec![[f64::NAN; 6]; cells];
        for w in self.turb.walls() {
            let WallCell {
                cell,
                axis,
                high,
                nu_wall,
                alpha_wall,
                ..
            } = *w;
            nu[cell][2 * axis + usize::from(high)] = nu_wall;
            alpha[cell][2 * axis + usize::from(high)] = alpha_wall;
        }
        self.flow.nu_t = Some(self.turb.nu_t.clone());
        self.flow.wall_nu = Some(nu);
        self.flow.wall_alpha = Some(alpha);
    }

    /// One coupled step: flow with the current eddy viscosity, then the
    /// turbulence equations for the new velocity and temperature.
    /// Returns the steadiness `(max velocity change per second, max
    /// temperature change per second)` of the flow step.
    pub fn step(&mut self, dt: f64) -> (f64, f64) {
        self.push_to_flow();
        let info = self.flow.step(dt);
        self.turb
            .advance(&self.flow.vel, Some(&self.flow.t), dt, self.sweeps);
        (info.max_accel, info.max_dtdt)
    }
}
