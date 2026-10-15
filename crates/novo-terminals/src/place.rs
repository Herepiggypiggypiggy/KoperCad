//! Putting terminals into wires (WORKFLOW.md J5): `T`, then a click on a
//! wire or a drag across several.
//!
//! Each drop cuts the wire where it was hit and puts a terminal in the gap.
//! The terminal's internal side (`1`) goes to the top on a vertical wire and
//! to the left on a horizontal one. The wire part from the wire's start to
//! the terminal keeps the wire's id, number and cable core; the part from
//! the terminal to the wire's end is a new wire. That kept part is on the
//! internal side when the wire runs top to bottom (left to right), and on
//! the external side when it runs the other way; [`Placed`] says which.

use novo_core::{
    DeviceId, DeviceKind, DeviceTag, Direction, Endpoint, ModelError, Netlist, PageId, Pin,
    Placement, Point, PotentialKind, Project, Result, Rotation, SameName, SymbolKey, SymbolRole,
    Terminal, TerminalId, TerminalKind, Tx, Wire, WireId,
};

use crate::geometry::symbol_pins;
use crate::naming::{
    base_name_for_potential, kind_for_name, kind_for_potential, next_free_name, next_free_number,
    next_order,
};
use crate::rules::rules;
use crate::strings;
use crate::style::strip_style;

/// A click on a wire, or one wire crossed by a drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drop {
    pub wire: WireId,
    /// Where the wire was hit; the terminal centres on the nearest point of
    /// the wire.
    pub at: Point,
}

/// A terminal put into a wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placed {
    pub terminal: TerminalId,
    pub symbol: novo_core::SymbolId,
    /// The wire part on the internal side.
    pub internal_wire: WireId,
    /// The wire part on the external side. One of the two is the original
    /// wire (the part at its start), the other a new wire.
    pub external_wire: WireId,
    /// The drop reused a terminal of the same name (the "join" rule).
    pub joined: bool,
}

/// Connection points of a terminal symbol: `1` at the origin pointing up,
/// `2` straight below it at `length` pointing down (novo-symbols geometry).
/// Placing and redrawing read the actual symbol with
/// [`crate::symbol_pins`]; this is for drawing a terminal by hand.
pub fn terminal_pins(length: f64) -> Vec<Pin> {
    vec![
        Pin::new("1", 0.0, 0.0).with_direction(Direction::Up),
        Pin::new("2", 0.0, length).with_direction(Direction::Down),
    ]
}

/// Length of the built-in terminal symbol of a kind from `1` to `2`, in
/// millimetres.
pub fn terminal_length(kind: TerminalKind) -> f64 {
    match kind {
        TerminalKind::Fuse | TerminalKind::Disconnect => 16.0,
        _ => 8.0,
    }
}

/// The strip `T` places on: the one last used on the page, else an existing
/// `-X1`, else a new `-X1`.
pub fn strip_in_use(tx: &mut Tx<'_>, page: PageId) -> Result<DeviceId> {
    let last = tx
        .project()
        .symbols()
        .filter(|s| s.page == page)
        .filter_map(|s| match &s.role {
            SymbolRole::Terminal { terminals } => terminals.first().map(|&t| (s.id, t)),
            _ => None,
        })
        .max_by_key(|&(symbol, _)| symbol)
        .and_then(|(_, t)| tx.project().terminal(t))
        .map(|t| t.strip);
    match last {
        Some(strip) => Ok(strip),
        None => strip_by_tag(tx, "-X1"),
    }
}

/// The strip with this tag (`X2` and `-X2` both work), created when missing.
pub fn strip_by_tag(tx: &mut Tx<'_>, tag: &str) -> Result<DeviceId> {
    let text = tag.trim();
    let text = if text.starts_with(['-', '=', '+']) {
        text.to_owned()
    } else {
        format!("-{text}")
    };
    let tag = DeviceTag::parse(&text)?;
    let existing = tx
        .project()
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip && d.tag == tag)
        .map(|d| d.id)
        .min();
    match existing {
        Some(strip) => Ok(strip),
        None => tx.add_terminal_strip(tag),
    }
}

