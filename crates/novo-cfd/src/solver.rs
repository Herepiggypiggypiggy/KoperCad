//! The solver interface every lane builds against.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use novo_thermal::{Error as ThermalError, Scenario};

use crate::result::{CfdSnapshot, Fidelity};

#[derive(Debug, thiserror::Error)]
pub enum CfdError {
    #[error("the scenario cannot be simulated: {0}")]
    Scenario(#[from] ThermalError),
    #[error("{0:?} is not available in this solver")]
    Unsupported(Fidelity),
    #[error("the simulation could not be set up: {0}")]
    Setup(String),
    #[error("cancelled")]
    Cancelled,
    #[error("the solution diverged at iteration {0}")]
    Diverged(usize),
}

/// Cancel flag shared with the caller. Solvers check it every iteration;
/// clones share the flag.
#[derive(Debug, Clone, Default)]
pub struct Control {
    cancel: Arc<AtomicBool>,
}

impl Control {
    pub fn new() -> Control {
        Control::default()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

/// Turns a scenario into snapshots. Runs on the caller's thread (the
/// scheduler of lane E puts it on a worker); never touches a UI.
pub trait Solver: Send + Sync {
    /// Solve at `fidelity`. Intermediate states go to `publish` (at least
    /// when the level improves); the final snapshot is also returned.
    /// Cancelling returns `Err(CfdError::Cancelled)` promptly.
    fn solve(
        &self,
        scenario: &Scenario,
        fidelity: Fidelity,
        control: &Control,
        publish: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError>;
}
