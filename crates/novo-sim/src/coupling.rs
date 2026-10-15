//! Coil to contact coupling: a coil with two different live potentials on
//! its pins is energized, and that closes the normally open contacts of its
//! device and opens the normally closed ones. Contacts feed back into the
//! circuit, so [`settle`] iterates until the coils agree with their own
//! supply; a coil that was energized stays so while its own contact (a seal-in
//! circuit) keeps it fed, which is why the previous state is passed in.
//!
//! Timer contacts (`..._delay_on`, `..._delay_off`) count on a [`Clock`]: an
//! on-delay contact operates once its coil has been on for the device's
//! `delay` property (seconds), an off-delay contact releases that long after
//! the coil went off. Nothing moves until the caller advances the clock.
//!
//! Only devices that draw a coil are driven. A contact of a device without
//! one (a push button) stays under the hand-set [`PoleState`], as before.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{DeviceId, Endpoint, Netlist, Project, SymbolId, SymbolInstance, Value};

use crate::{PoleState, Solution, solve};

/// Most rounds before a circuit that never settles (a buzzer) is given up
/// on; the last round stands.
const ROUNDS: usize = 32;

/// Seconds a timer device delays when it carries no `delay` property.
pub const DEFAULT_DELAY: f64 = 5.0;

/// How a switched symbol reacts to its device's coil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rest {
    /// Normally open: closes when the coil is on.
    Open,
    /// Normally closed: opens when the coil is on.
    Closed,
    /// Changeover: pole 0 (the break side) is closed at rest, pole 1 (the
    /// make side) when the coil is on, as `Symbol::pass_through` lists them.
    Changeover,
}

/// The delay of a timer contact (IEC 60617 draws it on the contact itself).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delay {
    /// Follows the coil at once.
    None,
    /// Operates only after the coil has been on for the set time; releases
    /// at once.
    On,
    /// Operates at once; releases the set time after the coil went off.
    Off,
}

/// Time of the bench: how long each coil has been on or off, so timer
/// contacts can count. It only moves when the caller advances it.
#[derive(Debug, Clone, Default)]
pub struct Clock {
    now: f64,
    /// Per coil: whether it was on at its last change, and when that was.
    coils: BTreeMap<SymbolId, (bool, f64)>,
}

impl Clock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seconds since the clock started.
    pub fn now(&self) -> f64 {
        self.now
    }

    /// Lets `seconds` pass.
    pub fn advance(&mut self, seconds: f64) {
        self.now += seconds.max(0.0);
    }

    /// Forgets all coil history: every timer starts from rest.
    pub fn reset(&mut self) {
        self.coils.clear();
    }
}

/// The set time of a timer device in seconds: its `delay` property, a number
/// or text starting with one (`"3.5 s"`).
fn delay_of(project: &Project, device: Option<DeviceId>) -> f64 {
    let value = device
        .and_then(|d| project.device(d))
        .and_then(|d| d.properties.get("delay"));
    let seconds = match value {
        Some(Value::Int(v)) => *v as f64,
        Some(Value::Float(v)) => *v,
        Some(Value::Text(t)) => {
            let t = t.trim();
            let end = t
                .find(|c: char| !(c.is_ascii_digit() || c == '.'))
                .unwrap_or(t.len());
            t[..end].parse().unwrap_or(DEFAULT_DELAY)
        }
        _ => DEFAULT_DELAY,
    };
    seconds.max(0.0)
}

/// Whether a symbol is an on-delay or off-delay contact, which waits for the
/// `delay` of its device.
pub fn is_timer_contact(symbol: &SymbolInstance) -> bool {
    contact_of(symbol).is_some_and(|(_, delay)| !matches!(delay, Delay::None))
}

