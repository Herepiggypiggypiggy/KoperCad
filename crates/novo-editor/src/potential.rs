//! Potentials on wires (WORKFLOW.md J10): select L1, L2 and L3, right click,
//! take "Three phase 400 V". PE, N and PEN are suggested from what a wire
//! touches.
//!
//! A potential is given to a net by a potential feed symbol on one of its
//! wires (the model's potential definition point), so it flows through
//! connected wires and terminals and changing it on one wire changes the
//! whole net.

use novo_core::{
    Endpoint, Netlist, PageId, Placement, Point, PotentialKind, Project, Result, Rotation,
    SymbolRole, TerminalKind, Tx, WireId,
};
use novo_symbols::Symbol;

use crate::edit::{self, PlaceOptions, find_or_add_potential};
use crate::geometry::{EPS, snap, wire_points};

/// Library id of the potential feed symbol.
pub const FEED_SYMBOL: &str = "iec/connectivity/potential_feed";

/// One potential to give one wire.
#[derive(Debug, Clone, PartialEq)]
pub struct PotentialSpec {
    pub name: String,
    pub kind: PotentialKind,
    /// Nominal voltage in volts.
    pub voltage: Option<f64>,
}

impl PotentialSpec {
    pub fn new(name: &str, kind: PotentialKind, voltage: Option<f64>) -> Self {
        PotentialSpec {
            name: name.into(),
            kind,
            voltage,
        }
    }
}

/// A right-click choice: one potential per selected wire, in page order.
#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub label: String,
    pub specs: Vec<PotentialSpec>,
}

fn line(name: &str, v: f64) -> PotentialSpec {
    PotentialSpec::new(name, PotentialKind::Line, Some(v))
}
fn pe() -> PotentialSpec {
    PotentialSpec::new("PE", PotentialKind::ProtectiveEarth, None)
}
fn n() -> PotentialSpec {
    PotentialSpec::new("N", PotentialKind::Neutral, None)
}
fn pen() -> PotentialSpec {
    PotentialSpec::new("PEN", PotentialKind::ProtectiveEarth, None)
}

/// Sorts wires the way they read on the page: parallel horizontal wires top
/// to bottom, parallel vertical wires left to right.
pub fn page_order(project: &Project, wires: &[WireId]) -> Vec<WireId> {
    let mut keyed: Vec<(WireId, Point, bool)> = wires
        .iter()
        .filter_map(|&w| {
            let pts = wire_points(project, w)?;
            // The longest segment decides the wire's direction.
            let (a, b) = pts
                .windows(2)
                .map(|s| (s[0], s[1]))
                .max_by(|x, y| x.0.distance(x.1).total_cmp(&y.0.distance(y.1)))?;
            let vertical = (b.y - a.y).abs() > (b.x - a.x).abs();
            Some((
                w,
                Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0),
                vertical,
            ))
        })
        .collect();
    let vertical = keyed.iter().filter(|k| k.2).count() * 2 > keyed.len();
    keyed.sort_by(|a, b| {
        if vertical {
            a.1.x.total_cmp(&b.1.x).then(a.1.y.total_cmp(&b.1.y))
        } else {
            a.1.y.total_cmp(&b.1.y).then(a.1.x.total_cmp(&b.1.x))
        }
    });
    keyed.into_iter().map(|k| k.0).collect()
}

/// The potential a net's connections imply: a pin or terminal named `PE`,
/// `N` or `PEN` (`-X4:PE`), or a PE or N terminal.
pub fn inferred(project: &Project, netlist: &Netlist, wire: WireId) -> Option<PotentialSpec> {
    let w = project.wire(wire)?;
    let net = netlist.net(netlist.net_of(w.from)?);
    for end in &net.endpoints {
        let Endpoint::Pin { symbol, pin } = *end else {
            continue;
        };
        let Some(sym) = project.symbol(symbol) else {
            continue;
        };
        let mut names: Vec<String> = Vec::new();
        if let Some(t) = sym.terminal_of_pin(pin).and_then(|t| project.terminal(t)) {
            match t.kind {
                TerminalKind::ProtectiveEarth => return Some(pe()),
                TerminalKind::Neutral => return Some(n()),
                _ => names.push(t.name.clone()),
            }
        } else if matches!(sym.role, SymbolRole::Device { .. } | SymbolRole::Unassigned) {
            names.extend(sym.pins.get(usize::from(pin)).map(|p| p.name.clone()));
        }
        for name in names {
            match name.to_uppercase().as_str() {
                "PE" => return Some(pe()),
                "PEN" => return Some(pen()),
                "N" => return Some(n()),
                _ => {}
            }
        }
    }
    None
}

