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
    /// Terminal strip for terminal symbols; `None` joins the strip the
    /// terminal is drawn next to, else starts a strip with the next free
    /// `-X` number (see [`adjacent_strip`]).
    pub strip: Option<DeviceId>,
    /// Potential of a potential feed; `None` gives the first of `L1`, `L2`,
    /// `L3`, ... that no feed on the page carries yet (see [`next_feed_name`]).
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
                None => strip_for(tx, page, placement.origin)?,
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
                // The earth feed is PE, not the next line conductor.
                _ if symbol.id.ends_with("/potential_feed_pe") => {
                    find_or_add_potential(tx, "PE", PotentialKind::ProtectiveEarth, None)?
                }
                _ => {
                    let name = next_feed_name(tx.project(), page);
                    find_or_add_potential(tx, &name, PotentialKind::Line, None)?
                }
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
    } else {
        // Whatever the preference, a pin on an open wire end is that wire's end.
        join_coincident(tx, page)?;
    }
    Ok(id)
}

/// The name a new potential feed gets: the first of `L1`, `L2`, `L3`, ...
/// that no feed on `page` carries yet, so a row of feeds reads L1, L2, L3.
/// (Same rule as `novo_workflow::insert::next_feed_name`.)
pub fn next_feed_name(project: &Project, page: PageId) -> String {
    let taken: Vec<&str> = project
        .symbols()
        .filter(|s| s.page == page)
        .filter_map(|s| match s.role {
            SymbolRole::PotentialFeed { potential } => {
                project.potential(potential).map(|p| p.name.as_str())
            }
            _ => None,
        })
        .collect();
    (1u32..)
        .map(|n| format!("L{n}"))
        .find(|name| !taken.iter().any(|t| t.eq_ignore_ascii_case(name)))
        .expect("an unbounded range has a free name")
}

fn strip_exists(project: &Project, strip: DeviceId) -> bool {
    project
        .device(strip)
        .is_some_and(|d| d.kind == DeviceKind::TerminalStrip)
}

/// Terminals this close (mm between symbol origins) and in line belong to
/// one strip run, the same gap the canvas uses to name a strip once.
pub const STRIP_ADJACENT: f64 = 20.0;

/// How far off the line two terminals may sit and still be in line.
const IN_LINE: f64 = 2.0;

/// The strip of the terminal drawn next to `at` on the page: the nearest
/// terminal symbol within [`STRIP_ADJACENT`] along a row or a column.
/// `None` when the spot is not next to any strip.
pub fn adjacent_strip(project: &Project, page: PageId, at: Point) -> Option<DeviceId> {
    project
        .page_contents(page)
        .symbols
        .iter()
        .filter_map(|&s| project.symbol(s))
        .filter_map(|s| match &s.role {
            SymbolRole::Terminal { terminals } => {
                let strip = project.terminal(*terminals.first()?)?.strip;
                let o = s.placement.origin;
                let (dx, dy) = ((o.x - at.x).abs(), (o.y - at.y).abs());
                let near = (dy <= IN_LINE && dx <= STRIP_ADJACENT)
                    || (dx <= IN_LINE && dy <= STRIP_ADJACENT);
                near.then_some((dx + dy, strip))
            }
            _ => None,
        })
        .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
        .map(|(_, strip)| strip)
}

/// The strip a placed terminal symbol belongs to.
pub fn strip_of_symbol(project: &Project, symbol: SymbolId) -> Option<DeviceId> {
    match &project.symbol(symbol)?.role {
        SymbolRole::Terminal { terminals } => Some(project.terminal(*terminals.first()?)?.strip),
        _ => None,
    }
}

