//! Checks of the kind EPLAN's project check runs, which KoperCAD did not
//! have yet. Sources are listed in `docs/design/eplan-nets.md`; the rules
//! are written from the behaviour the findings describe, not from EPLAN's
//! content.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{AnyId, Endpoint, Netlist, SymbolRole};

use crate::{Context, Problem, ProblemKind};

/// A name used by more than two interruption points. EPLAN pairs interruption
/// points two by two in page order, so a third point of one name has no
/// partner there; KoperCAD joins every point of a name. Either way the name
/// is ambiguous.
pub(crate) fn ambiguous_interruptions(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let mut by_name: BTreeMap<&str, Vec<AnyId>> = BTreeMap::new();
    for symbol in cx.project.symbols() {
        if let SymbolRole::Interruption { name } = &symbol.role {
            by_name
                .entry(name.as_str())
                .or_default()
                .push(symbol.id.into());
        }
    }
    for (name, symbols) in by_name {
        if symbols.len() > 2 {
            out.push(Problem::new(
                ProblemKind::AmbiguousInterruption,
                symbols.clone(),
                format!(
                    "Interruption point {name} is used {} times; interruption points go in pairs",
                    symbols.len()
                ),
            ));
        }
    }
}

/// A wire number on two nets, or two numbers on one net.
pub(crate) fn wire_numbers(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let mut by_number: BTreeMap<&str, BTreeMap<u32, Vec<AnyId>>> = BTreeMap::new();
    let mut by_net: BTreeMap<u32, BTreeMap<&str, Vec<AnyId>>> = BTreeMap::new();
    for wire in cx.project.wires() {
        let Some(number) = wire.number.as_deref().filter(|n| !n.is_empty()) else {
            continue;
        };
        let Some(net) = netlist.net_of_wire(wire) else {
            continue;
        };
        by_number
            .entry(number)
            .or_default()
            .entry(net.0)
            .or_default()
            .push(wire.id.into());
        by_net
            .entry(net.0)
            .or_default()
            .entry(number)
            .or_default()
            .push(wire.id.into());
    }
    for (number, nets) in by_number {
        if nets.len() > 1 {
            let at = nets.into_values().flatten().collect::<Vec<_>>();
            out.push(Problem::new(
                ProblemKind::WireNumber,
                at,
                format!("Wire number {number} is on more than one net"),
            ));
        }
    }
    for numbers in by_net.into_values() {
        if numbers.len() > 1 {
            let names: Vec<&str> = numbers.keys().copied().collect();
            let at = numbers.into_values().flatten().collect::<Vec<_>>();
            out.push(Problem::new(
                ProblemKind::WireNumber,
                at,
                format!("One net carries several wire numbers: {}", names.join(", ")),
            ));
        }
    }
}

/// A device without a product level (`-K1`): it cannot be told apart in
/// lists and cross references.
pub(crate) fn untagged_devices(cx: &Context<'_>, out: &mut Vec<Problem>) {
    for device in cx.project.devices() {
        if device.tag.product.is_empty() {
            let name = if device.tag.is_empty() {
                "A device".to_owned()
            } else {
                format!("Device {}", device.tag)
            };
            out.push(Problem::new(
                ProblemKind::UntaggedDevice,
                vec![device.id.into()],
                format!("{name} has no device designation"),
            ));
        }
    }
}

/// Two pages with one name in the same structure.
pub(crate) fn duplicate_pages(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let mut by_name: BTreeMap<(String, &str), Vec<AnyId>> = BTreeMap::new();
    for page in cx.project.pages() {
        by_name
            .entry((page.structure.to_string(), page.name.as_str()))
            .or_default()
            .push(page.id.into());
    }
    for ((structure, name), pages) in by_name {
        if pages.len() > 1 {
            let full = if structure.is_empty() {
                name.to_owned()
            } else {
                format!("{structure}/{name}")
            };
            out.push(Problem::new(
                ProblemKind::DuplicatePage,
                pages,
                format!("Page {full} exists more than once"),
            ));
        }
    }
}

/// A junction no wire ends on: a leftover dot that joins nothing (EPLAN
/// reports a node whose legs have no connection). A junction with a single
/// wire is not reported: it is a legitimate wire end, for example where a
/// bus is started.
pub(crate) fn dangling_junctions(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let used: BTreeSet<_> = cx
        .project
        .wires()
        .flat_map(|w| [w.from, w.to])
        .filter_map(|end| match end {
            Endpoint::Junction { junction } => Some(junction),
            _ => None,
        })
        .collect();
    for junction in cx.project.junctions() {
        if !used.contains(&junction.id) {
            out.push(Problem::new(
                ProblemKind::DanglingJunction,
                vec![junction.id.into()],
                "A junction with no wire joins nothing".to_owned(),
            ));
        }
    }
}
