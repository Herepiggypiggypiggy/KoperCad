//! Record and replay of timed presses, deterministic by construction: events carry integer
//! milliseconds, replay applies them in order (ties keep file order) with no clock or random.

use serde::{Deserialize, Serialize};

use crate::binding::SignalKey;
use crate::set::{ControllerSet, SetError};

/// A replay may take at most this many samples.
pub const MAX_SAMPLES: u64 = 1_000_000;
/// Events later than this (24 hours) are taken for typing mistakes.
pub const MAX_EVENT_MS: u64 = 86_400_000;

/// One thing a person or a script does to a controller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Press { control: String },
    Release { control: String },
    SetPosition { control: String, position: String },
    Reset { control: String },
    Link { ok: bool },
    BatteryLow { low: bool },
    Power { on: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimedAction {
    pub t_ms: u64,
    pub instance: String,
    #[serde(flatten)]
    pub action: Action,
}

/// A `.sequence.toml`: a name and timed actions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sequence {
    #[serde(default)]
    pub name: String,
    #[serde(default, rename = "event")]
    pub events: Vec<TimedAction>,
}

impl Sequence {
    pub fn parse(text: &str) -> Result<Sequence, String> {
        let mut seq: Sequence = toml::from_str(text).map_err(|e| e.to_string())?;
        // Stable sort: events at the same time keep the order of the file.
        seq.events.sort_by_key(|e| e.t_ms);
        Ok(seq)
    }

    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn duration_ms(&self) -> u64 {
        self.events.iter().map(|e| e.t_ms).max().unwrap_or(0)
    }

    /// Problems that would stop a replay on this set (unknown instances, controls, positions).
    pub fn check(&self, set: &ControllerSet) -> Vec<String> {
        let mut problems = Vec::new();
        for (i, e) in self.events.iter().enumerate() {
            if e.t_ms > MAX_EVENT_MS {
                problems.push(format!("event {i}: t_ms {} is later than 24 hours", e.t_ms));
            }
            let Some(inst) = set.instance(&e.instance) else {
                problems.push(format!(
                    "event {i}: no controller instance `{}`",
                    e.instance
                ));
                continue;
            };
            let profile = inst.controller.profile();
            let control = match &e.action {
                Action::Press { control }
                | Action::Release { control }
                | Action::Reset { control }
                | Action::SetPosition { control, .. } => Some(control),
                _ => None,
            };
            if let Some(c) = control {
                match profile.control(c) {
                    None => problems.push(format!(
                        "event {i}: instance `{}` has no control `{c}`",
                        e.instance
                    )),
                    Some(def) => {
                        if let Action::SetPosition { position, .. } = &e.action
                            && !def.position_list().contains(position)
                        {
                            problems.push(format!(
                                "event {i}: control `{c}` has no position `{position}`"
                            ));
                        }
                    }
                }
            }
        }
        problems
    }
}

/// Collects actions with the time they happened (see [`ControllerSet::start_recording`]).
#[derive(Debug, Clone, Default)]
pub struct Recorder {
    events: Vec<TimedAction>,
}

impl Recorder {
    pub(crate) fn push(&mut self, event: TimedAction) {
        self.events.push(event);
    }

    /// The sequence, shifted so the first event is at 0 ms.
    pub(crate) fn finish(self, name: &str) -> Sequence {
        let start = self.events.first().map_or(0, |e| e.t_ms);
        let events = self.events.into_iter().map(|mut e| {
            e.t_ms -= start;
            e
        });
        Sequence {
            name: name.to_string(),
            events: events.collect(),
        }
    }
}

/// One sample of every signal.
#[derive(Debug, Clone, PartialEq)]
pub struct TraceSample {
    pub t_ms: u64,
    pub signals: Vec<(SignalKey, f64)>,
}

/// What a replay produced: two replays of one sequence give equal traces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Trace {
    pub samples: Vec<TraceSample>,
}

/// Steps through a sequence in time, for the fixed-step loop and for [`Sequence::replay`].
#[derive(Debug, Clone)]
pub struct Player {
    events: Vec<TimedAction>,
    next: usize,
    start_ms: u64,
}

impl Player {
    /// Replay starts at the set's current time: event time 0 is "now".
    pub fn new(sequence: &Sequence, set: &ControllerSet) -> Player {
        let mut events = sequence.events.clone();
        events.sort_by_key(|e| e.t_ms);
        Player {
            events,
            next: 0,
            start_ms: set.time_ms(),
        }
    }

    pub fn finished(&self) -> bool {
        self.next >= self.events.len()
    }

    /// Time since the replay began, as the set's clock sees it.
    pub fn elapsed_ms(&self, set: &ControllerSet) -> u64 {
        set.time_ms().saturating_sub(self.start_ms)
    }

    /// Apply every event due up to `elapsed_ms` since the start, advancing the set's clock to
    /// each event time and finally to `elapsed_ms`. Blocked presses are part of the replay
    /// (the controller decides, as it did when recording); unknown names are errors.
    pub fn advance_to(&mut self, set: &mut ControllerSet, elapsed_ms: u64) -> Result<(), SetError> {
        while let Some(e) = self.events.get(self.next) {
            if e.t_ms > elapsed_ms {
                break;
            }
            let now = self.elapsed_ms(set);
            if e.t_ms > now {
                set.advance(e.t_ms - now);
            }
            set.act(&e.instance, &e.action)?;
            self.next += 1;
        }
        let now = self.elapsed_ms(set);
        if elapsed_ms > now {
            set.advance(elapsed_ms - now);
        }
        Ok(())
    }
}

impl Sequence {
    /// Replay on `set`, sampling all signals after each event time and every `sample_ms`
    /// (0 = only at events). Ends `settle_ms` after the last event.
    pub fn replay(
        &self,
        set: &mut ControllerSet,
        sample_ms: u64,
        settle_ms: u64,
    ) -> Result<Trace, SetError> {
        let end = self.duration_ms().saturating_add(settle_ms);
        let periodic = end
            .checked_div(sample_ms)
            .map_or(0, |n| n.saturating_add(1));
        if (self.events.len() as u64).saturating_add(periodic) > MAX_SAMPLES {
            return Err(SetError::TooManySamples);
        }
        let mut player = Player::new(self, set);
        let mut trace = Trace::default();
        let mut times: Vec<u64> = self.events.iter().map(|e| e.t_ms).collect();
        if sample_ms > 0 {
            times.extend((0..=end).step_by(usize::try_from(sample_ms).unwrap_or(usize::MAX)));
        }
        times.push(end);
        times.sort_unstable();
        times.dedup();
        for t in times {
            player.advance_to(set, t)?;
            trace.samples.push(TraceSample {
                t_ms: t,
                signals: set.signals(),
            });
        }
        Ok(trace)
    }
}