/// The strip a terminal placed at `at` joins: the one it is drawn next to
/// (a terminal directly left of a strip joins it too, and the strip's name
/// moves to it as the new leftmost). Anywhere else it starts a new strip
/// with the next free `-X` number in the page's structure.
fn strip_for(tx: &mut Tx<'_>, page: PageId, at: Point) -> Result<DeviceId> {
    if let Some(strip) = adjacent_strip(tx.project(), page, at) {
        return Ok(strip);
    }
    let structure = page_structure(tx.project(), page);
    let mut tag = structure.clone();
    tag.product.push("X1".into());
    let n = tx.project().next_tag_number(&structure, "X");
    tx.add_terminal_strip(tag.with_number(n))
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

/// Renames what a symbol's label names when it is not a device: its
/// terminal (and the others of the symbol that share the name), the
/// potential a feed carries (an existing one by that name, else a new one
/// of the same kind), or an interruption point. Refused with the reason
/// when the name is empty or already taken on the strip's level.
pub fn rename_label(
    tx: &mut Tx<'_>,
    symbol: SymbolId,
    name: &str,
) -> std::result::Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("A name cannot be empty".into());
    }
    let role = tx
        .project()
        .symbol(symbol)
        .map(|s| s.role.clone())
        .ok_or("The symbol no longer exists")?;
    let fail = |e: novo_core::ModelError| e.to_string();
    match role {
        SymbolRole::Terminal { terminals } => {
            let Some(first) = terminals.first().and_then(|t| tx.project().terminal(*t)) else {
                return Err("The terminal no longer exists".into());
            };
            let old = first.name.clone();
            if old == name {
                return Ok(());
            }
            let mine: Vec<_> = terminals
                .iter()
                .filter(|t| tx.project().terminal(**t).is_some_and(|t| t.name == old))
                .copied()
                .collect();
            for &t in &mine {
                let this = tx.project().terminal(t).ok_or("The terminal is gone")?;
                let taken = tx.project().terminals_of_strip(this.strip).iter().any(|o| {
                    o.id != t && !mine.contains(&o.id) && o.level == this.level && o.name == name
                });
                if taken {
                    return Err(format!("{name} is already used on this strip"));
                }
            }
            for t in mine {
                tx.update::<novo_core::Terminal>(t, |t| t.name = name.to_string())
                    .map_err(fail)?;
            }
            Ok(())
        }
        SymbolRole::PotentialFeed { potential } => {
            let (kind, voltage) = tx
                .project()
                .potential(potential)
                .map(|p| (p.kind, p.voltage))
                .ok_or("The potential no longer exists")?;
            let id = find_or_add_potential(tx, name, kind, voltage).map_err(fail)?;
            tx.set_role(symbol, SymbolRole::PotentialFeed { potential: id })
                .map_err(fail)
        }
        SymbolRole::Interruption { .. } => tx
            .set_role(
                symbol,
                SymbolRole::Interruption {
                    name: name.to_string(),
                },
            )
            .map_err(fail),
        _ => Err("This label cannot be renamed here".into()),
    }
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
    for page in symbols
        .iter()
        .filter_map(|&s| tx.project().symbol(s).map(|s| s.page))
        .collect::<BTreeSet<_>>()
    {
        join_coincident(tx, page)?;
    }
    Ok(())
}

/// How far apart (mm, each axis) an open wire end and a pin or junction may
/// be and still count as the same point. Coordinates that went through
/// rotation, mirroring or a unit conversion differ by rounding noise only;
/// a micrometre is far below anything a person can place.
pub const JOIN_TOLERANCE: f64 = 1e-3;

/// Links every open wire end on `page` that lies exactly on a pin or a
/// junction to it, so a wire and a terminal at the same position are one
/// connection and the open-end flag goes away. Returns how many ends were
/// joined.
///
/// A coincidence arises in many ways (a wire drawn to a pin, a part placed
/// or moved onto a wire end, a wire end dragged onto a pin, a project loaded
/// from a file or generated by a lesson), so this runs at the end of each of
/// those edits and when a project is opened ([`heal_project`]).
pub fn join_coincident(tx: &mut Tx<'_>, page: PageId) -> Result<usize> {
    let project = tx.project();
    let contents = project.page_contents(page);
    let mut joins: Vec<(WireId, bool, Endpoint)> = Vec::new();
    for &w in &contents.wires {
        let Some(wire) = project.wire(w) else {
            continue;
        };
        for (is_from, end, other) in [(true, wire.from, wire.to), (false, wire.to, wire.from)] {
            let Endpoint::Free { at } = end else {
                continue;
            };
            let junction = contents.junctions.iter().copied().find(|&j| {
                project
                    .junction(j)
                    .is_some_and(|j| j.position.approx_eq(at, JOIN_TOLERANCE))
            });
            let target = junction.map(Endpoint::from).or_else(|| {
                let mut found: Vec<Endpoint> = contents
                    .symbols
                    .iter()
                    .flat_map(|&s| {
                        pin_positions(project, s)
                            .into_iter()
                            .filter(move |&(_, q)| q.approx_eq(at, JOIN_TOLERANCE))
                            .map(move |(pin, _)| Endpoint::pin(s, pin))
                    })
                    .collect();
                // Of several pins on one point prefer one nothing is on.
                found.sort_by_key(|&e| !is_free(project, e));
                found.into_iter().next()
            });
            if let Some(target) = target
                && target != other
            {
                joins.push((w, is_from, target));
            }
        }
    }
    let count = joins.len();
    for (w, is_from, target) in joins {
        tx.update::<Wire>(w, |wire| {
            if is_from {
                wire.from = target;
            } else {
                wire.to = target;
            }
        })?;
    }
    Ok(count)
}

