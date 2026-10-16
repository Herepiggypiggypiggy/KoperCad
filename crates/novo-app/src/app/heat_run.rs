//! The solver run behind the Heat view's Simulate button: one worker thread
//! per run, snapshots handed back over a channel, cancel through the
//! solver's own flag. The UI thread only polls; it never waits.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

use novo_cfd::{CfdError, CfdSnapshot, Control, Fidelity, Solver, StubSolver};
use novo_thermal::Scenario;

/// A published result with what the picture needs to place it on the plate.
#[derive(Debug)]
pub(super) struct Solution {
    pub snapshot: Arc<CfdSnapshot>,
    /// Counts the runs, for the cache key.
    pub id: u64,
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
    Finished(Result<(), CfdError>),
}

/// A run in flight.
pub(super) struct Runner {
    control: Control,
    rx: Receiver<Event>,
    pub fidelity: Fidelity,
    started: Instant,
}

/// The solver the app uses. One place to change when the flow core lands.
pub(super) fn default_solver() -> Arc<dyn Solver> {
    Arc::new(StubSolver)
}

impl Runner {
    /// Starts `solver` on a worker thread. `wake` is called after every
    /// event so the UI repaints.
    pub(super) fn start(
        solver: Arc<dyn Solver>,
        scenario: Scenario,
        fidelity: Fidelity,
        wake: impl Fn() + Send + 'static,
    ) -> Runner {
        let control = Control::new();
        let (tx, rx) = channel();
        let worker_control = control.clone();
        std::thread::spawn(move || {
            let mut publish = |s: &CfdSnapshot| {
                let _ = tx.send(Event::Snapshot(Arc::new(s.clone())));
                wake();
            };
            let result = solver
                .solve(&scenario, fidelity, &worker_control, &mut publish)
                .map(|_| ());
            let _ = tx.send(Event::Finished(result));
            wake();
        });
        Runner {
            control,
            rx,
            fidelity,
            started: Instant::now(),
        }
    }

    pub(super) fn cancel(&self) {
        self.control.cancel();
    }

    pub(super) fn elapsed_s(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Everything that arrived since the last call. A worker that died
    /// without a word counts as a failed run.
    pub(super) fn poll(&mut self) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !matches!(out.last(), Some(Event::Finished(_))) {
                        out.push(Event::Finished(Err(CfdError::Diverged(0))));
                    }
                    break;
                }
            }
        }
        out
    }
}
