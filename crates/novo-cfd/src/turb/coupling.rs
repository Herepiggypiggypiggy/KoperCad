//! The flow core with a turbulence model: keeps the eddy viscosity and the
//! wall-function values of [`TurbState`] in the hooks of [`FlowSolver`] and
//! steps both together.

use crate::flow::FlowSolver;
use crate::result::Grid3;
use crate::solids::MIN_OPEN;

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

impl TurbState {
    /// A turbulence state for `flow`. `ref_speed` (m/s) sets the initial
    /// turbulence level. Domain faces with free slip count as symmetry, all
    /// others as walls except the open faces of vents and fans; cells of the
    /// medium with almost no air are solid walls. `Laminar` gives a state
    /// that never touches the flow.
    pub fn for_flow(
        flow: &FlowSolver,
        model: TurbModel,
        ref_speed: f64,
        length_scale: f64,
    ) -> TurbState {
        let mut cfg = TurbConfig::air(model);
        cfg.nu = flow.cfg.nu;
        // a flow config without a thermal diffusivity keeps air's Prandtl number
        cfg.pr = if flow.cfg.alpha > 0.0 {
            flow.cfg.nu / flow.cfg.alpha
        } else {
            cfg.pr
        };
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
        let grid = grid_of(flow);
        // cells the flow core treats as solid (almost no air) are walls here
        let solid: Option<Vec<bool>> = flow
            .cfg
            .medium
            .as_ref()
            .map(|md| md.fluid_frac.iter().map(|f| *f < MIN_OPEN).collect());
        let mut turb = TurbState::new(&grid, solid.as_deref(), cfg, ref_speed);
        if let Some(md) = &flow.cfg.medium {
            // air cells with a thin solid share (a sheet-metal wall inside the
            // cell) count as well: their heat capacity and conductivity are blended
            turb.cut = (0..md.fluid_frac.len())
                .map(|c| {
                    md.fluid_frac[c] < 1.0 - 1e-9 || md.c_solid[c] > 0.0 || md.k_solid[c] > 0.0
                })
                .collect();
        }
        if let Some(md) = flow
            .cfg
            .medium
            .as_ref()
            .filter(|_| !flow.cfg.device_film_h.is_empty())
        {
            turb.film = md.dev_frac.iter().map(|d| *d >= 0.5).collect();
        }
        let open: Vec<(usize, usize, bool)> = flow
            .open
            .faces
            .iter()
            .map(|f| (f.cell, f.axis, f.upper))
            .collect();
        if !open.is_empty() {
            turb.mark_open(&open);
        }
        turb
    }

    /// Put the eddy viscosity and wall values into the hooks of `flow`;
    /// call before every flow step. Nothing happens for `Laminar`.
    pub fn push_to_flow(&self, flow: &mut FlowSolver) {
        if self.cfg.model == TurbModel::Laminar {
            return;
        }
        let cells = self.mesh.cells();
        let mut nu = vec![[f64::NAN; 6]; cells];
        let mut alpha = vec![[f64::NAN; 6]; cells];
        for w in self.walls() {
            let WallCell {
                cell,
                axis,
                high,
                nu_wall,
                alpha_wall,
                ..
            } = *w;
            nu[cell][2 * axis + usize::from(high)] = nu_wall;
            // a cell that also holds solid keeps the flow core's own
            // conductance (a wall alpha there is put in series wrongly)
            // and so does a device surface: the flow core's film model
            // (correlation, radiation, exposed area) sets its conductance
            let ix = self.mesh.ijk(cell);
            let film_nb = self
                .mesh
                .neighbour(ix, axis, high)
                .is_some_and(|nb| self.film.get(self.mesh.cell(nb)).copied().unwrap_or(false));
            if !film_nb && !self.cut.get(cell).copied().unwrap_or(false) {
                alpha[cell][2 * axis + usize::from(high)] = alpha_wall;
            }
        }
        flow.nu_t = Some(self.nu_t.clone());
        flow.wall_nu = Some(nu);
        flow.wall_alpha = Some(alpha);
    }

    /// Advance the turbulence with the flow state after a step of `dt`.
    pub fn advance_with(&mut self, flow: &FlowSolver, dt: f64, sweeps: usize) {
        self.advance(&flow.vel, Some(&flow.t), dt, sweeps);
    }
}

impl TurbulentFlow {
    pub fn new(flow: FlowSolver, model: TurbModel, ref_speed: f64, length_scale: f64) -> Self {
        let turb = TurbState::for_flow(&flow, model, ref_speed, length_scale);
        TurbulentFlow {
            flow,
            turb,
            sweeps: 3,
        }
    }

    /// One coupled step: flow with the current eddy viscosity, then the
    /// turbulence equations for the new velocity and temperature.
    /// Returns the steadiness `(max velocity change per second, max
    /// temperature change per second)` of the flow step.
    pub fn step(&mut self, dt: f64) -> (f64, f64) {
        self.turb.push_to_flow(&mut self.flow);
        let info = self.flow.step(dt);
        self.turb.advance_with(&self.flow, dt, self.sweeps);
        (info.max_accel, info.max_dtdt)
    }
}
