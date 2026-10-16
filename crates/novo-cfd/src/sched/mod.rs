//! Background scheduler (lane CFD-E).
//!
//! Runs a [`Solver`] through a ladder of fidelities (Coarse, then Medium,
//! then Fine) on a worker thread with its own rayon pool, and hands the UI
//! snapshots without ever blocking it: the UI calls [`Job::poll`] once per
//! frame and [`Job::cancel`] when the user stops the run. Workers never touch
//! UI state.
//!
//! Where the user finds it: Cabinet view, Heat layer, Simulate
//! (`cabinet.thermal.simulate`) starts a [`Job`]; the progress bar and cancel
//! button read it. The app side is lane CFD-F.

mod job;
mod pool;

pub use job::{Event, Job, JobStatus, Outcome};
pub use pool::{Threads, default_threads, install};

use std::sync::Arc;
use std::time::Duration;

use novo_thermal::Scenario;

use crate::result::Fidelity;
use crate::solver::Solver;

/// Which levels to run and how.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Levels in order; each later level refines the earlier one.
    pub levels: Vec<Fidelity>,
    /// Intermediate snapshots closer together than this are dropped (the
    /// last snapshot of a level is always delivered).
    pub min_publish_interval: Duration,
    /// Worker threads for the kernels.
    pub threads: Threads,
}

impl Default for Plan {
    fn default() -> Plan {
        Plan::up_to(Fidelity::Fine)
    }
}

impl Plan {
    /// Coarse, Medium and Fine up to and including `top`.
    pub fn up_to(top: Fidelity) -> Plan {
        let levels = [Fidelity::Coarse, Fidelity::Medium, Fidelity::Fine]
            .into_iter()
            .filter(|f| *f <= top)
            .collect();
        Plan {
            levels,
            min_publish_interval: Duration::from_millis(250),
            threads: Threads::Auto,
        }
    }

    /// Only one level.
    pub fn single(level: Fidelity) -> Plan {
        Plan {
            levels: vec![level],
            ..Plan::default()
        }
    }
}

/// Owns the current job; starting a new one cancels the old one so two
/// simulations never compete for the cores.
pub struct Scheduler {
    solver: Arc<dyn Solver>,
    current: Option<Job>,
}

impl Scheduler {
    pub fn new(solver: Arc<dyn Solver>) -> Scheduler {
        Scheduler {
            solver,
            current: None,
        }
    }

    /// Start a run, cancelling a previous one that is still going.
    pub fn start(&mut self, scenario: Scenario, plan: Plan) -> &Job {
        if let Some(old) = self.current.take() {
            old.cancel();
        }
        self.current
            .insert(Job::spawn(self.solver.clone(), scenario, plan))
    }

    pub fn current(&self) -> Option<&Job> {
        self.current.as_ref()
    }

    /// Cancel the running job, if any.
    pub fn cancel(&self) {
        if let Some(j) = &self.current {
            j.cancel();
        }
    }
}
