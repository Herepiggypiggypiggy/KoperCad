//! Coil to contact coupling: a coil with two different live potentials on
//! its pins is energized, and that closes the normally open contacts of its
//! device and opens the normally closed ones. Contacts feed back into the
//! circuit, so [`settle`] iterates until the coils agree with their own
//! supply; a coil that was energized stays so while its own contact (a seal-in
//! circuit) keeps it fed, which is why the previous state is passed in.
//!
//! Only devices that draw a coil are driven. A contact of a device without
//! one (a push button) stays under the hand-set [`PoleState`], as before.

use std::collections::BTreeSet;

use novo_core::{DeviceId, Endpoint, Netlist, Project, SymbolId, SymbolInstance};

use crate::{PoleState, Solution, solve};

/// Most rounds before a circuit that never settles (a buzzer) is given up
/// on; the last round stands.
const ROUNDS: usize = 32;

/// How a switched symbol reacts to its device's coil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rest {
    Open,
    Closed,
}

fn name_of(symbol: &SymbolInstance) -> &str {
    symbol.symbol.0.rsplit('/').next().unwrap_or("")
}

/// A coil is a device symbol with exactly the two pins of a winding and no
/// switched poles, drawn from a `coil` library symbol.
pub fn is_coil(symbol: &SymbolInstance) -> bool {
    symbol.device().is_some()
        && symbol.pass_through.is_empty()
        && symbol.pins.len() >= 2
        && symbol.symbol.0.contains("coil")
}

/// The rest state of a contact: normally closed ones are named `nc` or
/// `..._nc`; changeover contacts are left to hand-setting.
fn rest_state(symbol: &SymbolInstance) -> Option<Rest> {
    if symbol.pass_through.is_empty() || symbol.device().is_none() {
        return None;
    }
    let name = name_of(symbol);
    if name == "co" || name.ends_with("_co") {
        None
    } else if name == "nc" || name.ends_with("_nc") {
        Some(Rest::Closed)
    } else {
        Some(Rest::Open)
    }
}

/// The result of [`settle`].
#[derive(Debug, Clone)]
pub struct Settled {
    /// The poles closed now: the hand-set ones plus those the coils drive.
    pub poles: PoleState,
    pub solution: Solution,
    /// Coils that are energized (fed by two different potentials, or forced).
    pub energized: BTreeSet<SymbolId>,
    /// Contacts a coil drives; a hand-set state can only add to them.
    pub driven: BTreeSet<SymbolId>,
}

/// Solves `manual` poles with the coils coupled to their contacts.
/// `forced` coils are energized whatever feeds them (the bench's own
/// switch); `held` is the energized set of the previous solution, the
/// starting point that lets a seal-in circuit keep itself on.
pub fn settle(
    project: &Project,
    netlist: &Netlist,
    manual: &PoleState,
    forced: &BTreeSet<SymbolId>,
    held: &BTreeSet<SymbolId>,
) -> Settled {
    let coils: Vec<&SymbolInstance> = project.symbols().filter(|s| is_coil(s)).collect();
    let coil_devices: BTreeSet<DeviceId> = coils.iter().filter_map(|c| c.device()).collect();
    let driven: BTreeSet<SymbolId> = project
        .symbols()
        .filter(|s| s.device().is_some_and(|d| coil_devices.contains(&d)))
        .filter(|s| rest_state(s).is_some())
        .map(|s| s.id)
        .collect();

    let mut energized: BTreeSet<SymbolId> = held
        .iter()
        .chain(forced)
        .copied()
        .filter(|c| coils.iter().any(|k| k.id == *c))
        .collect();
    let mut last = None;
    for _ in 0..ROUNDS {
        let poles = poles_for(project, manual, &driven, &coils, &energized);
        let solution = solve(project, netlist, &poles);
        let next: BTreeSet<SymbolId> = coils
            .iter()
            .filter(|c| forced.contains(&c.id) || fed(c, netlist, &solution))
            .map(|c| c.id)
            .collect();
        let settled = next == energized;
        energized = next;
        last = Some((poles, solution));
        if settled {
            break;
        }
    }
    let (poles, solution) = last.expect("at least one round");
    Settled {
        poles,
        solution,
        energized,
        driven,
    }
}

/// Two different potentials on the two pins of the winding.
fn fed(coil: &SymbolInstance, netlist: &Netlist, solution: &Solution) -> bool {
    let a = solution.of(netlist, Endpoint::pin(coil.id, 0)).potential();
    let b = solution.of(netlist, Endpoint::pin(coil.id, 1)).potential();
    matches!((a, b), (Some(a), Some(b)) if a != b)
}

fn poles_for(
    project: &Project,
    manual: &PoleState,
    driven: &BTreeSet<SymbolId>,
    coils: &[&SymbolInstance],
    energized: &BTreeSet<SymbolId>,
) -> PoleState {
    let mut poles = manual.clone();
    for symbol in project.symbols().filter(|s| driven.contains(&s.id)) {
        let on = coils
            .iter()
            .any(|c| c.device() == symbol.device() && energized.contains(&c.id));
        let closed = match rest_state(symbol) {
            Some(Rest::Open) => on,
            Some(Rest::Closed) => !on,
            None => continue,
        };
        if closed {
            poles.set_all(project, symbol.id, true);
        }
    }
    poles
}
