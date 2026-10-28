//! One controller: the state of its controls and the signals they make.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::profile::{
    Condition, ControlDef, ControlKind, ControllerKind, InterlockMode, Profile, SignalDef,
    SignalKind, split_group_step,
};
use crate::step_contact;
use crate::validate::{Issue, Severity, validate};

/// Everything that changes while the controller runs; saved with the project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Position of each control, by control id.
    pub positions: BTreeMap<String, String>,
    pub link_ok: bool,
    pub battery_low: bool,
    pub powered: bool,
    /// Controller clock in milliseconds.
    pub time_ms: u64,
    /// When the receiver last powered up (the self-test delay counts from here).
    pub powered_at_ms: u64,
}

/// Why a change was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A receiver-side control needs the radio link and power.
    LinkDown,
    /// An E-stop leaves `pressed` only through `reset`.
    NeedsReset,
    /// A condition the control requires does not hold (`control=position`).
    Requires(String),
    /// A condition that keeps the control at rest holds.
    Inhibited(String),
    /// Another control of the same interlock is already down.
    Interlock { with: String },
}

/// What a request did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Applied {
    Changed,
    Unchanged,
    Blocked(Block),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    UnknownControl(String),
    UnknownPosition {
        control: String,
        position: String,
    },
    /// `press` on a control with several active positions: name one with `set_position`.
    NeedsPosition(String),
}

impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ControlError::UnknownControl(c) => write!(f, "no control `{c}`"),
            ControlError::UnknownPosition { control, position } => {
                write!(f, "control `{control}` has no position `{position}`")
            }
            ControlError::NeedsPosition(c) => {
                write!(f, "control `{c}` needs a position, use set_position")
            }
        }
    }
}

impl std::error::Error for ControlError {}

/// The value of one signal now.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalState {
    pub name: String,
    pub kind: SignalKind,
    /// 0 or 1 for contact and voltage signals, the analog value otherwise.
    pub level: f64,
}

impl SignalState {
    pub fn is_high(&self) -> bool {
        self.level != 0.0
    }
}

/// A signal changing level, for the monitor's history strip.
#[derive(Debug, Clone, PartialEq)]
pub struct SignalChange {
    pub t_ms: u64,
    pub signal: String,
    pub level: f64,
}

const HISTORY_LIMIT: usize = 512;

/// A running controller: a validated profile plus its [`State`].
#[derive(Debug, Clone)]
pub struct Controller {
    profile: Profile,
    state: State,
    last: BTreeMap<String, f64>,
    history: Vec<SignalChange>,
}

impl Controller {
    /// All controls at rest, link up, powered at time 0. Fails when the profile has errors.
    pub fn new(profile: Profile) -> Result<Controller, Vec<Issue>> {
        let state = State {
            positions: profile
                .controls
                .iter()
                .map(|c| (c.id.clone(), c.rest_position()))
                .collect(),
            link_ok: true,
            battery_low: false,
            powered: true,
            time_ms: 0,
            powered_at_ms: 0,
        };
        Self::restore(profile, state)
    }

    /// Continue from a saved state. Positions the profile does not know are dropped and
    /// controls missing from the state start at rest, so an edited profile still loads.
    pub fn restore(profile: Profile, mut state: State) -> Result<Controller, Vec<Issue>> {
        let errors: Vec<Issue> = validate(&profile)
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut positions = BTreeMap::new();
        for c in &profile.controls {
            let pos = state
                .positions
                .get(&c.id)
                .filter(|p| c.position_list().contains(p))
                .cloned()
                .unwrap_or_else(|| c.rest_position());
            positions.insert(c.id.clone(), pos);
        }
        state.positions = positions;
        let mut ctl = Controller {
            profile,
            state,
            last: BTreeMap::new(),
            history: Vec::new(),
        };
        ctl.last = ctl.levels();
        Ok(ctl)
    }

    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn time_ms(&self) -> u64 {
        self.state.time_ms
    }

