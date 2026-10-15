//! What differs inside one changed object, as words: the field, its old
//! and its new value (DESIGN §7: the comparison reads like a change list,
//! not a diff of internals).

use std::collections::BTreeSet;

use novo_core::{AnyId, Project};
use serde_json::{Map, Value};

use crate::ChangeKind;
use crate::list::ChangeRow;

/// Fields that only record the issue or internal bookkeeping.
const HIDDEN: &[&str] = &["frozen"];

/// One field that differs between two states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldChange {
    /// The field, by its JSON name; `properties.voltage` for a property.
    pub name: String,
    /// The value before, `None` when the field was not set.
    pub old: Option<String>,
    /// The value after, `None` when the field is no longer set.
    pub new: Option<String>,
    /// A sentence for a change that reads better than two values, such as
    /// "moved 40 mm right, 10 mm down" (page y grows downward).
    pub note: Option<String>,
}

/// The fields that differ for a changed `row`, in name order. Empty for
/// an added or removed object, and for one that is on neither side.
pub fn field_changes(row: &ChangeRow, old: &Project, new: &Project) -> Vec<FieldChange> {
    let ChangeKind::Changed { fields } = &row.kind else {
        return Vec::new();
    };
    let (Some(a), Some(b)) = (object(old, row.id), object(new, row.id)) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in fields.iter().filter(|f| !HIDDEN.contains(&f.as_str())) {
        let (x, y) = (a.get(name), b.get(name));
        if name == "placement" {
            out.extend(placement_changes(x, y));
        } else {
            out.extend(value_changes(name, x, y));
        }
    }
    out
}

fn object(project: &Project, id: AnyId) -> Option<Map<String, Value>> {
    match serde_json::to_value(project.record(id)?).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// Differences of one field. Two objects are compared key by key so a
/// property change names the property; anything else is shown whole.
fn value_changes(name: &str, a: Option<&Value>, b: Option<&Value>) -> Vec<FieldChange> {
    if let (Some(Value::Object(x)), Some(Value::Object(y))) = (a, b) {
        let keys: BTreeSet<&String> = x.keys().chain(y.keys()).collect();
        return keys
            .into_iter()
            .filter(|k| x.get(*k) != y.get(*k))
            .flat_map(|k| value_changes(&format!("{name}.{k}"), x.get(k), y.get(k)))
            .collect();
    }
    let (old, new) = (show(a), show(b));
    if old == new {
        return Vec::new();
    }
    vec![FieldChange {
        name: name.to_owned(),
        old,
        new,
        note: None,
    }]
}

/// A moved, turned or mirrored symbol, said as such.
fn placement_changes(a: Option<&Value>, b: Option<&Value>) -> Vec<FieldChange> {
    let (Some(Value::Object(x)), Some(Value::Object(y))) = (a, b) else {
        return value_changes("placement", a, b);
    };
    let mut out = Vec::new();
    let origin = |m: &Map<String, Value>| {
        let o = m.get("origin")?;
        Some((o.get("x")?.as_f64()?, o.get("y")?.as_f64()?))
    };
    if let (Some((x0, y0)), Some((x1, y1))) = (origin(x), origin(y))
        && (x0, y0) != (x1, y1)
    {
        out.push(FieldChange {
            name: "position".into(),
            old: Some(format!("{}, {} mm", num(x0), num(y0))),
            new: Some(format!("{}, {} mm", num(x1), num(y1))),
            note: Some(moved(x1 - x0, y1 - y0)),
        });
    }
    for key in ["rotation", "mirrored"] {
        out.extend(value_changes(key, x.get(key), y.get(key)));
    }
    out
}

/// "moved 40 mm right, 10 mm down". Page y grows downward, so a positive
/// y change is down.
fn moved(dx: f64, dy: f64) -> String {
    let mut parts = Vec::new();
    for (d, plus, minus) in [(dx, "right", "left"), (dy, "down", "up")] {
        if d.abs() >= 0.005 {
            parts.push(format!(
                "{} mm {}",
                num(d.abs()),
                if d > 0.0 { plus } else { minus }
            ));
        }
    }
    format!("moved {}", parts.join(", "))
}

/// A number with at most two decimals and no trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

fn show(v: Option<&Value>) -> Option<String> {
    let text = match v? {
        Value::Null => return None,
        Value::Bool(b) => if *b { "yes" } else { "no" }.to_owned(),
        Value::Number(n) => n.as_f64().map_or_else(|| n.to_string(), num),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    const LIMIT: usize = 80;
    Some(if text.chars().count() > LIMIT {
        let cut: String = text.chars().take(LIMIT).collect();
        format!("{cut}…")
    } else {
        text
    })
}

/// The project information fields in `names` (from [`crate::ChangeSet`]'s
/// `info`) with their old and new values.
pub(crate) fn info_changes(old: &Project, new: &Project, names: &[String]) -> Vec<FieldChange> {
    let (Ok(Value::Object(a)), Ok(Value::Object(b))) = (
        serde_json::to_value(old.info()),
        serde_json::to_value(new.info()),
    ) else {
        return Vec::new();
    };
    names
        .iter()
        .flat_map(|n| value_changes(n, a.get(n), b.get(n)))
        .collect()
}
