//! Checks a symbol against the rules in `FORMAT.md` that serde alone cannot
//! express: grid alignment, unique designations, leads touching connection
//! points and a known behavior vocabulary.

use std::collections::HashSet;
use std::fmt;

use crate::symbol::{FORMAT_VERSION, GRID, Graphic, Point, Symbol};

/// Behavior kinds the library uses. The simulation workstream owns their
/// meaning; this list only catches typos.
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
        let ok = match g {
            Graphic::Line { from, to, .. } => finite(*from) && finite(*to) && !same(*from, *to),
            Graphic::Polyline { points, .. } => {
                points.len() >= 2 && points.iter().all(|p| finite(*p))
            }
            Graphic::Rect { at, size, .. } => finite(*at) && size[0] > 0.0 && size[1] > 0.0,
            Graphic::Circle { center, r, .. } => finite(*center) && *r > 0.0,
            Graphic::Ellipse { center, rx, ry, .. } => finite(*center) && *rx > 0.0 && *ry > 0.0,
            Graphic::Arc {
                center,
                r,
                start,
                end,
                ..
            } => finite(*center) && *r > 0.0 && end > start && end - start <= 360.0,
            Graphic::Text { at, text, size, .. } => finite(*at) && !text.is_empty() && *size > 0.0,
        };
        if !ok {
            fail(format!("graphic #{} is degenerate or invalid", i + 1));
        }
    }

    let kind = symbol.behavior.kind.as_str();
    if !BEHAVIOR_KINDS.contains(&kind) {
        fail(format!("unknown behavior kind '{kind}'"));
    }
    if let Some(poles) = symbol.behavior.params.get("poles") {
        let refs: Vec<&str> = poles
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|p| p.as_array().into_iter().flatten())
            .filter_map(|v| v.as_str())
            .collect();
        if refs.is_empty() {
            fail("behavior.poles must be a list of lists of connection ids".into());
        }
        for r in refs {
            if !ids.contains(r) {
                fail(format!("behavior.poles names unknown connection '{r}'"));
            }
        }
    }

    issues
}
