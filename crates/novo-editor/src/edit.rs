//! Schematic edits: placing, wiring, moving, deleting and renaming.
//!
//! Each function runs inside a [`Tx`], so the caller decides what one undo
//! step is (one gesture, one undo step: DESIGN.md §2). Everything goes
//! through the `novo-core` edit API; nothing here touches the model
//! directly.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{
    AnyId, Device, DeviceId, DeviceKind, DeviceTag, Endpoint, JunctionId, ModelError, PageId,
    Placement, Point, PotentialId, PotentialKind, Project, Result, Rotation, SymbolId,
    SymbolInstance, SymbolRole, Terminal, TerminalId, TerminalKind, Tx, Wire, WireId,
};
use novo_symbols::Symbol;

use crate::catalog::{SymbolClass, classify, model_groups, model_pins};
use crate::geometry::{
    self, EPS, Target, arc_position, endpoint_direction, locate_on_polyline, route, route_through,
    simplify, wire_points, wires_through,
};
use crate::naming::{Numbering, child_pin_names, new_device_tag, page_structure};

/// What a placement attaches to, beyond the symbol itself.
#[derive(Debug, Clone, Default)]
pub struct PlaceOptions {
    /// Parent device of a child symbol (an auxiliary contact of `-K1`).
    /// A tag that names no device yet creates it.
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
    /// Also wire free pins that face a free pin ([`facing_wires`]). The
    /// editor turns this on only while it shows those wires in the preview.
    pub connect_facing: bool,
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
            let tag = new_device_tag(tx.project(), page, letter, options.numbering);
            let device = tx.add_device(tag, symbol.name.clone())?;
            SymbolRole::Device { device, main: true }
        }
        SymbolClass::Child { letter } => {
            let device = match &options.parent {
                Some(tag) => match tx.project().devices_by_tag(tag).first() {
                    Some(&d) => d,
                    None => tx.add_device(tag.clone(), "")?,
                },
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
        properties: Default::default(),
    })?;
    if options.auto_connect {
        auto_connect(tx, id, options.connect_facing)?;
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
/// own"):
///
/// - pins inside a wire split it; a symbol with two pins on one wire is
///   inserted in series (the piece between its pins goes away), one pin on a
///   wire makes a T there;
/// - with `facing`, a free pin that faces a free pin of another symbol on the
///   same line is wired to it straight. The editor shows those wires as
///   ghosts before the click ([`facing_wires`]), so what the preview shows
///   is what you get.
pub fn auto_connect(tx: &mut Tx<'_>, symbol: SymbolId, facing: bool) -> Result<()> {
    let wires = match (facing, tx.project().symbol(symbol)) {
        (true, Some(sym)) => facing_wires(tx.project(), std::slice::from_ref(sym)),
        _ => Vec::new(),
    };
    settle(tx, &[symbol], &wires)
}

/// Finishes a placement or move: pins dropped inside wires split them, then
/// the facing wires the preview showed are drawn where both ends are still
/// free.
pub fn settle(tx: &mut Tx<'_>, symbols: &[SymbolId], facing: &[FacingWire]) -> Result<()> {
    for &s in symbols {
        insert_into_wires(tx, s)?;
    }
    for f in facing {
        let project = tx.project();
        let exists = |e: Endpoint| project.endpoint_position(e).is_some();
        if exists(f.from) && exists(f.to) && is_free(project, f.from) && is_free(project, f.to) {
            tx.connect(f.from, f.to)?;
        }
    }
    Ok(())
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
        for (wire, seg) in wires_through(tx.project(), page, p) {
            if tx.project().wire(wire).is_some_and(|w| {
                w.ends()
                    .iter()
                    .any(|e| matches!(e, Endpoint::Pin { symbol: s, .. } if *s == symbol))
            }) {
                continue;
            }
            let pts = wire_points(tx.project(), wire).expect("wire on page");
            on_wire
                .entry(wire)
                .or_default()
                .push((arc_position(&pts, seg, p), seg, p, pin));
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

/// The facing wires for symbols at the placement they would get. `ghosts`
/// are symbol instances as they would be (a placement preview, or moved
/// symbols at their new place, keeping their ids); other ghosts are ignored
/// as partners. A pin that already has a wire, or that lands inside a wire,
/// gets none.
pub fn facing_wires(project: &Project, ghosts: &[SymbolInstance]) -> Vec<FacingWire> {
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
            if facing && is_free(project, to) && !crosses_wire(project, page, p, len, v) {
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
    let from = resolve_target(tx, page, start)?;
    let to = resolve_target(tx, page, end)?;
    let project = tx.project();
    let a = project
        .endpoint_position(from)
        .ok_or(ModelError::NotFound(endpoint_id(from)))?;
    let b = project
        .endpoint_position(to)
        .ok_or(ModelError::NotFound(endpoint_id(to)))?;
    let bends = route_through(
        a,
        endpoint_direction(project, from),
        corners,
        b,
        endpoint_direction(project, to),
    );
    let wire = tx.connect(from, to)?;
    if !bends.is_empty() {
        tx.update::<Wire>(wire, |w| w.route = bends)?;
    }
    Ok(wire)
}

fn endpoint_id(end: Endpoint) -> AnyId {
    match end {
        Endpoint::Pin { symbol, .. } => symbol.into(),
        Endpoint::Junction { junction } => junction.into(),
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
    for (wire, both) in wires {
        if both {
            tx.update::<Wire>(wire, |w| {
                for p in &mut w.route {
                    *p = how.point(*p);
                }
            })?;
        } else {
            reroute(tx, wire)?;
        }
    }
    Ok(())
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

/// Replaces a wire's bends with a fresh orthogonal route between its ends.
pub fn reroute(tx: &mut Tx<'_>, wire: WireId) -> Result<()> {
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
    let bends = route(
        a,
        endpoint_direction(project, w.from),
        b,
        endpoint_direction(project, w.to),
    );
    tx.update::<Wire>(wire, |w| w.route = bends)
}

/// Deletes objects and tidies up behind them: wires on deleted pins go too,
/// a junction left with two wires joins them into one, a junction left with
/// none disappears, and devices and terminals no longer drawn anywhere are
/// removed.
pub fn delete(tx: &mut Tx<'_>, ids: &[AnyId]) -> Result<()> {
    let mut pages: BTreeSet<PageId> = BTreeSet::new();
    let mut devices: BTreeSet<DeviceId> = BTreeSet::new();
    let mut terminals: BTreeSet<TerminalId> = BTreeSet::new();
    for id in ids {
        let project = tx.project();
        if !project.contains(*id) {
            continue;
        }
        if let Some(record) = project.record(*id) {
            pages.extend(record.page());
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
    for page in pages {
        tidy_junctions(tx, page)?;
    }
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

/// Removes junctions without wires and joins the two wires of a junction
/// that is no longer a T.
pub fn tidy_junctions(tx: &mut Tx<'_>, page: PageId) -> Result<()> {
    for j in tx.project().page_contents(page).junctions {
        let end = Endpoint::junction(j);
        let wires: Vec<WireId> = tx.project().wires_at(end).collect();
        match wires.as_slice() {
            [] => tx.remove(j)?,
            [a, b] if a != b => join_wires(tx, j, *a, *b)?,
            _ => {}
        }
    }
    Ok(())
}

fn join_wires(tx: &mut Tx<'_>, junction: JunctionId, keep: WireId, other: WireId) -> Result<()> {
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
        return Ok(()); // a loop back to the same point; leave it alone
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
    tx.remove(junction)
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
