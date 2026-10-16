//! The cabinet solver: the real [`Solver`] behind the Heat layer (lane CFD-INT).
//!
//! It strings the engines of this crate together for one scenario: the
//! graded mesh with exact material shares (`mesh`), solids with conjugate
//! heat transfer, slotted ducts as porous media and the enclosure wall films
//! (`solids`), vents and fans (`openings`) and the flow core (`flow`). It
//! marches the flow in time until the stored heat stops changing (see
//! [`march`]), publishes progress while it runs and turns the final state
//! into a [`CfdSnapshot`] (see [`snapshot`]).
//!
//! What is modelled and what is not, per level:
//!
//! - Laminar buoyant flow, conduction in devices, plate, rails and walls,
//!   outside films from IEC 60890, vents and fans, slotted ducts as porous
//!   media. All levels.
//! - Turbulence and radiation are not coupled yet: the hooks exist in
//!   `turb` and `rad`, the coupling is the next step. The result's accuracy
//!   statement says so; radiation is missing from the heat paths and the
//!   surface temperatures of hot devices are slightly high because of it.
//!
//! Where the user finds it: Cabinet view, Heat layer, Simulate
//! (`cabinet.thermal.simulate`).

mod march;
mod snapshot;
mod statement;
mod tuning;
mod warm;

use std::time::Instant;

use novo_thermal::Scenario;

use crate::mesh::MeshSettings;
use crate::result::{CfdSnapshot, Fidelity};
use crate::solids::{FlowSetup, SolidsSettings};
use crate::solver::{CfdError, Control, Solver};

pub use march::{MarchLimits, MarchState, MarchStatus};
pub use statement::{ENCLOSURE_VALIDATED, accuracy_for};
pub use tuning::{LevelTuning, Tuning};

/// The solver the app runs for Coarse, Medium and Fine.
#[derive(Debug, Clone, Default)]
pub struct CabinetSolver {
    pub tuning: Tuning,
}

impl CabinetSolver {
    pub fn new(tuning: Tuning) -> CabinetSolver {
        CabinetSolver { tuning }
    }
}

impl Solver for CabinetSolver {
    fn solve(
        &self,
        scenario: &Scenario,
        fidelity: Fidelity,
        control: &Control,
        publish: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        let lvl = self
            .tuning
            .level(fidelity)
            .ok_or(CfdError::Unsupported(fidelity))?;
        let t0 = Instant::now();
        let mesh = MeshSettings::for_fidelity(fidelity)
            .map(|m| MeshSettings {
                target_cells: lvl.target_cells.unwrap_or(m.target_cells),
                ..m
            })
            .map_err(|e| CfdError::Setup(e.to_string()))?;
        let st = SolidsSettings {
            mesh,
            solid_speedup: lvl.solid_speedup,
            ..SolidsSettings::default()
        };
        let setup = FlowSetup::new(scenario, &st).map_err(|e| CfdError::Setup(e.to_string()))?;
        if control.is_cancelled() {
            return Err(CfdError::Cancelled);
        }
        let mut flow = setup.solver();
        flow.cfg.pressure_tol = lvl.pressure_tol;
        let warm = lvl.warm_start && warm::warm_start(&setup, scenario, &mut flow);
        let view = snapshot::View::new(&setup, scenario, fidelity);
        let limits = MarchLimits::from(lvl);
        let mut march = MarchState::new(&setup, &flow);
        let mut last_full = Instant::now() - lvl.full_snapshot_every;
        let mut snap = view.build(&flow, &march.progress(fidelity, 0.0, &flow, &setup));
        loop {
            let mut status = march.advance(&setup, &mut flow, &limits, control, lvl.report_every);
            if !status.finished() && lvl.max_wall.is_some_and(|w| t0.elapsed() >= w) {
                status = MarchStatus::OutOfTime;
            }
            if control.is_cancelled() {
                return Err(CfdError::Cancelled);
            }
            if flow.divergence_max() > 1.0 || !flow.mean_temperature().is_finite() {
                return Err(CfdError::Diverged(march.steps));
            }
            let progress = march.progress(fidelity, t0.elapsed().as_secs_f64(), &flow, &setup);
            if status.finished() {
                let mut done = view.build(&flow, &progress);
                done.progress.converged = march.converged;
                done.notes.push(
                    if march.converged {
                        "Steady: the stored heat changes by less than the tolerance."
                    } else {
                        "Stopped before a steady state (time budget): the field is still \
                         developing; temperatures are not final."
                    }
                    .into(),
                );
                if warm {
                    done.notes.push(
                        "Started from the 2D estimate (the end state does not depend on it)."
                            .into(),
                    );
                }
                return Ok(done);
            }
            if last_full.elapsed() >= lvl.full_snapshot_every {
                snap = view.build(&flow, &progress);
                last_full = Instant::now();
            } else {
                snap.progress = progress;
            }
            publish(&snap);
        }
    }
}
