//! A multi-level stand-in so the UI lane can build the progress bar, the
//! residual sparkline and level switching before the real solver is wired
//! to the [`Solver`] trait.

use std::time::Duration;

use novo_thermal::Scenario;

use crate::result::{CfdSnapshot, Fidelity};
use crate::solver::{CfdError, Control, Solver};
use crate::stub::StubSolver;

/// Wraps [`StubSolver`]: serves every level with the same extruded 2D
/// estimate, relabelled, with a synthetic iteration count and decaying
/// residuals. Not physics; its notes say so.
#[derive(Debug, Clone, Copy)]
pub struct DemoLadder {
    /// Pause per fake iteration, so progress is visible.
    pub step: Duration,
}

impl Default for DemoLadder {
    fn default() -> DemoLadder {
        DemoLadder {
            step: Duration::from_millis(20),
        }
    }
}

impl Solver for DemoLadder {
    fn solve(
        &self,
        scenario: &Scenario,
        fidelity: Fidelity,
        control: &Control,
        publish: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        if fidelity == Fidelity::Preview2d {
            return Err(CfdError::Unsupported(fidelity));
        }
        let mut snap = StubSolver.solve(scenario, Fidelity::Coarse, control, &mut |_| {})?;
        let iters = match fidelity {
            Fidelity::Coarse => 20,
            Fidelity::Medium => 40,
            _ => 60,
        };
        snap.progress.fidelity = fidelity;
        snap.notes
            .push("Demo ladder: synthetic iterations on the stub field.".into());
        for it in 1..=iters {
            if control.is_cancelled() {
                return Err(CfdError::Cancelled);
            }
            let r = 0.5f64.powf(it as f64 / 4.0);
            snap.progress.iteration = it;
            snap.progress.converged = it == iters;
            snap.progress.residuals.momentum = r;
            snap.progress.residuals.continuity = r * 0.5;
            snap.progress.residuals.energy = r * 0.1;
            publish(&snap);
            std::thread::sleep(self.step);
        }
        Ok(snap)
    }
}
