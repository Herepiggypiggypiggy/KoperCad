//! Shorts that the potential definitions alone do not show: a protective
//! earth or neutral pin on a net that carries a phase, and two conductors
//! met by the poles of one device.
//!
//! [`ProblemKind::PotentialConflict`] reports nets where two potentials are
//! *defined* (two feeds on one wire). A wire bridge downstream of a breaker
//! defines nothing: both phases reach the net through switched poles, and a
//! net reached through switched poles is also how a changeover or a
//! reversing starter is drawn. These checks tell the two apart where the
//! drawing allows it, and say plainly where it does not:
//!
//! 1. A pin named `PE` or `N` (a motor, a converter, a supply) on a net that
//!    defines or can be reached by a phase or a DC positive. Protective
//!    earth is never switched, so a phase reaching it through a pole is a
//!    fault as well.
//! 2. Two poles of one multi-pole protective, switching or contactor symbol
//!    (they close together) that are joined on one side while their other
//!    sides carry different conductors: the device then joins them. A
//!    conductor counts on a side only when it gets there through fuses,
//!    breakers and switches (devices that are normally closed); one that
//!    arrives through a contactor may not be there when the poles close, as
//!    at the star point of a star-delta starter, and is left alone.
//!
//! Not found: two phases joined on a net that they reach through *separate*
//! devices (two single-pole fuses, two contactors), because that wiring is
//! how a changeover is drawn and the drawing does not say whether the
//! devices can close together. Those are left to a person or to the live
//! simulation.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, Endpoint, NetId, Netlist, Potential, PotentialId, PotentialKind, Project, SymbolId,
    SymbolInstance, SymbolRole, WireId,
};

use crate::{Context, Problem, ProblemKind};

/// Symbol library folders whose poles all close together: protective
/// devices, main switches and contactor main contacts. Changeover switches
/// and auxiliary contacts are left out on purpose.
fn closes_together(symbol: &SymbolInstance) -> bool {
    let key = symbol.symbol.0.as_str();
    let folder = ["/protection/", "/contactors/", "/switches/"]
        .iter()
        .any(|f| key.contains(f));
    folder && !key.contains("changeover")
}

pub(crate) fn run(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    named_conductors(cx, netlist, out);
    bridged_poles(cx, netlist, out);
}

/// What a pin's designation says it must be joined to.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Named {
    Earth,
    Neutral,
}

impl Named {
    fn what(self) -> &'static str {
        match self {
            Named::Earth => "Protective earth",
            Named::Neutral => "Neutral",
        }
    }
}

fn named_pin(symbol: &SymbolInstance, pin: usize) -> Option<Named> {
    if !matches!(
        symbol.role,
        SymbolRole::Device { .. } | SymbolRole::Unassigned
    ) {
        return None;
    }
    match symbol.pins.get(pin)?.name.to_uppercase().as_str() {
        "PE" => Some(Named::Earth),
        "N" => Some(Named::Neutral),
        _ => None,
    }
}

/// Potentials a net defines, carries or can reach, each once.
fn conductors(net: &novo_core::Net) -> BTreeSet<PotentialId> {
    net.defined
        .iter()
        .chain(&net.carried)
        .chain(&net.reachable)
        .copied()
        .collect()
}

fn is_live(kind: PotentialKind) -> bool {
    matches!(kind, PotentialKind::Line | PotentialKind::DcPositive)
}

fn list(names: &[String]) -> String {
    match names.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => names.join(""),
    }
}

/// Where a problem on `net` is drawn: the wires of the net that touch the
/// given symbols, else the net's own wires (up to a handful).
fn wires_at(project: &Project, net: &novo_core::Net, symbols: &[SymbolId]) -> Vec<AnyId> {
    let touching: Vec<WireId> = net
        .wires
        .iter()
        .copied()
        .filter(|&w| {
            project.wire(w).is_some_and(|w| {
                [w.from, w.to]
                    .iter()
                    .any(|e| matches!(e, Endpoint::Pin { symbol, .. } if symbols.contains(symbol)))
            })
        })
        .collect();
    let wires = if touching.is_empty() {
        net.wires.iter().copied().take(4).collect()
    } else {
        touching
    };
    wires.into_iter().map(AnyId::Wire).collect()
}

