//! Schematic edits: placing, wiring, moving, deleting and renaming.
//!
//! Each function runs inside a [`Tx`], so the caller decides what one undo
//! step is (one gesture, one undo step: DESIGN.md §2). Everything goes
//! through the `novo-core` edit API; nothing here touches the model
//! directly.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{
    AnyId, Device, DeviceId, DeviceKind, DeviceTag, Direction, Endpoint, JunctionId, ModelError,
    PageId, PinRef, Placement, Point, PotentialId, PotentialKind, Project, Result, Rotation,
    SymbolId, SymbolInstance, SymbolRole, Terminal, TerminalId, TerminalKind, Tx, Wire, WireId,
};
use novo_symbols::Symbol;

use crate::catalog::{SymbolClass, classify, model_groups, model_pins};
use crate::geometry::{
    self, EPS, Target, arc_position, endpoint_direction, locate_on_polyline, route, simplify,
    wire_points, wires_through,
};
use crate::naming::{Numbering, child_pin_names, complete_tag, new_device_tag, page_structure};
use crate::overlap::{Refusal, plan_route, route_refusal};

/// What a placement attaches to, beyond the symbol itself.
#[derive(Debug, Clone, Default)]
pub struct PlaceOptions {
    /// The device a symbol belongs to: the parent of a child symbol (an
    /// auxiliary contact of `-K1`) or the device whose main function is
    /// placed (`coil K1`).
    /// Untyped aspects come from the page's structure. A tag that names no
    /// device yet creates it (scripts); the editor refuses that with a
    /// notice instead.
    pub parent: Option<DeviceTag>,
    /// Terminal strip for terminal symbols; `None` uses the strip of the
    /// last terminal on the page, else `-X1`.
    pub strip: Option<DeviceId>,
    /// Potential of a potential feed; `None` uses `L1`.
    pub potential: Option<PotentialId>,
    /// Name of an interruption point; `None` gives the next free one.
    pub interruption: Option<String>,
    /// Connect pins that land on wires (on by default; scripts may turn it
    /// off).
    pub auto_connect: bool,
    /// How a new device is numbered.
    pub numbering: Numbering,
}

impl PlaceOptions {
    pub fn new() -> Self {
        PlaceOptions {
            auto_connect: true,
            ..PlaceOptions::default()
        }
    }
}

/// The symbol carrying `device`'s main function, if it has one.
pub fn main_symbol(project: &Project, device: DeviceId) -> Option<SymbolId> {
    project
        .symbols_of_device(device)
        .into_iter()
        .find(|s| matches!(s.role, SymbolRole::Device { main: true, .. }))
        .map(|s| s.id)
}

/// Places a library symbol as a tagged device (or terminal, potential feed,
/// interruption point) with no questions asked, and connects its pins to
/// what they land on. Returns the new symbol.
pub fn place(
    tx: &mut Tx<'_>,
    page: PageId,
    symbol: &Symbol,
    placement: Placement,
    options: &PlaceOptions,
) -> Result<SymbolId> {
    let mut pins = model_pins(symbol);
    let (bridges, pass_through) = model_groups(symbol);
    let role = match classify(symbol) {
        SymbolClass::Main { letter } => {
            let device = match &options.parent {
                // `coil K1`: the main function of that device, created when
                // it does not exist yet. A device has one main function, so
                // a second one is refused.
                Some(tag) => {
                    let tag = complete_tag(tx.project(), page, tag);
                    match tx.project().devices_by_tag(&tag).first() {
                        Some(&d) => {
                            if let Some(main) = main_symbol(tx.project(), d) {
                                return Err(ModelError::AlreadyExists(main.into()));
                            }
                            d
                        }
                        None => tx.add_device(tag, symbol.name.clone())?,
                    }
                }
                None => {
                    let tag = new_device_tag(tx.project(), page, letter, options.numbering);
                    tx.add_device(tag, symbol.name.clone())?
                }
            };
            SymbolRole::Device { device, main: true }
        }
        SymbolClass::Child { letter } => {
            let device = match &options.parent {
                Some(tag) => {
                    let tag = complete_tag(tx.project(), page, tag);
                    match tx.project().devices_by_tag(&tag).first() {
                        Some(&d) => d,
                        None => tx.add_device(tag, "")?,
                    }
                }
                None => {
                    let tag = new_device_tag(tx.project(), page, letter, options.numbering);
                    tx.add_device(tag, symbol.name.clone())?
                }
            };
            let defaults: Vec<&str> = pins.iter().map(|p| p.name.as_str()).collect();
            if let Some(names) = child_pin_names(tx.project(), device, &defaults) {
                for (pin, name) in pins.iter_mut().zip(names) {
                    pin.name = name;
                }
            }
            SymbolRole::Device {
                device,
                main: false,
            }
        }
        SymbolClass::Terminal { kind } => {
            let strip = match options.strip.filter(|s| strip_exists(tx.project(), *s)) {
                Some(s) => s,
                None => default_strip(tx, page)?,
            };
            let levels = bridges.len().max(1);
            let name = next_terminal_name(tx.project(), strip, kind);
            let mut terminals = Vec::with_capacity(levels);
            for level in 0..levels {
                let t = tx.add_terminal(strip, name.clone())?;
                tx.update::<Terminal>(t, |t| {
                    t.kind = kind;
                    t.level = u8::try_from(level + 1).unwrap_or(u8::MAX);
                })?;
                terminals.push(t);
            }
            SymbolRole::Terminal { terminals }
        }
        SymbolClass::PotentialFeed => {
            let potential = match options.potential {
                Some(p) if tx.project().potential(p).is_some() => p,
                _ => find_or_add_potential(tx, "L1", PotentialKind::Line, None)?,
            };
            SymbolRole::PotentialFeed { potential }
        }
        SymbolClass::Interruption => SymbolRole::Interruption {
            name: options
                .interruption
                .clone()
                .unwrap_or_else(|| next_interruption_name(tx.project())),
        },
        SymbolClass::Plain => SymbolRole::Unassigned,
    };
    let key = novo_core::SymbolKey(symbol.id.clone());
    let id = tx.insert(|id| SymbolInstance {
        id,
        page,
        symbol: key,
        placement,
        role,
        pins,
        bridges,
        pass_through,
        feeds: Vec::new(),
        properties: Default::default(),
    })?;
    if options.auto_connect {
        auto_connect(tx, id)?;
    }
    Ok(id)
}