/// [`join_coincident`] on every page of a project that was just loaded or
/// generated: open ends that sit on a pin or junction are linked. This is
/// not an undo step (the project has no history yet); returns how many ends
/// were joined.
pub fn heal_project(project: &mut Project) -> Result<usize> {
    let pages: Vec<PageId> = project.pages().map(|p| p.id).collect();
    let (count, _) = project.transact("Join coincident wire ends", |tx| {
        let mut count = 0;
        for page in &pages {
            count += join_coincident(tx, *page)?;
        }
        Ok(count)
    })?;
    Ok(count)
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
        &[],
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
        &[],
    )
    .map_err(|r| refusal_error(Some(project), r))?;
    let wire = tx.connect(from, to)?;
    if !bends.is_empty() {
        tx.update::<Wire>(wire, |w| w.route = bends)?;
    }
    join_coincident(tx, page)?;
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
    transform_with(tx, ids, how, false, false)
}

/// [`transform`] for a person's drag or key press: where no adjustment keeps
/// the moved wires clear of other wires the whole move fails and changes
/// nothing (the transaction rolls back). Scripts, AI edits, undo and redo
/// use [`transform`], which accepts the overlap for `wire.overlap` to report.
pub fn transform_interactive(tx: &mut Tx<'_>, ids: &[AnyId], how: Transform) -> Result<()> {
    transform_with(tx, ids, how, true, false)
}

/// [`transform_interactive`] for dragging devices away: the wires on the
/// moved pins stay where they are and end in the air there (open ends,
/// DESIGN.md §2), instead of stretching along. Wires between two moved
/// symbols and wires that are selected too move with them. Dropping the
/// device on a wire or on facing pins reconnects it ([`settle`]).
pub fn transform_detached(tx: &mut Tx<'_>, ids: &[AnyId], how: Transform) -> Result<()> {
    transform_with(tx, ids, how, true, true)
}

/// How many wire ends are open (in the air) on the whole project.
pub fn open_end_count(project: &Project) -> usize {
    project
        .wires()
        .flat_map(|w| w.ends())
        .filter(|e| matches!(e, Endpoint::Free { .. }))
        .count()
}

/// How many wire ends a detaching move of `ids` would leave in the air: the
/// ends on moved symbols' pins whose wire is not carried along with both
/// ends moving or selected.
pub fn detaching_ends(project: &Project, ids: &[AnyId]) -> usize {
    let symbols: BTreeSet<SymbolId> = symbols_in(ids).into_iter().collect();
    project
        .wires()
        .filter(|w| !ids.contains(&AnyId::Wire(w.id)))
        .map(|w| {
            let on_moved = |e: Endpoint| {
                matches!(e, Endpoint::Pin { symbol, .. } if symbols.contains(&symbol))
            };
            let [a, b] = w.ends();
            if on_moved(a) && on_moved(b) {
                0
            } else {
                usize::from(on_moved(a)) + usize::from(on_moved(b))
            }
        })
        .sum()
}

