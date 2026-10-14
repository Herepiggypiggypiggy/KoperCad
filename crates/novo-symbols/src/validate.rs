//! Checks a symbol against the rules in `FORMAT.md` that serde alone cannot
//! express: grid alignment, unique designations, leads touching connection
//! points and a known behavior vocabulary.

use std::collections::HashSet;
use std::fmt;

use crate::symbol::{FORMAT_VERSION, GRID, Graphic, Point, Symbol};

/// Behavior kinds the library accepts. The simulation workstream owns
/// their meaning; see [`behavior_keys`] for each kind's parameters.
pub const BEHAVIOR_KINDS: &[&str] = &[
    "none",
    "conductor",
    "contact",
    "coil",
    "fuse",
    "breaker",
    "overload",
    "motor",
    "lamp",
    "signal",
    "source",
    "load",
    "passive",
    "transformer",
    "converter",
    "meter",
    "plc_input",
    "plc_output",
];

/// Anchor roles the library uses.
pub const ANCHOR_ROLES: &[&str] = &["tag", "function_text", "address", "part"];

/// One problem found in a symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct Issue {
    pub symbol: String,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.symbol, self.message)
    }
}

const EPS: f64 = 1e-6;

fn on_grid(v: f64) -> bool {
    let k = v / GRID;
    (k - k.round()).abs() < EPS
}

fn same(a: Point, b: Point) -> bool {
    (a[0] - b[0]).abs() < EPS && (a[1] - b[1]).abs() < EPS
}

fn finite(p: Point) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

/// Whether some stroke ends or bends exactly at `p`, so a wire attached
/// there visibly meets the artwork.
fn touches(graphics: &[Graphic], p: Point) -> bool {
    graphics.iter().any(|g| match g {
        Graphic::Line { from, to, .. } => same(*from, p) || same(*to, p),
        Graphic::Polyline { points, .. } => points.iter().any(|q| same(*q, p)),
        _ => false,
    })
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.split('/').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        })
}

/// Validates one symbol and returns every issue found.
pub fn validate(symbol: &Symbol) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut fail = |message: String| {
        issues.push(Issue {
            symbol: symbol.id.clone(),
            message,
        })
    };

    if symbol.format != FORMAT_VERSION {
        fail(format!(
            "format {} is not supported (expected {FORMAT_VERSION})",
            symbol.format
        ));
    }
    if !valid_id(&symbol.id) {
        fail("id must be lowercase path segments of a-z, 0-9, '_' or '-'".into());
    }
    let parent = symbol.id.rsplit('/').nth(1);
    if parent != Some(symbol.category.as_str()) {
        fail(format!(
            "category '{}' must equal the directory the symbol is in",
            symbol.category
        ));
    }
    if symbol.name.trim().is_empty() {
        fail("name is empty".into());
    }
    if symbol.function.trim().is_empty() {
        fail("function is empty".into());
    }
    if symbol.graphics.is_empty() {
        fail("symbol has no graphics".into());
    }

    let mut ids = HashSet::new();
    for c in &symbol.connections {
        if !ids.insert(c.id.as_str()) {
            fail(format!("connection '{}' is defined twice", c.id));
        }
        if !finite(c.at) || !on_grid(c.at[0]) || !on_grid(c.at[1]) {
            fail(format!(
                "connection '{}' at {:?} is not on the {GRID} mm grid",
                c.id, c.at
            ));
        }
        if !touches(&symbol.graphics, c.at) {
            fail(format!(
                "connection '{}' at {:?} is not the end of a line or polyline",
                c.id, c.at
            ));
        }
    }
    let mut seen_at: Vec<Point> = Vec::new();
    for c in &symbol.connections {
        if seen_at.iter().any(|p| same(*p, c.at)) {
            fail(format!("connection '{}' shares its position", c.id));
        }
        seen_at.push(c.at);
    }

    if symbol.anchor("tag").is_none() && symbol.behavior.kind != "none" {
        fail("symbol has no 'tag' anchor".into());
    }
    for a in &symbol.anchors {
        if !ANCHOR_ROLES.contains(&a.role.as_str()) {
            fail(format!("unknown anchor role '{}'", a.role));
        }
        if !finite(a.at) {
            fail(format!("anchor '{}' is not finite", a.role));
        }
    }

    for (i, g) in symbol.graphics.iter().enumerate() {
        let pos = |v: f64| v.is_finite() && v > 0.0;
        let ok = match g {
            Graphic::Line { from, to, .. } => finite(*from) && finite(*to) && !same(*from, *to),
            Graphic::Polyline { points, .. } => {
                points.len() >= 2 && points.iter().all(|p| finite(*p))
            }
            Graphic::Rect { at, size, .. } => finite(*at) && pos(size[0]) && pos(size[1]),
            Graphic::Circle { center, r, .. } => finite(*center) && pos(*r),
            Graphic::Ellipse { center, rx, ry, .. } => finite(*center) && pos(*rx) && pos(*ry),
            Graphic::Arc {
                center,
                r,
                start,
                end,
                ..
            } => {
                finite(*center)
                    && pos(*r)
                    && start.is_finite()
                    && end.is_finite()
                    && end > start
                    && end - start <= 360.0
            }
            Graphic::Text { at, text, size, .. } => finite(*at) && !text.is_empty() && pos(*size),
        };
        if !ok {
            fail(format!("graphic #{} is degenerate or invalid", i + 1));
        }
    }

    let mut bridged = HashSet::new();
    for group in &symbol.bridges {
        if group.len() < 2 {
            fail("each bridges group needs at least two connections".into());
        }
        for id in group {
            if !ids.contains(id.as_str()) {
                fail(format!("bridges names unknown connection '{id}'"));
            }
            if !bridged.insert(id.as_str()) {
                fail(format!(
                    "connection '{id}' is in more than one bridges group"
                ));
            }
        }
    }

    for message in check_behavior(symbol, &ids) {
        fail(message);
    }

    issues
}

