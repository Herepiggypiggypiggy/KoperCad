//! The loop's view of the schematic (lane L5a, design `docs/design/electromechanical-loop.md`).
//!
//! `novo-machine` never depends on the logic simulator. The app implements [`CircuitPort`] on
//! top of `novo-sim` (settle, pole states); tests use [`FakeCircuit`]. Devices are named by their
//! device tag, for example `=A+B-K1`.

use std::collections::{BTreeMap, BTreeSet};

pub use novo_drive::motor::{Direction, WindingSel};

/// What the circuit feeds a motor right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotorFeed {
    /// No voltage on the terminals.
    Off,
    /// One winding fed in a sense of rotation.
    Fed {
        winding: WindingSel,
        direction: Direction,
    },
}

/// The circuit as the co-simulation loop sees it.
pub trait CircuitPort {
    /// Advances the logic by one tick (contacts follow coils, timers run).
    fn settle(&mut self, dt_s: f64);
    /// What the motor with this device tag is fed with (winding and direction from the
    /// closed contactors and the phase order).
    fn motor_feed(&self, motor: &str) -> MotorFeed;
    /// Whether the device (a brake rectifier feed, the main contactor coil) is energised.
    fn is_energised(&self, device: &str) -> bool;
    /// Drives a machine sensor contact (limit switch): `closed` is its conducting state.
    fn set_contact(&mut self, device: &str, closed: bool);
}

type Rule = Box<dyn FnMut(&mut FakeState)>;

/// A scriptable circuit for tests: set feeds and energised devices by hand, or install a rule
/// that computes them from the sensor contacts each tick.
#[derive(Default)]
pub struct FakeCircuit {
    pub feeds: BTreeMap<String, MotorFeed>,
    pub energised: BTreeSet<String>,
    /// Sensor contacts as last written by the loop (`true` = closed).
    pub contacts: BTreeMap<String, bool>,
    pub settles: u64,
    rule: Option<Rule>,
}

/// What a [`FakeCircuit`] rule may read and change.
pub struct FakeState<'a> {
    pub feeds: &'a mut BTreeMap<String, MotorFeed>,
    pub energised: &'a mut BTreeSet<String>,
    pub contacts: &'a BTreeMap<String, bool>,
}

impl FakeCircuit {
    pub fn new() -> Self {
        Self::default()
    }

    /// Installs a rule that runs in every `settle`, after the sensor contacts are known.
    pub fn with_rule(mut self, rule: impl FnMut(&mut FakeState) + 'static) -> Self {
        self.rule = Some(Box::new(rule));
        self
    }
}

impl CircuitPort for FakeCircuit {
    fn settle(&mut self, _dt_s: f64) {
        self.settles += 1;
        if let Some(rule) = self.rule.as_mut() {
            rule(&mut FakeState {
                feeds: &mut self.feeds,
                energised: &mut self.energised,
                contacts: &self.contacts,
            });
        }
    }

    fn motor_feed(&self, motor: &str) -> MotorFeed {
        self.feeds.get(motor).copied().unwrap_or(MotorFeed::Off)
    }

    fn is_energised(&self, device: &str) -> bool {
        self.energised.contains(device)
    }

    fn set_contact(&mut self, device: &str, closed: bool) {
        self.contacts.insert(device.to_owned(), closed);
    }
}
