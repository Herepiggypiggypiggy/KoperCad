//! Where the numbers behind the rules come from, and how far to trust them.
//!
//! Most rules are structural (is there a disconnecting device, is the wire
//! green-yellow) and need no number. A few compare against a limit or a
//! table value that was entered without access to the licensed text of the
//! standard. Those are listed here, each with the rules that use it, so the
//! compliance document can mark a pass that rests on one as resting on a
//! *planning value* instead of silently passing.
//!
//! Nothing in this register is "verified": verifying means reading the
//! licensed clause. The engineer does that, and records the outcome by
//! overriding the value in the project (`standards.<set>.<name>`), which the
//! document then shows as a project value.

use super::StandardSet;

/// What kind of basis a value has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BasisKind {
    /// A limit or table value entered from secondary knowledge, not checked
    /// against the licensed standard.
    Planning,
    /// A simplified calculation method, a model of what the standard asks.
    Model,
}

impl BasisKind {
    pub fn label(self) -> &'static str {
        match self {
            BasisKind::Planning => "Planning value",
            BasisKind::Model => "Simplified model",
        }
    }
}

/// One number, table or method that rules depend on.
#[derive(Debug, PartialEq, Eq)]
pub struct BasisItem {
    pub id: &'static str,
    pub kind: BasisKind,
    pub set: StandardSet,
    /// What the value is, in our words.
    pub what: &'static str,
    /// What was done to check it, and what was not.
    pub checked: &'static str,
    /// Rule ids that compare against it.
    pub used_by: &'static [&'static str],
}

const NOT_LICENSED: &str = "Entered without the licensed text; compared by hand with independent recollection of the usual published values only. Not checked against the standard.";

/// Every planning value and model the rules use.
pub static BASIS: &[BasisItem] = &[
    BasisItem {
        id: "iec.pe-steps",
        kind: BasisKind::Planning,
        set: StandardSet::Iec60204,
        what: "Protective conductor size steps for copper: equal up to 16 mm2, then 16 mm2, then half above 35 mm2.",
        checked: NOT_LICENSED,
        used_by: &["iec60204.pe-size"],
    },
    BasisItem {
        id: "iec.colours",
        kind: BasisKind::Planning,
        set: StandardSet::Iec60204,
        what: "Conductor colours by circuit type (power, ac control, dc control, live with supply off).",
        checked: NOT_LICENSED,
        used_by: &["iec60204.conductor-colour"],
    },
    BasisItem {
        id: "iec.control-voltage",
        kind: BasisKind::Planning,
        set: StandardSet::Iec60204,
        what: "Control circuit voltage limits: 277 V ac and 250 V dc.",
        checked: NOT_LICENSED,
        used_by: &["iec60204.control-voltage"],
    },
    BasisItem {
        id: "iec.ampacity-tables",
        kind: BasisKind::Planning,
        set: StandardSet::Iec60204,
        what: "Conductor ampacities of copper cables by installation method (B1, B2, C), insulation (PVC, XLPE) and loaded conductors, with the ambient temperature and grouping correction factors, in the layout of IEC 60364-5-52. Methods and combinations without a value are reported as not checkable, never guessed.",
        checked: "Entered without the licensed text; the cells agree with independent recollection of the usual published values, with no discrepancy seen. Not checked against the standard. Load your licensed copy through the CSV table loader to replace them.",
        used_by: &["iec60204.conductor-ampacity"],
    },
    BasisItem {
        id: "nfpa.ground-steps",
        kind: BasisKind::Planning,
        set: StandardSet::Nfpa79,
        what: "Smallest equipment grounding conductor per overcurrent device rating (copper steps, 14 AWG up to 4/0 AWG).",
        checked: NOT_LICENSED,
        used_by: &["nfpa79.ground-size", "ul508a.ground-size"],
    },
    BasisItem {
        id: "nfpa.colours",
        kind: BasisKind::Planning,
        set: StandardSet::Nfpa79,
        what: "Conductor colours by circuit type.",
        checked: NOT_LICENSED,
        used_by: &["nfpa79.conductor-colour"],
    },
    BasisItem {
        id: "nfpa.control-voltage",
        kind: BasisKind::Planning,
        set: StandardSet::Nfpa79,
        what: "Control circuit voltage limit: 120 V ac.",
        checked: NOT_LICENSED,
        used_by: &["nfpa79.control-voltage"],
    },
    BasisItem {
        id: "awg.ampacity",
        kind: BasisKind::Planning,
        set: StandardSet::Nfpa79,
        what: "Copper conductor ampacities in AWG at 60 and 75 degrees C terminations (18 to 4/0 AWG), and the metric to AWG mapping (a wire counts as the largest AWG size it covers).",
        checked: NOT_LICENSED,
        used_by: &["nfpa79.conductor-ampacity", "ul508a.conductor-ampacity"],
    },
    BasisItem {
        id: "motor.branch-percent",
        kind: BasisKind::Planning,
        set: StandardSet::Nfpa79,
        what: "Motor branch protection limits as a percentage of full-load current (300 % fuse, 175 % time-delay fuse, 250 % inverse-time breaker, 800 % instantaneous).",
        checked: NOT_LICENSED,
        used_by: &["nfpa79.motor-branch-size", "ul508a.motor-branch-size"],
    },
    BasisItem {
        id: "motor.overload-percent",
        kind: BasisKind::Planning,
        set: StandardSet::Ul508a,
        what: "Motor overload setting limits as a percentage of the nameplate current.",
        checked: NOT_LICENSED,
        used_by: &["ul508a.overload-setting"],
    },
    BasisItem {
        id: "sccr.method",
        kind: BasisKind::Model,
        set: StandardSet::Ul508a,
        what: "Short-circuit current rating: lowest component rating, raised behind a current-limiting device within that component's rating. Combination ratings and transformer secondaries are not modelled. Unmarked-component defaults are not built in.",
        checked: "A simplified model of the Supplement SB method, tested against hand-worked cases. Not a substitute for the full method.",
        used_by: &[
            "nfpa79.sccr",
            "ul508a.sccr-components",
            "ul508a.sccr-adequate",
            "ul508a.sccr-marking",
        ],
    },
];

/// Items a rule rests on.
pub fn basis_of(rule_id: &str) -> Vec<&'static BasisItem> {
    BASIS
        .iter()
        .filter(|b| b.used_by.contains(&rule_id))
        .collect()
}

/// The project's overrides for a set: keys `standards.<set>.<name>`, which
/// replace a built-in limit with a value the engineer entered.
pub fn overrides(project: &novo_core::Project, set: StandardSet) -> Vec<String> {
    let prefix = format!("{}{}.", super::keys::OVERRIDE_PREFIX, set.id());
    let mut keys: Vec<String> = project
        .info()
        .properties
        .keys()
        .filter(|k| k.starts_with(&prefix))
        .cloned()
        .collect();
    keys.sort();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standards::RULES;

    #[test]
    fn every_basis_rule_exists() {
        for b in BASIS {
            for id in b.used_by {
                assert!(RULES.iter().any(|r| r.id == *id), "{id} in {}", b.id);
            }
        }
    }

    #[test]
    fn every_rule_that_asks_for_confirmation_has_a_basis() {
        for r in RULES.iter().filter(|r| r.verify.is_some()) {
            assert!(!basis_of(r.id).is_empty(), "{} has no basis entry", r.id);
        }
    }
}