/// Parameter keys each behavior kind accepts. The simulation workstream
/// owns their meaning; this table catches typos and stray keys.
const BEHAVIOR_SCHEMA: &[(&str, &[&str])] = &[
    ("none", &[]),
    ("conductor", &["poles"]),
    ("contact", &["poles", "normally", "latching", "delay"]),
    ("coil", &["poles", "delay"]),
    ("fuse", &["poles", "switched"]),
    ("breaker", &["poles", "trip", "adjustable"]),
    ("overload", &["poles"]),
    ("motor", &["ports", "phases", "speeds"]),
    ("lamp", &["poles"]),
    ("signal", &["poles"]),
    ("source", &["poles", "ports", "type", "phases"]),
    ("load", &["poles", "type"]),
    ("passive", &["poles", "type", "wiper"]),
    ("transformer", &["ports"]),
    ("converter", &["ports", "input", "output"]),
    ("meter", &["poles", "quantity"]),
    ("plc_input", &["signal"]),
    ("plc_output", &["signal"]),
];

/// Keys that must be present for a kind.
const BEHAVIOR_REQUIRED: &[(&str, &[&str])] = &[
    ("conductor", &["poles"]),
    ("contact", &["poles", "normally"]),
    ("coil", &["poles"]),
    ("fuse", &["poles"]),
    ("breaker", &["poles", "trip"]),
    ("overload", &["poles"]),
    ("motor", &["ports", "phases"]),
    ("transformer", &["ports"]),
    ("converter", &["ports", "input", "output"]),
    ("plc_input", &["signal"]),
    ("plc_output", &["signal"]),
];

/// The parameter keys a behavior kind accepts, or `None` for an unknown
/// kind.
pub fn behavior_keys(kind: &str) -> Option<&'static [&'static str]> {
    BEHAVIOR_SCHEMA
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, keys)| *keys)
}

