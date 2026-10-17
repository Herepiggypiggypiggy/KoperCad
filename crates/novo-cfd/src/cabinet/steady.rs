//! The steady path of the cabinet solver (lane CFD-FAST): pseudo-transient
//! iteration instead of the false-transient march, 30 to 40 times faster for
//! the same answer (see `flow::solver::steady`).
//!
//! Two stages: a first-order upwind preview (published with a note that says
//! so, about 1 K low on the hottest device) and then the configured second
//! order scheme, each converged on window means.

use super::radiation::RadiationCoupling;
use super::tuning::LevelTuning;
use crate::flow::{FlowSolver, RunStatus, SteadyConfig, SteadyLimits, SteadyReport, SteadyRun};
use crate::result::{Fidelity, Progress, Residuals};
use crate::solids::FlowSetup;
use crate::turb::TurbState;

/// Note carried by every snapshot published during the first-order stage.
pub const PREVIEW_NOTE: &str = "First-order preview: the numerical diffusion of the upwind scheme \
     smears the plume, so the hottest device reads about 1 K low. The second-order stage follows.";

/// A steady run for one cabinet.
pub struct CabinetSteady {
    pub run: SteadyRun,
    pub status: RunStatus,
    /// Turbulence model driven with the iteration (none = laminar).
    pub turb: Option<TurbState>,
    /// Surface radiation coupled into the iteration (none = off).
    pub rad: Option<RadiationCoupling>,
}

/// Gauss-Seidel sweeps of the turbulence equations per iteration.
const TURB_SWEEPS: usize = 3;
/// The turbulence equations take at most this pseudo step, s.
const TURB_DT_MAX: f64 = 0.1;

impl CabinetSteady {
    pub fn new(setup: &FlowSetup, flow: &FlowSolver, lvl: &LevelTuning) -> CabinetSteady {
        let probes = (0..setup.classified.grid.cells())
            .filter(|&c| {
                setup.classified.material[c] != 0
                    && setup
                        .cfg
                        .medium
                        .as_ref()
                        .is_some_and(|m| m.heat_w_m3[c] > 0.0)
            })
            .collect();
        let limits = SteadyLimits {
            max_iterations: lvl.max_steps,
            ..SteadyLimits::default()
        };
        let run = SteadyRun::staged(
            flow,
            vec![SteadyConfig::first_order(), SteadyConfig::second_order()],
            limits,
            setup.ambient_c,
            probes,
        );
        CabinetSteady {
            run,
            status: RunStatus::Running,
            turb: None,
            rad: None,
        }
    }

    /// Drives `turb` with the iteration: eddy viscosity and wall values go
    /// into the flow before every iteration, the model advances after it.
    pub fn with_turbulence(mut self, turb: Option<TurbState>) -> CabinetSteady {
        self.turb = turb;
        self
    }

    /// Couples surface radiation into the iteration.
    pub fn with_radiation(mut self, rad: Option<RadiationCoupling>) -> CabinetSteady {
        self.rad = rad;
        self
    }

    /// Runs `iterations` more iterations.
    pub fn advance(&mut self, flow: &mut FlowSolver, iterations: usize) -> RunStatus {
        let cell = std::cell::RefCell::new(self.turb.take());
        let rad = std::cell::RefCell::new(self.rad.take());
        self.status = self.run.advance_with(
            flow,
            iterations,
            &mut |f| {
                if let Some(r) = rad.borrow_mut().as_mut() {
                    r.tick(f);
                }
                if let Some(t) = cell.borrow().as_ref() {
                    t.push_to_flow(f);
                }
            },
            &mut |f, dt| {
                if let Some(t) = cell.borrow_mut().as_mut() {
                    t.advance_with(f, dt.min(TURB_DT_MAX), TURB_SWEEPS);
                }
            },
        );
        self.turb = cell.into_inner();
        self.rad = rad.into_inner();
        self.status
    }

    /// True while the first-order preview stage runs.
    pub fn is_preview(&self) -> bool {
        self.status == RunStatus::Running && self.run.report().stage == 0
    }

    pub fn converged(&self) -> bool {
        self.status == RunStatus::Converged
    }

    pub fn progress(&self, fidelity: Fidelity, elapsed_s: f64, flow: &FlowSolver) -> Progress {
        let r: SteadyReport = self.run.report();
        let info = self.run.last;
        let umax = (0..3)
            .flat_map(|c| flow.vel[c].iter())
            .fold(1e-3_f64, |a, v| a.max(v.abs()));
        Progress {
            fidelity,
            iteration: r.iterations,
            residuals: Residuals {
                momentum: info.map_or(1.0, |i| (i.max_du / umax).min(1.0)),
                continuity: info.map_or(1.0, |i| i.continuity.min(1.0)),
                energy: r.imbalance.abs().min(1.0),
            },
            converged: self.converged(),
            elapsed_s,
            energy_balance_rel: r.imbalance.abs(),
            monitor_drift_k: if r.drift_k.is_finite() {
                r.drift_k.abs()
            } else {
                0.0
            },
        }
    }
}

/// Scenario heat note for steady runs that ended short of convergence.
pub fn unconverged_note(r: &SteadyReport) -> String {
    format!(
        "Stopped before a steady state ({} iterations, heat balance off by {:.1} %): the field is \
         still developing; temperatures are not final. After a time-budget stop the fields are the last iterate, not a window mean.",
        r.iterations,
        100.0 * r.imbalance.abs()
    )
}
