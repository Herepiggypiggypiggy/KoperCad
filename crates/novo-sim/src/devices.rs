//! Device models: what the loads of a circuit do under a [`Settled`] state.
//! A lamp lights when its two pins carry different potentials; a motor runs
//! when its windings are fed, in the direction the phase sequence gives, and
//! hums on two phases. Nothing here moves or is stored: it reads the
//! solution, and the live view animates the result.
//!
//! Loads are recognised by their library key, like contacts and coils in
//! `coupling`: `iec/signalling/lamp...` and `iec/motors/motor_...`.

use std::collections::BTreeMap;

use novo_core::{
    Endpoint, Netlist, PotentialId, PotentialKind, Project, SymbolId, SymbolInstance, Value,
};

use crate::{Clock, Live, Settled, delay_seconds, is_coil};

/// How a motor behaves right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotorRun {
    /// Not fed.
    Stopped,
    /// Turning at `speed` (0 = the first winding set, 1 = the second).
    Running { direction: Direction, speed: u8 },
    /// Fed on two of three phases: hums, does not turn.
    PhaseLoss,
    /// A short on a winding pin, or two speeds fed at once.
    Fault,
}

/// Which way a motor turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// The derived state of every lamp and motor.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceStates {
    pub lamps: BTreeMap<SymbolId, bool>,
    pub motors: BTreeMap<SymbolId, MotorRun>,
}

/// Whether a symbol is an indicator lamp.
pub fn is_lamp(symbol: &SymbolInstance) -> bool {
    symbol.pins.len() >= 2 && symbol.symbol.0.starts_with("iec/signalling/lamp")
}

/// Whether a symbol is a motor.
pub fn is_motor(symbol: &SymbolInstance) -> bool {
    symbol.pins.len() >= 2 && symbol.symbol.0.starts_with("iec/motors/motor_")
}

/// The colour name of a lamp: the `color` property of its device, if any.
pub fn lamp_color(project: &Project, symbol: &SymbolInstance) -> Option<String> {
    let device = project.device(symbol.device()?)?;
    match device.properties.get("color")? {
        Value::Text(t) if !t.trim().is_empty() => Some(t.trim().to_lowercase()),
        _ => None,
    }
}

/// Derives lamp and motor states from a settled solution.
pub fn device_states(project: &Project, netlist: &Netlist, settled: &Settled) -> DeviceStates {
    let mut out = DeviceStates::default();
    for symbol in project.symbols() {
        if is_lamp(symbol) {
            let a = settled.solution.of(netlist, Endpoint::pin(symbol.id, 0));
            let b = settled.solution.of(netlist, Endpoint::pin(symbol.id, 1));
            out.lamps.insert(symbol.id, differ(a, b));
        } else if is_motor(symbol) {
            out.motors
                .insert(symbol.id, motor_run(project, netlist, settled, symbol));
        }
    }
    out
}

/// Two nets each carrying one potential, and not the same one.
fn differ(a: &Live, b: &Live) -> bool {
    matches!((a.potential(), b.potential()), (Some(a), Some(b)) if a != b)
}

/// Trailing digit of a potential's name (`L2` is 2), for phase order.
fn phase_digit(project: &Project, id: PotentialId) -> Option<i64> {
    project
        .potential(id)?
        .name
        .chars()
        .rev()
        .find_map(|c| c.to_digit(10))
        .map(i64::from)
}

