//! Reads the project into the pure calculations of [`crate::calc`] and
//! turns their findings into problems. Every check here says nothing when a
//! number it needs is missing, so a project that has not entered lengths,
//! currents or ratings stays quiet.
//!
//! Numbers come from properties:
//!
//! | Key | On | Meaning |
//! | --- | --- | --- |
//! | `length_m` | wire (else its cable's length) | run length, m |
//! | `load_current_a` | wire | design current, A |
//! | `rated_current_a` | device or part | rated current, A |
//! | `breaking_capacity_ka` | device or part | breaking capacity, kA |
//! | `trip_curve` | device or part | `B`, `C` or `D` |
//! | `trip_factor` | device or part | instantaneous trip as a multiple of the rating |
//! | `let_through_i2t` | device or part | let-through energy at the supply, A²s |
//! | `setting_min_a`, `setting_max_a` | overload relay | setting range, A |
//! | `ac3_current_a` | contactor | AC-3 current, A |
//! | `max_scpd_a` | contactor or relay | largest coordinated fuse, A |
//! | `instantaneous_trip_a` | breaker | current for instant tripping, A |
//! | `insulation` | wire or cable | `PVC` (default) or `XLPE` |
//! | `supply.short_circuit_ka` | project | prospective short-circuit current, kA |
//! | `supply.voltage_v` | project | line-to-line voltage for the loop, V (400) |
//! | `check.voltage_drop_pct` | project | drop limit, percent (5) |

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{
    AnyId, DeviceId, Endpoint, NetId, Netlist, PotentialKind, Project, Properties, SymbolId,
    SymbolInstance, SymbolRole, Value, Wire, WireId,
};

use crate::calc::drop::{Run, Supply, check_drop};
use crate::calc::motor::{ShortCircuitDevice, Starter, check_starter};
use crate::calc::pe::check_pe_section;
use crate::calc::short_circuit::{
    Source, Trip, cable_impedance, check_breaking_capacity, check_disconnection, check_withstand,
    ik1_min,
};
use crate::calc::{Finding, Insulation};
use crate::checks::protects_against_overload;
use crate::{Context, Problem, ProblemKind};

pub const LENGTH_M: &str = "length_m";
pub const LOAD_CURRENT_A: &str = "load_current_a";
pub const BREAKING_CAPACITY_KA: &str = "breaking_capacity_ka";
pub const TRIP_CURVE: &str = "trip_curve";
pub const TRIP_FACTOR: &str = "trip_factor";
pub const LET_THROUGH_I2T: &str = "let_through_i2t";
pub const SETTING_MIN_A: &str = "setting_min_a";
pub const SETTING_MAX_A: &str = "setting_max_a";
pub const AC3_CURRENT_A: &str = "ac3_current_a";
pub const MAX_SCPD_A: &str = "max_scpd_a";
pub const INSTANTANEOUS_TRIP_A: &str = "instantaneous_trip_a";
pub const INSULATION: &str = "insulation";
pub const SUPPLY_SHORT_CIRCUIT_KA: &str = "supply.short_circuit_ka";
pub const SUPPLY_VOLTAGE_V: &str = "supply.voltage_v";
pub const VOLTAGE_DROP_PCT: &str = "check.voltage_drop_pct";

const DEFAULT_DROP_LIMIT_PCT: f64 = 5.0;
const DEFAULT_SUPPLY_VOLTAGE: f64 = 400.0;
/// Conductor temperature for the earth-fault loop, °C.
const LOOP_TEMPERATURE_C: f64 = 80.0;

pub(crate) fn run(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    voltage_drop(cx, netlist, out);
    breaking_capacity(cx, out);
    protected_wires_checks(cx, netlist, out);
    motor_starters(cx, netlist, out);
    protective_conductors(cx, out);
}

fn number(properties: &Properties, key: &str) -> Option<f64> {
    match properties.get(key)? {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn text<'a>(properties: &'a Properties, key: &str) -> Option<&'a str> {
    match properties.get(key)? {
        Value::Text(t) => Some(t.as_str()),
        _ => None,
    }
}

/// A number on a device, else on the first of its parts that has it.
fn device_number(project: &Project, device: DeviceId, key: &str) -> Option<f64> {
    let device = project.device(device)?;
    number(&device.properties, key).or_else(|| {
        device
            .parts
            .iter()
            .filter_map(|p| project.part(*p))
            .find_map(|p| number(&p.properties, key))
    })
}

