//! Electrical checks: voltage drop, short-circuit rating, disconnection and
//! protective conductors, through the calculations in [`crate::calc`]. Both read
//! plain numbers from properties and say nothing when a number is missing,
//! so a project that has not entered lengths or ratings stays quiet.

use std::collections::BTreeMap;

use novo_core::{
    AnyId, DeviceId, Endpoint, Netlist, PotentialKind, Project, Properties, SymbolRole, Value, Wire,
};

use crate::calc::Finding;
use crate::calc::drop::{Run, Supply, check_drop};
use crate::calc::pe::check_pe_section;
use crate::calc::short_circuit::{
    Source, Trip, cable_impedance, check_breaking_capacity, check_disconnection, ik1_min,
};
use crate::{Context, Problem, ProblemKind};

/// Wire property: run length in metres. A wire on a cable core falls back to
/// the cable's `length_m`.
pub const LENGTH_M: &str = "length_m";
/// Wire property: design current in amperes. Without it the smallest rated
/// current of the devices on the net stands in for the load.
pub const LOAD_CURRENT_A: &str = "load_current_a";
/// Device or part property: breaking capacity in kiloamperes.
pub const BREAKING_CAPACITY_KA: &str = "breaking_capacity_ka";
/// Project property: prospective short-circuit current at the supply, kA.
pub const SUPPLY_SHORT_CIRCUIT_KA: &str = "supply.short_circuit_ka";
/// Project property: voltage drop limit in percent (default 5).
pub const VOLTAGE_DROP_PCT: &str = "check.voltage_drop_pct";

const DEFAULT_LIMIT_PCT: f64 = 5.0;