fn motor_run(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    symbol: &SymbolInstance,
) -> MotorRun {
    let name = symbol.symbol.0.rsplit('/').next().unwrap_or("");
    // Winding pins in order; the protective earth clamp is no winding.
    let windings: Vec<u16> = symbol
        .pins
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.name.eq_ignore_ascii_case("pe"))
        .map(|(i, _)| i as u16)
        .collect();
    let live_of = |pin: u16| settled.solution.of(netlist, Endpoint::pin(symbol.id, pin));
    if windings
        .iter()
        .any(|&p| matches!(live_of(p), Live::Shorted(_)))
    {
        return MotorRun::Fault;
    }
    let potential = |pin: u16| live_of(pin).potential();

    if name.starts_with("motor_3ph") {
        // Dahlander: each group of three is a speed. The 6-terminal motor
        // is fed at its first three pins (the star point is its own wiring).
        let sets: Vec<&[u16]> = if name == "motor_3ph_dahlander" {
            windings.chunks(3).collect()
        } else {
            vec![&windings[..windings.len().min(3)]]
        };
        if windings.is_empty() {
            return MotorRun::Stopped;
        }
        let mut result = MotorRun::Stopped;
        let mut fed = 0;
        for (speed, set) in sets.iter().enumerate() {
            let run = three_phase(project, set, &potential, speed as u8);
            if run != MotorRun::Stopped {
                fed += 1;
                result = run;
            }
        }
        return if fed > 1 { MotorRun::Fault } else { result };
    }

    // Single phase and DC: two winding pins on two different potentials.
    if windings.len() < 2 {
        return MotorRun::Stopped;
    }
    let (a, b) = (potential(windings[0]), potential(windings[1]));
    match (a, b) {
        (Some(a), Some(b)) if a != b => {
            let negative = |id| {
                project
                    .potential(id)
                    .is_some_and(|p| p.kind == PotentialKind::DcNegative)
            };
            let direction = if name == "motor_dc" && negative(a) {
                Direction::Reverse
            } else {
                Direction::Forward
            };
            MotorRun::Running {
                direction,
                speed: 0,
            }
        }
        _ => MotorRun::Stopped,
    }
}

/// Three winding pins fed from `potential`: three different potentials run
/// the motor in the direction of their sequence, two hum.
fn three_phase(
    project: &Project,
    set: &[u16],
    potential: &dyn Fn(u16) -> Option<PotentialId>,
    speed: u8,
) -> MotorRun {
    let fed: Vec<Option<PotentialId>> = set.iter().map(|&p| potential(p)).collect();
    let mut distinct: Vec<PotentialId> = fed.iter().flatten().copied().collect();
    distinct.sort();
    distinct.dedup();
    match (set.len(), distinct.len()) {
        (3, 3) => {
            // Rank of each pin's potential: the name's digit, else the
            // order of the three ids.
            let rank = |id: PotentialId| -> i64 {
                phase_digit(project, id)
                    .unwrap_or_else(|| distinct.iter().position(|&d| d == id).unwrap_or(0) as i64)
            };
            let r: Vec<i64> = fed.iter().flatten().map(|&p| rank(p)).collect();
            let step = |a: i64, b: i64| (b - a).rem_euclid(3);
            let direction = if step(r[0], r[1]) == 1 && step(r[1], r[2]) == 1 {
                Direction::Forward
            } else {
                Direction::Reverse
            };
            MotorRun::Running { direction, speed }
        }
        (_, 2) | (_, 3) => MotorRun::PhaseLoss,
        _ => MotorRun::Stopped,
    }
}

/// Which delay a timer counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerKind {
    /// Its contacts operate once the coil has been on for the set time.
    OnDelay,
    /// Its contacts release the set time after the coil went off.
    OffDelay,
}

/// A timer that is counting right now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimerProgress {
    pub kind: TimerKind,
    /// Share of the set time that has passed, 0 to 1 (exclusive of 1: a
    /// finished timer is not reported).
    pub elapsed: f64,
    /// Seconds still to go.
    pub remaining: f64,
}

/// The timers counting on `clock`, keyed by the coil symbol of their device.
/// A device is a timer when it has an on-delay or off-delay contact.
pub fn timer_progress(project: &Project, clock: &Clock) -> BTreeMap<SymbolId, TimerProgress> {
    let mut out = BTreeMap::new();
    for coil in project.symbols().filter(|s| is_coil(s)) {
        let Some(device) = coil.device() else {
            continue;
        };
        let kind = project
            .symbols()
            .filter(|s| s.device() == Some(device))
            .find_map(|s| {
                let name = s.symbol.0.rsplit('/').next().unwrap_or("");
                if name.contains("_delay_on") {
                    Some(TimerKind::OnDelay)
                } else if name.contains("_delay_off") {
                    Some(TimerKind::OffDelay)
                } else {
                    None
                }
            });
        let (Some(kind), Some((on, age))) = (kind, clock.coil_age(coil.id)) else {
            continue;
        };
        let wait = delay_seconds(project, device);
        let counting = match kind {
            TimerKind::OnDelay => on,
            TimerKind::OffDelay => !on,
        };
        if counting && wait > 0.0 && age < wait {
            out.insert(
                coil.id,
                TimerProgress {
                    kind,
                    elapsed: (age / wait).clamp(0.0, 1.0),
                    remaining: wait - age,
                },
            );
        }
    }
    out
}