fn transform_with(
    tx: &mut Tx<'_>,
    ids: &[AnyId],
    how: Transform,
    refuse: bool,
    detach: bool,
) -> Result<()> {
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
    if detach {
        detach_pins(tx, ids, &symbols)?;
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
    // A moved pin or junction that now sits on an open wire end owns it.
    let pages: BTreeSet<PageId> = symbols
        .iter()
        .filter_map(|&s| tx.project().symbol(s).map(|s| s.page))
        .chain(
            junctions
                .iter()
                .filter_map(|&j| tx.project().junction(j).map(|j| j.page)),
        )
        .collect();
    for page in pages {
        join_coincident(tx, page)?;
    }
    Ok(())
}

/// Frees the ends of wires on the pins of `symbols` where they are, unless
/// the wire moves with them (its other end moves too, or it is selected).
fn detach_pins(tx: &mut Tx<'_>, ids: &[AnyId], symbols: &BTreeSet<SymbolId>) -> Result<()> {
    let on_moved =
        |e: &Endpoint| matches!(e, Endpoint::Pin { symbol, .. } if symbols.contains(symbol));
    let mut todo: Vec<(WireId, Endpoint, Endpoint)> = Vec::new();
    for w in tx.project().wires() {
        let carried = ids.contains(&AnyId::Wire(w.id));
        let [a, b] = w.ends();
        let both_on = on_moved(&a) && on_moved(&b);
        if carried || both_on || !(on_moved(&a) || on_moved(&b)) {
            continue;
        }
        let free = |e: Endpoint| {
            if on_moved(&e) {
                tx.project()
                    .endpoint_position(e)
                    .map_or(e, |at| Endpoint::Free { at })
            } else {
                e
            }
        };
        todo.push((w.id, free(a), free(b)));
    }
    for (wire, from, to) in todo {
        tx.update::<Wire>(wire, |w| {
            w.from = from;
            w.to = to;
        })?;
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

/// Takes the bend at `index` out of a wire's route (a selected corner,
/// Delete). The wire then runs straight between its neighbours; a result
/// that would share a stretch with another wire is refused.
pub fn remove_corner(tx: &mut Tx<'_>, wire: WireId, index: usize) -> Result<()> {
    let Some(w) = tx.project().wire(wire) else {
        return Ok(());
    };
    if index >= w.route.len() {
        return Ok(());
    }
    tx.update::<Wire>(wire, |w| {
        w.route.remove(index);
    })?;
    check_kept(tx.project(), wire, &[wire])
}

/// The part of a wire that is dragged (the canvas's `WirePartKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartKind {
    /// A bend: index into the route.
    Corner(usize),
    /// A straight run: 0 leaves the first end, the last reaches the second.
    Segment(usize),
}

/// The route indices a dragged wire part moves: a corner moves itself; a
/// run moves the corners at its ends (an end of the wire is not a corner).
pub fn part_corners(kind: PartKind, route_len: usize) -> Vec<usize> {
    match kind {
        PartKind::Corner(i) => vec![i],
        PartKind::Segment(k) => [k.checked_sub(1), Some(k)]
            .into_iter()
            .flatten()
            .filter(|&i| i < route_len)
            .collect(),
    }
}

/// How far a wire part dragged from `from` to `to` moves, snapped to the
/// grid. A corner is free. A run keeps its direction: an inner run slides
/// across itself, a run that reaches an end of the wire along itself, so
/// no slanted piece appears. A run with no corner to move stays put.
pub fn part_delta(
    project: &Project,
    wire: WireId,
    kind: PartKind,
    grid: f64,
    from: Point,
    to: Point,
) -> Point {
    let delta = geometry::snap(to, grid) - geometry::snap(from, grid);
    let PartKind::Segment(k) = kind else {
        return delta;
    };
    let Some(pts) = wire_points(project, wire) else {
        return Point::ZERO;
    };
    if k + 1 >= pts.len() || part_corners(kind, pts.len() - 2).is_empty() {
        return Point::ZERO;
    }
    let (a, b) = (pts[k], pts[k + 1]);
    let vertical = (a.x - b.x).abs() < 1e-6;
    let horizontal = (a.y - b.y).abs() < 1e-6;
    let end = k == 0 || k + 2 == pts.len();
    match (vertical, horizontal, end) {
        (true, _, false) | (_, true, true) => Point::new(delta.x, 0.0),
        (_, true, false) | (true, _, true) => Point::new(0.0, delta.y),
        _ => delta,
    }
}

/// A wire's route from `a` to `b` with a dragged part moved by `delta`.
/// Stretches that were horizontal or vertical stay so: where a moved corner
/// would leave one slanted, an elbow is added next to the corner that did
/// not move, so the stretch keeps its direction at the fixed end. Stretches
/// that were slanted to begin with (a wire drawn in Lines mode) are left
/// alone, unless `square` (Corners mode) asks for every stretch at a moved
/// corner to end up horizontal or vertical.
pub fn moved_route(
    a: Point,
    route: &[Point],
    b: Point,
    kind: PartKind,
    delta: Point,
    square: bool,
) -> Vec<Point> {
    const EPS: f64 = 1e-6;
    let aligned = |u: Point, v: Point| (u.x - v.x).abs() < EPS || (u.y - v.y).abs() < EPS;
    let mut old = vec![a];
    old.extend_from_slice(route);
    old.push(b);
    let moved: Vec<usize> = part_corners(kind, route.len())
        .into_iter()
        .map(|i| i + 1)
        .collect();
    let new: Vec<Point> = old
        .iter()
        .enumerate()
        .map(|(i, &p)| if moved.contains(&i) { p + delta } else { p })
        .collect();
    let mut out: Vec<Point> = Vec::new();
    for i in 0..new.len() - 1 {
        if i > 0 {
            out.push(new[i]);
        }
        let (u, v) = (new[i], new[i + 1]);
        if aligned(u, v) || !(square || aligned(old[i], old[i + 1])) {
            continue;
        }
        let horizontal = (old[i].y - old[i + 1].y).abs() < EPS;
        let elbow = match (moved.contains(&i), moved.contains(&(i + 1))) {
            (false, _) if horizontal => Point::new(v.x, u.y),
            (false, _) => Point::new(u.x, v.y),
            (true, false) if horizontal => Point::new(u.x, v.y),
            (true, false) => Point::new(v.x, u.y),
            (true, true) => continue,
        };
        out.push(elbow);
    }
    // Drop bends that landed on their neighbour or on an end.
    let mut route: Vec<Point> = Vec::new();
    for p in out {
        let prev = route.last().copied().unwrap_or(a);
        if (p.x - prev.x).abs() > EPS || (p.y - prev.y).abs() > EPS {
            route.push(p);
        }
    }
    while route
        .last()
        .is_some_and(|p| (p.x - b.x).abs() < EPS && (p.y - b.y).abs() < EPS)
    {
        route.pop();
    }
    route
}

/// Moves a dragged wire part by `delta`; a result that would share a
/// stretch with another wire is refused.
pub fn move_wire_part(
    tx: &mut Tx<'_>,
    wire: WireId,
    kind: PartKind,
    delta: Point,
    square: bool,
) -> Result<()> {
    let Some(w) = tx.project().wire(wire) else {
        return Ok(());
    };
    let (Some(a), Some(b)) = (
        tx.project().endpoint_position(w.from),
        tx.project().endpoint_position(w.to),
    ) else {
        return Ok(());
    };
    let route = moved_route(a, &w.route, b, kind, delta, square);
    tx.update::<Wire>(wire, |w| w.route = route)?;
    check_kept(tx.project(), wire, &[wire])
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
        &[wire],
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
    #[test]
    fn a_dragged_part_never_leaves_a_slanted_stretch_on_an_orthogonal_wire() {
        let p = |x: f64, y: f64| Point::new(x, y);
        let routes: Vec<(Point, Vec<Point>, Point)> = vec![
            (p(0.0, 0.0), vec![p(0.0, 20.0)], p(40.0, 20.0)),
            (
                p(0.0, 0.0),
                vec![p(0.0, 20.0), p(40.0, 20.0)],
                p(40.0, 50.0),
            ),
            (
                p(0.0, 0.0),
                vec![p(0.0, 10.0), p(30.0, 10.0), p(30.0, 40.0), p(60.0, 40.0)],
                p(60.0, 70.0),
            ),
            (p(0.0, 0.0), vec![p(25.0, 0.0)], p(25.0, 30.0)),
            (p(0.0, 0.0), vec![], p(30.0, 0.0)),
        ];
        let aligned = |u: Point, v: Point| (u.x - v.x).abs() < 1e-6 || (u.y - v.y).abs() < 1e-6;
        for (a, route, b) in routes {
            let mut kinds: Vec<PartKind> = (0..route.len()).map(PartKind::Corner).collect();
            kinds.extend((0..=route.len()).map(PartKind::Segment));
            for kind in kinds {
                for dx in [-15.0, -5.0, 0.0, 5.0, 20.0] {
                    for dy in [-15.0, -5.0, 0.0, 5.0, 20.0] {
                        let moved = moved_route(a, &route, b, kind, p(dx, dy), false);
                        let mut pts = vec![a];
                        pts.extend(moved.iter().copied());
                        pts.push(b);
                        assert!(
                            pts.windows(2).all(|s| aligned(s[0], s[1])),
                            "{kind:?} by ({dx}, {dy}) on {route:?} gave {pts:?}"
                        );
                    }
                }
            }
        }
    }

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