    /// Where the control physically is.
    pub fn position(&self, control: &str) -> Option<&str> {
        self.state.positions.get(control).map(String::as_str)
    }

    /// Where the control counts as being once interlocks are applied.
    pub fn effective_position(&self, control: &str) -> Option<String> {
        let idx = self.profile.controls.iter().position(|c| c.id == control)?;
        Some(self.effective()[idx].clone())
    }

    pub fn link_ok(&self) -> bool {
        self.state.link_ok
    }

    pub fn battery_low(&self) -> bool {
        self.state.battery_low
    }

    pub fn powered(&self) -> bool {
        self.state.powered
    }

    /// True while the receiver self-test after power-up still holds every signal low.
    pub fn in_self_test(&self) -> bool {
        self.state.powered
            && self.state.time_ms.saturating_sub(self.state.powered_at_ms)
                < self.profile.effects.self_test_ms
    }

    /// Signal changes since the start, newest last (the last 512).
    pub fn history(&self) -> &[SignalChange] {
        &self.history
    }

    // ----- requests -----

    /// Press a button, deadman, key, E-stop, or toggle a latch.
    pub fn press(&mut self, control: &str) -> Result<Applied, ControlError> {
        let def = self.control_def(control)?;
        let target = match def.kind {
            ControlKind::Latch => {
                let rest = def.rest_position();
                if self.state.positions[control] != rest {
                    rest
                } else {
                    active_of(def)
                }
            }
            ControlKind::Rocker2
            | ControlKind::Rocker3
            | ControlKind::Axis
            | ControlKind::Selector => {
                return Err(ControlError::NeedsPosition(control.to_string()));
            }
            _ => active_of(def),
        };
        self.set_position(control, &target)
    }

    /// Let go: a spring control returns to rest, anything else stays where it is.
    pub fn release(&mut self, control: &str) -> Result<Applied, ControlError> {
        let def = self.control_def(control)?;
        let rest = def.rest_position();
        if !def.is_spring() || self.state.positions[control] == rest {
            return Ok(Applied::Unchanged);
        }
        self.state.positions.insert(control.to_string(), rest);
        self.settle_and_refresh();
        Ok(Applied::Changed)
    }

    /// Move a control to a named position (rocker step, axis detent, selector, latch state).
    pub fn set_position(&mut self, control: &str, position: &str) -> Result<Applied, ControlError> {
        let def = self.control_def(control)?;
        if !def.position_list().iter().any(|p| p == position) {
            return Err(ControlError::UnknownPosition {
                control: control.to_string(),
                position: position.to_string(),
            });
        }
        let rest = def.rest_position();
        if self.state.positions[control] == position {
            return Ok(Applied::Unchanged);
        }
        if def.kind == ControlKind::EStop && position == rest {
            return Ok(Applied::Blocked(Block::NeedsReset));
        }
        if position != rest
            && let Some(block) = self.leaving_rest_blocked(control)
        {
            return Ok(Applied::Blocked(block));
        }
        self.state
            .positions
            .insert(control.to_string(), position.to_string());
        self.settle_and_refresh();
        Ok(Applied::Changed)
    }

    /// Twist-release: send a control (an E-stop) back to rest whatever it was.
    pub fn reset(&mut self, control: &str) -> Result<Applied, ControlError> {
        let def = self.control_def(control)?;
        let rest = def.rest_position();
        if self.state.positions[control] == rest {
            return Ok(Applied::Unchanged);
        }
        self.state.positions.insert(control.to_string(), rest);
        self.settle_and_refresh();
        Ok(Applied::Changed)
    }

    /// The radio link comes up or is lost. Receiver-side latches drop on loss and stay dropped.
    pub fn set_link(&mut self, ok: bool) -> Applied {
        self.flag(|s| &mut s.link_ok, ok)
    }

    pub fn set_battery_low(&mut self, low: bool) -> Applied {
        self.flag(|s| &mut s.battery_low, low)
    }