fn page_names(cx: &Context<'_>, netlist: &Netlist, net: NetId) -> String {
    let names: Vec<String> = netlist
        .pages_of_net(cx.project, net)
        .into_iter()
        .filter_map(|p| cx.project.page(p).map(|p| p.name.clone()))
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!(
            " (page{} {})",
            if names.len() == 1 { "" } else { "s" },
            names.join(", ")
        )
    }
}

fn named_conductors(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    // Nets with two defined potentials are reported as a conflict already.
    let conflicts: BTreeSet<NetId> = netlist.potential_conflicts().map(|n| n.id).collect();
    let mut named: BTreeMap<NetId, BTreeMap<Named, Vec<SymbolId>>> = BTreeMap::new();
    for symbol in project.symbols() {
        for pin in 0..symbol.pins.len() {
            let Some(what) = named_pin(symbol, pin) else {
                continue;
            };
            let Ok(pin) = u16::try_from(pin) else {
                continue;
            };
            if let Some(net) = netlist.net_of(Endpoint::pin(symbol.id, pin)) {
                named
                    .entry(net)
                    .or_default()
                    .entry(what)
                    .or_default()
                    .push(symbol.id);
            }
        }
    }
    for (net_id, by_name) in named {
        if conflicts.contains(&net_id) {
            continue;
        }
        let net = netlist.net(net_id);
        let potentials: Vec<&Potential> = conductors(net)
            .into_iter()
            .filter_map(|p| project.potential(p))
            .collect();
        for (what, symbols) in by_name {
            let mut live: Vec<String> = potentials
                .iter()
                .filter(|p| is_live(p.kind))
                .map(|p| p.name.clone())
                .collect();
            live.sort();
            if live.is_empty() {
                continue;
            }
            let mut symbols = symbols;
            symbols.sort();
            symbols.dedup();
            let labels: Vec<String> = symbols
                .iter()
                .filter_map(|&s| project.symbol(s))
                .map(|s| cx.symbol_label(s))
                .collect();
            let mut at: Vec<AnyId> = symbols.iter().map(|&s| AnyId::Symbol(s)).collect();
            at.extend(wires_at(project, net, &symbols));
            out.push(Problem::new(
                ProblemKind::PotentialConflict,
                at,
                format!(
                    "{} pin of {} is on a net with {}{}: {} is joined to a live conductor",
                    what.what(),
                    list(&labels),
                    list(&live),
                    page_names(cx, netlist, net_id),
                    what.what().to_lowercase(),
                ),
            ));
        }
    }
}

/// Whether two conductors must never meet.
fn clash(a: &Potential, b: &Potential) -> bool {
    use PotentialKind::{DcNegative, DcPositive, Line, Neutral, Pen, ProtectiveEarth};
    if a.id == b.id {
        return false;
    }
    let pair = |x: PotentialKind, y: PotentialKind| {
        (a.kind == x && b.kind == y) || (a.kind == y && b.kind == x)
    };
    (a.kind == Line && b.kind == Line)
        || pair(Line, Neutral)
        || pair(Line, ProtectiveEarth)
        || pair(Line, Pen)
        || pair(Line, DcPositive)
        || pair(Line, DcNegative)
        || (a.kind == DcPositive && b.kind == DcPositive)
        || pair(DcPositive, DcNegative)
        || pair(DcPositive, ProtectiveEarth)
        || pair(DcPositive, Neutral)
        || pair(DcPositive, Pen)
}

/// Whether a symbol's poles are normally closed, so a conductor passes
/// through them for good: fuses, breakers and switches. Contactors and
/// contacts are left out on purpose, and so are changeover switches.
fn normally_closed(symbol: &SymbolInstance) -> bool {
    let key = symbol.symbol.0.as_str();
    (key.contains("/protection/") || key.contains("/switches/")) && !key.contains("changeover")
}