/// The time a timer device is set to, in seconds (its `delay` property, or
/// [`DEFAULT_DELAY`] when it has none).
pub fn delay_seconds(project: &Project, device: DeviceId) -> f64 {
    delay_of(project, Some(device))
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
/// `..._nc`, changeover ones `co`, `..._co` or `changeover`; a timer contact
/// adds `_delay_on` or `_delay_off`.
fn contact_of(symbol: &SymbolInstance) -> Option<(Rest, Delay)> {
    if symbol.pass_through.is_empty() || symbol.device().is_none() {
        return None;
    }
    let full = name_of(symbol);
    let (name, delay) = match full.split_once("_delay_") {
        Some((base, "on")) => (base, Delay::On),
        Some((base, "off")) => (base, Delay::Off),
        _ => (full, Delay::None),
    };
    let rest = if name == "co" || name.ends_with("_co") || name.contains("changeover") {
        // A changeover without its two paths cannot be driven.
        if symbol.pass_through.len() != 2 {
            return None;
        }
        Rest::Changeover
    } else if name == "nc" || name.ends_with("_nc") {
        Rest::Closed
    } else {
        Rest::Open
    };
    Some((rest, delay))
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
    /// A timer is counting: the state will change by itself as the clock
    /// advances, so the caller should keep stepping.
    pub timing: bool,
    /// Seconds until the last running timer is done; 0 when none runs.
    pub remaining: f64,
}

/// Solves `manual` poles with the coils coupled to their contacts, with
/// every timer at rest (time does not pass). See [`settle_timed`].
pub fn settle(
    project: &Project,
    netlist: &Netlist,
    manual: &PoleState,
    forced: &BTreeSet<SymbolId>,
    held: &BTreeSet<SymbolId>,
) -> Settled {
    settle_timed(
        project,
        netlist,
        manual,
        forced,
        held,
        &mut Clock::default(),
    )
}

/// Solves `manual` poles with the coils coupled to their contacts.
/// `forced` coils are energized whatever feeds them (the bench's own
/// switch); `held` is the energized set of the previous solution, the
/// starting point that lets a seal-in circuit keep itself on. `clock`
/// remembers when each coil switched, and is updated to this solution.
pub fn settle_timed(
    project: &Project,
    netlist: &Netlist,
    manual: &PoleState,
    forced: &BTreeSet<SymbolId>,
    held: &BTreeSet<SymbolId>,
    clock: &mut Clock,
) -> Settled {
    let coils: Vec<&SymbolInstance> = project.symbols().filter(|s| is_coil(s)).collect();
    let coil_devices: BTreeSet<DeviceId> = coils.iter().filter_map(|c| c.device()).collect();
    let driven: BTreeSet<SymbolId> = project
        .symbols()
        .filter(|s| s.device().is_some_and(|d| coil_devices.contains(&d)))
        .filter(|s| contact_of(s).is_some())
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
        let (poles, _) = poles_for(project, manual, &driven, &coils, &energized, clock);
        let solution = solve(project, netlist, &poles);
        let next: BTreeSet<SymbolId> = coils
            .iter()
            .filter(|c| forced.contains(&c.id) || fed(c, netlist, &solution))
            .map(|c| c.id)
            .collect();
        record(clock, &coils, &next);
        let settled = next == energized;
        energized = next;
        last = Some((poles, solution));
        if settled {
            break;
        }
    }
    let (poles, solution) = last.expect("at least one round");
    let (_, remaining) = poles_for(project, manual, &driven, &coils, &energized, clock);
    Settled {
        poles,
        solution,
        energized,
        driven,
        timing: remaining > 0.0,
        remaining,
    }
}

/// Notes the coils whose state differs from what the clock last saw.
fn record(clock: &mut Clock, coils: &[&SymbolInstance], on: &BTreeSet<SymbolId>) {
    let now = clock.now;
    for coil in coils {
        let state = on.contains(&coil.id);
        match clock.coils.get(&coil.id) {
            Some(&(was, _)) if was == state => {}
            // A coil first seen off has been off forever.
            None if !state => {
                clock.coils.insert(coil.id, (false, f64::NEG_INFINITY));
            }
            _ => {
                clock.coils.insert(coil.id, (state, now));
            }
        }
    }
}

/// Two different potentials on the two pins of the winding.
fn fed(coil: &SymbolInstance, netlist: &Netlist, solution: &Solution) -> bool {
    let a = solution.of(netlist, Endpoint::pin(coil.id, 0)).potential();
    let b = solution.of(netlist, Endpoint::pin(coil.id, 1)).potential();
    matches!((a, b), (Some(a), Some(b)) if a != b)
}

/// The poles for the coils in `energized`, and whether a timer is counting.
fn poles_for(
    project: &Project,
    manual: &PoleState,
    driven: &BTreeSet<SymbolId>,
    coils: &[&SymbolInstance],
    energized: &BTreeSet<SymbolId>,
    clock: &Clock,
) -> (PoleState, f64) {
    let mut poles = manual.clone();
    let mut remaining = 0.0_f64;
    for symbol in project.symbols().filter(|s| driven.contains(&s.id)) {
        let Some((rest, delay)) = contact_of(symbol) else {
            continue;
        };
        let coil = coils.iter().find(|c| c.device() == symbol.device());
        let on = coil.is_some_and(|c| energized.contains(&c.id));
        let seen = coil.and_then(|c| clock.coils.get(&c.id)).copied();
        let operated = match delay {
            Delay::None => on,
            Delay::On => {
                let wait = delay_of(project, symbol.device());
                let elapsed = seen.filter(|&(was, _)| was).map(|(_, at)| clock.now - at);
                let done = on && elapsed.is_some_and(|e| e >= wait);
                if on && !done {
                    remaining = remaining.max(wait - elapsed.unwrap_or(0.0));
                }
                done
            }
            Delay::Off => {
                let wait = delay_of(project, symbol.device());
                // Right after the coil dropped, before the clock has seen
                // it, the contact still holds.
                let holding = !on && seen.is_some_and(|(was, at)| was || clock.now - at < wait);
                if let Some((false, at)) = seen.filter(|_| holding) {
                    remaining = remaining.max(wait - (clock.now - at));
                }
                on || holding
            }
        };
        match rest {
            Rest::Open if operated => poles.set_all(project, symbol.id, true),
            Rest::Closed if !operated => poles.set_all(project, symbol.id, true),
            Rest::Changeover => poles.set(symbol.id, usize::from(operated), true),
            _ => {}
        }
    }
    (poles, remaining)
}
