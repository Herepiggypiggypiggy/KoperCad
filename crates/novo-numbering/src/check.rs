//! Conflicts: what is wrong with the numbers a project has.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Project, WireId};

use crate::group::{Group, fits, groups};
use crate::scheme::Scheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConflictKind {
    /// Two nets carry the same number.
    Duplicate,
    /// The wires of one net carry different numbers.
    Split,
    /// Some wires of a net have a number, others none.
    Partial,
    /// A net without any number.
    Unnumbered,
    /// A number that the scheme would not produce.
    OffScheme,
}

/// One problem with the numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub severity: Severity,
    pub number: Option<String>,
    /// The wires to select to look at it.
    pub wires: Vec<WireId>,
    pub message: String,
}

/// Checks a project against its scheme.
pub fn check(project: &Project, scheme: &Scheme) -> Vec<Conflict> {
    let netlist = project.netlist();
    conflicts_of(project, &groups(project, &netlist), scheme, None)
}

/// Like [`check`] on groups already built; `after` replaces the numbers the
/// project holds (a plan's outcome).
pub(crate) fn conflicts_of(
    project: &Project,
    all: &[Group],
    scheme: &Scheme,
    after: Option<&BTreeMap<WireId, Option<String>>>,
) -> Vec<Conflict> {
    let number_of = |w: WireId| -> Option<String> {
        match after {
            Some(map) => map.get(&w).cloned().flatten(),
            None => project
                .wire(w)
                .and_then(|w| w.number.clone())
                .filter(|n| !n.is_empty()),
        }
    };
    let mut out = Vec::new();
    // number -> groups using it
    let mut users: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, g) in all.iter().enumerate() {
        let distinct: BTreeSet<String> = g.wires.iter().filter_map(|&w| number_of(w)).collect();
        let numbered = g.wires.iter().filter(|&&w| number_of(w).is_some()).count();
        if distinct.is_empty() {
            out.push(Conflict {
                kind: ConflictKind::Unnumbered,
                severity: Severity::Warning,
                number: None,
                wires: g.wires.clone(),
                message: format!(
                    "Net on page {}.{} has no wire number",
                    g.page_name, g.column
                ),
            });
            continue;
        }
        if distinct.len() > 1 {
            out.push(Conflict {
                kind: ConflictKind::Split,
                severity: Severity::Error,
                number: None,
                wires: g.wires.clone(),
                message: format!(
                    "One net on page {}.{} carries {}",
                    g.page_name,
                    g.column,
                    distinct.iter().cloned().collect::<Vec<_>>().join(" and ")
                ),
            });
        } else if numbered < g.wires.len() {
            out.push(Conflict {
                kind: ConflictKind::Partial,
                severity: Severity::Warning,
                number: distinct.first().cloned(),
                wires: g.wires.clone(),
                message: format!(
                    "{} has {} wire(s) without the number on page {}.{}",
                    distinct.first().cloned().unwrap_or_default(),
                    g.wires.len() - numbered,
                    g.page_name,
                    g.column
                ),
            });
        }
        for n in &distinct {
            users.entry(n.clone()).or_default().push(i);
            if !g.shares(n, scheme) && !fits(&g.target(scheme), n) && distinct.len() == 1 {
                out.push(Conflict {
                    kind: ConflictKind::OffScheme,
                    severity: Severity::Info,
                    number: Some(n.clone()),
                    wires: g.wires.clone(),
                    message: format!("{n} is not a number the scheme would give here"),
                });
            }
        }
    }
    for (number, idx) in users {
        let distinct_groups: Vec<&Group> = idx.iter().map(|&i| &all[i]).collect();
        if distinct_groups.len() < 2 || distinct_groups.iter().all(|g| g.shares(&number, scheme)) {
            continue;
        }
        let pages: BTreeSet<String> = distinct_groups
            .iter()
            .map(|g| format!("{}.{}", g.page_name, g.column))
            .collect();
        out.push(Conflict {
            kind: ConflictKind::Duplicate,
            severity: Severity::Error,
            number: Some(number.clone()),
            wires: distinct_groups
                .iter()
                .flat_map(|g| g.wires.clone())
                .collect(),
            message: format!(
                "{number} is used on {} different nets ({})",
                distinct_groups.len(),
                pages.into_iter().collect::<Vec<_>>().join(", ")
            ),
        });
    }
    out.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.kind.cmp(&b.kind)));
    out
}