/// The one conductor each net is permanently on: the potential it defines,
/// or the one that reaches it through normally closed poles only. A net two
/// different conductors reach has none.
fn permanent_conductors(project: &Project, netlist: &Netlist) -> BTreeMap<NetId, PotentialId> {
    let mut edges: BTreeMap<NetId, Vec<NetId>> = BTreeMap::new();
    for symbol in project.symbols().filter(|s| normally_closed(s)) {
        for group in &symbol.pass_through {
            let nets: Vec<NetId> = group
                .iter()
                .filter_map(|&pin| netlist.net_of(Endpoint::pin(symbol.id, pin)))
                .collect();
            for &a in &nets {
                for &b in &nets {
                    if a != b {
                        edges.entry(a).or_default().push(b);
                    }
                }
            }
        }
    }
    let mut reached: BTreeMap<NetId, BTreeSet<PotentialId>> = BTreeMap::new();
    for net in netlist.nets() {
        let [potential] = net.defined.as_slice() else {
            continue;
        };
        // Walk outward; a net that defines its own potential stops the walk.
        let mut queue = vec![net.id];
        let mut seen: BTreeSet<NetId> = BTreeSet::from([net.id]);
        reached.entry(net.id).or_default().insert(*potential);
        while let Some(at) = queue.pop() {
            for &next in edges.get(&at).into_iter().flatten() {
                if !netlist.net(next).defined.is_empty() || !seen.insert(next) {
                    continue;
                }
                reached.entry(next).or_default().insert(*potential);
                queue.push(next);
            }
        }
    }
    reached
        .into_iter()
        .filter_map(
            |(net, set)| match set.iter().collect::<Vec<_>>().as_slice() {
                [single] => Some((net, **single)),
                _ => None,
            },
        )
        .collect()
}

fn bridged_poles(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let permanent = permanent_conductors(project, netlist);
    for symbol in project.symbols() {
        if !closes_together(symbol) || symbol.pass_through.len() < 2 {
            continue;
        }
        let net_of = |pin: u16| netlist.net_of(Endpoint::pin(symbol.id, pin));
        // The nets a bridge joined, with the potentials arriving at the other
        // sides of the poles it joined.
        let mut met: BTreeMap<NetId, BTreeSet<PotentialId>> = BTreeMap::new();
        for (i, g1) in symbol.pass_through.iter().enumerate() {
            for g2 in &symbol.pass_through[i + 1..] {
                if g1.iter().any(|p| g2.contains(p)) {
                    continue; // a changeover shares its common pin
                }
                for &a in g1 {
                    for &b in g2 {
                        let (Some(x), Some(y)) = (net_of(a), net_of(b)) else {
                            continue;
                        };
                        if x != y {
                            continue;
                        }
                        for &a2 in g1.iter().filter(|&&p| p != a) {
                            for &b2 in g2.iter().filter(|&&p| p != b) {
                                let (Some(n1), Some(n2)) = (net_of(a2), net_of(b2)) else {
                                    continue;
                                };
                                let (Some(&p1), Some(&p2)) =
                                    (permanent.get(&n1), permanent.get(&n2))
                                else {
                                    continue;
                                };
                                let (Some(p1), Some(p2)) =
                                    (project.potential(p1), project.potential(p2))
                                else {
                                    continue;
                                };
                                if clash(p1, p2) {
                                    let entry = met.entry(x).or_default();
                                    entry.insert(p1.id);
                                    entry.insert(p2.id);
                                }
                            }
                        }
                    }
                }
            }
        }
        for (net_id, potentials) in met {
            let mut names: Vec<String> = potentials
                .iter()
                .filter_map(|&p| project.potential(p))
                .map(|p| p.name.clone())
                .collect();
            names.sort();
            let label = cx.symbol_label(symbol);
            let mut at: Vec<AnyId> = vec![AnyId::Symbol(symbol.id)];
            at.extend(wires_at(project, netlist.net(net_id), &[symbol.id]));
            out.push(Problem::new(
                ProblemKind::PotentialConflict,
                at,
                format!(
                    "{} are joined by the poles of {label}{}: a wire connects two of its poles on \
                     one side while they carry different conductors, a short when it closes",
                    list(&names),
                    page_names(cx, netlist, net_id),
                ),
            ));
        }
    }
}
