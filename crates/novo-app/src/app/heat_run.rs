//! The solver run behind the Heat view's Simulate button: the scheduler of
//! `novo-cfd` runs Coarse, Medium and Fine on a worker thread and hands
//! snapshots and progress back; cancel goes through the solver's own flag.
//! The UI thread only polls; it never waits.

use std::sync::Arc;
use std::time::Instant;

use novo_cfd::sched::{Event as SchedEvent, Outcome, Plan, Scheduler};
use novo_cfd::{CfdSnapshot, Fidelity, Progress, Solver};

/// A published result with what the picture needs to place it on the plate.
#[derive(Debug)]
pub(super) struct Solution {
    pub snapshot: Arc<CfdSnapshot>,
    /// Counts the snapshots, for the cache key.
    pub id: u64,
    /// The scenario this result solved.
    pub scenario: Arc<novo_thermal::Scenario>,
    /// `heat_live::scenario_key` of it.
    pub scenario_key: u64,
    /// Hash of the layout the run was started from; a different one marks
    /// the result out of date.
    pub layout_key: u64,
    /// Outside air, degrees C.
    pub ambient: f64,
    /// Plate rectangle in the interior frame of the solver: x, y, width,
    /// height, mm.
    pub plate: [f64; 4],
    /// What the bridge found missing ("3 devices have no loss data").
    pub messages: Vec<String>,
}

pub(super) enum Event {
    Snapshot(Arc<CfdSnapshot>),
    /// Iteration and residuals of the level that is running.
    Progress(Progress),
    Finished(Outcome),
    /// The worker ended without reporting.
    Lost,
}

/// A run in flight.
pub(super) struct Runner {
    scheduler: Scheduler,
    /// The levels the run goes through, in order.
    pub levels: Vec<Fidelity>,
    started: Instant,
}

/// The solver the app uses: the cabinet solver of `novo-cfd` (mesh, solids,
/// ducts, openings, buoyant flow). Tests run the synthetic demo ladder so
/// they stay fast.
pub(super) fn default_solver() -> Arc<dyn Solver> {
    #[cfg(test)]
    {
        Arc::new(novo_cfd::sched::DemoLadder {
            step: std::time::Duration::ZERO,
        })
    }
    #[cfg(not(test))]
    {
        Arc::new(novo_cfd::CabinetSolver::default())
    }
}

impl Runner {
    /// Starts `solver` on a worker thread, level by level up to `top`.
    pub(super) fn start(
        solver: Arc<dyn Solver>,
        scenario: novo_thermal::Scenario,
        top: Fidelity,
    ) -> Runner {
        let plan = Plan::up_to(top);
        let levels = plan.levels.clone();
        let mut scheduler = Scheduler::new(solver);
        scheduler.start(scenario, plan);
        Runner {
            scheduler,
            levels,
            started: Instant::now(),
        }
    }

    pub(super) fn cancel(&self) {
        self.scheduler.cancel();
    }

    pub(super) fn elapsed_s(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Everything that arrived since the last call. A worker that died
    /// without a word counts as a failed run.
    pub(super) fn poll(&mut self) -> Vec<Event> {
        let Some(job) = self.scheduler.current() else {
            return vec![Event::Lost];
        };
        let finished = job.is_finished();
        let mut out: Vec<Event> = job
            .poll()
            .into_iter()
            .filter_map(|e| match e {
                SchedEvent::Snapshot(s) => Some(Event::Snapshot(s)),
                SchedEvent::Progress(p) => Some(Event::Progress(p)),
                SchedEvent::Finished(o) => Some(Event::Finished(o)),
                SchedEvent::LevelDone { .. } | SchedEvent::LevelSkipped(_) => None,
            })
            .collect();
        if finished && !matches!(out.last(), Some(Event::Finished(_))) {
            out.push(Event::Lost);
        }
        out
    }
}