/// The potential kind of a wire's net, as the netlist assigns it. Under
/// novo-core's `rules.potentials.from_terminal_names` that includes a
/// terminal named like a supply potential (`-X4:PE`) on the net.
fn potential_of_wire(project: &Project, netlist: &Netlist, wire: &Wire) -> Option<PotentialKind> {
    let net = netlist.net(netlist.net_of_wire(wire)?);
    project.potential(net.potential()?).map(|p| p.kind)
}

fn polyline(project: &Project, wire: &Wire) -> Option<Vec<Point>> {
    let mut points = vec![project.endpoint_position(wire.from)?];
    points.extend(&wire.route);
    points.push(project.endpoint_position(wire.to)?);
    Some(points)
}

fn closest_on_segment(p: Point, a: Point, b: Point) -> Point {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 == 0.0 {
        return a;
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    Point::new(a.x + t * dx, a.y + t * dy)
}

/// Where a terminal of `length` fits on a wire near `at`: the segment index,
/// the placement, and whether pin `1` faces the start of the wire.
fn fit(points: &[Point], at: Point, length: f64, wire: WireId) -> Result<(usize, Placement, bool)> {
    let segment = (0..points.len() - 1)
        .min_by(|&i, &j| {
            let di = closest_on_segment(at, points[i], points[i + 1]).distance(at);
            let dj = closest_on_segment(at, points[j], points[j + 1]).distance(at);
            di.total_cmp(&dj)
        })
        .unwrap_or(0);
    let (a, b) = (points[segment], points[segment + 1]);
    let vertical = (a.x - b.x).abs() < 1e-9;
    let horizontal = (a.y - b.y).abs() < 1e-9;
    if !vertical && !horizontal || a.distance(b) <= length {
        return Err(ModelError::Format(strings::no_room(wire, length)));
    }
    let centre = closest_on_segment(at, a, b);
    // Keep both pins on the segment.
    let half = length / 2.0;
    let clamp = |v: f64, lo: f64, hi: f64| v.clamp(lo.min(hi) + half, lo.max(hi) - half);
    let centre = if vertical {
        Point::new(a.x, clamp(centre.y, a.y, b.y))
    } else {
        Point::new(clamp(centre.x, a.x, b.x), a.y)
    };
    // Pin 1 at the top (vertical) or left (horizontal).
    let (rotation, pin1) = if vertical {
        (Rotation::R0, Point::new(centre.x, centre.y - half))
    } else {
        // A quarter turn maps the symbol's downward axis to the right.
        (Rotation::R90, Point::new(centre.x - half, centre.y))
    };
    // Pin 1 is the top (left) end, so it faces the start of the wire when
    // the wire runs down (right) here, wherever on the segment it lands.
    let pin1_faces_start = if vertical { a.y <= b.y } else { a.x <= b.x };
    Ok((
        segment,
        Placement {
            origin: pin1,
            rotation,
            mirrored: false,
        },
        pin1_faces_start,
    ))
}

/// Cuts `drop.wire` and puts a drawing of `terminal` in the gap.
fn insert_into_wire(
    tx: &mut Tx<'_>,
    drop: Drop,
    terminal: TerminalId,
    symbol: SymbolKey,
) -> Result<(novo_core::SymbolId, WireId, WireId)> {
    let pins = symbol_pins(&symbol)?;
    let length = pins[1].offset.y;
    let original = tx
        .project()
        .wire(drop.wire)
        .ok_or(ModelError::NotFound(drop.wire.into()))?
        .clone();
    let points = polyline(tx.project(), &original).ok_or(ModelError::NotFound(drop.wire.into()))?;
    let (segment, placement, pin1_faces_start) = fit(&points, drop.at, length, drop.wire)?;
    let id = tx.place_symbol(
        original.page,
        symbol,
        placement,
        pins,
        SymbolRole::Terminal {
            terminals: vec![terminal],
        },
    )?;
    let (near, far) = if pin1_faces_start { (0, 1) } else { (1, 0) };
    tx.update::<Wire>(drop.wire, |w| {
        w.to = Endpoint::pin(id, near);
        w.route.truncate(segment);
    })?;
    let mut tail = original.clone();
    let second = tx.insert(|wid| {
        tail.id = wid;
        tail.from = Endpoint::pin(id, far);
        tail.route = original.route[segment..].to_vec();
        tail.number = None;
        tail
    })?;
    let (internal, external) = if pin1_faces_start {
        (drop.wire, second)
    } else {
        (second, drop.wire)
    };
    Ok((id, internal, external))
}

/// Puts one terminal per drop on `strip`, in drop order (the order a drag
/// crossed the wires), numbered next free. PE wires get a PE terminal named
/// `PE` (`PE2`, ... when taken), PEN wires a PE terminal named `PEN`, N
/// wires an N terminal named `N`.
pub fn place_terminals(tx: &mut Tx<'_>, strip: DeviceId, drops: &[Drop]) -> Result<Vec<Placed>> {
    let netlist = tx.project().netlist();
    let potentials: Vec<Option<PotentialKind>> = drops
        .iter()
        .map(|d| {
            tx.project()
                .wire(d.wire)
                .and_then(|w| potential_of_wire(tx.project(), &netlist, w))
        })
        .collect();
    let mut placed = Vec::with_capacity(drops.len());
    for (&drop, potential) in drops.iter().zip(potentials) {
        let kind = potential.map_or(TerminalKind::Feedthrough, kind_for_potential);
        let name = match potential.and_then(base_name_for_potential) {
            Some(base) => next_free_name(tx.project(), strip, base),
            None => next_free_number(tx.project(), strip),
        };
        placed.push(place_named(tx, strip, drop, &name, Some(kind))?);
    }
    Ok(placed)
}

/// Puts a terminal with a given name into a wire, following the project's
/// terminal rule ([`crate::rules`]):
///
/// - **Join**: a terminal of that name and level 1 already on the strip is
///   reused, so the drawings are one terminal record.
/// - **Refuse**: a name already on the strip is refused with novo-core's
///   [`ModelError::DuplicateTerminal`]; nothing is created.
/// - **Off**: a separate record is made, and [`crate::check`] lists it.
///
/// The kind of a new record comes from `kind`, else from the name
/// ([`crate::kind_for_name`]).
pub fn place_named(
    tx: &mut Tx<'_>,
    strip: DeviceId,
    drop: Drop,
    name: &str,
    kind: Option<TerminalKind>,
) -> Result<Placed> {
    let existing = match rules(tx.project()).same_name {
        SameName::Join => tx
            .project()
            .terminals_of_strip(strip)
            .into_iter()
            .find(|t| t.level == 1 && t.name == name)
            .map(|t| t.id),
        SameName::Refuse | SameName::Off => None,
    };
    let (terminal, joined) = match existing {
        Some(t) => (t, true),
        None => {
            let kind = kind
                .or_else(|| kind_for_name(tx.project(), name))
                .unwrap_or(TerminalKind::Feedthrough);
            let order = next_order(tx.project(), strip);
            // Under Refuse, novo-core rejects the record if the name is taken.
            let t = tx.add_terminal(strip, name)?;
            tx.update::<Terminal>(t, |t| {
                t.kind = kind;
                t.order = order;
            })?;
            (t, false)
        }
    };
    let kind = tx
        .project()
        .terminal(terminal)
        .map_or(TerminalKind::Feedthrough, |t| t.kind);
    let symbol = crate::style::style_of(tx.project(), terminal).map_or_else(
        || strip_style(tx.project(), strip, kind).symbol,
        |s| s.symbol,
    );
    let (symbol, internal_wire, external_wire) = insert_into_wire(tx, drop, terminal, symbol)?;
    Ok(Placed {
        terminal,
        symbol,
        internal_wire,
        external_wire,
        joined,
    })
}
