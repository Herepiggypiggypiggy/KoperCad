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
mod steady;
mod tuning;
mod warm;

use std::time::Instant;

use novo_thermal::Scenario;

use crate::mesh::MeshSettings;
use crate::result::{CfdSnapshot, CfdSnapshot as Snap, Fidelity};
use crate::solids::{FlowSetup, SolidsSettings};
use crate::solver::{CfdError, Control, Solver};
use crate::turb::{TurbModel, TurbState};

pub use march::{MarchLimits, MarchState, MarchStatus};
pub use statement::{ENCLOSURE_VALIDATED, accuracy_for, accuracy_with_turbulence};
pub use steady::{CabinetSteady, PREVIEW_NOTE};
pub use tuning::{LevelTuning, Method, Tuning};

/// The solver the app runs for Coarse, Medium and Fine.
#[derive(Debug, Clone, Default)]
pub struct CabinetSolver {
    pub tuning: Tuning,
    /// Turbulence model; `Laminar` (the default) leaves the flow untouched.
    pub turbulence: TurbModel,
}

impl CabinetSolver {
    pub fn new(tuning: Tuning) -> CabinetSolver {
        CabinetSolver {
            tuning,
            turbulence: TurbModel::Laminar,
        }
    }

    /// The same solver with a turbulence model (k-omega SST is the
    /// recommended one; k-epsilon is the cross check).
    pub fn with_turbulence(mut self, model: TurbModel) -> CabinetSolver {
        self.turbulence = model;
        self
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
                ..lvl.mesh.unwrap_or(m)
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
        // A level flagged `turbulent` marches; otherwise the steady iteration
        // drives the turbulence model (if any) itself. k-epsilon marches: in
        // the pseudo-time iteration its eddy viscosity runs away (nu_t/nu of
        // 1e4 and more where the march gives 30 to 50).
        let steady_model = matches!(self.turbulence, TurbModel::Laminar | TurbModel::KOmegaSst);
        if lvl.method == Method::Steady && steady_model && flow.nu_t.is_none() && !lvl.turbulent {
            let turb = (self.turbulence != TurbModel::Laminar).then(|| {
                let h = flow.mesh.edges[1].last().copied().unwrap_or(1.0);
                TurbState::for_flow(&flow, self.turbulence, 0.1, 0.02 * h)
            });
            return solve_steady(
                &setup,
                SteadyJob {
                    fidelity,
                    lvl,
                    model: self.turbulence,
                    turb,
                    warm,
                    t0,
                },
                control,
                publish,
                flow,
                view,
            );
        }
        let limits = MarchLimits::from(lvl);
        let mut march = MarchState::new(&setup, &flow);
        if self.turbulence != TurbModel::Laminar {
            let h = flow.mesh.edges[1].last().copied().unwrap_or(1.0);
            // natural convection in a cabinet moves at about 0.1 m/s
            march.turb = Some(TurbState::for_flow(&flow, self.turbulence, 0.1, 0.02 * h));
        }
        let model = self.turbulence;
        let finish = |mut s: Snap, march: &MarchState| -> Snap {
            if let Some(t) = &march.turb {
                s.turb_visc_ratio = t.viscosity_ratio();
                s.accuracy = accuracy_with_turbulence(s.progress.fidelity, model);
            }
            s
        };
        let mut last_full = Instant::now() - lvl.full_snapshot_every;
        let mut snap = finish(
            view.build(&flow, &march.progress(fidelity, 0.0, &flow, &setup)),
            &march,
        );
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
                let mut done = finish(view.build(&flow, &progress), &march);
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
                snap = finish(view.build(&flow, &progress), &march);
                last_full = Instant::now();
            } else {
                snap.progress = progress;
            }
            publish(&snap);
        }
    }
}

/// What `solve_steady` needs besides the setup and the flow.
struct SteadyJob<'a> {
    fidelity: Fidelity,
    lvl: &'a LevelTuning,
    model: TurbModel,
    turb: Option<TurbState>,
    warm: bool,
    t0: Instant,
}

/// The steady path: iterate, publish progress, finish with window means.
fn solve_steady(
    setup: &FlowSetup,
    job: SteadyJob,
    control: &Control,
    publish: &mut dyn FnMut(&CfdSnapshot),
    mut flow: crate::flow::FlowSolver,
    view: snapshot::View,
) -> Result<CfdSnapshot, CfdError> {
    use crate::flow::RunStatus;
    let SteadyJob {
        fidelity,
        lvl,
        model,
        turb,
        warm,
        t0,
    } = job;
    let mut run = CabinetSteady::new(setup, &flow, lvl).with_turbulence(turb);
    let finish = |mut s: Snap, run: &CabinetSteady| -> Snap {
        if let Some(t) = &run.turb {
            s.turb_visc_ratio = t.viscosity_ratio();
            s.accuracy = accuracy_with_turbulence(s.progress.fidelity, model);
        }
        s
    };
    let mut last_full = Instant::now() - lvl.full_snapshot_every;
    let mut snap = finish(view.build(&flow, &run.progress(fidelity, 0.0, &flow)), &run);
    loop {
        let mut status = run.advance(&mut flow, lvl.report_every.max(5));
        let out_of_time = lvl.max_wall.is_some_and(|w| t0.elapsed() >= w);
        if status == RunStatus::Running && out_of_time {
            status = RunStatus::OutOfIterations;
        }
        if control.is_cancelled() {
            return Err(CfdError::Cancelled);
        }
        if flow.divergence_max() > 1.0 || !flow.mean_temperature().is_finite() {
            return Err(CfdError::Diverged(run.run.report().iterations));
        }
        let progress = run.progress(fidelity, t0.elapsed().as_secs_f64(), &flow);
        if status != RunStatus::Running {
            let mut done = finish(view.build(&flow, &progress), &run);
            done.progress.converged = status == RunStatus::Converged;
            done.notes.push(if status == RunStatus::Converged {
                "Steady: heat in and out balance and the temperatures stopped drifting \
                     (means over the last window)."
                    .to_string()
            } else {
                steady::unconverged_note(&run.run.report())
            });
            if status != RunStatus::Converged && run.run.report().stage == 0 {
                // stopped inside the first-order stage: say so
                done.notes.push(PREVIEW_NOTE.into());
            }
            if warm {
                done.notes.push(
                    "Started from the 2D estimate (the end state does not depend on it).".into(),
                );
            }
            return Ok(done);
        }
        if last_full.elapsed() >= lvl.full_snapshot_every {
            snap = finish(view.build(&flow, &progress), &run);
            if run.is_preview() {
                snap.notes.push(PREVIEW_NOTE.into());
            }
            last_full = Instant::now();
        } else {
            snap.progress = progress;
        }
        publish(&snap);
    }
}
