//! Electrical estimates: voltage drop and short-circuit rating. Both read
//! plain numbers from properties and say nothing when a number is missing,
//! so a project that has not entered lengths or ratings stays quiet.

use novo_core::{AnyId, DeviceId, Netlist, PotentialKind, Project, Properties, Value, Wire};

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

/// Copper at 70 degrees C, ohm mm2 per metre.
const RHO: f64 = 0.0225;
const DEFAULT_LIMIT_PCT: f64 = 5.0;

fn number(properties: &Properties, key: &str) -> Option<f64> {
    match properties.get(key)? {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn trim(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
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
        // line they pair with: counted there (factor 2), not twice.
        let factor = match potential.kind {
            PotentialKind::Line | PotentialKind::DcPositive | PotentialKind::Signal => {
                match potential.voltage {
                    Some(v) if potential.kind == PotentialKind::Line && v >= 380.0 => 3f64.sqrt(),
                    _ => 2.0,
                }
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
        let drop = factor * RHO * length * current / section;
        let percent = drop / voltage * 100.0;
        if percent > limit {
            out.push(Problem::new(
                ProblemKind::VoltageDrop,
                vec![AnyId::Wire(wire.id)],
                format!(
                    "{} m of {} mm² at {} A drops {} V, {}% of {} V (limit {}%)",
                    trim(length),
                    trim(section),
                    trim(current),
                    trim(drop),
                    trim((percent * 10.0).round() / 10.0),
                    trim(voltage),
                    trim(limit)
                ),
            ));
        }
    }
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
        if capacity + 1e-9 < supply {
            let mut at: Vec<AnyId> = cx
                .by_device
                .get(&device.id)
                .map(|s| s.iter().map(|s| AnyId::Symbol(s.id)).collect())
                .unwrap_or_default();
            if at.is_empty() {
                at.push(AnyId::Device(device.id));
            }
            out.push(Problem::new(
                ProblemKind::ShortCircuit,
                at,
                format!(
                    "{} breaks {} kA, but the supply can deliver {} kA",
                    device.tag,
                    trim(capacity),
                    trim(supply)
                ),
            ));
        }
    }
}