    /// Power the receiver. Switching on restarts the self-test delay.
    pub fn power(&mut self, on: bool) -> Applied {
        if self.state.powered == on {
            return Applied::Unchanged;
        }
        self.state.powered = on;
        if on {
            self.state.powered_at_ms = self.state.time_ms;
        }
        self.settle_and_refresh();
        Applied::Changed
    }

    /// Let time pass. Signals can change when the self-test delay ends.
    pub fn advance(&mut self, dt_ms: u64) {
        self.state.time_ms = self.state.time_ms.saturating_add(dt_ms);
        self.refresh();
    }

    fn flag(&mut self, field: impl Fn(&mut State) -> &mut bool, value: bool) -> Applied {
        let slot = field(&mut self.state);
        if *slot == value {
            return Applied::Unchanged;
        }
        *slot = value;
        self.settle_and_refresh();
        Applied::Changed
    }

    // ----- signals -----

    /// Every signal in profile order.
    pub fn signals(&self) -> Vec<SignalState> {
        let eff = self.effective();
        let live = self.live();
        self.profile
            .signals
            .iter()
            .map(|s| SignalState {
                name: s.name.clone(),
                kind: s.kind,
                level: self.level_of(s, &eff, live),
            })
            .collect()
    }

    pub fn level(&self, signal: &str) -> Option<f64> {
        let s = self.profile.signal(signal)?;
        Some(self.level_of(s, &self.effective(), self.live()))
    }

    pub fn is_high(&self, signal: &str) -> bool {
        self.level(signal).is_some_and(|l| l != 0.0)
    }

