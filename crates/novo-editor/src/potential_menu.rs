//! The right-click rows for potentials (DESIGN.md "Potential feeds" and "PE
//! terminals"): the chips of a selected feed and the PE rows of selected
//! terminals. Pure model, no egui, so the rows can be tested and the same
//! choices reused by the command palette.

use novo_core::{
    AnyId, Endpoint, PageId, PeOverride, PotentialKind, Project, SymbolId, SymbolRole, TerminalId,
    TerminalKind, WireId,
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

/// The potential typed under "New…": a trimmed, non-empty name. The kind
/// follows the usual names (L1.. line, N, PE/PEN, +24 V and 0 V as DC),
/// anything else is a signal potential.
pub fn new_spec(name: &str) -> Option<PotentialSpec> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let upper = name.to_uppercase();
    let kind = if upper == "N" {
        PotentialKind::Neutral
    } else if upper == "PE" {
        PotentialKind::ProtectiveEarth
    } else if upper == "PEN" {
        PotentialKind::Pen
    } else if upper.len() == 2
        && upper.starts_with('L')
        && upper[1..].chars().all(|c| c.is_ascii_digit())
    {
        PotentialKind::Line
    } else if upper.starts_with('+') {
        PotentialKind::DcPositive
    } else if upper.starts_with('0') {
        PotentialKind::DcNegative
    } else {
        PotentialKind::Signal
    };
    Some(PotentialSpec::new(name, kind, None))
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

/// The override rows of selected wires: one conductor takes a potential of
/// its own, whatever its net carries (DESIGN "one conductor can be
/// overridden").
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WireOverrideMenu {
    pub wires: Vec<WireId>,
    /// Some selected wire already has an override ("Remove override").
    pub overridden: bool,
    pub chips: Vec<PotentialSpec>,
}

/// The override menu for the wires in `selection`, or `None` without one.
pub fn wire_override_menu(project: &Project, selection: &[AnyId]) -> Option<WireOverrideMenu> {
    let wires: Vec<WireId> = selection
        .iter()
        .filter_map(|id| match id {
            AnyId::Wire(w) => Some(*w),
            _ => None,
        })
        .collect();
    if wires.is_empty() {
        return None;
    }
    let overridden = wires
        .iter()
        .any(|&w| project.wire(w).is_some_and(|w| w.potential.is_some()));
    Some(WireOverrideMenu {
        wires,
        overridden,
        chips: chips(),
    })
}

/// Show where it goes: every wire and terminal of `page` on a net that
/// carries the potential of the feed `feed`, to select and so light up.
pub fn where_it_goes(project: &Project, page: PageId, feed: SymbolId) -> Vec<AnyId> {
    let Some(SymbolRole::PotentialFeed { potential }) = project.symbol(feed).map(|s| &s.role)
    else {
        return Vec::new();
    };
    let netlist = project.netlist();
    let carries = |net: novo_core::NetId| netlist.net(net).potentials.contains(potential);
    let mut found: Vec<AnyId> = project
        .wires()
        .filter(|w| w.page == page)
        .filter(|w| netlist.net_of_wire(w).is_some_and(carries))
        .map(|w| AnyId::Wire(w.id))
        .collect();
    found.extend(
        project
            .symbols()
            .filter(|s| s.page == page && matches!(s.role, SymbolRole::Terminal { .. }))
            .filter(|s| {
                (0..s.pins.len()).any(|pin| {
                    netlist
                        .net_of(Endpoint::pin(s.id, pin as u16))
                        .is_some_and(carries)
                })
            })
            .map(|s| AnyId::Symbol(s.id)),
    );
    found
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_name_gets_its_usual_kind() {
        let kind = |n: &str| new_spec(n).map(|s| s.kind);
        assert_eq!(kind(" L4 "), Some(PotentialKind::Line));
        assert_eq!(kind("pe"), Some(PotentialKind::ProtectiveEarth));
        assert_eq!(kind("+12 V"), Some(PotentialKind::DcPositive));
        assert_eq!(kind("0V2"), Some(PotentialKind::DcNegative));
        assert_eq!(kind("Pump run"), Some(PotentialKind::Signal));
        assert_eq!(kind("   "), None);
        assert_eq!(new_spec(" L4 ").unwrap().name, "L4");
    }
}
