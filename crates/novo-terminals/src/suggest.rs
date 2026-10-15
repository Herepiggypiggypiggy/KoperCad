//! Terminals where they belong: a wire that runs from a device in one
//! location (`+CAB`) to a device in another (`+FLD`) crosses a boundary,
//! and a boundary needs a terminal. These are offered as suggestions; the
//! user accepts them (all at once or one by one) instead of placing
//! terminals by hand.
//!
//! Each side is an enclosure when the location is in a structure marked as
//! one (`Project::enclosure_of`, so a door inside the cabinet is the
//! cabinet), else its location; a wire within one enclosure needs no
//! terminal. The cabinet side is the enclosure; when both or neither are,
//! the side that already holds a terminal strip, then the one with more
//! devices. A declined
//! suggestion is remembered as a core `Dismissal` record about its wire
//! (proposal `terminal.suggest`, see [`dismiss_suggestion`]) so it does not
//! come back after every edit.
//!
//! Wires are followed through junctions. The terminal goes on the one
//! wire where the connection leaves its location: a wire with a device pin
//! of one location at one end, and only devices of one other location
//! beyond the other end. A branch that already reaches a terminal, or
//! reaches both locations, gets no suggestion.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, Endpoint, Point, Project, Result, SymbolRole, Tx, Value,
    WireId,
};

/// The proposal name of a declined terminal suggestion in a core
/// `Dismissal` record. Older files kept it as a wire property of the same
/// name set to `false`, which still counts.
pub const DISMISS_KEY: &str = "terminal.suggest";

fn is_dismissed(project: &Project, wire: &novo_core::Wire) -> bool {
    project.is_dismissed(DISMISS_KEY, &[wire.id.into()])
        || matches!(wire.properties.get(DISMISS_KEY), Some(Value::Bool(false)))
}

use crate::place::{Drop, Placed, place_terminals};
use crate::strings;

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

/// Junctions joined by wires, one group per connected set, with the device
/// pins wired straight to each group. Built once, so every wire is judged
/// without walking its net again.
struct Groups {
    parent: BTreeMap<Endpoint, Endpoint>,
    /// Per group root: the (wire, device) of each pin wired to it, and how
    /// many pins on it are not a device's (a terminal is already there).
    leaves: BTreeMap<Endpoint, (Vec<(WireId, DeviceId)>, usize)>,
}

impl Groups {
    fn new(project: &Project) -> Self {
        let mut groups = Groups {
            parent: BTreeMap::new(),
            leaves: BTreeMap::new(),
        };
        for w in project.wires() {
            if w.from.as_pin().is_none() && w.to.as_pin().is_none() {
                let (a, b) = (groups.root(w.from), groups.root(w.to));
                if a != b {
                    groups.parent.insert(a, b);
                }
            }
        }
        for w in project.wires() {
            let (pin, junction) = match (w.from.as_pin(), w.to.as_pin()) {
                (Some(_), None) => (w.from, w.to),
                (None, Some(_)) => (w.to, w.from),
                _ => continue,
            };
            let root = groups.root(junction);
            let entry = groups.leaves.entry(root).or_default();
            match device_at(project, pin) {
                Some(device) => entry.0.push((w.id, device)),
                None => entry.1 += 1,
            }
        }
        for (leaves, _) in groups.leaves.values_mut() {
            leaves.sort_by_key(|&(wire, device)| (device, wire));
        }
        groups
    }

    fn root(&mut self, mut at: Endpoint) -> Endpoint {
        while let Some(&up) = self.parent.get(&at) {
            if let Some(&top) = self.parent.get(&up) {
                self.parent.insert(at, top);
            }
            at = up;
        }
        at
    }

    /// The devices beyond the junction end of `wire`: every device pin of
    /// its group except the one `wire` brings. `None` when the group also
    /// holds a terminal or nothing else.
    fn beyond(&mut self, wire: WireId, junction: Endpoint) -> Option<Vec<DeviceId>> {
        let root = self.root(junction);
        let (leaves, others) = self.leaves.get(&root)?;
        if *others > 0 {
            return None;
        }
        let mut skipped = false;
        let devices: Vec<DeviceId> = leaves
            .iter()
            .filter(|&&(w, _)| {
                let skip = !skipped && w == wire;
                skipped |= skip;
                !skip
            })
            .map(|&(_, d)| d)
            .collect();
        (!devices.is_empty()).then_some(devices)
    }
}

