//! Live terminal checks for the checker panel (WS-14 lists them).

use std::collections::HashMap;

use novo_core::{Netlist, Project, SameName, SymbolId, TerminalId, TerminalKind};

use crate::naming::{kind_for_name, potential_fits};
use crate::occurrence::{State, occurrences};
use crate::rules::rules;
use crate::strings;
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
    /// The name is already used on the strip: another record with the same
    /// name and level when no rule joins them (Off), or another drawing of
    /// the terminal under the Refuse rule.
    DuplicateName,
    /// Under novo-core's `rules.potentials.from_terminal_names` the name
    /// defines a PE, PEN or N potential, but the terminal is another kind.
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

/// Checks every terminal of the project.
pub fn check(project: &Project, netlist: &Netlist) -> Vec<Issue> {
    let rule = rules(project).same_name;
    let mut issues = Vec::new();
    for strip in strips(project) {
        let mut by_name: HashMap<(&str, u8), Vec<TerminalId>> = HashMap::new();
        let mut names: Vec<(&str, u8)> = Vec::new();
        for t in project.terminals_of_strip(strip) {
            let designation = project
                .terminal_designation(t.id)
                .unwrap_or_else(|| t.name.clone());
            let ids = by_name.entry((t.name.as_str(), t.level)).or_default();
            if ids.is_empty() {
                names.push((t.name.as_str(), t.level));
            }
            ids.push(t.id);
            let mut push = |symbol, kind, severity, message| {
                issues.push(Issue {
                    terminal: t.id,
                    symbol,
                    kind,
                    severity,
                    message,
                })
            };
            if let Some(expected) = kind_for_name(project, &t.name)
                && expected != t.kind
            {
                push(
                    None,
                    IssueKind::KindDiffersFromName { expected },
                    Severity::Warning,
                    strings::kind_differs_from_name(&designation, expected, t.kind),
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
                        strings::unwired(&designation),
                    );
                } else if state != State::Through
                    && matches!(t.kind, TerminalKind::Fuse | TerminalKind::Disconnect)
                {
                    push(
                        Some(o.symbol),
                        IssueKind::SingleSided,
                        Severity::Error,
                        strings::single_sided(&designation, t.kind),
                    );
                }
                if i > 0 && rule == SameName::Refuse {
                    push(
                        Some(o.symbol),
                        IssueKind::DuplicateName,
                        Severity::Error,
                        strings::drawn_again_refused(&designation),
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
                    strings::potential_differs_from_kind(&designation, t.kind, &potential.name),
                );
            }
        }
        // Under Join the netlist makes same-named records one terminal, so
        // they need no finding. Refuse cannot have them (novo-core rejects
        // the record), unless an older file was loaded without the check.
        if rule != SameName::Join {
            for key in &names {
                let ids = &by_name[key];
                let designation = project.terminal_designation(ids[0]).unwrap_or_default();
                let severity = match rule {
                    SameName::Refuse => Severity::Error,
                    _ => Severity::Warning,
                };
                for &id in &ids[1..] {
                    issues.push(Issue {
                        terminal: id,
                        symbol: None,
                        kind: IssueKind::DuplicateName,
                        severity,
                        message: strings::duplicate_record(&designation, rule),
                    });
                }
            }
        }
        for id in overrides(project, strip) {
            let designation = project.terminal_designation(id).unwrap_or_default();
            issues.push(Issue {
                terminal: id,
                symbol: None,
                kind: IssueKind::StyleOverride,
                severity: Severity::Info,
                message: strings::style_override(&designation),
            });
        }
    }
    issues
}