    /// Notes the UI shows next to the panel: link lost, battery low, self-test running.
    pub fn warnings(&self) -> Vec<&'static str> {
        let mut w = Vec::new();
        if !self.state.powered {
            w.push("receiver off");
        }
        if self.in_self_test() {
            w.push("receiver self-test running");
        }
        if !self.state.link_ok {
            w.push("radio link lost");
        }
        if self.state.battery_low {
            w.push("transmitter battery low");
        }
        w
    }

    // ----- internals -----

    fn control_def(&self, id: &str) -> Result<&ControlDef, ControlError> {
        self.profile
            .control(id)
            .ok_or_else(|| ControlError::UnknownControl(id.to_string()))
    }

    fn live(&self) -> bool {
        self.state.powered && !self.in_self_test()
    }

    fn receiver_up(&self) -> bool {
        self.state.powered && self.state.link_ok
    }

    /// Positions in control order with both-off interlocks applied.
    fn effective(&self) -> Vec<String> {
        let mut eff: Vec<String> = self
            .profile
            .controls
            .iter()
            .map(|c| self.state.positions[&c.id].clone())
            .collect();
        let idx = |id: &str| self.profile.controls.iter().position(|c| c.id == id);
        let rests: Vec<String> = self
            .profile
            .controls
            .iter()
            .map(ControlDef::rest_position)
            .collect();
        for il in self
            .profile
            .interlocks
            .iter()
            .filter(|i| i.mode == InterlockMode::BothOff)
        {
            let members: Vec<usize> = il.between.iter().filter_map(|id| idx(id)).collect();
            let down = members.iter().filter(|&&i| eff[i] != rests[i]).count();
            if down >= 2 {
                for i in members {
                    eff[i] = rests[i].clone();
                }
            }
        }
        eff
    }

    fn holds(&self, text: &str, eff: &[String]) -> bool {
        let Ok(Condition {
            control,
            position,
            negate,
        }) = text.parse()
        else {
            return false;
        };
        match self.profile.controls.iter().position(|c| c.id == control) {
            Some(i) => (eff[i] == position) != negate,
            None => false,
        }
    }

    fn first_failing<'a>(&self, conditions: &'a [String], eff: &[String]) -> Option<&'a String> {
        conditions.iter().find(|c| !self.holds(c, eff))
    }

    fn leaving_rest_blocked(&self, control: &str) -> Option<Block> {
        let def = self.profile.control(control)?;
        if def.needs_link && !self.receiver_up() {
            return Some(Block::LinkDown);
        }
        let eff = self.effective();
        if let Some(c) = self.first_failing(&def.requires, &eff) {
            return Some(Block::Requires(c.clone()));
        }
        if let Some(c) = def.drops_when.iter().find(|c| self.holds(c, &eff)) {
            return Some(Block::Inhibited(c.clone()));
        }
        for il in self
            .profile
            .interlocks
            .iter()
            .filter(|i| i.mode == InterlockMode::FirstWins)
        {
            if !il.between.iter().any(|id| id == control) {
                continue;
            }
            for other in il.between.iter().filter(|id| *id != control) {
                let Some(oi) = self.profile.controls.iter().position(|c| c.id == *other) else {
                    continue;
                };
                if eff[oi] != self.profile.controls[oi].rest_position() {
                    return Some(Block::Interlock {
                        with: other.clone(),
                    });
                }
            }
        }
        None
    }

    /// Send receiver-side controls back to rest while their conditions fail, until stable.
    fn settle(&mut self) {
        for _ in 0..=self.profile.controls.len() {
            let eff = self.effective();
            let mut dropped = None;
            for (i, def) in self.profile.controls.iter().enumerate() {
                let rest = def.rest_position();
                if def.kind == ControlKind::EStop || self.state.positions[&def.id] == rest {
                    continue;
                }
                let drop = (def.needs_link && !self.receiver_up())
                    || self.first_failing(&def.requires, &eff).is_some()
                    || def.drops_when.iter().any(|c| self.holds(c, &eff));
                if drop {
                    dropped = Some((i, rest));
                    break;
                }
            }
            let Some((i, rest)) = dropped else { return };
            let id = self.profile.controls[i].id.clone();
            self.state.positions.insert(id, rest);
        }
    }

    fn settle_and_refresh(&mut self) {
        self.settle();
        self.refresh();
    }

    fn levels(&self) -> BTreeMap<String, f64> {
        let eff = self.effective();
        let live = self.live();
        self.profile
            .signals
            .iter()
            .map(|s| (s.name.clone(), self.level_of(s, &eff, live)))
            .collect()
    }

    fn refresh(&mut self) {
        let now = self.levels();
        for s in &self.profile.signals {
            let new = now[&s.name];
            if self.last.get(&s.name).copied() != Some(new) {
                self.history.push(SignalChange {
                    t_ms: self.state.time_ms,
                    signal: s.name.clone(),
                    level: new,
                });
            }
        }
        if self.history.len() > HISTORY_LIMIT {
            let excess = self.history.len() - HISTORY_LIMIT;
            self.history.drain(..excess);
        }
        self.last = now;
    }

    fn level_of(&self, sig: &SignalDef, eff: &[String], live: bool) -> f64 {
        if !live {
            return 0.0;
        }
        let needs_link = sig
            .needs_link
            .unwrap_or(self.profile.controller.kind == ControllerKind::Radio);
        if needs_link && !self.state.link_ok {
            return 0.0;
        }
        if !sig.ungated
            && self
                .first_failing(&self.profile.controller.gate, eff)
                .is_some()
        {
            return 0.0;
        }
        if self.first_failing(&sig.requires, eff).is_some() {
            return 0.0;
        }
        let Some(ci) = self
            .profile
            .controls
            .iter()
            .position(|c| c.id == sig.control)
        else {
            return 0.0;
        };
        let pos = eff[ci].as_str();
        match sig.kind {
            SignalKind::Analog => sig.levels.get(pos).copied().unwrap_or(0.0),
            SignalKind::Contact | SignalKind::Voltage => {
                let high = if let (Some(group), Some(step)) = (sig.group.as_deref(), sig.step) {
                    split_group_step(pos).is_some_and(|(g, at)| {
                        g == group
                            && step_contact(at, step, self.profile.controller.step2_includes_step1)
                    })
                } else {
                    sig.when.iter().any(|w| w == pos)
                };
                if high { 1.0 } else { 0.0 }
            }
        }
    }
}

fn active_of(def: &ControlDef) -> String {
    def.first_active_position().unwrap_or_default()
}
