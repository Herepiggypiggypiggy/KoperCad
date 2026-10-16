//! The wire production rules of a project: routing parameters, termination
//! allowances and the estimate constants, kept as project properties
//! (`wire.*` in the project information), so a file carries the rules its
//! lengths were made with and no model change is needed.
//!
//! A missing, non-numeric or out-of-range property reads as the default, so
//! a bad value never changes a length silently into nonsense; [`WireRules`]
//! says which keys it ignored in [`WireRules::ignored`].

use novo_core::{ModelError, Project, Result, Tx, Value};

use crate::Rules;

/// Service loop at each end of a wire, mm.
pub const SLACK_MM: &str = "wire.slack_mm";
/// Longest stub from a terminal to its duct that is preferred, mm.
pub const MAX_STUB_MM: &str = "wire.max_stub_mm";
/// Share of a duct's cross-section that wires may fill, 0.1 to 1.
pub const FILL_LIMIT: &str = "wire.fill_limit";
/// Extra length per 90 degree bend, mm.
pub const BEND_MM: &str = "wire.bend_mm";
/// Strip length plus ferrule collar per wire end (cross-sections below
/// [`LUG_FROM_MM2`]), mm.
pub const FERRULE_MM: &str = "wire.ferrule_mm";
/// Strip length plus lug barrel per wire end, mm.
pub const LUG_MM: &str = "wire.lug_mm";
/// Smallest cross-section that gets a cable lug, mm2.
pub const LUG_FROM_MM2: &str = "wire.lug_from_mm2";
/// Shortest estimated wire between two pins, before the slack, mm.
pub const ESTIMATE_MIN_MM: &str = "wire.estimate_min_mm";
/// Estimated wire between pins on different pages, before the slack, mm.
pub const ESTIMATE_ACROSS_PAGES_MM: &str = "wire.estimate_across_pages_mm";

/// One rule: key, label, unit, default and the range it accepts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuleInfo {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

const fn info(
    key: &'static str,
    label: &'static str,
    unit: &'static str,
    default: f64,
    min: f64,
    max: f64,
) -> RuleInfo {
    RuleInfo {
        key,
        label,
        unit,
        default,
        min,
        max,
    }
}

/// Every rule, in the order a settings panel lists them.
pub const RULES: [RuleInfo; 9] = [
    info(SLACK_MM, "Service loop per end", "mm", 50.0, 0.0, 500.0),
    info(
        MAX_STUB_MM,
        "Longest stub to a duct",
        "mm",
        150.0,
        1.0,
        2000.0,
    ),
    info(FILL_LIMIT, "Duct fill limit", "ratio", 0.5, 0.1, 1.0),
    info(BEND_MM, "Extra per bend", "mm", 0.0, 0.0, 100.0),
    info(FERRULE_MM, "Ferrule end allowance", "mm", 10.0, 0.0, 100.0),
    info(LUG_MM, "Lug end allowance", "mm", 20.0, 0.0, 200.0),
    info(LUG_FROM_MM2, "Lugs from", "mm2", 25.0, 0.1, 1000.0),
    info(
        ESTIMATE_MIN_MM,
        "Shortest estimate",
        "mm",
        100.0,
        0.0,
        5000.0,
    ),
    info(
        ESTIMATE_ACROSS_PAGES_MM,
        "Estimate across pages",
        "mm",
        500.0,
        0.0,
        20000.0,
    ),
];

/// The rule for `key`.
pub fn rule(key: &str) -> Option<&'static RuleInfo> {
    RULES.iter().find(|r| r.key == key)
}

/// All wire production rules of a project.
#[derive(Debug, Clone, PartialEq)]
pub struct WireRules {
    /// The routing parameters.
    pub routing: Rules,
    pub ferrule_mm: f64,
    pub lug_mm: f64,
    pub lug_from_mm2: f64,
    pub estimate_min_mm: f64,
    pub estimate_across_pages_mm: f64,
    /// Keys present in the project but not usable (wrong type or out of
    /// range); their defaults were used.
    pub ignored: Vec<&'static str>,
}