fn strip_exists(project: &Project, strip: DeviceId) -> bool {
    project
        .device(strip)
        .is_some_and(|d| d.kind == DeviceKind::TerminalStrip)
}

/// The strip of the last terminal placed on the page, else `-X1` in the
/// page's structure (created when missing).
fn default_strip(tx: &mut Tx<'_>, page: PageId) -> Result<DeviceId> {
    let project = tx.project();
    let last = project
        .page_contents(page)
        .symbols
        .iter()
        .rev()
        .filter_map(|&s| project.symbol(s))
        .find_map(|s| match &s.role {
            SymbolRole::Terminal { terminals } => terminals
                .first()
                .and_then(|t| project.terminal(*t))
                .map(|t| t.strip),
            _ => None,
        });
    if let Some(strip) = last {
        return Ok(strip);
    }
    let mut tag = page_structure(project, page);
    tag.product.push("X1".into());
    if let Some(&d) = project.devices_by_tag(&tag).first() {
        if strip_exists(project, d) {
            return Ok(d);
        }
        let n = project.next_tag_number(&page_structure(project, page), "X");
        tag = tag.with_number(n);
    }
    tx.add_terminal_strip(tag)
}

/// Next terminal name on a strip: `PE` / `N` for those kinds (then `PE2`),
/// otherwise one more than the highest number.
pub fn next_terminal_name(project: &Project, strip: DeviceId, kind: TerminalKind) -> String {
    let names: BTreeSet<String> = project
        .terminals_of_strip(strip)
        .iter()
        .map(|t| t.name.clone())
        .collect();
    let base = match kind {
        TerminalKind::ProtectiveEarth => Some("PE"),
        TerminalKind::Neutral => Some("N"),
        _ => None,
    };
    if let Some(base) = base {
        if !names.contains(base) {
            return base.into();
        }
        return (2..)
            .map(|n| format!("{base}{n}"))
            .find(|n| !names.contains(n))
            .expect("some name is free");
    }
    let max = names.iter().filter_map(|n| n.parse::<u32>().ok()).max();
    (max.unwrap_or(0) + 1).to_string()
}

