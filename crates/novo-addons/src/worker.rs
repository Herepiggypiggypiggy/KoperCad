//! An engine on its own thread, so a slow call never stops the window from drawing.
//!
//! The engine is loaded and called on the worker; the program submits a request, carries on,
//! and collects the answer on a later frame. Requests are answered in the order they were
//! sent. A call is still bounded by the step budget (`HostLimits::fuel_per_call`), so a runaway
//! engine ends on the worker and is stopped as before; the window never waits for it.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use serde_json::Value;

use crate::host::{CallError, EngineHost, HostLimits};

/// What an engine has used so far, as of its latest answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineStats {
    /// Calls so far, the start-up included.
    pub calls: u64,
    /// Most steps one call used.
    pub peak_fuel: u64,
    /// Steps one call may use.
    pub fuel_budget: u64,
}

/// The answer to one submitted request.
#[derive(Debug)]
pub struct Done {
    /// The number [`EngineWorker::submit`] returned for the request.
    pub ticket: u64,
    pub result: Result<Value, CallError>,
}

struct Job {
    ticket: u64,
    op: String,
    data: Value,
}

struct Answer {
    done: Done,
    stats: Option<EngineStats>,
}

/// A running engine and its thread.
pub struct EngineWorker {
    jobs: Sender<Job>,
    answers: Receiver<Answer>,
    next: u64,
    pending: VecDeque<u64>,
    stats: Option<EngineStats>,
}

impl EngineWorker {
    /// Starts a thread that loads `wasm` (the engine's start-up runs there too) and answers
    /// requests. `wake` is called after every answer, from the worker thread, so the window
    /// can ask for another frame.
    pub fn start(wasm: Vec<u8>, limits: HostLimits, wake: impl Fn() + Send + 'static) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (answer_tx, answers) = mpsc::channel::<Answer>();
        thread::Builder::new()
            .name("add-on engine".to_owned())
            .spawn(move || run(wasm, limits, job_rx, answer_tx, wake))
            .expect("the operating system can start a thread");
        Self {
            jobs,
            answers,
            next: 0,
            pending: VecDeque::new(),
            stats: None,
        }
    }

    /// Queues one request and returns its ticket at once.
    pub fn submit(&mut self, op: &str, data: &Value) -> u64 {
        let ticket = self.next;
        self.next += 1;
        self.pending.push_back(ticket);
        let job = Job {
            ticket,
            op: op.to_owned(),
            data: data.clone(),
        };
        // A send fails only when the thread has ended; the ticket is then answered with a stop.
        let _ = self.jobs.send(job);
        ticket
    }

    /// The next answer that has arrived, if any. Never waits.
    pub fn try_next(&mut self) -> Option<Done> {
        match self.answers.try_recv() {
            Ok(answer) => Some(self.take(answer)),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => self.lost(),
        }
    }

    /// Waits for the next answer. `None` when nothing is pending.
    pub fn wait_next(&mut self) -> Option<Done> {
        if self.pending.is_empty() {
            return None;
        }
        match self.answers.recv() {
            Ok(answer) => Some(self.take(answer)),
            Err(_) => self.lost(),
        }
    }

    fn take(&mut self, answer: Answer) -> Done {
        self.pending.pop_front();
        if answer.stats.is_some() {
            self.stats = answer.stats;
        }
        answer.done
    }

    /// The thread ended with requests unanswered (it panicked): each is answered with a stop.
    fn lost(&mut self) -> Option<Done> {
        let ticket = self.pending.pop_front()?;
        Some(Done {
            ticket,
            result: Err(CallError::Stopped(
                "its engine thread ended unexpectedly".to_owned(),
            )),
        })
    }

    /// Requests sent and not yet answered.
    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// What the engine had used at its latest answer; `None` before the first.
    pub fn stats(&self) -> Option<EngineStats> {
        self.stats
    }
}

fn run(
    wasm: Vec<u8>,
    limits: HostLimits,
    jobs: Receiver<Job>,
    answers: Sender<Answer>,
    wake: impl Fn(),
) {
    let budget = limits.fuel_per_call;
    let loaded = EngineHost::load(&wasm, limits);
    drop(wasm);
    let mut host = match loaded {
        Ok(host) => host,
        Err(e) => {
            // A start that fails stops the add-on: every request, the first included, says why.
            let why = e.to_string();
            for job in jobs {
                let done = Done {
                    ticket: job.ticket,
                    result: Err(CallError::Stopped(why.clone())),
                };
                if answers.send(Answer { done, stats: None }).is_err() {
                    return;
                }
                wake();
            }
            return;
        }
    };
    for job in jobs {
        let result = host.call(&job.op, &job.data);
        let stats = EngineStats {
            calls: host.calls(),
            peak_fuel: host.peak_fuel(),
            fuel_budget: host.fuel_budget(),
        };
        debug_assert_eq!(stats.fuel_budget, budget);
        let done = Done {
            ticket: job.ticket,
            result,
        };
        let answer = Answer {
            done,
            stats: Some(stats),
        };
        if answers.send(answer).is_err() {
            return;
        }
        wake();
    }
}
