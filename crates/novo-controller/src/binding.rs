//! Bindings: which signal drives which target in this drawing, keyed `controller/signal`.
//!
//! A target is a schematic pin, a hand-operated symbol, a PLC address or a module-instance
//! path of the machine model. The engine only stores and checks bindings; the host maps the
//! resolved levels onto the drawing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::set::ControllerSet;
use crate::validate::Severity;

/// `controller instance id` plus `signal name`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SignalKey {
    pub controller: String,
    pub signal: String,
}

impl SignalKey {
    pub fn new(controller: &str, signal: &str) -> Self {
        SignalKey {
            controller: controller.to_string(),
            signal: signal.to_string(),
        }
    }
}

impl fmt::Display for SignalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.controller, self.signal)
    }
}

/// What a signal drives.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Target {
    /// A pin of a device in the schematic (receiver or pendant output).
    Pin(String),
    /// A hand-operated symbol (pushbutton, switch) pressed by the signal.
    Symbol(String),
    /// A PLC input address, such as `%I0.0` or `%IW4`.
    PlcAddress(String),
    /// A module instance in the machine model, such as `runway1/bridge2/crab1/hoistA`.
    ModulePath(String),
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Target::Pin(s) => write!(f, "pin {s}"),
            Target::Symbol(s) => write!(f, "symbol {s}"),
            Target::PlcAddress(s) => write!(f, "PLC {s}"),
            Target::ModulePath(s) => write!(f, "module {s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub key: SignalKey,
    pub target: Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingProblem {
    /// A signal drives nothing.
    Unbound,
    /// The binding names an instance or signal that does not exist.
    UnknownSignal,
    /// The same signal is bound to the same target twice.
    Duplicate,
    /// Two controllers drive one target and the profile does not mark them `shared`.
    TargetConflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingFinding {
    pub problem: BindingProblem,
    pub severity: Severity,
    pub key: SignalKey,
    pub message: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BindingTable {
    #[serde(default, rename = "binding")]
    pub bindings: Vec<Binding>,
}

impl BindingTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a binding; returns false (and adds nothing) when the same pair already exists.
    pub fn bind(&mut self, key: SignalKey, target: Target) -> bool {
        let b = Binding { key, target };
        if self.bindings.contains(&b) {
            return false;
        }
        self.bindings.push(b);
        true
    }

    pub fn unbind(&mut self, key: &SignalKey) {
        self.bindings.retain(|b| &b.key != key);
    }

    pub fn targets_of(&self, key: &SignalKey) -> impl Iterator<Item = &Target> {
        self.bindings
            .iter()
            .filter(move |b| &b.key == key)
            .map(|b| &b.target)
    }

    pub fn signals_of<'a>(&'a self, target: &'a Target) -> impl Iterator<Item = &'a SignalKey> {
        self.bindings
            .iter()
            .filter(move |b| &b.target == target)
            .map(|b| &b.key)
    }

    /// The checks of the plan: signal bound to nothing, binding to a missing signal, the same
    /// binding twice, two controllers on one target (unless every one of them is `shared`).
    pub fn check(&self, set: &ControllerSet) -> Vec<BindingFinding> {
        let mut out = Vec::new();
        let mut finding = |problem, severity, key: &SignalKey, message: String| {
            out.push(BindingFinding {
                problem,
                severity,
                key: key.clone(),
                message,
            });
        };

        let mut seen = BTreeSet::new();
        for b in &self.bindings {
            if !set.has_signal(&b.key) {
                finding(
                    BindingProblem::UnknownSignal,
                    Severity::Error,
                    &b.key,
                    format!("{} is bound to {} but does not exist", b.key, b.target),
                );
            }
            if !seen.insert((&b.key, &b.target)) {
                finding(
                    BindingProblem::Duplicate,
                    Severity::Warning,
                    &b.key,
                    format!("{} is bound to {} twice", b.key, b.target),
                );
            }
        }

        let bound: BTreeSet<&SignalKey> = self.bindings.iter().map(|b| &b.key).collect();
        for (key, _) in set.signals() {
            if !bound.contains(&key) {
                finding(
                    BindingProblem::Unbound,
                    Severity::Warning,
                    &key,
                    format!("{key} drives nothing"),
                );
            }
        }

        let mut by_target: BTreeMap<&Target, BTreeSet<&str>> = BTreeMap::new();
        for b in &self.bindings {
            by_target
                .entry(&b.target)
                .or_default()
                .insert(b.key.controller.as_str());
        }
        for (target, controllers) in by_target {
            if controllers.len() > 1 && controllers.iter().any(|c| !set.is_shared(c)) {
                let names: Vec<&str> = controllers.iter().copied().collect();
                for b in self.bindings.iter().filter(|b| &b.target == target) {
                    finding(
                        BindingProblem::TargetConflict,
                        Severity::Error,
                        &b.key,
                        format!(
                            "{target} is driven by controllers {} and not all are marked shared",
                            names.join(", ")
                        ),
                    );
                }
            }
        }
        out
    }

    /// The level each target gets now. Several signals on one target combine as the value of
    /// largest magnitude (any high contact wins), so a service pendant overriding a radio shows.
    pub fn resolve(&self, set: &ControllerSet) -> BTreeMap<Target, f64> {
        let mut out: BTreeMap<Target, f64> = BTreeMap::new();
        for b in &self.bindings {
            let level = set.level(&b.key).unwrap_or(0.0);
            let slot = out.entry(b.target.clone()).or_insert(0.0);
            if level.abs() > slot.abs() {
                *slot = level;
            }
        }
        out
    }

    /// Who drives a target right now: the signals bound to it that are not low.
    pub fn owners(&self, target: &Target, set: &ControllerSet) -> Vec<SignalKey> {
        self.signals_of(target)
            .filter(|k| set.level(k).is_some_and(|l| l != 0.0))
            .cloned()
            .collect()
    }
}
