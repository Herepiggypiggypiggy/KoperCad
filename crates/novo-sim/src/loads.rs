//! Outputs: the lamps, motors and horns a circuit finally drives. A load is
//! on when two different potentials sit on its pins (a protective earth pin
//! does not count: a motor's PE lead is not what drives it).

use std::collections::BTreeSet;

use novo_core::{Endpoint, Netlist, PotentialKind, Project, SymbolId, SymbolInstance};

use crate::Solution;

/// Library names that are consumers rather than switches or windings.
const LOADS: [&str; 5] = ["lamp", "motor", "horn", "buzzer", "valve"];

/// A device symbol that turns electricity into light, motion or sound: it
/// has no switched poles and its library name says what it is.
pub fn is_load(symbol: &SymbolInstance) -> bool {
    let name = symbol.symbol.0.rsplit('/').next().unwrap_or("");
    symbol.device().is_some()
        && symbol.pass_through.is_empty()
        && symbol.pins.len() >= 2
        && LOADS.iter().any(|l| name.contains(l))
}

/// The loads with two different, non-earth potentials on their pins.
pub fn loads_on(project: &Project, netlist: &Netlist, solution: &Solution) -> BTreeSet<SymbolId> {
    project
        .symbols()
        .filter(|s| is_load(s))
        .filter(|s| {
            let live: BTreeSet<_> = (0..s.pins.len())
                .filter_map(|pin| {
                    solution
                        .of(netlist, Endpoint::pin(s.id, pin as u16))
                        .potential()
                })
                .filter(|&p| {
                    project
                        .potential(p)
                        .is_some_and(|p| p.kind != PotentialKind::ProtectiveEarth)
                })
                .collect();
            live.len() >= 2
        })
        .map(|s| s.id)
        .collect()
}