/// Which side of a boundary a device is on: its enclosure (a structure
/// marked as one, so a door inside the cabinet counts as the cabinet), else
/// its location.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Place<'a> {
    enclosure: bool,
    location: &'a [String],
}

fn place_of<'a>(project: &'a Project, device: &'a novo_core::Device) -> Place<'a> {
    match project.enclosure_of(&device.tag) {
        Some(s) => Place {
            enclosure: true,
            location: &s.tag.location,
        },
        None => Place {
            enclosure: false,
            location: &device.tag.location,
        },
    }
}

/// The one place all of `devices` share, with the first of them.
fn single_place<'a>(
    project: &'a Project,
    devices: &[DeviceId],
) -> Option<(&'a novo_core::Device, Place<'a>)> {
    let first = project.device(*devices.first()?)?;
    let place = place_of(project, first);
    devices
        .iter()
        .all(|&d| {
            project
                .device(d)
                .is_some_and(|d| place_of(project, d) == place)
        })
        .then_some((first, place))
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
    strips.sort_by(|a, b| novo_core::natural_cmp(&a.to_string(), &b.to_string()).then(a.cmp(b)));
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

/// Wires that cross a location boundary with no terminal on them and no
/// dismissal, in page and wire order. The cabinet side is the location that
/// holds a terminal strip, else the one with more devices; a tie goes to the
/// wire's start.
///
/// Wires are followed through junctions: a wire from a device pin of one
/// location to a junction qualifies when every other device pin joined
/// there is in one other location. One connection gets one suggestion, even
/// when several of its wires qualify (a junction with just two wires): the
/// first in drawing order that starts at a cabinet-side pin, else the
/// first. Declining that one declines the connection.
pub fn suggest_terminals(project: &Project) -> Vec<Suggestion> {
    let mut strips: BTreeSet<&[String]> = BTreeSet::new();
    let mut counts: BTreeMap<&[String], usize> = BTreeMap::new();
    // Counted by place, so devices on a cabinet's door count for it.
    for d in project.devices() {
        let place = place_of(project, d);
        if d.kind == DeviceKind::TerminalStrip {
            strips.insert(place.location);
        }
        *counts.entry(place.location).or_default() += 1;
    }
    let rank = |place: Place| {
        (
            place.enclosure,
            strips.contains(place.location),
            counts.get(place.location).copied().unwrap_or(0),
        )
    };
    let mut groups = Groups::new(project);
    let pages: Vec<_> = project.pages_in_order().iter().map(|p| p.id).collect();
    let mut wires: Vec<_> = project.wires().collect();
    wires.sort_by_key(|w| (pages.iter().position(|&p| p == w.page), w.id));
    // (connection, not at a cabinet pin, dismissed, suggestion)
    let mut found: Vec<(Vec<Endpoint>, bool, bool, Suggestion)> = Vec::new();
    for w in wires {
        let (a, b, connection) = match (w.from.as_pin(), w.to.as_pin()) {
            (Some(_), Some(_)) => {
                let (Some(a), Some(b)) = (device_at(project, w.from), device_at(project, w.to))
                else {
                    continue;
                };
                let mut pins = vec![w.from, w.to];
                pins.sort();
                (vec![a], vec![b], pins)
            }
            (Some(_), None) => {
                let (Some(a), Some(b)) = (device_at(project, w.from), groups.beyond(w.id, w.to))
                else {
                    continue;
                };
                (vec![a], b, vec![groups.root(w.to)])
            }
            (None, Some(_)) => {
                let (Some(a), Some(b)) = (groups.beyond(w.id, w.from), device_at(project, w.to))
                else {
                    continue;
                };
                (a, vec![b], vec![groups.root(w.from)])
            }
            (None, None) => continue,
        };
        let (Some((da, pa)), Some((db, pb))) =
            (single_place(project, &a), single_place(project, &b))
        else {
            continue;
        };
        if pa == pb {
            continue;
        }
        let dismissed = is_dismissed(project, w);
        let (internal, external, internal_end, place) = if rank(pb) > rank(pa) {
            (db, da, w.to, pb)
        } else {
            (da, db, w.from, pa)
        };
        let Some(at) = midpoint_of_longest_stretch(project, w.id) else {
            continue;
        };
        found.push((
            connection,
            internal_end.as_pin().is_none(),
            dismissed,
            Suggestion {
                wire: w.id,
                at,
                strip: strip_tag(
                    project,
                    &DeviceTag {
                        function: internal.tag.function.clone(),
                        location: place.location.to_vec(),
                        product: Vec::new(),
                    },
                ),
                internal: internal.id,
                external: external.id,
            },
        ));
    }
    // A decline on any of a connection's wires declines the connection, so
    // it holds when an edit changes which wire is chosen.
    let declined: BTreeSet<&[Endpoint]> = found
        .iter()
        .filter(|(_, _, dismissed, _)| *dismissed)
        .map(|(connection, ..)| connection.as_slice())
        .collect();
    let mut best: BTreeMap<&[Endpoint], (bool, usize)> = BTreeMap::new();
    for (i, (connection, rank, ..)) in found.iter().enumerate() {
        let entry = best.entry(connection).or_insert((*rank, i));
        if entry.0 && !*rank {
            *entry = (*rank, i);
        }
    }
    let keep: BTreeSet<usize> = best
        .into_iter()
        .filter(|(connection, _)| !declined.contains(connection))
        .map(|(_, (_, i))| i)
        .collect();
    found
        .iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, (.., s))| s.clone())
        .collect()
}