fn check_behavior(symbol: &Symbol, ids: &HashSet<&str>) -> Vec<String> {
    let mut out = Vec::new();
    let behavior = &symbol.behavior;
    let kind = behavior.kind.as_str();
    let Some(allowed) = behavior_keys(kind) else {
        out.push(format!("unknown behavior kind '{kind}'"));
        return out;
    };
    for key in behavior.params.keys() {
        if !allowed.contains(&key.as_str()) {
            out.push(format!("behavior kind '{kind}' has no parameter '{key}'"));
        }
    }
    let required = BEHAVIOR_REQUIRED
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or(&[][..], |(_, keys)| *keys);
    for key in required {
        if !behavior.params.contains_key(*key) {
            out.push(format!("behavior kind '{kind}' needs '{key}'"));
        }
    }

    let params = &behavior.params;
    let string_in = |key: &str, options: &[&str], out: &mut Vec<String>| {
        if let Some(v) = params.get(key) {
            match v.as_str() {
                Some(s) if options.contains(&s) => {}
                _ => out.push(format!(
                    "behavior.{key} must be one of {}",
                    options.join(", ")
                )),
            }
        }
    };
    string_in("normally", &["open", "closed", "changeover"], &mut out);
    string_in("delay", &["on", "off"], &mut out);
    string_in("input", &["ac", "dc"], &mut out);
    string_in("output", &["ac", "dc"], &mut out);
    string_in("signal", &["digital", "analog"], &mut out);
    for key in ["latching", "adjustable", "switched"] {
        if params.get(key).is_some_and(|v| v.as_bool().is_none()) {
            out.push(format!("behavior.{key} must be true or false"));
        }
    }
    for key in ["type", "quantity"] {
        if params.get(key).is_some_and(|v| v.as_str().is_none()) {
            out.push(format!("behavior.{key} must be a string"));
        }
    }
    if let Some(v) = params.get("phases")
        && !matches!(v.as_integer(), Some(0 | 1 | 3))
    {
        out.push("behavior.phases must be 0 (DC), 1 or 3".into());
    }
    if let Some(v) = params.get("speeds")
        && !v.as_integer().is_some_and(|n| n >= 1)
    {
        out.push("behavior.speeds must be a positive integer".into());
    }
    if let Some(v) = params.get("trip") {
        let ok = v.as_array().is_some_and(|a| {
            !a.is_empty()
                && a.iter().all(|t| {
                    t.as_str()
                        .is_some_and(|t| ["thermal", "magnetic", "residual"].contains(&t))
                })
        });
        if !ok {
            out.push("behavior.trip must list thermal, magnetic and/or residual".into());
        }
    }
    if let Some(v) = params.get("wiper")
        && !v.as_str().is_some_and(|w| ids.contains(w))
    {
        out.push("behavior.wiper must name a connection".into());
    }

    // A changeover lists [common, break, make]; every other path joins two
    // connections.
    let pole_len = if params.get("normally").and_then(|v| v.as_str()) == Some("changeover") {
        3
    } else {
        2
    };
    check_groups(params.get("poles"), "poles", Some(pole_len), ids, &mut out);
    check_groups(params.get("ports"), "ports", None, ids, &mut out);
    out
}

/// Checks a list of connection-id groups: well formed, every id known, no
/// id in two groups, and (when given) the exact group size.
fn check_groups(
    value: Option<&toml::Value>,
    key: &str,
    len: Option<usize>,
    ids: &HashSet<&str>,
    out: &mut Vec<String>,
) {
    let Some(value) = value else { return };
    let malformed = || format!("behavior.{key} must be a list of lists of connection ids");
    let Some(groups) = value.as_array().filter(|g| !g.is_empty()) else {
        out.push(malformed());
        return;
    };
    let mut seen = HashSet::new();
    for group in groups {
        let Some(items) = group.as_array() else {
            out.push(malformed());
            continue;
        };
        let names: Vec<_> = items.iter().filter_map(|v| v.as_str()).collect();
        if names.len() != items.len() || names.is_empty() {
            out.push(malformed());
            continue;
        }
        if let Some(len) = len
            && names.len() != len
        {
            out.push(format!(
                "behavior.{key} group {names:?} must have {len} connections"
            ));
        }
        for name in names {
            if !ids.contains(name) {
                out.push(format!("behavior.{key} names unknown connection '{name}'"));
            }
            if !seen.insert(name) {
                out.push(format!(
                    "behavior.{key} uses connection '{name}' more than once"
                ));
            }
        }
    }
}