/// Right-click suggestions for the selected wires, best first.
pub fn suggestions(project: &Project, wires: &[WireId]) -> Vec<Suggestion> {
    let s = |label: &str, specs: Vec<PotentialSpec>| Suggestion {
        label: label.into(),
        specs,
    };
    match wires.len() {
        1 => {
            let netlist = project.netlist();
            let mut out = Vec::new();
            if let Some(spec) = inferred(project, &netlist, wires[0]) {
                out.push(s(
                    &format!("{} (from its connection)", spec.name),
                    vec![spec],
                ));
            }
            for (label, spec) in [
                ("PE", pe()),
                ("N", n()),
                ("PEN", pen()),
                ("L 230 V", line("L", 230.0)),
                (
                    "+24 V DC",
                    PotentialSpec::new("24V", PotentialKind::DcPositive, Some(24.0)),
                ),
                (
                    "0 V DC",
                    PotentialSpec::new("0V", PotentialKind::DcNegative, Some(0.0)),
                ),
            ] {
                if !out
                    .iter()
                    .any(|o: &Suggestion| o.specs[0].name == spec.name)
                {
                    out.push(s(label, vec![spec]));
                }
            }
            out
        }
        2 => vec![
            s("230 V (L, N)", vec![line("L", 230.0), n()]),
            s(
                "24 V DC (+24 V, 0 V)",
                vec![
                    PotentialSpec::new("24V", PotentialKind::DcPositive, Some(24.0)),
                    PotentialSpec::new("0V", PotentialKind::DcNegative, Some(0.0)),
                ],
            ),
            s("230 V (L, PE)", vec![line("L", 230.0), pe()]),
        ],
        3 => vec![
            s(
                "Three phase 400 V (L1, L2, L3)",
                vec![line("L1", 400.0), line("L2", 400.0), line("L3", 400.0)],
            ),
            s("230 V (L, N, PE)", vec![line("L", 230.0), n(), pe()]),
        ],
        4 => vec![
            s(
                "Three phase 400 V + PE (L1, L2, L3, PE)",
                vec![
                    line("L1", 400.0),
                    line("L2", 400.0),
                    line("L3", 400.0),
                    pe(),
                ],
            ),
            s(
                "Three phase 400 V + N (L1, L2, L3, N)",
                vec![line("L1", 400.0), line("L2", 400.0), line("L3", 400.0), n()],
            ),
            s(
                "Three phase 400 V, PEN (L1, L2, L3, PEN)",
                vec![
                    line("L1", 400.0),
                    line("L2", 400.0),
                    line("L3", 400.0),
                    pen(),
                ],
            ),
        ],
        5 => vec![s(
            "Three phase 400 V + N + PE (L1, L2, L3, N, PE)",
            vec![
                line("L1", 400.0),
                line("L2", 400.0),
                line("L3", 400.0),
                n(),
                pe(),
            ],
        )],
        _ => Vec::new(),
    }
}