fn device_text(project: &Project, device: DeviceId, key: &str) -> Option<String> {
    let device = project.device(device)?;
    text(&device.properties, key)
        .map(str::to_owned)
        .or_else(|| {
            device
                .parts
                .iter()
                .filter_map(|p| project.part(*p))
                .find_map(|p| text(&p.properties, key).map(str::to_owned))
        })
}

fn wire_length(project: &Project, wire: &Wire) -> Option<f64> {
    number(&wire.properties, LENGTH_M)
        .or_else(|| {
            let (cable, _) = project.core_of_wire(wire.id)?;
            project.cable(cable)?.length_m
        })
        .filter(|l| *l > 0.0)
}

fn wire_section(project: &Project, wire: &Wire) -> Option<f64> {
    wire.cross_section_mm2
        .or_else(|| {
            let (cable, core) = project.core_of_wire(wire.id)?;
            project.cable(cable)?.cores.get(core)?.cross_section_mm2
        })
        .filter(|s| *s > 0.0)
}

fn wire_insulation(project: &Project, wire: &Wire) -> Insulation {
    let name = text(&wire.properties, INSULATION)
        .or_else(|| {
            let (cable, _) = project.core_of_wire(wire.id)?;
            text(&project.cable(cable)?.properties, INSULATION)
        })
        .unwrap_or_default()
        .to_ascii_uppercase();
    if name == "XLPE" || name == "EPR" {
        Insulation::Xlpe
    } else {
        Insulation::Pvc
    }
}

/// A finding as a problem on `at`, keeping the finding's own severity (a
/// drop far above the limit is an error, a little above a warning).
fn problem(kind: ProblemKind, finding: Finding, at: Vec<AnyId>) -> Problem {
    let mut problem = Problem::new(kind, at, finding.message);
    problem.severity = finding.severity;
    problem
}

/// The symbols of a device as marks, or the device itself when undrawn.
fn marks(cx: &Context<'_>, device: DeviceId) -> Vec<AnyId> {
    let mut at: Vec<AnyId> = cx
        .by_device
        .get(&device)
        .map(|symbols| symbols.iter().map(|s| AnyId::Symbol(s.id)).collect())
        .unwrap_or_default();
    if at.is_empty() {
        at.push(AnyId::Device(device));
    }
    at
}

fn supply_ik_ka(project: &Project) -> Option<f64> {
    number(&project.info().properties, SUPPLY_SHORT_CIRCUIT_KA).filter(|k| *k > 0.0)
}

fn supply_voltage(project: &Project) -> f64 {
    number(&project.info().properties, SUPPLY_VOLTAGE_V)
        .filter(|v| *v > 0.0)
        .unwrap_or(DEFAULT_SUPPLY_VOLTAGE)
}

/// Voltage drop of every wire with a length, a cross-section, a load and a
/// net of known voltage. Neutral, earth and DC return carry the same
/// current as the line they pair with and are counted there.
fn voltage_drop(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let limit = number(&project.info().properties, VOLTAGE_DROP_PCT)
        .filter(|l| *l > 0.0)
        .unwrap_or(DEFAULT_DROP_LIMIT_PCT);
    for wire in project.wires() {
        let (Some(length), Some(section)) =
            (wire_length(project, wire), wire_section(project, wire))
        else {
            continue;
        };
        let Some(net) = netlist.net_of_wire(wire) else {
            continue;
        };
        let Some(potential) = netlist
            .net(net)
            .potential()
            .and_then(|p| project.potential(p))
        else {
            continue;
        };
        let Some(voltage) = potential.voltage.filter(|v| *v > 0.0) else {
            continue;
        };
        let supply = match potential.kind {
            PotentialKind::Line if voltage >= 380.0 => Supply::ThreePhase,
            PotentialKind::Line | PotentialKind::DcPositive | PotentialKind::Signal => {
                Supply::TwoWire
            }
            _ => continue,
        };
        let current = number(&wire.properties, LOAD_CURRENT_A).or_else(|| {
            netlist
                .devices_on_net(project, net)
                .into_iter()
                .filter_map(|d| project.rated_current_a(d))
                .filter(|a| *a > 0.0)
                .min_by(f64::total_cmp)
        });
        let Some(current) = current else { continue };
        let mut run = Run::new(length, section, current, supply, voltage);
        run.temperature_c = wire_insulation(project, wire).max_temperature();
        if let Some(finding) = check_drop(&run, limit) {
            out.push(problem(
                ProblemKind::VoltageDrop,
                finding,
                vec![AnyId::Wire(wire.id)],
            ));
        }
    }
}

