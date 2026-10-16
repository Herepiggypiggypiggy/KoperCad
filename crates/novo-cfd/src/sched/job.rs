//! One background run.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Instant;

use novo_thermal::Scenario;

use super::Plan;
use crate::result::{CfdSnapshot, Fidelity, Progress};
use crate::solver::{CfdError, Control, Solver};

/// What the UI hears from the worker.
#[derive(Debug, Clone)]
pub enum Event {
    /// A better state of the solution (shared, not copied again).
    Snapshot(Arc<CfdSnapshot>),
    /// Cheap per-publish progress (iteration, residuals), never throttled:
    /// feeds the progress bar and the residual sparkline.
    Progress(Progress),
    /// A level converged or ran out of iterations.
    LevelDone { fidelity: Fidelity, elapsed_s: f64 },
    /// A level was left out (the solver does not offer it).
    LevelSkipped(Fidelity),
    /// The job ended; always the last event.
    Finished(Outcome),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Every level ran.
    Completed,
    /// The user cancelled; the last snapshot stays valid.
    Cancelled,
    /// A level failed; earlier snapshots stay valid.
    Failed(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JobStatus {
    Running { fidelity: Fidelity },
    Finished,
}

#[derive(Default)]
struct Shared {
    latest: Option<Arc<CfdSnapshot>>,
    running: Option<Fidelity>,
}

/// Handle to a run; dropping it cancels the run.
pub struct Job {
    control: Control,
    events: Receiver<Event>,
    shared: Arc<Mutex<Shared>>,
    thread: Option<JoinHandle<()>>,
}

impl Job {
    pub(super) fn spawn(solver: Arc<dyn Solver>, scenario: Scenario, plan: Plan) -> Job {
        let control = Control::new();
        let (tx, events) = channel();
        let shared = Arc::new(Mutex::new(Shared::default()));
        let worker = (control.clone(), shared.clone());
        let thread = std::thread::Builder::new()
            .name("cfd-scheduler".into())
            .spawn(move || run(&*solver, &scenario, &plan, &worker.0, &worker.1, &tx))
            .ok();
        Job {
            control,
            events,
            shared,
            thread,
        }
    }

    /// Ask the worker to stop; it notices at its next iteration.
    pub fn cancel(&self) {
        self.control.cancel();
    }

    /// Everything that happened since the last call; never blocks.
    pub fn poll(&self) -> Vec<Event> {
        let mut out = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return out,
            }
        }
    }

    /// The best snapshot so far.
    pub fn latest(&self) -> Option<Arc<CfdSnapshot>> {
        self.shared.lock().ok()?.latest.clone()
    }

    pub fn status(&self) -> JobStatus {
        let running = self.shared.lock().ok().and_then(|s| s.running);
        match running {
            Some(fidelity) if !self.is_finished() => JobStatus::Running { fidelity },
            _ => JobStatus::Finished,
        }
    }

    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(|t| t.is_finished())
    }

    /// Wait for the end and return the last snapshot (for tests, the CLI and
    /// benchmarks; the UI polls instead).
    pub fn wait(mut self) -> (Outcome, Option<Arc<CfdSnapshot>>) {
        let mut outcome = Outcome::Failed("the worker thread did not start".into());
        let mut last = None;
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        for e in self.poll() {
            match e {
                Event::Snapshot(s) => last = Some(s),
                Event::Finished(o) => outcome = o,
                _ => {}
            }
        }
        (outcome, last.or_else(|| self.latest()))
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // The worker finishes by itself soon after; do not join in the UI.
        self.control.cancel();
    }
}

fn run(
    solver: &dyn Solver,
    scenario: &Scenario,
    plan: &Plan,
    control: &Control,
    shared: &Mutex<Shared>,
    tx: &Sender<Event>,
) {
    let outcome = super::pool::install(plan.threads, || {
        catch_unwind(AssertUnwindSafe(|| {
            run_levels(solver, scenario, plan, control, shared, tx)
        }))
        .unwrap_or_else(|_| Outcome::Failed("the solver stopped unexpectedly".into()))
    });
    if let Ok(mut s) = shared.lock() {
        s.running = None;
    }
    let _ = tx.send(Event::Finished(outcome));
}

fn run_levels(
    solver: &dyn Solver,
    scenario: &Scenario,
    plan: &Plan,
    control: &Control,
    shared: &Mutex<Shared>,
    tx: &Sender<Event>,
) -> Outcome {
    for &fidelity in &plan.levels {
        if control.is_cancelled() {
            return Outcome::Cancelled;
        }
        if let Ok(mut s) = shared.lock() {
            s.running = Some(fidelity);
        }
        let t0 = Instant::now();
        let mut last_sent: Option<Instant> = None;
        let mut publish = |snap: &CfdSnapshot| {
            let _ = tx.send(Event::Progress(snap.progress.clone()));
            let due = last_sent.is_none_or(|t| t.elapsed() >= plan.min_publish_interval);
            if !due {
                return;
            }
            last_sent = Some(Instant::now());
            deliver(shared, tx, Arc::new(snap.clone()));
        };
        match solver.solve(scenario, fidelity, control, &mut publish) {
            Ok(final_snap) => {
                // The final state of a level is never throttled away.
                deliver(shared, tx, Arc::new(final_snap));
                let _ = tx.send(Event::LevelDone {
                    fidelity,
                    elapsed_s: t0.elapsed().as_secs_f64(),
                });
            }
            Err(CfdError::Unsupported(_)) => {
                let _ = tx.send(Event::LevelSkipped(fidelity));
            }
            Err(CfdError::Cancelled) => return Outcome::Cancelled,
            Err(e) => return Outcome::Failed(e.to_string()),
        }
    }
    Outcome::Completed
}

fn deliver(shared: &Mutex<Shared>, tx: &Sender<Event>, snap: Arc<CfdSnapshot>) {
    if let Ok(mut s) = shared.lock() {
        s.latest = Some(snap.clone());
    }
    let _ = tx.send(Event::Snapshot(snap));
}
