//! Live terminal checks for the checker panel (WS-14 lists them).

use std::collections::BTreeMap;

use novo_core::{Netlist, PotentialKind, Project, SymbolId, TerminalId, TerminalKind};

use crate::naming::{kind_for_name, kind_label};
use crate::occurrence::{State, occurrences};
use crate::settings::{SameName, Settings};
use crate::strip::strips;
use crate::style::overrides;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueKind {
    /// A drawn terminal with no wire on either side.
    Unwired,
    /// A fuse or disconnect terminal wired on one side only.
    SingleSided,
    /// The "refuse" rule is on and this name is already used on the strip.
    DuplicateName,
    /// The "join" rule is on, but several records carry the name, so the
    /// drawings are not connected yet. [`crate::join_duplicates`] fixes it.
    NotJoined,
    /// The name says PE or N but the terminal is another kind.
    KindDiffersFromName { expected: TerminalKind },
    /// The wire's potential does not fit the terminal kind (an L1 wire on a
    /// PE terminal, a PE wire on a feed-through terminal).
    PotentialDiffersFromKind { potential: String },
    /// The terminal has its own symbol or part instead of its strip's.
    StyleOverride,
}

/// One finding about one terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub terminal: TerminalId,
    /// The drawing concerned, when the finding is about one drawing.
    pub symbol: Option<SymbolId>,
    pub kind: IssueKind,
    pub severity: Severity,
    pub message: String,
}

fn potential_fits(kind: TerminalKind, potential: PotentialKind) -> bool {
    match kind {
        TerminalKind::ProtectiveEarth => potential == PotentialKind::ProtectiveEarth,
        TerminalKind::Neutral => potential == PotentialKind::Neutral,
        _ => potential != PotentialKind::ProtectiveEarth,
    }
}

/// Checks every terminal of the project.
pub fn check(project: &Project, netlist: &Netlist) -> Vec<Issue> {
    let settings = Settings::of(project);
    let mut issues = Vec::new();
    for strip in strips(project) {
        let mut by_name: BTreeMap<(String, u8), Vec<TerminalId>> = BTreeMap::new();
        for t in project.terminals_of_strip(strip) {
            let designation = project
                .terminal_designation(t.id)
                .unwrap_or_else(|| t.name.clone());
            by_name
                .entry((t.name.to_ascii_uppercase(), t.level))
                .or_default()
                .push(t.id);
            let mut push = |symbol, kind, severity, message| {
                issues.push(Issue {
                    terminal: t.id,
                    symbol,
                    kind,
                    severity,
                    message,
                })
            };
            if let Some(expected) = kind_for_name(&t.name)
                && expected != t.kind
            {
                push(
                    None,
                    IssueKind::KindDiffersFromName { expected },
                    Severity::Warning,
                    format!(
                        "{designation} is named like a {} terminal but is a {} terminal.",
                        kind_label(expected),
                        kind_label(t.kind)
                    ),
                );
            }
            let drawings = occurrences(project, t.id);
            for (i, o) in drawings.iter().enumerate() {
                let state = o.state();
                if state == State::Open {
                    push(
                        Some(o.symbol),
                        IssueKind::Unwired,
                        Severity::Warning,
                        format!("{designation} is drawn but not wired."),
                    );
                } else if state != State::Through
                    && matches!(t.kind, TerminalKind::Fuse | TerminalKind::Disconnect)
                {
                    push(
                        Some(o.symbol),
                        IssueKind::SingleSided,
                        Severity::Error,
                        format!(
                            "{designation} is a {} terminal but is wired on one side only.",
                            kind_label(t.kind)
                        ),
                    );
                }
                if i > 0 && settings.same_name == SameName::Refuse {
                    push(
                        Some(o.symbol),
                        IssueKind::DuplicateName,
                        Severity::Error,
                        format!(
                            "{designation} is drawn again; the project refuses duplicate terminal names."
                        ),
                    );
                }
            }
            let pin = drawings
                .iter()
                .flat_map(|o| {
                    o.internal
                        .pins
                        .iter()
                        .chain(&o.external.pins)
                        .map(|&p| (o.symbol, p))
                })
                .next();
            if let Some((symbol, pin)) = pin
                && let Some(net) = netlist.net_of(novo_core::Endpoint::pin(symbol, pin))
                && let Some(potential) = netlist
                    .net(net)
                    .potential()
                    .and_then(|p| project.potential(p))
                && !potential_fits(t.kind, potential.kind)
            {
                push(
                    None,
                    IssueKind::PotentialDiffersFromKind {
                        potential: potential.name.clone(),
                    },
                    Severity::Error,
                    format!(
                        "{designation} is a {} terminal on potential {}.",
                        kind_label(t.kind),
                        potential.name
                    ),
                );
            }
        }
        for ids in by_name.values().filter(|ids| ids.len() > 1) {
            let designation = project.terminal_designation(ids[0]).unwrap_or_default();
            for &id in &ids[1..] {
                let (kind, severity, message) = match settings.same_name {
                    SameName::Refuse => (
                        IssueKind::DuplicateName,
                        Severity::Error,
                        format!(
                            "{designation} exists more than once; the project refuses duplicate terminal names."
                        ),
                    ),
                    SameName::Join => (
                        IssueKind::NotJoined,
                        Severity::Warning,
                        format!(
                            "{designation} exists as separate terminals that are not connected yet; join them."
                        ),
                    ),
                };
                issues.push(Issue {
                    terminal: id,
                    symbol: None,
                    kind,
                    severity,
                    message,
                });
            }
        }
        for id in overrides(project, strip) {
            let designation = project.terminal_designation(id).unwrap_or_default();
            issues.push(Issue {
                terminal: id,
                symbol: None,
                kind: IssueKind::StyleOverride,
                severity: Severity::Info,
                message: format!(
                    "{designation} has its own symbol or part instead of its strip's."
                ),
            });
        }
    }
    issues
}
