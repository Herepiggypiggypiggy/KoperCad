//! Bindings: which signal drives which target in this drawing, keyed `controller/signal`.
//!
//! A target is a schematic pin, a hand-operated symbol, a PLC address or a module-instance
//! path of the machine model. The engine only stores and checks bindings; the host maps the
//! resolved levels onto the drawing.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::profile::Combine;
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
    /// Two signals of one controller drive the same target.
    SameControllerTwice,
    /// Several controllers share a target that carries an ungated signal (an E-stop chain)
    /// which does not combine as `and`: a pressed E-stop on one could be hidden by another.
    SharedChainNotAnd,
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

        let mut pairs: BTreeMap<(&str, &Target), Vec<&SignalKey>> = BTreeMap::new();
        for b in &self.bindings {
            pairs
                .entry((b.key.controller.as_str(), &b.target))
                .or_default()
                .push(&b.key);
        }
        for ((controller, target), keys) in pairs {
            let distinct: BTreeSet<&&SignalKey> = keys.iter().collect();
            if distinct.len() > 1 {
                let names: Vec<&str> = distinct.iter().map(|k| k.signal.as_str()).collect();
                finding(
                    BindingProblem::SameControllerTwice,
                    Severity::Warning,
                    keys[0],
                    format!(
                        "signals {} of {controller} all drive {target}",
                        names.join(", ")
                    ),
                );
            }
        }

        let mut chain_targets: BTreeSet<&Target> = BTreeSet::new();
        for (target, controllers) in &controllers_by_target(&self.bindings) {
            if controllers.len() < 2 {
                continue;
            }
            for b in self.bindings.iter().filter(|b| &b.target == *target) {
                let def = set
                    .instance(&b.key.controller)
                    .and_then(|i| i.controller.profile().signal(&b.key.signal));
                if def.is_some_and(|s| s.ungated && s.combine != Combine::And)
                    && chain_targets.insert(target)
                {
                    finding(
                        BindingProblem::SharedChainNotAnd,
                        Severity::Warning,
                        &b.key,
                        format!(
                            "{target} is shared and carries ungated signal {}: set `combine = \"and\"` so one pressed E-stop opens it",
                            b.key
                        ),
                    );
                }
            }
        }
        out
    }

    /// The level each target gets now. By default several signals combine as the value of
    /// largest magnitude (any high contact wins), so a service pendant overriding a radio shows.
    /// If any signal on a target says `combine = "and"` the target takes the smallest magnitude
    /// instead (a normally-closed chain is open as soon as one controller opens it).
    pub fn resolve(&self, set: &ControllerSet) -> BTreeMap<Target, f64> {
        let mut levels: BTreeMap<Target, Vec<f64>> = BTreeMap::new();
        let mut and: BTreeSet<Target> = BTreeSet::new();
        for b in &self.bindings {
            levels
                .entry(b.target.clone())
                .or_default()
                .push(set.level(&b.key).unwrap_or(0.0));
            let combine = set
                .instance(&b.key.controller)
                .and_then(|i| i.controller.profile().signal(&b.key.signal))
                .map(|s| s.combine);
            if combine == Some(Combine::And) {
                and.insert(b.target.clone());
            }
        }
        levels
            .into_iter()
            .map(|(t, v)| {
                let pick = if and.contains(&t) {
                    v.into_iter()
                        .reduce(|a, b| if b.abs() < a.abs() { b } else { a })
                } else {
                    v.into_iter()
                        .reduce(|a, b| if b.abs() > a.abs() { b } else { a })
                };
                (t, pick.unwrap_or(0.0))
            })
            .collect()
    }

    /// Who drives a target right now: the signals bound to it that are not low.
    pub fn owners(&self, target: &Target, set: &ControllerSet) -> Vec<SignalKey> {
        self.signals_of(target)
            .filter(|k| set.level(k).is_some_and(|l| l != 0.0))
            .cloned()
            .collect()
    }
}

fn controllers_by_target(bindings: &[Binding]) -> BTreeMap<&Target, BTreeSet<&str>> {
    let mut m: BTreeMap<&Target, BTreeSet<&str>> = BTreeMap::new();
    for b in bindings {
        m.entry(&b.target)
            .or_default()
            .insert(b.key.controller.as_str());
    }
    m
}
