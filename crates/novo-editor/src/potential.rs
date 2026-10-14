//! Potentials (DESIGN.md §2 and §9): declared once at a source and flowing
//! from there, never assigned wire by wire. Select the three incoming
//! conductors, right click, take "Declare source: 3~ 400 V (L1, L2, L3)".
//!
//! A source is a potential feed symbol on a wire (the model's potential
//! definition point), so the potential flows through connected wires,
//! contacts and terminals, and changing the source changes everything
//! downstream. Wires that already carry a potential from upstream show
//! what they carry instead; overriding one conductor waits for the model's
//! wire potential (WS-02). PE, N and PEN recognised from connection names
//! (`-X4:PE`) are automatic under a switchable rule, not a menu choice.

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

/// The potential part of the context menu for selected wires.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PotentialMenu {
    /// The selected wires in page order; `Suggestion::specs` pair with them.
    pub wires: Vec<WireId>,
    /// What the wires carry, shown first when any carries something
    /// ("Carries L1 · L2 · L3").
    pub carried: Option<String>,
    /// Sources to declare (or change, where the source sits on these very
    /// wires), best first. Empty when a wire's potential flows in from
    /// upstream: that is changed at its source.
    pub declare: Vec<Suggestion>,
}

/// Builds the potential menu for wires. `supply_names` is the rule that
/// recognises PE, N and PEN from connection names.
pub fn menu(project: &Project, wires: &[WireId], supply_names: bool) -> PotentialMenu {
    let wires = page_order(project, wires);
    if wires.is_empty() {
        return PotentialMenu::default();
    }
    let netlist = project.netlist();
    let mut names = Vec::new();
    let mut any = false;
    let mut upstream = false;
    let mut own_source = false;
    for &w in &wires {
        let has_feed = net_has_feed(project, &netlist, w);
        if let Some(name) = wire_potential(project, &netlist, w) {
            any = true;
            own_source |= has_feed;
            upstream |= !has_feed;
            names.push(name);
        } else if let Some(spec) = supply_names
            .then(|| inferred(project, &netlist, w))
            .flatten()
        {
            any = true;
            upstream = true;
            names.push(format!("{} (by name)", spec.name));
        } else {
            names.push("none".into());
        }
    }
    let carried = any.then(|| format!("Carries {}", names.join(" · ")));
    let declare = if upstream {
        Vec::new()
    } else {
        let verb = if own_source {
            "Change source"
        } else {
            "Declare source"
        };
        suggestions(wires.len())
            .into_iter()
            .map(|s| Suggestion {
                label: format!("{verb}: {}", s.label),
                ..s
            })
            .collect()
    };
    PotentialMenu {
        wires,
        carried,
        declare,
    }
}

/// Whether a wire's net holds a potential feed itself (its source is here).
fn net_has_feed(project: &Project, netlist: &Netlist, wire: WireId) -> bool {
    let Some(net) = project.wire(wire).and_then(|w| netlist.net_of(w.from)) else {
        return false;
    };
    netlist.net(net).endpoints.iter().any(|e| match *e {
        Endpoint::Pin { symbol, .. } => project
            .symbol(symbol)
            .is_some_and(|s| matches!(s.role, SymbolRole::PotentialFeed { .. })),
        _ => false,
    })
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

/// The standard sets for a number of wires, best first, unlabelled by
/// verb; [`menu`] decides whether they are offered.
pub fn suggestions(wires: usize) -> Vec<Suggestion> {
    let s = |label: &str, specs: Vec<PotentialSpec>| Suggestion {
        label: label.into(),
        specs,
    };
    match wires {
        1 => vec![
            s("PE", vec![pe()]),
            s("N", vec![n()]),
            s("PEN", vec![pen()]),
            s("L 230 V", vec![line("L", 230.0)]),
            s(
                "+24 V DC",
                vec![PotentialSpec::new(
                    "24V",
                    PotentialKind::DcPositive,
                    Some(24.0),
                )],
            ),
            s(
                "0 V DC",
                vec![PotentialSpec::new(
                    "0V",
                    PotentialKind::DcNegative,
                    Some(0.0),
                )],
            ),
        ],
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
                "3~ 400 V (L1, L2, L3)",
                vec![line("L1", 400.0), line("L2", 400.0), line("L3", 400.0)],
            ),
            s("230 V (L, N, PE)", vec![line("L", 230.0), n(), pe()]),
        ],
        4 => vec![
            s(
                "3~ 400 V + PE (L1, L2, L3, PE)",
                vec![
                    line("L1", 400.0),
                    line("L2", 400.0),
                    line("L3", 400.0),
                    pe(),
                ],
            ),
            s(
                "3~ 400 V + N (L1, L2, L3, N)",
                vec![line("L1", 400.0), line("L2", 400.0), line("L3", 400.0), n()],
            ),
            s(
                "3~ 400 V, PEN (L1, L2, L3, PEN)",
                vec![
                    line("L1", 400.0),
                    line("L2", 400.0),
                    line("L3", 400.0),
                    pen(),
                ],
            ),
        ],
        5 => vec![s(
            "3~ 400 V + N + PE (L1, L2, L3, N, PE)",
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