/// Gives each wire its potential: removes any potential feed already in its
/// net and hangs a new one on the wire, so the whole net changes at once.
/// `wires` and `specs` pair up in order.
pub fn assign(
    tx: &mut Tx<'_>,
    feed: &Symbol,
    wires: &[WireId],
    specs: &[PotentialSpec],
) -> Result<()> {
    for (&wire, spec) in wires.iter().zip(specs) {
        let Some(page) = tx.project().wire(wire).map(|w| w.page) else {
            continue;
        };
        let potential = find_or_add_potential(tx, &spec.name, spec.kind, spec.voltage)?;
        // Feeds already defining this net go; the new one replaces them.
        let netlist = tx.project().netlist();
        let w = tx.project().wire(wire).expect("checked above").clone();
        if let Some(net) = netlist.net_of(w.from) {
            let old: Vec<_> = netlist
                .net(net)
                .endpoints
                .iter()
                .filter_map(|e| match *e {
                    Endpoint::Pin { symbol, .. } => Some(symbol),
                    _ => None,
                })
                .filter(|&s| {
                    tx.project()
                        .symbol(s)
                        .is_some_and(|s| matches!(s.role, SymbolRole::PotentialFeed { .. }))
                })
                .collect();
            if !old.is_empty() {
                let ids: Vec<_> = old.into_iter().map(Into::into).collect();
                edit::delete(tx, &ids)?;
            }
        }
        // The wire may have been joined with another by the tidy-up; find
        // the wire at the same place again.
        let wire = if tx.project().wire(wire).is_some() {
            wire
        } else {
            let Some(p) = w
                .route
                .first()
                .copied()
                .or_else(|| tx.project().endpoint_position(w.from))
            else {
                continue;
            };
            match edit::wire_near(tx.project(), page, p, EPS) {
                Some(w) => w,
                None => continue,
            }
        };
        let Some(placement) = feed_placement(tx.project(), wire) else {
            continue;
        };
        let options = PlaceOptions {
            potential: Some(potential),
            ..PlaceOptions::new()
        };
        edit::place(tx, page, feed, placement, &options)?;
    }
    Ok(())
}

/// Where a feed goes on a wire: on its first segment, standing on the wire
/// from above. On a vertical wire the feed's arrow lies along the wire
/// itself, so feeds on parallel phases 8 mm apart do not collide; on a
/// horizontal wire it stands on the wire. The pin lands 12 mm in from the
/// start (on the 4 mm grid) or halfway along a shorter segment.
fn feed_placement(project: &Project, wire: WireId) -> Option<Placement> {
    let pts = wire_points(project, wire)?;
    let (seg_a, seg_b) = pts
        .windows(2)
        .map(|s| (s[0], s[1]))
        .find(|(a, b)| a.distance(*b) > 2.0 * EPS)?;
    let len = seg_a.distance(seg_b);
    let dir = Point::new((seg_b.x - seg_a.x) / len, (seg_b.y - seg_a.y) / len);
    let along = if len > 24.0 { 12.0 } else { len / 2.0 };
    let mut at = Point::new(seg_a.x + dir.x * along, seg_a.y + dir.y * along);
    let snapped = snap(at, 4.0);
    let inside = |p: Point| {
        let t = (p.x - seg_a.x) * dir.x + (p.y - seg_a.y) * dir.y;
        let off = ((p.x - seg_a.x) * dir.y - (p.y - seg_a.y) * dir.x).abs();
        t > EPS && t < len - EPS && off < EPS
    };
    if inside(snapped) {
        at = snapped;
    }
    // The feed's pin is 8 mm below its origin and points down.
    Some(Placement {
        origin: Point::new(at.x, at.y - 8.0),
        rotation: Rotation::R0,
        mirrored: false,
    })
}

/// The potential names of a wire's net, for status text.
pub fn wire_potential(project: &Project, netlist: &Netlist, wire: WireId) -> Option<String> {
    let w = project.wire(wire)?;
    let net = netlist.net(netlist.net_of(w.from)?);
    let names: Vec<&str> = net
        .potentials
        .iter()
        .filter_map(|p| project.potential(*p).map(|p| p.name.as_str()))
        .collect();
    (!names.is_empty()).then(|| names.join(" / "))
}

/// Wires on a page, for scripts that select by page.
pub fn page_wires(project: &Project, page: PageId) -> Vec<WireId> {
    project.page_contents(page).wires
}
