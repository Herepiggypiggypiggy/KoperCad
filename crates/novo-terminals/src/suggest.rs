//! Terminals where they belong: a wire that runs from a device in one
//! location (`+CAB`) to a device in another (`+FLD`) crosses a boundary,
//! and a boundary needs a terminal. These are offered as suggestions; the
//! user accepts them (all at once or one by one) instead of placing
//! terminals by hand.
//!
//! The first cut looks at wires that run straight from one device pin to
//! another. Wires through junctions are not suggested yet.

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, Endpoint, Point, Project, Result, SymbolRole, Tx, WireId,
};

use crate::place::{Drop, Placed, place_terminals};

/// One suggested terminal.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub wire: WireId,
    /// Where on the wire the terminal would go: the middle of its longest
    /// straight stretch.
    pub at: Point,
    /// Tag of the strip it would go on: the cabinet side's location plus
    /// `-X1`, or an existing strip there.
    pub strip: DeviceTag,
    /// The device on the cabinet (internal) side and on the field side.
    pub internal: DeviceId,
    pub external: DeviceId,
}

fn device_at(project: &Project, end: Endpoint) -> Option<DeviceId> {
    let pin = end.as_pin()?;
    match project.symbol(pin.symbol)?.role {
        SymbolRole::Device { device, .. } => Some(device),
        _ => None,
    }
}

fn structure(tag: &DeviceTag) -> DeviceTag {
    DeviceTag {
        function: tag.function.clone(),
        location: tag.location.clone(),
        product: Vec::new(),
    }
}

/// The strip for terminals at `location`: an existing strip with exactly
/// that function and location (lowest tag first), else `-X1` there.
fn strip_tag(project: &Project, location: &DeviceTag) -> DeviceTag {
    let mut strips: Vec<&DeviceTag> = project
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip && structure(&d.tag) == *location)
        .map(|d| &d.tag)
        .collect();
    strips.sort();
    match strips.first() {
        Some(tag) => (*tag).clone(),
        None => {
            let mut tag = location.clone();
            tag.product = vec!["X1".into()];
            tag
        }
    }
}

fn midpoint_of_longest_stretch(project: &Project, wire: WireId) -> Option<Point> {
    let w = project.wire(wire)?;
    let mut points = vec![project.endpoint_position(w.from)?];
    points.extend(&w.route);
    points.push(project.endpoint_position(w.to)?);
    points
        .windows(2)
        .max_by(|a, b| a[0].distance(a[1]).total_cmp(&b[0].distance(b[1])))
        .map(|s| Point::new((s[0].x + s[1].x) / 2.0, (s[0].y + s[1].y) / 2.0))
}

/// Wires that cross a location boundary with no terminal on them, in page
/// and wire order. The cabinet side is the location with more devices in
/// the project; a tie goes to the wire's start.
pub fn suggest_terminals(project: &Project) -> Vec<Suggestion> {
    let count_at = |location: &[String]| {
        project
            .devices()
            .filter(|d| d.tag.location == location)
            .count()
    };
    let pages: Vec<_> = project.pages_in_order().iter().map(|p| p.id).collect();
    let mut wires: Vec<_> = project.wires().collect();
    wires.sort_by_key(|w| (pages.iter().position(|&p| p == w.page), w.id));
    let mut out = Vec::new();
    for w in wires {
        let (Some(a), Some(b)) = (device_at(project, w.from), device_at(project, w.to)) else {
            continue;
        };
        let (Some(da), Some(db)) = (project.device(a), project.device(b)) else {
            continue;
        };
        if da.tag.location == db.tag.location {
            continue;
        }
        let (internal, external) = if count_at(&db.tag.location) > count_at(&da.tag.location) {
            (db, da)
        } else {
            (da, db)
        };
        let Some(at) = midpoint_of_longest_stretch(project, w.id) else {
            continue;
        };
        out.push(Suggestion {
            wire: w.id,
            at,
            strip: strip_tag(project, &structure(&internal.tag)),
            internal: internal.id,
            external: external.id,
        });
    }
    out
}

/// Accepts suggestions: puts the terminals on their strips (created when
/// missing), grouped per strip in suggestion order, numbered next free.
pub fn accept_suggestions(tx: &mut Tx<'_>, suggestions: &[Suggestion]) -> Result<Vec<Placed>> {
    let mut placed = Vec::with_capacity(suggestions.len());
    let mut tags: Vec<&DeviceTag> = Vec::new();
    for s in suggestions {
        if !tags.contains(&&s.strip) {
            tags.push(&s.strip);
        }
    }
    for tag in tags {
        let existing = tx
            .project()
            .devices()
            .filter(|d| d.kind == DeviceKind::TerminalStrip && d.tag == *tag)
            .map(|d| d.id)
            .min();
        let strip = match existing {
            Some(s) => s,
            None => tx.add_terminal_strip(tag.clone())?,
        };
        let drops: Vec<Drop> = suggestions
            .iter()
            .filter(|s| s.strip == *tag)
            .map(|s| Drop {
                wire: s.wire,
                at: s.at,
            })
            .collect();
        placed.extend(place_terminals(tx, strip, &drops)?);
    }
    Ok(placed)
}
