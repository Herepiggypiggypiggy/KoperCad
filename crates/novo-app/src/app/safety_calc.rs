//! The free calculator of the Safety window, kept in the project under
//! [`CALCULATOR_KEY`] so its inputs survive save and reopen (and undo).
//!
//! Stored as plain numbers and short codes, not as the engine types, so the
//! stored form stays readable when the engine changes. An unreadable value
//! leaves the calculator at its defaults.

use eframe::egui;
use novo_core::{Project, Value};
use novo_safety::{Category, Frequency, Probability, Severity};
use serde::{Deserialize, Serialize};

use super::NovoApp;
use super::safety::{PartBasis, PartInput, SafetyWindow, SubsystemInput};

/// Project property holding the calculator inputs (JSON text).
pub(super) const CALCULATOR_KEY: &str = "safety.calculator";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Stored {
    severity: String,
    frequency: String,
    avoidance: String,
    days: f64,
    hours: f64,
    cycle_s: f64,
    subsystems: Vec<StoredSubsystem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredSubsystem {
    name: String,
    category: String,
    channel1: Vec<StoredPart>,
    channel2: Vec<StoredPart>,
    dc_percent: f64,
    ccf: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredPart {
    name: String,
    /// "mttfd" or "b10d".
    basis: String,
    value: f64,
}

fn part_to(p: &PartInput) -> StoredPart {
    StoredPart {
        name: p.name.clone(),
        basis: match p.basis {
            PartBasis::Mttfd => "mttfd",
            PartBasis::B10d => "b10d",
        }
        .into(),
        value: p.value,
    }
}

fn part_from(p: &StoredPart) -> PartInput {
    PartInput {
        name: p.name.clone(),
        basis: if p.basis == "b10d" {
            PartBasis::B10d
        } else {
            PartBasis::Mttfd
        },
        value: p.value,
    }
}

fn category_from(s: &str) -> Category {
    match s {
        "B" => Category::B,
        "1" => Category::C1,
        "2" => Category::C2,
        "4" => Category::C4,
        _ => Category::C3,
    }
}

fn to_stored(w: &SafetyWindow) -> Stored {
    Stored {
        severity: if w.severity == Severity::S1 {
            "S1"
        } else {
            "S2"
        }
        .into(),
        frequency: if w.frequency == Frequency::F1 {
            "F1"
        } else {
            "F2"
        }
        .into(),
        avoidance: if w.avoidance == Probability::P1 {
            "P1"
        } else {
            "P2"
        }
        .into(),
        days: w.days,
        hours: w.hours,
        cycle_s: w.cycle_s,
        subsystems: w
            .subsystems
            .iter()
            .map(|s| StoredSubsystem {
                name: s.name.clone(),
                category: s.category.label().into(),
                channel1: s.channel1.iter().map(part_to).collect(),
                channel2: s.channel2.iter().map(part_to).collect(),
                dc_percent: s.dc_percent,
                ccf: s.ccf,
            })
            .collect(),
    }
}

fn apply(w: &mut SafetyWindow, s: &Stored) {
    w.severity = if s.severity == "S1" {
        Severity::S1
    } else {
        Severity::S2
    };
    w.frequency = if s.frequency == "F2" {
        Frequency::F2
    } else {
        Frequency::F1
    };
    w.avoidance = if s.avoidance == "P2" {
        Probability::P2
    } else {
        Probability::P1
    };
    w.days = s.days.clamp(1.0, 366.0);
    w.hours = s.hours.clamp(0.1, 24.0);
    w.cycle_s = s.cycle_s.clamp(0.1, 86_400.0);
    w.subsystems = s
        .subsystems
        .iter()
        .map(|x| SubsystemInput {
            name: x.name.clone(),
            category: category_from(&x.category),
            channel1: x.channel1.iter().map(part_from).collect(),
            channel2: x.channel2.iter().map(part_from).collect(),
            dc_percent: x.dc_percent.clamp(0.0, 99.9),
            ccf: x.ccf.min(100),
        })
        .collect();
}

/// The stored form of the calculator, or `None` while it equals the
/// defaults (nothing worth keeping, and the property is removed).
fn encode(w: &SafetyWindow) -> Option<Value> {
    let now = to_stored(w);
    if now == to_stored(&SafetyWindow::default()) {
        return None;
    }
    serde_json::to_string(&now).ok().map(Value::Text)
}

fn stored(project: &Project) -> Option<Stored> {
    match project.info().properties.get(CALCULATOR_KEY) {
        Some(Value::Text(t)) => serde_json::from_str(t).ok(),
        _ => None,
    }
}

/// Loads the stored calculator into a fresh window.
pub(super) fn load(win: &mut SafetyWindow, project: &Project) {
    if let Some(s) = stored(project) {
        apply(win, &s);
    }
}

/// Whether the window's calculator differs from what the project holds.
fn differs(win: &SafetyWindow, project: &Project) -> bool {
    let held = stored(project).unwrap_or_else(|| to_stored(&SafetyWindow::default()));
    to_stored(win) != held
}

impl NovoApp {
    /// Writes the calculator into the project when it was edited, but not
    /// while a drag or text edit is going on.
    pub(super) fn commit_safety_calculator(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.pointer.any_down()) || ctx.memory(|m| m.focused()).is_some() {
            return;
        }
        self.store_safety_calculator();
    }

    /// Writes the calculator now if it differs from the stored one.
    pub(super) fn store_safety_calculator(&mut self) {
        let Some(win) = self.wb.safety.as_ref() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return;
        };
        if !differs(win, &project) {
            return;
        }
        let value = encode(win);
        self.edit_project("Safety calculator", |tx| {
            novo_checks::standards::set_info_property(tx, CALCULATOR_KEY, value)
        });
    }
}