impl Default for WireRules {
    fn default() -> WireRules {
        WireRules {
            routing: Rules::default(),
            ferrule_mm: 10.0,
            lug_mm: 20.0,
            lug_from_mm2: 25.0,
            estimate_min_mm: 100.0,
            estimate_across_pages_mm: 500.0,
            ignored: Vec::new(),
        }
    }
}

fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Text(t) => t.trim().replace(',', ".").parse().ok(),
        Value::Bool(_) => None,
    }
}

impl WireRules {
    /// The rules of `project`; defaults for what it does not set.
    pub fn of(project: &Project) -> WireRules {
        let props = &project.info().properties;
        let mut ignored = Vec::new();
        let mut get = |r: &RuleInfo| -> f64 {
            match props.get(r.key) {
                None => r.default,
                Some(v) => match as_f64(v).filter(|n| n.is_finite() && (r.min..=r.max).contains(n))
                {
                    Some(n) => n,
                    None => {
                        ignored.push(r.key);
                        r.default
                    }
                },
            }
        };
        let [
            slack,
            stub,
            fill,
            bend,
            ferrule,
            lug,
            lug_from,
            est_min,
            est_pages,
        ] = RULES;
        let routing = Rules {
            slack_mm: get(&slack),
            max_stub_mm: get(&stub),
            fill_limit: get(&fill),
            bend_mm: get(&bend),
        };
        let (ferrule_mm, lug_mm, lug_from_mm2) = (get(&ferrule), get(&lug), get(&lug_from));
        let (estimate_min_mm, estimate_across_pages_mm) = (get(&est_min), get(&est_pages));
        WireRules {
            routing,
            ferrule_mm,
            lug_mm,
            lug_from_mm2,
            estimate_min_mm,
            estimate_across_pages_mm,
            ignored,
        }
    }

    /// The value of the rule `key` as these rules hold it.
    pub fn value(&self, key: &str) -> Option<f64> {
        Some(match key {
            SLACK_MM => self.routing.slack_mm,
            MAX_STUB_MM => self.routing.max_stub_mm,
            FILL_LIMIT => self.routing.fill_limit,
            BEND_MM => self.routing.bend_mm,
            FERRULE_MM => self.ferrule_mm,
            LUG_MM => self.lug_mm,
            LUG_FROM_MM2 => self.lug_from_mm2,
            ESTIMATE_MIN_MM => self.estimate_min_mm,
            ESTIMATE_ACROSS_PAGES_MM => self.estimate_across_pages_mm,
            _ => return None,
        })
    }

    /// What one wire end takes for its termination: a ferrule below
    /// `lug_from_mm2`, a cable lug from there on (the ferrule when the size
    /// is unknown).
    pub fn end_allowance_mm(&self, cross_section_mm2: Option<f64>) -> f64 {
        match cross_section_mm2 {
            Some(s) if s >= self.lug_from_mm2 => self.lug_mm,
            _ => self.ferrule_mm,
        }
    }
}

/// Sets rule `key` to `value`, or back to its default with `None` (the
/// property is removed, so a default project file stays clean). Errors on an
/// unknown key or a value outside the rule's range.
pub fn set_rule(tx: &mut Tx<'_>, key: &str, value: Option<f64>) -> Result<()> {
    let Some(r) = rule(key) else {
        return Err(ModelError::Refused {
            what: "Wire rule".into(),
            reason: format!("unknown rule {key}"),
        });
    };
    if let Some(v) = value
        && !(v.is_finite() && (r.min..=r.max).contains(&v))
    {
        return Err(ModelError::Refused {
            what: r.label.into(),
            reason: format!("must be between {} and {} {}", r.min, r.max, r.unit),
        });
    }
    tx.set_info(|info| match value {
        Some(v) if v != r.default => {
            info.properties.insert(key.to_owned(), Value::Float(v));
        }
        _ => {
            info.properties.remove(key);
        }
    })
}
