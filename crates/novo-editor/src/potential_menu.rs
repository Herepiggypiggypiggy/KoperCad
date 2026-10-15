//! The right-click rows for potentials (DESIGN.md "Potential feeds" and "PE
//! terminals"): the chips of a selected feed and the PE rows of selected
//! terminals. Pure model, no egui, so the rows can be tested and the same
//! choices reused by the command palette.

use novo_core::{
    AnyId, PeOverride, PotentialKind, Project, SymbolId, SymbolRole, TerminalId, TerminalKind,
};

use crate::potential::PotentialSpec;

/// The fixed chips of the feed menu, in DESIGN order.
pub fn chips() -> Vec<PotentialSpec> {
    let line = |n: &str| PotentialSpec::new(n, PotentialKind::Line, Some(400.0));
    vec![
        line("L1"),
        line("L2"),
        line("L3"),
        PotentialSpec::new("N", PotentialKind::Neutral, None),
        PotentialSpec::new("PE", PotentialKind::ProtectiveEarth, None),
        PotentialSpec::new("+24 V", PotentialKind::DcPositive, Some(24.0)),
        PotentialSpec::new("0 V", PotentialKind::DcNegative, Some(0.0)),
    ]
}

/// The "Set potential" part of the menu of selected feeds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FeedMenu {
    /// The selected feeds, in selection order.
    pub feeds: Vec<SymbolId>,
    /// What the feeds declare now; `None` when they differ.
    pub current: Option<String>,
    /// The chips, with the one in use first-class (`current` names it).
    pub chips: Vec<PotentialSpec>,
    /// The project's other potentials ("Other…").
    pub others: Vec<PotentialSpec>,
}

/// The menu for the potential feeds in `selection`, or `None` without one.
pub fn feed_menu(project: &Project, selection: &[AnyId]) -> Option<FeedMenu> {
    let mut feeds = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for id in selection {
        let AnyId::Symbol(s) = id else { continue };
        let Some(SymbolRole::PotentialFeed { potential }) = project.symbol(*s).map(|s| &s.role)
        else {
            continue;
        };
        feeds.push(*s);
        if let Some(p) = project.potential(*potential)
            && !names.contains(&p.name)
        {
            names.push(p.name.clone());
        }
    }
    if feeds.is_empty() {
        return None;
    }
    let chips = chips();
    let others = project
        .potentials()
        .filter(|p| !chips.iter().any(|c| c.name == p.name))
        .map(|p| PotentialSpec::new(&p.name, p.kind, p.voltage))
        .collect();
    Some(FeedMenu {
        feeds,
        current: (names.len() == 1).then(|| names.remove(0)),
        chips,
        others,
    })
}

/// The PE rows of selected terminals.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PeMenu {
    pub terminals: Vec<TerminalId>,
    /// Every selected terminal is a PE terminal now (the row shows a check).
    pub all_pe: bool,
    /// Some terminal's state was set by hand: "Reset PE override" applies.
    pub overridden: bool,
}

/// The menu for the terminals in `selection`, or `None` without one.
pub fn pe_menu(project: &Project, selection: &[AnyId]) -> Option<PeMenu> {
    let mut terminals: Vec<TerminalId> = Vec::new();
    for id in selection {
        let AnyId::Symbol(s) = id else { continue };
        if let Some(SymbolRole::Terminal { terminals: ts }) = project.symbol(*s).map(|s| &s.role) {
            for &t in ts {
                if !terminals.contains(&t) {
                    terminals.push(t);
                }
            }
        }
    }
    if terminals.is_empty() {
        return None;
    }
    let all_pe = terminals.iter().all(|&t| {
        project
            .terminal(t)
            .is_some_and(|t| t.kind == TerminalKind::ProtectiveEarth)
    });
    let overridden = terminals.iter().any(|&t| {
        project
            .terminal(t)
            .is_some_and(|t| t.pe_override != PeOverride::Auto)
    });
    Some(PeMenu {
        terminals,
        all_pe,
        overridden,
    })
}
