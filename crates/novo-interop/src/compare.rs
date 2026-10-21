//! What an exchange kept and what it changed: two projects side by side.
//!
//! After an export and a re-import (or any import into an empty project) the
//! project that comes back should equal the one that went out, apart from
//! what the format cannot carry. [`compare_projects`] says exactly where the
//! two differ, per kind of thing, so a person moving a running project can
//! see it and a test can insist on it. Nothing is rounded away silently:
//! positions match to a tenth of a millimetre.
//!
//! Compared: devices (tag, description, kind, assigned articles), terminals,
//! cables and their cores, parts, pages (structure, name, title), drawn
//! symbols (page, artwork, owner, position) and nets (the designations that
//! are one conductor). Wire routes are not compared: only which pins they
//! join.

use std::collections::BTreeSet;

use novo_core::{Endpoint, Project, SymbolRole};

use crate::report::Report;

/// How the two projects differ.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comparison {
    /// What matched, and every kind of difference with a count and examples.
    pub report: Report,
}

impl Comparison {
    /// Nothing differs in what is compared.
    pub fn is_equal(&self) -> bool {
        self.report.lossy.is_empty()
    }
}

/// How many examples a difference line names.
const EXAMPLES: usize = 4;

fn examples(items: &[String]) -> String {
    let mut shown: Vec<&str> = items.iter().take(EXAMPLES).map(String::as_str).collect();
    if items.len() > EXAMPLES {
        shown.push("...");
    }
    shown.join(", ")
}

/// Compares sets of keys: what both have, only `before` has, only `after` has.
fn diff(
    report: &mut Report,
    what: &str,
    before: BTreeSet<String>,
    after: BTreeSet<String>,
) -> usize {
    let both = before.intersection(&after).count();
    let missing: Vec<String> = before.difference(&after).cloned().collect();
    let extra: Vec<String> = after.difference(&before).cloned().collect();
    report.moved(format!("{what} kept"), both);
    report.lost(
        format!("{what} missing or changed"),
        missing.len(),
        examples(&missing),
    );
    report.lost(
        format!("{what} added or changed"),
        extra.len(),
        examples(&extra),
    );
    missing.len() + extra.len()
}

fn mm(v: f64) -> i64 {
    (v * 10.0).round() as i64
}

fn page_label(p: &Project, id: novo_core::PageId) -> String {
    p.page(id).map_or_else(String::new, |page| {
        if page.structure.is_empty() {
            page.name.clone()
        } else {
            format!("{}/{}", page.structure, page.name)
        }
    })
}

fn devices(p: &Project) -> BTreeSet<String> {
    p.devices()
        .map(|d| {
            let mut parts: Vec<String> = d
                .parts
                .iter()
                .filter_map(|id| p.part(*id))
                .map(|x| format!("{}.{}", x.manufacturer, x.order_number))
                .collect();
            parts.sort();
            format!(
                "{} [{:?}] \"{}\" {}",
                d.tag,
                d.kind,
                d.description,
                parts.join("+")
            )
        })
        .collect()
}

fn terminals(p: &Project) -> BTreeSet<String> {
    p.terminals()
        .filter_map(|t| {
            p.terminal_designation(t.id)
                .map(|d| format!("{d} L{}", t.level))
        })
        .collect()
}

fn cables(p: &Project) -> BTreeSet<String> {
    p.cables()
        .map(|c| {
            let cores: Vec<String> = c.cores.iter().map(|x| x.name.clone()).collect();
            format!("{} cores {}", c.tag, cores.join("/"))
        })
        .collect()
}

fn parts(p: &Project) -> BTreeSet<String> {
    p.parts()
        .map(|x| format!("{}.{} {}", x.manufacturer, x.order_number, x.description))
        .collect()
}

fn pages(p: &Project) -> BTreeSet<String> {
    p.pages()
        .map(|x| format!("{} \"{}\"", page_label(p, x.id), x.title))
        .collect()
}

fn owner(p: &Project, role: &SymbolRole, pin_name: &str) -> String {
    match role {
        SymbolRole::Device { device, .. } => p
            .device(*device)
            .map_or_else(String::new, |d| d.tag.to_string()),
        SymbolRole::Terminal { terminals } => terminals
            .first()
            .and_then(|t| p.terminal_designation(*t))
            .unwrap_or_default(),
        SymbolRole::Interruption { name } => format!("interruption {name}"),
        _ => format!("unassigned {pin_name}"),
    }
}

fn symbols(p: &Project) -> BTreeSet<String> {
    p.symbols()
        .map(|s| {
            let first = s.pins.first().map_or("", |x| x.name.as_str());
            let at = s.pin_position(0).unwrap_or(s.placement.origin);
            format!(
                "{} {} {} @{},{}",
                page_label(p, s.page),
                s.symbol.0,
                owner(p, &s.role, first),
                mm(at.x),
                mm(at.y)
            )
        })
        .collect()
}

/// The designations of each net's device and terminal pins; a net with fewer
/// than two is no connection worth comparing.
fn nets(p: &Project) -> BTreeSet<String> {
    let netlist = p.netlist();
    let mut out = BTreeSet::new();
    for net in netlist.nets() {
        let mut members: Vec<String> = Vec::new();
        for e in &net.endpoints {
            let Endpoint::Pin { symbol, pin } = *e else {
                continue;
            };
            let Some(sym) = p.symbol(symbol) else {
                continue;
            };
            if matches!(
                sym.role,
                SymbolRole::Device { .. } | SymbolRole::Terminal { .. }
            ) && let Some(d) = p.pin_designation(novo_core::PinRef::new(symbol, pin))
            {
                members.push(d);
            }
        }
        members.sort();
        members.dedup();
        if members.len() >= 2 {
            out.insert(members.join(" = "));
        }
    }
    out
}

/// Compares `before` with `after` and says where they differ.
pub fn compare_projects(before: &Project, after: &Project) -> Comparison {
    let mut report = Report::default();
    type Section = (&'static str, fn(&Project) -> BTreeSet<String>);
    let sections: [Section; 7] = [
        ("devices", devices),
        ("terminals", terminals),
        ("cables", cables),
        ("parts", parts),
        ("pages", pages),
        ("drawn symbols", symbols),
        ("nets", nets),
    ];
    let mut total = 0;
    for (what, f) in sections {
        total += diff(&mut report, what, f(before), f(after));
    }
    if total == 0 {
        report.note("Nothing differs in what is compared.");
    }
    Comparison { report }
}