/// Page index, on a horizontal wire, then the main and the cross axis.
type SortKey = (Option<usize>, bool, f64, f64);

/// Whether the stretch of `wire` nearest `at` runs horizontally.
fn horizontal_at(project: &Project, wire: WireId, at: Point) -> bool {
    let Some(w) = project.wire(wire) else {
        return false;
    };
    let (Some(from), Some(to)) = (
        project.endpoint_position(w.from),
        project.endpoint_position(w.to),
    ) else {
        return false;
    };
    let mut points = vec![from];
    points.extend(&w.route);
    points.push(to);
    let distance = |a: Point, b: Point| {
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len2 = dx * dx + dy * dy;
        let t = if len2 == 0.0 {
            0.0
        } else {
            (((at.x - a.x) * dx + (at.y - a.y) * dy) / len2).clamp(0.0, 1.0)
        };
        Point::new(a.x + t * dx, a.y + t * dy).distance(at)
    };
    points
        .windows(2)
        .min_by(|s, t| distance(s[0], s[1]).total_cmp(&distance(t[0], t[1])))
        .is_some_and(|s| (s[1].x - s[0].x).abs() > (s[1].y - s[0].y).abs())
}

/// Accepts suggestions: puts the terminals on their strips (created when
/// missing), numbered next free in drawing order on each strip, page by
/// page: terminals on vertical wires stand in a row and are numbered left
/// to right, terminals on horizontal wires stand in a column and are
/// numbered top to bottom, the way a drag across the wires numbers them.
/// The order the suggestions were listed or picked in does not matter, and
/// neither do the wires' lengths.
pub fn accept_suggestions(tx: &mut Tx<'_>, suggestions: &[Suggestion]) -> Result<Vec<Placed>> {
    let project = tx.project();
    let pages: Vec<_> = project.pages_in_order().iter().map(|p| p.id).collect();
    let key = |s: &Suggestion| {
        let page = project
            .wire(s.wire)
            .and_then(|w| pages.iter().position(|&p| p == w.page));
        if horizontal_at(project, s.wire, s.at) {
            (page, true, s.at.y, s.at.x)
        } else {
            (page, false, s.at.x, s.at.y)
        }
    };
    let mut ordered: Vec<(SortKey, &Suggestion)> =
        suggestions.iter().map(|s| (key(s), s)).collect();
    ordered.sort_by(|((pa, ha, a1, a2), sa), ((pb, hb, b1, b2), sb)| {
        pa.cmp(pb)
            .then(ha.cmp(hb))
            .then(a1.total_cmp(b1))
            .then(a2.total_cmp(b2))
            .then(sa.wire.cmp(&sb.wire))
    });
    let suggestions: Vec<&Suggestion> = ordered.into_iter().map(|(_, s)| s).collect();
    let mut placed = Vec::with_capacity(suggestions.len());
    let mut tags: Vec<&DeviceTag> = Vec::new();
    for s in &suggestions {
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

/// Declines the suggestion on a wire for good (undoable); clearing it with
/// [`restore_suggestion`] brings the suggestion back.
pub fn dismiss_suggestion(tx: &mut Tx<'_>, wire: WireId) -> Result<()> {
    tx.dismiss(DISMISS_KEY, vec![wire.into()]).map(|_| ())
}

/// Moves declines kept by older files as the wire property
/// [`DISMISS_KEY`]` = false` onto `Dismissal` records and removes the
/// property. Returns whether anything changed; one undo step. Until it runs,
/// the property is still read as a decline.
pub fn migrate_legacy_dismissals(tx: &mut Tx<'_>) -> Result<bool> {
    let wires: Vec<(WireId, bool)> = tx
        .project()
        .wires()
        .filter_map(|w| {
            w.properties
                .get(DISMISS_KEY)
                .map(|v| (w.id, matches!(v, Value::Bool(false))))
        })
        .collect();
    for &(wire, declined) in &wires {
        if declined {
            tx.dismiss(DISMISS_KEY, vec![wire.into()])?;
        }
        tx.set_property(wire, DISMISS_KEY, None)?;
    }
    Ok(!wires.is_empty())
}

/// The wires of the connection `wire` is part of: every wire joined to it
/// through junctions, or every wire between the same two pins.
fn connection_wires(project: &Project, wire: WireId) -> Vec<WireId> {
    let Some(w) = project.wire(wire) else {
        return vec![wire];
    };
    let Some(junction) = [w.from, w.to].into_iter().find(|e| e.as_pin().is_none()) else {
        let pins = |x: &novo_core::Wire| {
            let mut p = [x.from, x.to];
            p.sort();
            p
        };
        let these = pins(w);
        return project
            .wires()
            .filter(|x| pins(x) == these)
            .map(|x| x.id)
            .collect();
    };
    let mut groups = Groups::new(project);
    let root = groups.root(junction);
    let mut wires = Vec::new();
    for other in project.wires() {
        let joined = [other.from, other.to]
            .into_iter()
            .any(|e| e.as_pin().is_none() && groups.root(e) == root);
        if joined {
            wires.push(other.id);
        }
    }
    wires
}

/// Forgets a declined suggestion: the decline on any wire of the
/// connection `wire` is part of, so it can be restored from whichever wire
/// is suggested now.
pub fn restore_suggestion(tx: &mut Tx<'_>, wire: WireId) -> Result<()> {
    for wire in connection_wires(tx.project(), wire) {
        if let Some(record) = tx.project().dismissal(DISMISS_KEY, &[wire.into()]) {
            tx.remove(record.id)?;
        }
        let legacy = tx
            .project()
            .wire(wire)
            .is_some_and(|w| w.properties.contains_key(DISMISS_KEY));
        if legacy {
            tx.set_property(wire, DISMISS_KEY, None)?;
        }
    }
    Ok(())
}

/// One line for the "accept all" summary: how many terminals go on which
/// strip, e.g. `9 terminals on -X1, 2 terminals on +FLD-X2`.
pub fn summarize_suggestions(suggestions: &[Suggestion]) -> String {
    let mut counts: Vec<(&DeviceTag, usize)> = Vec::new();
    for s in suggestions {
        match counts.iter_mut().find(|(tag, _)| *tag == &s.strip) {
            Some((_, n)) => *n += 1,
            None => counts.push((&s.strip, 1)),
        }
    }
    counts
        .into_iter()
        .map(|(tag, n)| strings::terminals_on(n, &tag.to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}