fn number(properties: &Properties, key: &str) -> Option<f64> {
    match properties.get(key)? {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn wire_length(project: &Project, wire: &Wire) -> Option<f64> {
    number(&wire.properties, LENGTH_M)
        .or_else(|| {
            let (cable, _) = project.core_of_wire(wire.id)?;
            project.cable(cable)?.length_m
        })
        .filter(|l| *l > 0.0)
}

fn breaking_capacity_ka(project: &Project, device: DeviceId) -> Option<f64> {
    let device = project.device(device)?;
    number(&device.properties, BREAKING_CAPACITY_KA).or_else(|| {
        device
            .parts
            .iter()
            .filter_map(|p| project.part(*p))
            .find_map(|p| number(&p.properties, BREAKING_CAPACITY_KA))
    })
}

pub(crate) fn voltage_drop(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let limit = number(&project.info().properties, VOLTAGE_DROP_PCT).unwrap_or(DEFAULT_LIMIT_PCT);
    for wire in project.wires() {
        let Some(length) = wire_length(project, wire) else {
            continue;
        };
        let section = wire.cross_section_mm2.or_else(|| {
            let (cable, core) = project.core_of_wire(wire.id)?;
            project.cable(cable)?.cores.get(core)?.cross_section_mm2
        });
        let Some(section) = section.filter(|s| *s > 0.0) else {
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
        // Neutral, earth and the DC return carry the same current as the
        // line they pair with: counted there, not twice.
        let supply = match potential.kind {
            PotentialKind::Line if potential.voltage.is_some_and(|v| v >= 380.0) => {
                Supply::ThreePhase
            }
            PotentialKind::Line | PotentialKind::DcPositive | PotentialKind::Signal => {
                Supply::TwoWire
            }
            _ => continue,
        };
        let Some(voltage) = potential.voltage.filter(|v| *v > 0.0) else {
            continue;
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
        let run = Run::new(length, section, current, supply, voltage);
        if let Some(finding) = check_drop(&run, limit) {
            out.push(from_finding(
                ProblemKind::VoltageDrop,
                vec![AnyId::Wire(wire.id)],
                finding,
                String::new(),
            ));
        }
    }
}

/// A problem carrying a calculation's severity and sentence.
fn from_finding(kind: ProblemKind, at: Vec<AnyId>, finding: Finding, subject: String) -> Problem {
    let mut problem = Problem::new(kind, at, format!("{subject}{}", finding.message));
    problem.severity = finding.severity;
    problem
}

pub(crate) fn short_circuit(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let Some(supply) =
        number(&project.info().properties, SUPPLY_SHORT_CIRCUIT_KA).filter(|k| *k > 0.0)
    else {
        return;
    };
    for device in project.devices() {
        if !cx.by_device.contains_key(&device.id) {
            continue;
        }
        let Some(capacity) = breaking_capacity_ka(project, device.id) else {
            continue;
        };
        if let Some(finding) = check_breaking_capacity(capacity, supply * 1000.0) {
            let mut at: Vec<AnyId> = cx
                .by_device
                .get(&device.id)
                .map(|s| s.iter().map(|s| AnyId::Symbol(s.id)).collect())
                .unwrap_or_default();
            if at.is_empty() {
                at.push(AnyId::Device(device.id));
            }
            out.push(from_finding(
                ProblemKind::ShortCircuit,
                at,
                finding,
                format!("{}: ", device.tag),
            ));
        }
    }
}

/// Device property: trip curve of a breaker, `B`, `C` or `D`.
pub const TRIP_CURVE: &str = "trip_curve";
/// Project property: line-to-line voltage of the supply, V (default 400).
pub const SUPPLY_VOLTAGE: &str = "supply.voltage_v";

/// Whether the smallest fault current at the end of each cable behind a
/// protective device still trips it at once. The supply gives the source
/// (`supply.short_circuit_ka`); the load-side wires with a length and a
/// cross-section give the cable, with a protective conductor of the same
/// section as the phase conductor (the common case). Quiet without a trip
/// curve, a rating, a supply or a length.
pub(crate) fn disconnection(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let info = &project.info().properties;
    let Some(ik) = number(info, SUPPLY_SHORT_CIRCUIT_KA).filter(|k| *k > 0.0) else {
        return;
    };
    let volts = number(info, SUPPLY_VOLTAGE).unwrap_or(400.0);
    let Some(source) = Source::from_ik(ik, volts) else {
        return;
    };
    // The worst (smallest) fault current per device, with the wire it is at.
    let mut worst: BTreeMap<DeviceId, (f64, novo_core::WireId, f64, Trip)> = BTreeMap::new();
    for symbol in project
        .symbols()
        .filter(|s| crate::checks::protects_against_overload(s))
    {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        let Some(d) = project.device(device) else {
            continue;
        };
        let Some(Value::Text(curve)) = d.properties.get(TRIP_CURVE) else {
            continue;
        };
        let Some(trip) = Trip::parse(curve) else {
            continue;
        };
        let Some(rated) = project.rated_current_a(device).filter(|a| *a > 0.0) else {
            continue;
        };
        for (index, pin) in symbol.pins.iter().enumerate() {
            let Ok(index) = u16::try_from(index) else {
                break;
            };
            if !pin.name.parse::<u32>().is_ok_and(|n| n % 2 == 0) {
                continue;
            }
            let Some(net) = netlist.net_of(Endpoint::pin(symbol.id, index)) else {
                continue;
            };
            for &id in &netlist.net(net).wires {
                let Some(wire) = project.wire(id) else {
                    continue;
                };
                let (Some(length), Some(section)) = (
                    wire_length(project, wire),
                    wire.cross_section_mm2.filter(|s| *s > 0.0),
                ) else {
                    continue;
                };
                let Some(z) = cable_impedance(section, length, 70.0) else {
                    continue;
                };
                let found = ik1_min(&source, &[z], &[z]);
                let entry = worst.entry(device);
                match entry {
                    std::collections::btree_map::Entry::Vacant(v) => {
                        v.insert((found, id, rated, trip));
                    }
                    std::collections::btree_map::Entry::Occupied(mut o) => {
                        if found < o.get().0 {
                            o.insert((found, id, rated, trip));
                        }
                    }
                }
            }
        }
    }
    for (device, (found, wire, rated, trip)) in worst {
        let Some(finding) = check_disconnection(found, rated, trip) else {
            continue;
        };
        let Some(d) = project.device(device) else {
            continue;
        };
        let mut at: Vec<AnyId> = cx
            .by_device
            .get(&device)
            .map(|s| s.iter().map(|s| AnyId::Symbol(s.id)).collect())
            .unwrap_or_default();
        at.push(AnyId::Wire(wire));
        out.push(from_finding(
            ProblemKind::DisconnectionTime,
            at,
            finding,
            format!("{}: ", d.tag),
        ));
    }
}

/// A cable's green-yellow core against its largest live core.
pub(crate) fn protective_conductors(cx: &Context<'_>, out: &mut Vec<Problem>) {
    for cable in cx.project.cables() {
        let is_pe =
            |name: &str| name.eq_ignore_ascii_case("GNYE") || name.eq_ignore_ascii_case("PE");
        let pe = cable
            .cores
            .iter()
            .filter(|c| is_pe(&c.name))
            .filter_map(|c| c.cross_section_mm2)
            .reduce(f64::min);
        let phase = cable
            .cores
            .iter()
            .filter(|c| !is_pe(&c.name) && !c.shield)
            .filter_map(|c| c.cross_section_mm2)
            .reduce(f64::max);
        let (Some(pe), Some(phase)) = (pe, phase) else {
            continue;
        };
        if let Some(finding) = check_pe_section(phase, pe) {
            out.push(from_finding(
                ProblemKind::ProtectiveConductor,
                vec![AnyId::Cable(cable.id)],
                finding,
                format!("{}: ", cable.tag),
            ));
        }
    }
}