/// A device that cannot break the supply's short-circuit current. The
/// current is not reduced along cables, which errs on the safe side.
fn breaking_capacity(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let Some(supply) = supply_ik_ka(project) else {
        return;
    };
    for device in project.devices() {
        if !cx.by_device.contains_key(&device.id) {
            continue;
        }
        let Some(capacity) = device_number(project, device.id, BREAKING_CAPACITY_KA) else {
            continue;
        };
        if let Some(finding) = check_breaking_capacity(capacity, supply * 1000.0) {
            let mut finding = finding;
            finding.message = format!("{}: {}", device.tag, finding.message);
            out.push(problem(
                ProblemKind::ShortCircuit,
                finding,
                marks(cx, device.id),
            ));
        }
    }
}

/// Wires that end on a protective device or lie on its load side (the
/// even-numbered connection points, IEC 60445), through terminals and cable
/// cores up to the next device.
fn protected_wires(
    project: &Project,
    netlist: &Netlist,
    symbol: &SymbolInstance,
) -> BTreeSet<WireId> {
    let mut wires = BTreeSet::new();
    for (index, pin) in symbol.pins.iter().enumerate() {
        let Ok(index) = u16::try_from(index) else {
            break;
        };
        let end = Endpoint::pin(symbol.id, index);
        wires.extend(project.wires_at(end));
        let load_side = pin.name.parse::<u32>().is_ok_and(|n| n % 2 == 0);
        if load_side && let Some(net) = netlist.net_of(end) {
            wires.extend(netlist.net(net).wires.iter().copied());
        }
    }
    wires
}

/// Disconnection time and thermal withstand of the wires behind each
/// overcurrent protective device. One problem per device and rule, naming
/// the worst wire.
fn protected_wires_checks(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let supply_ka = supply_ik_ka(project);
    let voltage = supply_voltage(project);
    let mut done: BTreeSet<DeviceId> = BTreeSet::new();
    for symbol in project.symbols().filter(|s| protects_against_overload(s)) {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        if !done.insert(device) {
            continue;
        }
        let wires: BTreeSet<WireId> = cx
            .by_device
            .get(&device)
            .into_iter()
            .flatten()
            .flat_map(|s| protected_wires(project, netlist, s))
            .collect();
        let tag = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();

        // Thermal withstand: the thinnest wire against the let-through.
        if let Some(i2t) = device_number(project, device, LET_THROUGH_I2T) {
            let thinnest = wires
                .iter()
                .filter_map(|id| project.wire(*id))
                .filter_map(|w| Some((w, wire_section(project, w)?)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((wire, section)) = thinnest
                && let Some(mut finding) =
                    check_withstand(section, wire_insulation(project, wire), i2t)
            {
                finding.message = format!("{tag}: {}", finding.message);
                let mut at = marks(cx, device);
                at.push(AnyId::Wire(wire.id));
                out.push(problem(ProblemKind::ThermalWithstand, finding, at));
            }
        }

        // Disconnection: the wire with the weakest earth-fault current.
        let (Some(supply_ka), Some(rated)) =
            (supply_ka, device_number(project, device, "rated_current_a"))
        else {
            continue;
        };
        let trip = device_text(project, device, TRIP_CURVE)
            .and_then(|t| Trip::parse(&t))
            .or_else(|| {
                device_number(project, device, TRIP_FACTOR)
                    .filter(|f| *f > 0.0)
                    .map(Trip::Factor)
            });
        let (Some(trip), Some(source)) = (trip, Source::from_ik(supply_ka, voltage)) else {
            continue;
        };
        let mut worst: Option<(f64, WireId)> = None;
        for wire in wires.iter().filter_map(|id| project.wire(*id)) {
            let (Some(length), Some(section)) =
                (wire_length(project, wire), wire_section(project, wire))
            else {
                continue;
            };
            let pe_section = protective_section(project, wire).unwrap_or(section);
            let (Some(phase), Some(pe)) = (
                cable_impedance(section, length, LOOP_TEMPERATURE_C),
                cable_impedance(pe_section, length, LOOP_TEMPERATURE_C),
            ) else {
                continue;
            };
            let ik = ik1_min(&source, &[phase], &[pe]);
            if worst.is_none_or(|(w, _)| ik < w) {
                worst = Some((ik, wire.id));
            }
        }
        if let Some((ik, wire)) = worst
            && let Some(mut finding) = check_disconnection(ik, rated, trip)
        {
            finding.message = format!("{tag}: {}", finding.message);
            let mut at = marks(cx, device);
            at.push(AnyId::Wire(wire));
            out.push(problem(ProblemKind::DisconnectionTime, finding, at));
        }
    }
}

/// The protective core's section in the cable that carries `wire`.
fn protective_section(project: &Project, wire: &Wire) -> Option<f64> {
    let (cable, _) = project.core_of_wire(wire.id)?;
    project
        .cable(cable)?
        .cores
        .iter()
        .find(|core| is_protective_core(&core.name))
        .and_then(|core| core.cross_section_mm2)
        .filter(|s| *s > 0.0)
}

fn is_protective_core(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_uppercase().as_str(),
        "PE" | "GNYE" | "GN/YE" | "GNGE" | "GN-YE" | "GNYL"
    )
}

/// A protective core thinner than the rule for the thickest phase core of
/// its cable.
fn protective_conductors(cx: &Context<'_>, out: &mut Vec<Problem>) {
    for cable in cx.project.cables() {
        let pe = cable
            .cores
            .iter()
            .filter(|c| !c.shield && is_protective_core(&c.name))
            .filter_map(|c| c.cross_section_mm2)
            .filter(|s| *s > 0.0)
            .min_by(f64::total_cmp);
        let phase = cable
            .cores
            .iter()
            .filter(|c| !c.shield && !is_protective_core(&c.name))
            .filter_map(|c| c.cross_section_mm2)
            .max_by(f64::total_cmp);
        let (Some(pe), Some(phase)) = (pe, phase) else {
            continue;
        };
        if let Some(mut finding) = check_pe_section(phase, pe) {
            finding.message = format!("{}: {}", cable.tag, finding.message);
            out.push(problem(
                ProblemKind::ProtectiveConductor,
                finding,
                vec![AnyId::Cable(cable.id)],
            ));
        }
    }
}

/// What a protective or switching symbol is in a motor starter.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Role {
    Relay,
    Contactor,
    Fuse,
    Breaker,
}

