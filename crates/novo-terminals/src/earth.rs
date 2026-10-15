//! Protective-earth terminals: the right-click toggle and the automatic
//! marking of terminals that sit on an earthed net.
//!
//! A net is earthed when a PE (or PEN) potential is defined on it, or when a
//! pin of an earth symbol (`iec/earth/...`) touches it. Only the plain
//! feed-through terminals are marked automatically; a fuse, disconnect or
//! neutral terminal keeps its kind, and the check in [`crate::check`] still
//! reports a mismatch.

use novo_core::{
    Endpoint, Netlist, PeOverride, PotentialKind, Project, Result, TerminalId, TerminalKind, Tx,
};

use crate::strip::set_kind;

/// Whether a terminal's PE state was set by hand ([`PeOverride`]).
pub fn has_pe_override(project: &Project, terminal: TerminalId) -> bool {
    project
        .terminal(terminal)
        .is_some_and(|t| !t.pe_override.is_auto())
}

/// Gives the terminals back to the automatic rule. Does not change their
/// kind; the next [`mark_earthed`] decides.
pub fn reset_pe_override(tx: &mut Tx<'_>, terminals: &[TerminalId]) -> Result<()> {
    tx.set_terminal_pe(terminals, PeOverride::Auto)
}

/// Whether a symbol key is an earth/ground symbol that earths its net.
/// Chassis (`iec/earth/chassis`) is bonding, not protective earth.
pub fn is_earth_symbol(key: &str) -> bool {
    matches!(key, "iec/earth/earth" | "iec/earth/protective_earth")
}

/// Terminals that are plain feed-through but sit on an earthed net.
pub fn earthed_terminals(project: &Project, netlist: &Netlist) -> Vec<TerminalId> {
    let earthed: Vec<bool> = netlist
        .nets()
        .iter()
        .map(|net| {
            let by_potential = net.defined.iter().any(|&p| {
                project.potential(p).is_some_and(|p| {
                    matches!(p.kind, PotentialKind::ProtectiveEarth | PotentialKind::Pen)
                })
            });
            by_potential
                || net.endpoints.iter().any(|e| match e {
                    Endpoint::Pin { symbol, .. } => project
                        .symbol(*symbol)
                        .is_some_and(|s| is_earth_symbol(&s.symbol.0)),
                    _ => false,
                })
        })
        .collect();
    let mut found: Vec<TerminalId> = project
        .terminals()
        .filter(|t| t.kind == TerminalKind::Feedthrough && t.pe_override.is_auto())
        .filter(|t| {
            project.symbols_of_terminal(t.id).iter().any(|s| {
                (0..s.pins.len()).any(|pin| {
                    netlist
                        .net_of(Endpoint::pin(s.id, pin as u16))
                        .is_some_and(|n| earthed[n.0 as usize])
                })
            })
        })
        .map(|t| t.id)
        .collect();
    found.sort();
    found
}

/// Marks every terminal on an earthed net as PE; returns the ones changed.
pub fn mark_earthed(tx: &mut Tx<'_>) -> Result<Vec<TerminalId>> {
    let netlist = Netlist::build(tx.project());
    let terminals = earthed_terminals(tx.project(), &netlist);
    for &t in &terminals {
        set_kind(tx, t, TerminalKind::ProtectiveEarth)?;
    }
    Ok(terminals)
}

/// The right-click action: when every selected terminal is PE they become
/// feed-through, otherwise all become PE. The choice is an override that
/// [`mark_earthed`] respects until [`reset_pe_override`]. Returns the kind
/// they now have.
pub fn toggle_pe(tx: &mut Tx<'_>, terminals: &[TerminalId]) -> Result<TerminalKind> {
    let all_pe = !terminals.is_empty()
        && terminals.iter().all(|&t| {
            tx.project()
                .terminal(t)
                .is_some_and(|t| t.kind == TerminalKind::ProtectiveEarth)
        });
    let kind = if all_pe {
        TerminalKind::Feedthrough
    } else {
        TerminalKind::ProtectiveEarth
    };
    for &t in terminals {
        set_kind(tx, t, kind)?;
    }
    let mode = if kind == TerminalKind::ProtectiveEarth {
        PeOverride::ForcedPe
    } else {
        PeOverride::ForcedNotPe
    };
    tx.set_terminal_pe(terminals, mode)?;
    Ok(kind)
}