fn next_interruption_name(project: &Project) -> String {
    let used: BTreeSet<&str> = project
        .symbols()
        .filter_map(|s| match &s.role {
            SymbolRole::Interruption { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    (1..)
        .map(|n| format!("IP{n}"))
        .find(|n| !used.contains(n.as_str()))
        .expect("some name is free")
}

/// A potential by name, created with `kind` and `voltage` when missing.
pub fn find_or_add_potential(
    tx: &mut Tx<'_>,
    name: &str,
    kind: PotentialKind,
    voltage: Option<f64>,
) -> Result<PotentialId> {
    let existing = tx
        .project()
        .potentials()
        .find(|p| p.name == name)
        .map(|p| (p.id, p.voltage));
    if let Some((id, old)) = existing {
        if old.is_none() && voltage.is_some() {
            tx.update::<novo_core::Potential>(id, |p| p.voltage = voltage)?;
        }
        return Ok(id);
    }
    let id = tx.add_potential(name, kind)?;
    if voltage.is_some() {
        tx.update::<novo_core::Potential>(id, |p| p.voltage = voltage)?;
    }
    Ok(id)
}

/// Connects the pins of a newly placed symbol to what they land on
/// (DESIGN.md §2, "anything with exactly one correct answer happens on its
/// own"): pins inside a wire split it (a symbol with two pins on one wire is
/// inserted in series, one pin on a wire makes a T there), and pins dropped
/// on free pins connect. See [`settle`].
pub fn auto_connect(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<()> {
    settle(tx, &[symbol])
}

/// Finishes a placement or move with what has exactly one answer (DESIGN.md
/// §2): pins dropped inside a wire split it, and a free pin dropped on
/// another free pin connects to it. Facing pins at a distance are only
/// proposed ([`facing_wires`], [`accept`]).
pub fn settle(tx: &mut Tx<'_>, symbols: &[SymbolId]) -> Result<()> {
    for &s in symbols {
        insert_into_wires(tx, s)?;
    }
    for &s in symbols {
        connect_touching(tx, s)?;
    }
    Ok(())
}

/// Connects free pins of `symbol` to free pins of other symbols at the same
/// point.
fn connect_touching(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<()> {
    let Some(page) = tx.project().symbol(symbol).map(|s| s.page) else {
        return Ok(());
    };
    for (pin, p) in pin_positions(tx.project(), symbol) {
        let from = Endpoint::pin(symbol, pin);
        let project = tx.project();
        if !is_free(project, from) {
            continue;
        }
        let other = project
            .page_contents(page)
            .symbols
            .into_iter()
            .filter(|&o| o != symbol)
            .flat_map(|o| {
                pin_positions(project, o)
                    .into_iter()
                    .map(move |(opin, q)| (Endpoint::pin(o, opin), q))
            })
            .find(|&(e, q)| q.approx_eq(p, EPS) && is_free(project, e));
        if let Some((to, _)) = other {
            tx.connect(from, to)?;
        }
    }
    Ok(())
}

/// Whether a proposal still holds: both pins exist, are free and are still
/// where the preview showed them, so the wire drawn is the line that was
/// shown.
pub fn proposal_holds(project: &Project, proposal: &FacingWire) -> bool {
    let ok = |e: Endpoint, at: Point| {
        project
            .endpoint_position(e)
            .is_some_and(|p| p.approx_eq(at, EPS))
            && is_free(project, e)
    };
    ok(proposal.from, proposal.a) && ok(proposal.to, proposal.b)
}

/// Accepts a proposed facing wire: draws it if the proposal still holds
/// ([`proposal_holds`]), and otherwise changes nothing.
pub fn accept(tx: &mut Tx<'_>, proposal: &FacingWire) -> Result<Option<WireId>> {
    if !proposal_holds(tx.project(), proposal) {
        return Ok(None);
    }
    // A facing wire is straight and the user never placed its route, but it
    // still may not run along another wire.
    // Only pin to pin proposals exist (`FacingWire`), so the page is the
    // symbol's; a non-pin start would skip the check.
    let project = tx.project();
    let page = proposal
        .from
        .as_pin()
        .and_then(|r| project.symbol(r.symbol))
        .map(|s| s.page);
    if let Some(refusal) =
        page.and_then(|page| route_refusal(project, page, &[proposal.a, proposal.b], &[], &[], &[]))
    {
        return Err(refused(refusal));
    }
    tx.connect(proposal.from, proposal.to).map(Some)
}

fn pin_positions(project: &Project, symbol: SymbolId) -> Vec<(u16, Point)> {
    project.symbol(symbol).map_or_else(Vec::new, |s| {
        (0..s.pins.len() as u16)
            .filter_map(|i| s.pin_position(i).map(|p| (i, p)))
            .collect()
    })
}

fn insert_into_wires(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<()> {
    let Some(page) = tx.project().symbol(symbol).map(|s| s.page) else {
        return Ok(());
    };
    // Which pins lie inside which wire, in order along the wire.
    let mut on_wire: BTreeMap<WireId, Vec<(f64, usize, Point, u16)>> = BTreeMap::new();
    for (pin, p) in pin_positions(tx.project(), symbol) {
        let project = tx.project();
        let candidates: Vec<(WireId, usize, Vec<Point>)> = wires_through(project, page, p)
            .into_iter()
            .filter(|(wire, _)| {
                !project.wire(*wire).is_some_and(|w| {
                    w.ends()
                        .iter()
                        .any(|e| matches!(e, Endpoint::Pin { symbol: s, .. } if *s == symbol))
                })
            })
            .filter_map(|(wire, seg)| Some((wire, seg, wire_points(project, wire)?)))
            .collect();
        // On a crossing of unconnected wires the pin joins only the wire its
        // direction runs along; the other keeps crossing (joining both
        // would short two nets).
        let direction = project.symbol(symbol).and_then(|s| s.pin_direction(pin));
        let along = |pts: &[Point], seg: usize| {
            let (a, b) = (pts[seg], pts[seg + 1]);
            direction.is_some_and(|d| {
                let v = d.vector();
                ((b.x - a.x) * v.y - (b.y - a.y) * v.x).abs() < EPS
            })
        };
        let chosen = candidates
            .iter()
            .find(|(_, seg, pts)| along(pts, *seg))
            .or(candidates.first());
        if let Some((wire, seg, pts)) = chosen {
            on_wire
                .entry(*wire)
                .or_default()
                .push((arc_position(pts, *seg, p), *seg, p, pin));
        }
    }
    for (wire, mut pins) in on_wire {
        pins.sort_by(|a, b| a.0.total_cmp(&b.0));
        let (_, first_seg, _, first_pin) = pins[0];
        let (_, last_seg, _, last_pin) = pins[pins.len() - 1];
        let original = tx
            .project()
            .wire(wire)
            .cloned()
            .ok_or(ModelError::NotFound(wire.into()))?;
        let tail_route = original.route[last_seg..].to_vec();
        tx.update::<Wire>(wire, |w| {
            w.to = Endpoint::pin(symbol, first_pin);
            w.route.truncate(first_seg);
        })?;
        tx.insert(|id| Wire {
            id,
            from: Endpoint::pin(symbol, last_pin),
            route: tail_route,
            number: None,
            ..original.clone()
        })?;
    }
    Ok(())
}

fn is_free(project: &Project, end: Endpoint) -> bool {
    project.wires_at(end).next().is_none()
}

/// The drawn lines of a symbol on its page.
pub type Outline = Vec<(Point, Point)>;

/// A straight wire that placing or moving would draw between a free pin of
/// the symbol in hand and the free pin it faces.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FacingWire {
    pub from: Endpoint,
    pub to: Endpoint,
    /// Where the wire would run, for the preview.
    pub a: Point,
    pub b: Point,
}

/// The facing wires proposed for symbols at the placement they would get.
/// `ghosts` are symbol instances as they would be (a placement preview, or
/// moved symbols at their new place, keeping their ids); other ghosts are
/// ignored as partners. A pin that already has a wire, or that lands inside
/// a wire, gets none, and a line that would cross a wire or touch the drawn lines of another
/// symbol (`outline` gives a symbol's drawn segments on the page; empty
/// space inside its bounding box does not block) is not proposed.
pub fn facing_wires(
    project: &Project,
    ghosts: &[SymbolInstance],
    outline: &dyn Fn(&SymbolInstance) -> Option<Outline>,
) -> Vec<FacingWire> {
    let ignore: BTreeSet<SymbolId> = ghosts.iter().map(|g| g.id).collect();
    let mut out = Vec::new();
    for sym in ghosts {
        let page = sym.page;
        let others: Vec<SymbolId> = project
            .page_contents(page)
            .symbols
            .into_iter()
            .filter(|s| !ignore.contains(s))
            .collect();
        for pin in 0..sym.pins.len() as u16 {
            let from = Endpoint::pin(sym.id, pin);
            let (Some(p), Some(dir)) = (sym.pin_position(pin), sym.pin_direction(pin)) else {
                continue;
            };
            if !is_free(project, from) || !wires_through(project, page, p).is_empty() {
                continue;
            }
            let v = dir.vector();
            // Every pin ahead on the same line, nearest first.
            let mut ahead: Vec<(f64, Endpoint, Point, Option<novo_core::Direction>)> = Vec::new();
            for &o in &others {
                let Some(os) = project.symbol(o) else {
                    continue;
                };
                for opin in 0..os.pins.len() as u16 {
                    let Some(q) = os.pin_position(opin) else {
                        continue;
                    };
                    let d = q - p;
                    let along = d.x * v.x + d.y * v.y;
                    let across = (d.x * v.y - d.y * v.x).abs();
                    if along > EPS && across < EPS {
                        ahead.push((along, Endpoint::pin(o, opin), q, os.pin_direction(opin)));
                    }
                }
            }
            // Pins of this symbol in between block the line too.
            let own_block = (0..sym.pins.len() as u16).filter(|&o| o != pin).any(|o| {
                sym.pin_position(o).is_some_and(|q| {
                    let d = q - p;
                    (d.x * v.x + d.y * v.y) > EPS && (d.x * v.y - d.y * v.x).abs() < EPS
                })
            });
            if own_block {
                continue;
            }
            ahead.sort_by(|a, b| a.0.total_cmp(&b.0));
            let Some(&(len, to, q, to_dir)) = ahead.first() else {
                continue;
            };
            let facing = to_dir.is_some_and(|d| {
                let w = d.vector();
                (w.x + v.x).abs() < EPS && (w.y + v.y).abs() < EPS
            });
            let partner = to.as_pin().map(|r| r.symbol);
            let through_body = |s: SymbolId| {
                Some(s) != partner
                    && project
                        .symbol(s)
                        .and_then(outline)
                        .is_some_and(|lines| lines.iter().any(|&(c, d)| segments_touch(p, q, c, d)))
            };
            if facing
                && is_free(project, to)
                && !is_declined(project, sym, pin, to)
                && !crosses_wire(project, page, p, len, v)
                && !others.iter().any(|&s| through_body(s))
            {
                out.push(FacingWire {
                    from,
                    to,
                    a: p,
                    b: q,
                });
            }
        }
    }
    out
}

/// Kind of [`novo_core::Dismissal`] that remembers a declined facing-pin
/// proposal, so it does not return (DESIGN.md §2). The pins follow the
/// sorted subjects: `connect.facing:3-1` declines pin 3 of the first
/// symbol to pin 1 of the second.
pub const FACING_PROPOSAL: &str = "connect.facing";

/// The dismissal kind and subjects for a facing proposal between two pins.
fn facing_dismissal(a: PinRef, b: PinRef) -> (String, Vec<AnyId>) {
    let (first, second) = if (a.symbol, a.pin) <= (b.symbol, b.pin) {
        (a, b)
    } else {
        (b, a)
    };
    (
        format!("{FACING_PROPOSAL}:{}-{}", first.pin, second.pin),
        vec![first.symbol.into(), second.symbol.into()],
    )
}

/// Whether the proposal from pin `pin` of `sym` to `to` was declined.
fn is_declined(project: &Project, sym: &SymbolInstance, pin: u16, to: Endpoint) -> bool {
    to.as_pin().is_some_and(|r| {
        let (proposal, subjects) = facing_dismissal(
            PinRef {
                symbol: sym.id,
                pin,
            },
            r,
        );
        project.is_dismissed(&proposal, &subjects)
    })
}

/// Declines a proposed facing wire for good (undoable): it is not proposed
/// again for these two pins, until either symbol is deleted.
pub fn decline(tx: &mut Tx<'_>, proposal: &FacingWire) -> Result<()> {
    if let (Some(a), Some(b)) = (proposal.from.as_pin(), proposal.to.as_pin()) {
        let (kind, subjects) = facing_dismissal(a, b);
        tx.dismiss(kind, subjects)?;
    }
    Ok(())
}

/// Whether segment `ab` meets segment `cd`, touching included.
fn segments_touch(a: Point, b: Point, c: Point, d: Point) -> bool {
    let cross =
        |o: Point, p: Point, q: Point| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let on = |p: Point, q: Point, r: Point| {
        r.x >= p.x.min(q.x) - EPS
            && r.x <= p.x.max(q.x) + EPS
            && r.y >= p.y.min(q.y) - EPS
            && r.y <= p.y.max(q.y) + EPS
    };
    let (d1, d2, d3, d4) = (
        cross(c, d, a),
        cross(c, d, b),
        cross(a, b, c),
        cross(a, b, d),
    );
    if ((d1 > EPS && d2 < -EPS) || (d1 < -EPS && d2 > EPS))
        && ((d3 > EPS && d4 < -EPS) || (d3 < -EPS && d4 > EPS))
    {
        return true;
    }
    (d1.abs() <= EPS && on(c, d, a))
        || (d2.abs() <= EPS && on(c, d, b))
        || (d3.abs() <= EPS && on(a, b, c))
        || (d4.abs() <= EPS && on(a, b, d))
}

/// Whether a straight line from `p` along `v` for `len` would cross or run
/// over an existing wire.
fn crosses_wire(project: &Project, page: PageId, p: Point, len: f64, v: Point) -> bool {
    let end = Point::new(p.x + v.x * len, p.y + v.y * len);
    let mid = Point::new((p.x + end.x) / 2.0, (p.y + end.y) / 2.0);
    // A wire running along the line covers its middle; a crossing wire
    // passes through some point of it.
    if !wires_through(project, page, mid).is_empty() {
        return true;
    }
    project.page_contents(page).wires.into_iter().any(|w| {
        let Some(pts) = wire_points(project, w) else {
            return false;
        };
        pts.windows(2).any(|s| segments_cross(p, end, s[0], s[1]))
    })
}

/// Whether two axis-aligned segments cross in their interiors.
fn segments_cross(a: Point, b: Point, c: Point, d: Point) -> bool {
    let vertical = |u: Point, w: Point| (u.x - w.x).abs() < EPS;
    let (v, h) = match (vertical(a, b), vertical(c, d)) {
        (true, false) => ((a, b), (c, d)),
        (false, true) => ((c, d), (a, b)),
        _ => return false,
    };
    let x = v.0.x;
    let y = h.0.y;
    let within = |t: f64, u: f64, w: f64| t > u.min(w) + EPS && t < u.max(w) - EPS;
    within(x, h.0.x, h.1.x) && within(y, v.0.y, v.1.y)
}

/// Resolves a wire end: a pin or junction as is, a point on a wire by
/// splitting it there (a T-junction with no question asked).
pub fn resolve_target(tx: &mut Tx<'_>, page: PageId, target: Target) -> Result<Endpoint> {
    match target {
        Target::Endpoint(end, _) => Ok(end),
        Target::Wire(wire, at) => {
            if let Some(j) = geometry::junction_at(tx.project(), page, at) {
                return Ok(Endpoint::junction(j));
            }
            // The wire may have been split since the target was taken.
            let wire = if tx.project().wire(wire).is_some()
                && wires_through(tx.project(), page, at)
                    .iter()
                    .any(|(w, _)| *w == wire)
            {
                wire
            } else {
                wires_through(tx.project(), page, at)
                    .first()
                    .map(|(w, _)| *w)
                    .ok_or(ModelError::NotFound(wire.into()))?
            };
            let (junction, _) = tx.split_wire(wire, at)?;
            Ok(Endpoint::junction(junction))
        }
    }
}

/// The model error a refused route fails with, for callers that cannot use
/// [`Refusal`] as data (scripts, [`draw_wire`]). The reason names the other
/// wire by its ends and says in plain words what is wrong.
pub(crate) fn refused(r: Refusal) -> ModelError {
    refusal_error(None, r)
}

/// As [`refused`], naming the wire the refusal is about when the project is
/// at hand to label the other wire's ends.
pub(crate) fn refusal_error(project: Option<&Project>, r: Refusal) -> ModelError {
    let label = |e: Endpoint| match e {
        Endpoint::Pin { symbol, pin } => project
            .and_then(|p| {
                let s = p.symbol(symbol)?;
                let tag = s
                    .device()
                    .and_then(|d| p.device(d))
                    .map(|d| d.tag.to_string());
                Some(format!(
                    "{} pin {}",
                    tag.unwrap_or_else(|| "a symbol".into()),
                    s.pins.get(usize::from(pin))?.name
                ))
            })
            .unwrap_or_else(|| "a pin".into()),
        Endpoint::Junction { .. } => "a junction".into(),
        Endpoint::Free { .. } => "an open end".into(),
    };
    let which = project.and_then(|p| p.wire(r.other)).map_or_else(
        || "another wire".to_string(),
        |w| format!("the wire from {} to {}", label(w.from), label(w.to)),
    );
    let length = ((r.to.x - r.from.x).abs() + (r.to.y - r.from.y).abs()).round();
    ModelError::Refused {
        what: "This route".into(),
        reason: format!(
            "it would run along {which} for {length} mm; wires may cross or end on the side of another wire but not share its length. Move a corner or end the wire on its side."
        ),
    }
}

/// How a wire end on another wire's side should arrive: at right angles to
/// that wire, from the side `toward` lies on. A pin or junction keeps its
/// own exit direction. Without this a route to a point on a wire could
/// arrive along the wire and be refused as sharing its length.
fn approach_direction(
    project: &Project,
    page: PageId,
    t: Target,
    toward: Point,
) -> Option<Direction> {
    match t {
        Target::Endpoint(e, _) => endpoint_direction(project, e),
        Target::Wire(w, at) => {
            if geometry::junction_at(project, page, at).is_some() {
                return None;
            }
            let seg = wires_through(project, page, at)
                .into_iter()
                .find(|(x, _)| *x == w)
                .or_else(|| wires_through(project, page, at).into_iter().next())?
                .1;
            let pts = wire_points(project, w)?;
            let (a, b) = (*pts.get(seg)?, *pts.get(seg + 1)?);
            if (a.y - b.y).abs() < EPS {
                Some(if toward.y < at.y {
                    Direction::Up
                } else {
                    Direction::Down
                })
            } else if (a.x - b.x).abs() < EPS {
                Some(if toward.x < at.x {
                    Direction::Left
                } else {
                    Direction::Right
                })
            } else {
                None
            }
        }
    }
}

/// The wires already at a pin, other than `except`; nothing for a junction
/// or a point on a wire. Wires that leave one pin together may share their
/// first stretch (it becomes a derived junction later).
fn pin_wires(project: &Project, end: Endpoint, except: &[WireId]) -> Vec<WireId> {
    if matches!(end, Endpoint::Pin { .. }) {
        project
            .wires_at(end)
            .filter(|w| !except.contains(w))
            .collect()
    } else {
        Vec::new()
    }
}

/// Whether drawing a wire from `start` through `corners` to `end` would be
/// refused because it shares a stretch with another wire, as data: the
/// other wire and the stretch. Read-only, for the live preview and for the
/// click handler to ask before it commits ([`draw_wire`] makes the same
/// check and fails with a model error).
pub fn check_draw(
    project: &Project,
    page: PageId,
    start: Target,
    corners: &[Point],
    end: Target,
) -> std::result::Result<(), Refusal> {
    let endpoint = |t: Target| match t {
        Target::Endpoint(e, _) => Some(e),
        Target::Wire(..) => None,
    };
    let wires = |t: Target| endpoint(t).map_or_else(Vec::new, |e| pin_wires(project, e, &[]));
    plan_route(
        project,
        page,
        start.position(),
        approach_direction(project, page, start, end.position()),
        corners,
        end.position(),
        approach_direction(project, page, end, start.position()),
        &wires(start),
        &wires(end),
    )
    .map(|_| ())
}

/// Draws a wire from `start` through the user's `corners` to `end`, routed
/// orthogonally with at most two bends per leg. Ends on wires become
/// T-junctions.
pub fn draw_wire(
    tx: &mut Tx<'_>,
    page: PageId,
    start: Target,
    corners: &[Point],
    end: Target,
) -> Result<WireId> {
    let (start_dir, end_dir) = {
        let project = tx.project();
        (
            approach_direction(project, page, start, end.position()),
            approach_direction(project, page, end, start.position()),
        )
    };
    let from = resolve_target(tx, page, start)?;
    let to = resolve_target(tx, page, end)?;
    let project = tx.project();
    let a = project
        .endpoint_position(from)
        .ok_or_else(|| missing(from))?;
    let b = project.endpoint_position(to).ok_or_else(|| missing(to))?;
    // Wires that leave the same pin may run together for their first stretch;
    // that case becomes a derived junction later. Everything else is checked.
    let bends = plan_route(
        project,
        page,
        a,
        start_dir,
        corners,
        b,
        end_dir,
        &pin_wires(project, from, &[]),
        &pin_wires(project, to, &[]),
    )
    .map_err(|r| refusal_error(Some(project), r))?;
    let wire = tx.connect(from, to)?;
    if !bends.is_empty() {
        tx.update::<Wire>(wire, |w| w.route = bends)?;
    }
    Ok(wire)
}

/// The error for an endpoint that has no position: it names the missing
/// symbol or junction.
pub(crate) fn missing(end: Endpoint) -> ModelError {
    match end {
        Endpoint::Pin { symbol, .. } => ModelError::NotFound(symbol.into()),
        Endpoint::Junction { junction } => ModelError::NotFound(junction.into()),
        // An open end is its own position.
        Endpoint::Free { .. } => ModelError::Refused {
            what: "Wire".into(),
            reason: "an open end has no position".into(),
        },
    }
}

/// How to change the placement of selected objects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transform {
    Move(Point),
    /// A quarter turn counter-clockwise about a point.
    Rotate(Point),
    /// Mirror about the vertical line through a point.
    Mirror(Point),
}

impl Transform {
    fn point(self, p: Point) -> Point {
        match self {
            Transform::Move(d) => p + d,
            Transform::Rotate(c) => c + Rotation::R90.apply(p - c),
            Transform::Mirror(c) => Point::new(2.0 * c.x - p.x, p.y),
        }
    }

    fn placement(self, pl: Placement) -> Placement {
        match self {
            Transform::Move(_) => Placement {
                origin: self.point(pl.origin),
                ..pl
            },
            Transform::Rotate(_) => Placement {
                origin: self.point(pl.origin),
                rotation: pl.rotation.then(Rotation::R90),
                ..pl
            },
            // Reflecting a rotated symbol reverses its rotation.
            Transform::Mirror(_) => Placement {
                origin: self.point(pl.origin),
                rotation: Rotation::from_degrees(-i32::from(pl.rotation.degrees()))
                    .expect("quarter turn"),
                mirrored: !pl.mirrored,
            },
        }
    }
}

/// The point selected objects turn about: the origin of a single symbol,
/// else the centre of the selection snapped to `grid`.
pub fn pivot(project: &Project, ids: &[AnyId], grid: f64) -> Option<Point> {
    let symbols: Vec<&SymbolInstance> = ids
        .iter()
        .filter_map(|id| match id {
            AnyId::Symbol(s) => project.symbol(*s),
            _ => None,
        })
        .collect();
    if let [single] = symbols.as_slice() {
        return Some(single.placement.origin);
    }
    let mut points: Vec<Point> = Vec::new();
    for id in ids {
        match *id {
            AnyId::Symbol(s) => {
                if let Some(sym) = project.symbol(s) {
                    points.push(sym.placement.origin);
                    points.extend((0..sym.pins.len() as u16).filter_map(|i| sym.pin_position(i)));
                }
            }
            AnyId::Junction(j) => points.extend(project.junction(j).map(|j| j.position)),
            AnyId::Wire(w) => points.extend(wire_points(project, w).unwrap_or_default()),
            _ => {}
        }
    }
    if points.is_empty() {
        return None;
    }
    let (mut lo, mut hi) = (points[0], points[0]);
    for p in &points {
        lo = Point::new(lo.x.min(p.x), lo.y.min(p.y));
        hi = Point::new(hi.x.max(p.x), hi.y.max(p.y));
    }
    Some(geometry::snap(
        Point::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0),
        grid,
    ))
}

/// Moves, turns or mirrors symbols and junctions. Wires between two
/// transformed objects keep their shape; wires with one end left behind are
/// re-routed. Nothing new connects here: callers finish with [`settle`]
/// once every step of the gesture is done.
pub fn transform(tx: &mut Tx<'_>, ids: &[AnyId], how: Transform) -> Result<()> {
    transform_with(tx, ids, how, false)
}

/// [`transform`] for a person's drag or key press: where no adjustment keeps
/// the moved wires clear of other wires the whole move fails and changes
/// nothing (the transaction rolls back). Scripts, AI edits, undo and redo
/// use [`transform`], which accepts the overlap for `wire.overlap` to report.
pub fn transform_interactive(tx: &mut Tx<'_>, ids: &[AnyId], how: Transform) -> Result<()> {
    transform_with(tx, ids, how, true)
}

fn transform_with(tx: &mut Tx<'_>, ids: &[AnyId], how: Transform, refuse: bool) -> Result<()> {
    let mut symbols: BTreeSet<SymbolId> = BTreeSet::new();
    let mut junctions: BTreeSet<JunctionId> = BTreeSet::new();
    for id in ids {
        match *id {
            AnyId::Symbol(s) if tx.project().symbol(s).is_some() => {
                symbols.insert(s);
            }
            AnyId::Junction(j) if tx.project().junction(j).is_some() => {
                junctions.insert(j);
            }
            // A selected wire takes its junctions along; its pins stay.
            AnyId::Wire(w) => {
                if let Some(wire) = tx.project().wire(w) {
                    for end in wire.ends() {
                        if let Endpoint::Junction { junction } = end {
                            junctions.insert(junction);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let moved = |e: &Endpoint| match *e {
        Endpoint::Pin { symbol, .. } => symbols.contains(&symbol),
        Endpoint::Junction { junction } => junctions.contains(&junction),
        Endpoint::Free { .. } => false,
    };
    // Wires touching anything that moves, and whether both ends move.
    let mut wires: BTreeMap<WireId, bool> = BTreeMap::new();
    for w in tx.project().wires() {
        let [a, b] = w.ends();
        if moved(&a) || moved(&b) {
            wires.insert(w.id, moved(&a) && moved(&b));
        }
    }
    for &s in &symbols {
        tx.update::<SymbolInstance>(s, |sym| sym.placement = how.placement(sym.placement))?;
    }
    for &j in &junctions {
        tx.update::<novo_core::Junction>(j, |junction| {
            junction.position = how.point(junction.position)
        })?;
    }
    // Rigidly moved wires first, so the re-routed ones and the checks all see
    // every wire where it ends up, whatever the order of the ids.
    let rigid: Vec<WireId> = wires
        .iter()
        .filter(|(_, both)| **both)
        .map(|(&w, _)| w)
        .collect();
    for &wire in &rigid {
        tx.update::<Wire>(wire, |w| {
            for p in &mut w.route {
                *p = how.point(*p);
            }
        })?;
    }
    for (&wire, &both) in &wires {
        if !both {
            reroute_with(tx, wire, refuse)?;
        }
    }
    // A rigid route keeps its shape: it is checked, not adjusted. Wires that
    // moved together with it never clash with it.
    if refuse {
        for &wire in &rigid {
            check_kept(tx.project(), wire, &rigid)?;
        }
    }
    Ok(())
}

/// Fails if wire `wire` as it now lies shares a stretch with a wire that is
/// not in `together` (the wires that moved rigidly with it).
fn check_kept(project: &Project, wire: WireId, together: &[WireId]) -> Result<()> {
    let (Some(w), Some(points)) = (project.wire(wire), wire_points(project, wire)) else {
        return Ok(());
    };
    let (start, end) = (
        pin_wires(project, w.from, &[wire]),
        pin_wires(project, w.to, &[wire]),
    );
    match route_refusal(project, w.page, &points, together, &start, &end) {
        Some(r) => Err(refused(r)),
        None => Ok(()),
    }
}

/// The symbols among `ids`, for [`settle`].
pub fn symbols_in(ids: &[AnyId]) -> Vec<SymbolId> {
    ids.iter()
        .filter_map(|id| match id {
            AnyId::Symbol(s) => Some(*s),
            _ => None,
        })
        .collect()
}

/// Replaces a wire's bends with a fresh orthogonal route between its ends,
/// moving the router's cross-over clear of other wires where it can.
pub fn reroute(tx: &mut Tx<'_>, wire: WireId) -> Result<()> {
    reroute_with(tx, wire, false)
}

/// [`reroute`]; with `refuse`, a wire that cannot be kept clear fails the
/// transaction instead of keeping the plain route.
fn reroute_with(tx: &mut Tx<'_>, wire: WireId, refuse: bool) -> Result<()> {
    let project = tx.project();
    let Some(w) = project.wire(wire) else {
        return Ok(());
    };
    let (Some(a), Some(b)) = (
        project.endpoint_position(w.from),
        project.endpoint_position(w.to),
    ) else {
        return Ok(());
    };
    let (from_dir, to_dir) = (
        endpoint_direction(project, w.from),
        endpoint_direction(project, w.to),
    );
    // The wire's own old bends no longer lead anywhere: clear them so they
    // do not count as another wire's stretch.
    let (from_wires, to_wires) = (
        pin_wires(tx.project(), w.from, &[wire]),
        pin_wires(tx.project(), w.to, &[wire]),
    );
    let page = w.page;
    tx.update::<Wire>(wire, |w| w.route.clear())?;
    let planned = plan_route(
        tx.project(),
        page,
        a,
        from_dir,
        &[],
        b,
        to_dir,
        &from_wires,
        &to_wires,
    );
    let bends = match planned {
        Ok(bends) => bends,
        Err(r) if refuse => return Err(refused(r)),
        Err(_) => route(a, from_dir, b, to_dir),
    };
    tx.update::<Wire>(wire, |w| w.route = bends)
}

/// Deletes objects and tidies up behind them: wires on deleted pins go too,
/// a junction left with two wires joins them into one, a junction left with
/// none disappears, and devices and terminals no longer drawn anywhere are
/// removed.
pub fn delete(tx: &mut Tx<'_>, ids: &[AnyId]) -> Result<()> {
    let pages: BTreeSet<PageId> = ids
        .iter()
        .filter_map(|&id| tx.project().record(id).and_then(|r| r.page()))
        .collect();
    // The wires at each junction before the delete: only junctions the
    // delete touches are tidied, never others elsewhere on the page.
    let before: BTreeMap<JunctionId, BTreeSet<WireId>> = pages
        .iter()
        .flat_map(|&page| tx.project().page_contents(page).junctions)
        .map(|j| (j, tx.project().wires_at(Endpoint::junction(j)).collect()))
        .collect();
    let mut devices: BTreeSet<DeviceId> = BTreeSet::new();
    let mut terminals: BTreeSet<TerminalId> = BTreeSet::new();
    for id in ids {
        let project = tx.project();
        if !project.contains(*id) {
            continue;
        }
        match *id {
            AnyId::Symbol(s) => {
                keep_wires_through(tx, s)?;
                let project = tx.project();
                let sym = project.symbol(s).expect("checked above");
                match &sym.role {
                    SymbolRole::Device { device, .. } => {
                        devices.insert(*device);
                    }
                    SymbolRole::Terminal { terminals: ts } => terminals.extend(ts.iter().copied()),
                    _ => {}
                }
            }
            AnyId::Device(d) => {
                devices.remove(&d);
            }
            _ => {}
        }
        tx.remove(*id)?;
    }
    let touched: Vec<JunctionId> = pages
        .iter()
        .flat_map(|&page| tx.project().page_contents(page).junctions)
        .filter(|j| {
            let now: BTreeSet<WireId> = tx.project().wires_at(Endpoint::junction(*j)).collect();
            before.get(j) != Some(&now)
        })
        .collect();
    tidy_junctions(tx, &touched)?;
    let mut strips: BTreeSet<DeviceId> = BTreeSet::new();
    for t in terminals {
        let project = tx.project();
        if project.terminal(t).is_some() && project.symbols_of_terminal(t).is_empty() {
            strips.extend(project.terminal(t).map(|t| t.strip));
            tx.remove(t)?;
        }
    }
    for d in devices.into_iter().chain(strips) {
        let project = tx.project();
        let Some(device) = project.device(d) else {
            continue;
        };
        let unused = project.symbols_of_device(d).is_empty()
            && project.terminals_of_strip(d).is_empty()
            && device.parts.is_empty();
        if unused {
            tx.remove(d)?;
        }
    }
    Ok(())
}

/// A symbol with one pin that sits on a wire (a potential feed, an
/// interruption point hung on a T) must not cut the wire when it goes: its
/// wires move to a junction at the pin, which the tidy-up then joins.
fn keep_wires_through(tx: &mut Tx<'_>, symbol: SymbolId) -> Result<()> {
    let project = tx.project();
    let Some(sym) = project.symbol(symbol) else {
        return Ok(());
    };
    if sym.pins.len() != 1 {
        return Ok(());
    }
    let end = Endpoint::pin(symbol, 0);
    let wires: Vec<WireId> = project.wires_at(end).collect();
    let Some(at) = sym.pin_position(0) else {
        return Ok(());
    };
    if wires.len() < 2 {
        return Ok(());
    }
    let page = sym.page;
    let junction = tx.add_junction(page, at)?;
    for w in wires {
        tx.update::<Wire>(w, |w| {
            if w.from == end {
                w.from = Endpoint::junction(junction);
            }
            if w.to == end {
                w.to = Endpoint::junction(junction);
            }
        })?;
    }
    Ok(())
}

/// Tidies the given junctions and whatever that tidy-up reaches: a
/// junction without wires goes, a junction left with two wires joins them
/// into one, and a junction left with one wire is a dead stub, so that wire
/// goes too and the junction at its other end is looked at in turn. Two
/// wires that both run from the junction back to the same point lead
/// nowhere either and go the same way.
///
/// Every step removes at least one object, so this always ends.
pub fn tidy_junctions(tx: &mut Tx<'_>, junctions: &[JunctionId]) -> Result<()> {
    let mut work: Vec<JunctionId> = junctions.to_vec();
    while let Some(j) = work.pop() {
        if tx.project().junction(j).is_none() {
            continue;
        }
        let end = Endpoint::junction(j);
        let mut wires: Vec<WireId> = tx.project().wires_at(end).collect();
        wires.sort();
        wires.dedup();
        // A wire with both ends here connects nothing.
        let (looped, wires): (Vec<WireId>, Vec<WireId>) = wires
            .into_iter()
            .partition(|&w| far_end(tx.project(), w, end).is_none());
        if !looped.is_empty() {
            for w in looped {
                tx.remove(w)?;
            }
            work.push(j);
            continue;
        }
        let far: Vec<Endpoint> = wires
            .iter()
            .filter_map(|&w| far_end(tx.project(), w, end))
            .collect();
        match far.as_slice() {
            // No wires, or a dot on a dead stub: removed below.
            [] | [_] => {}
            // Two wires that both go back to the same point: a loop that
            // reaches nothing else, removed below.
            [a, b] if a == b => {}
            [_, _] => {
                join_wires(tx, j, wires[0], wires[1])?;
                continue;
            }
            // A real T or more: nothing to tidy.
            _ => continue,
        }
        for (w, far) in wires.iter().zip(&far) {
            tx.remove(*w)?;
            if let Endpoint::Junction { junction } = far {
                work.push(*junction);
            }
        }
        if tx.project().junction(j).is_some() {
            tx.remove(j)?;
        }
    }
    Ok(())
}

/// The end of `wire` that is not `at` (`None` for a wire with both ends
/// there, or no such wire).
fn far_end(project: &Project, wire: WireId, at: Endpoint) -> Option<Endpoint> {
    let w = project.wire(wire)?;
    match (w.from == at, w.to == at) {
        (true, false) => Some(w.to),
        (false, true) => Some(w.from),
        _ => None,
    }
}

/// Joins the two wires of a junction into one and removes the junction.
/// Returns whether it did: two wires whose far ends are the same point are
/// left alone.
fn join_wires(tx: &mut Tx<'_>, junction: JunctionId, keep: WireId, other: WireId) -> Result<bool> {
    let project = tx.project();
    let at = Endpoint::junction(junction);
    let jpos = project
        .junction(junction)
        .map(|j| j.position)
        .ok_or(ModelError::NotFound(junction.into()))?;
    let k = project
        .wire(keep)
        .cloned()
        .ok_or(ModelError::NotFound(keep.into()))?;
    let o = project
        .wire(other)
        .cloned()
        .ok_or(ModelError::NotFound(other.into()))?;
    // `keep` oriented to end at the junction, `other` to start there.
    let (k_from, mut k_route) = if k.to == at {
        (k.from, k.route.clone())
    } else {
        (k.to, k.route.iter().rev().copied().collect())
    };
    let (o_to, o_route): (Endpoint, Vec<Point>) = if o.from == at {
        (o.to, o.route.clone())
    } else {
        (o.from, o.route.iter().rev().copied().collect())
    };
    if k_from == o_to {
        return Ok(false); // a loop back to the same point; leave it alone
    }
    k_route.push(jpos);
    k_route.extend(o_route);
    let a = project.endpoint_position(k_from).unwrap_or(jpos);
    let b = project.endpoint_position(o_to).unwrap_or(jpos);
    let route = simplify(a, k_route, b);
    tx.remove(other)?;
    tx.update::<Wire>(keep, |w| {
        w.from = k_from;
        w.to = o_to;
        w.route = route;
        if w.number.is_none() {
            w.number = o.number.clone();
        }
    })?;
    tx.remove(junction)?;
    Ok(true)
}

/// Renames a device after checking the new tag, so a refused rename never
/// reaches the model. Contacts and references follow automatically: they
/// point at the device, not at its tag.
pub fn rename(tx: &mut Tx<'_>, device: DeviceId, tag: DeviceTag) -> Result<()> {
    tx.set_tag(device, tag)
}

/// Which wires are under a point, by distance (for selection in tests and
/// scripts).
pub fn wire_near(project: &Project, page: PageId, p: Point, tolerance: f64) -> Option<WireId> {
    project
        .page_contents(page)
        .wires
        .into_iter()
        .filter_map(|w| {
            let pts = wire_points(project, w)?;
            let (_, _, d) = locate_on_polyline(&pts, p)?;
            (d <= tolerance).then_some((w, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(w, _)| w)
}

/// Every symbol of a device, by page, for "go to" lists and highlights.
pub fn device_symbols_by_page(
    project: &Project,
    device: DeviceId,
) -> HashMap<PageId, Vec<SymbolId>> {
    let mut out: HashMap<PageId, Vec<SymbolId>> = HashMap::new();
    for s in project.symbols_of_device(device) {
        out.entry(s.page).or_default().push(s.id);
    }
    out
}

/// The device a selected object belongs to, if any.
pub fn device_of(project: &Project, id: AnyId) -> Option<DeviceId> {
    match id {
        AnyId::Device(d) => project.device(d).map(|d: &Device| d.id),
        AnyId::Symbol(s) => project.symbol(s)?.device(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn a_line_blocks_only_where_a_drawn_segment_meets_it() {
        let wire = (p(10.0, 0.0), p(10.0, 100.0));
        // Crossing, touching and running along block.
        assert!(segments_touch(wire.0, wire.1, p(0.0, 50.0), p(20.0, 50.0)));
        assert!(segments_touch(wire.0, wire.1, p(10.0, 50.0), p(30.0, 50.0)));
        assert!(segments_touch(wire.0, wire.1, p(10.0, 20.0), p(10.0, 60.0)));
        // A line beside it, or short of it, or past its end does not (the
        // empty corner of a bounding box).
        assert!(!segments_touch(
            wire.0,
            wire.1,
            p(12.0, 0.0),
            p(12.0, 100.0)
        ));
        assert!(!segments_touch(
            wire.0,
            wire.1,
            p(12.0, 50.0),
            p(30.0, 50.0)
        ));
        assert!(!segments_touch(
            wire.0,
            wire.1,
            p(0.0, 120.0),
            p(20.0, 120.0)
        ));
    }
}