fn role_of(symbol: &SymbolInstance) -> Option<Role> {
    let key = symbol.symbol.0.as_str();
    let name = key.rsplit('/').next().unwrap_or_default();
    if key.contains("/protection/") {
        if name.starts_with("overload_relay") || name.starts_with("motor_protection_breaker") {
            Some(Role::Relay)
        } else if name.starts_with("fuse") {
            Some(Role::Fuse)
        } else if name.starts_with("mcb")
            || name.starts_with("rcbo")
            || name.starts_with("motor_circuit_protector")
        {
            Some(Role::Breaker)
        } else {
            None
        }
    } else if key.contains("/contactors/") && name.starts_with("main") {
        Some(Role::Contactor)
    } else {
        None
    }
}

fn is_motor(symbol: &SymbolInstance) -> bool {
    symbol.symbol.0.contains("/motors/")
}

fn is_motor_protection_breaker(symbol: &SymbolInstance) -> bool {
    symbol
        .symbol
        .0
        .rsplit('/')
        .next()
        .is_some_and(|n| n.starts_with("motor_protection_breaker"))
}

/// Checks each motor with a rated current against the relay, contactor and
/// short-circuit device found upstream of it. Upstream means: a device
/// with an even-numbered connection point on the net the motor (or the
/// next device up) hangs on, followed through its odd-numbered points.
fn motor_starters(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    // Net -> symbols with an even-numbered point on it.
    let mut feeders: HashMap<NetId, Vec<&SymbolInstance>> = HashMap::new();
    for symbol in project.symbols() {
        if role_of(symbol).is_none() {
            continue;
        }
        for (index, pin) in symbol.pins.iter().enumerate() {
            let Ok(index) = u16::try_from(index) else {
                break;
            };
            if pin.name.parse::<u32>().is_ok_and(|n| n % 2 == 0)
                && let Some(net) = netlist.net_of(Endpoint::pin(symbol.id, index))
            {
                feeders.entry(net).or_default().push(symbol);
            }
        }
    }
    let nets_of = |symbol: &SymbolInstance, odd: Option<bool>| -> Vec<NetId> {
        let mut nets: Vec<NetId> = symbol
            .pins
            .iter()
            .enumerate()
            .filter(|(_, pin)| {
                odd.is_none_or(|want_odd| {
                    pin.name
                        .parse::<u32>()
                        .is_ok_and(|n| (n % 2 == 1) == want_odd)
                })
            })
            .filter_map(|(i, _)| netlist.net_of(Endpoint::pin(symbol.id, u16::try_from(i).ok()?)))
            .collect();
        nets.sort();
        nets.dedup();
        nets
    };

    let mut seen: BTreeSet<DeviceId> = BTreeSet::new();
    for motor in project.symbols().filter(|s| is_motor(s)) {
        let SymbolRole::Device { device, .. } = motor.role else {
            continue;
        };
        if !seen.insert(device) {
            continue;
        }
        let Some(current) = project
            .rated_current_a(device)
            .or_else(|| device_number(project, device, "rated_current_a"))
            .filter(|a| *a > 0.0)
        else {
            continue;
        };

        // Walk upstream, at most four hops: relay, contactor, fuse.
        let mut found: BTreeMap<Role, &SymbolInstance> = BTreeMap::new();
        let mut frontier = nets_of(motor, None);
        let mut visited: BTreeSet<SymbolId> = BTreeSet::new();
        for _ in 0..4 {
            let mut next = Vec::new();
            for net in &frontier {
                for symbol in feeders.get(net).into_iter().flatten() {
                    if !visited.insert(symbol.id) {
                        continue;
                    }
                    if let Some(role) = role_of(symbol) {
                        found.entry(role).or_insert(symbol);
                        // A motor protection breaker is relay and fuse in one.
                        next.extend(nets_of(symbol, Some(true)));
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }

        let device_of = |symbol: &SymbolInstance| match symbol.role {
            SymbolRole::Device { device, .. } => Some(device),
            _ => None,
        };
        let relay = found.get(&Role::Relay).copied();
        let contactor = found.get(&Role::Contactor).copied();
        let relay_device = relay.and_then(device_of);
        let contactor_device = contactor.and_then(device_of);
        let overload_range = relay_device.and_then(|d| {
            Some((
                device_number(project, d, SETTING_MIN_A)?,
                device_number(project, d, SETTING_MAX_A)?,
            ))
        });
        let short_circuit = if let Some(symbol) = relay.filter(|s| is_motor_protection_breaker(s)) {
            device_of(symbol).and_then(|d| {
                Some(ShortCircuitDevice::Breaker {
                    rated_a: device_number(project, d, "rated_current_a")?,
                    instantaneous_a: device_number(project, d, INSTANTANEOUS_TRIP_A).unwrap_or(0.0),
                })
            })
        } else if let Some(d) = found.get(&Role::Fuse).copied().and_then(device_of) {
            device_number(project, d, "rated_current_a")
                .map(|rated_a| ShortCircuitDevice::Fuse { rated_a })
        } else {
            found
                .get(&Role::Breaker)
                .copied()
                .and_then(device_of)
                .and_then(|d| {
                    Some(ShortCircuitDevice::Breaker {
                        rated_a: device_number(project, d, "rated_current_a")?,
                        instantaneous_a: device_number(project, d, INSTANTANEOUS_TRIP_A)
                            .unwrap_or(0.0),
                    })
                })
        };
        let starter = Starter {
            motor_current: current,
            overload_range,
            contactor_ac3_a: contactor_device
                .and_then(|d| device_number(project, d, AC3_CURRENT_A)),
            short_circuit,
            max_short_circuit_a: [contactor_device, relay_device]
                .into_iter()
                .flatten()
                .find_map(|d| device_number(project, d, MAX_SCPD_A)),
        };
        let tag = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        let mut chain: Vec<DeviceId> = vec![device];
        chain.extend(found.values().filter_map(|s| device_of(s)));
        chain.dedup();
        for mut finding in check_starter(&starter) {
            finding.message = format!("{tag}: {}", finding.message);
            let at: Vec<AnyId> = chain.iter().flat_map(|d| marks(cx, *d)).collect();
            out.push(problem(ProblemKind::MotorStarter, finding, at));
        }
    }
}
