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
//! | `install_method` | wire or cable | reference method `B1`, `B2` or `C` for the cable rating (silent without it) |
//! | `ambient_c` | wire or cable | ambient air temperature, °C (30) |
//! | `grouped_circuits` | wire or cable | circuits bunched together, this one included (1) |
//! | `supply.short_circuit_ka` | project | prospective short-circuit current, kA |
//! | `supply.min_short_circuit_ka` | project | smallest short-circuit current the supply can deliver, kA |
//! | `supply.transformer_kva`, `supply.uk_percent` | project | feeding transformer; replaces `supply.short_circuit_ka` |
//! | `supply.transformer_pk_w` | project | transformer load losses, W (else R/X 0.2) |
//! | `supply.network_sk_mva_max`, `supply.network_sk_mva_min` | project | fault level of the network behind the transformer, MVA |
//! | `supply.voltage_v` | project | line-to-line voltage for the loop, V (400) |
//! | `check.voltage_drop_pct` | project | drop limit, percent (5) |

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{
    AnyId, DeviceId, Endpoint, NetId, Netlist, PotentialKind, Project, Properties, SymbolId,
    SymbolInstance, SymbolRole, Value, Wire, WireId,
};

use crate::calc::cable::{Circuit, Loaded, Method, check_overload};
use crate::calc::drop::{Run, Supply, check_drop};
use crate::calc::motor::{ShortCircuitDevice, Starter, check_starter};
use crate::calc::network::{Branch, Network, SourceModel, Transformer};
use crate::calc::pe::check_pe_section;
use crate::calc::short_circuit::{
    Trip, check_breaking_capacity, check_disconnection, check_withstand,
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
pub const INSTALL_METHOD: &str = "install_method";
pub const AMBIENT_C: &str = "ambient_c";
pub const GROUPED_CIRCUITS: &str = "grouped_circuits";
pub const SUPPLY_MIN_SHORT_CIRCUIT_KA: &str = "supply.min_short_circuit_ka";
pub const SUPPLY_TRANSFORMER_KVA: &str = "supply.transformer_kva";
pub const SUPPLY_UK_PERCENT: &str = "supply.uk_percent";
pub const SUPPLY_TRANSFORMER_PK_W: &str = "supply.transformer_pk_w";
pub const SUPPLY_NETWORK_SK_MVA_MAX: &str = "supply.network_sk_mva_max";
pub const SUPPLY_NETWORK_SK_MVA_MIN: &str = "supply.network_sk_mva_min";
pub const SUPPLY_SHORT_CIRCUIT_KA: &str = "supply.short_circuit_ka";
pub const SUPPLY_VOLTAGE_V: &str = "supply.voltage_v";
pub const VOLTAGE_DROP_PCT: &str = "check.voltage_drop_pct";

const DEFAULT_SUPPLY_VOLTAGE: f64 = 400.0;

pub(crate) fn run(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    voltage_drop(cx, netlist, out);
    breaking_capacity(cx, out);
    protected_wires_checks(cx, netlist, out);
    motor_starters(cx, netlist, out);
    protective_conductors(cx, out);
    cable_rating(cx, netlist, out);
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

/// The supply as a source model: a transformer (with the network behind it
/// when stated) or a stated short-circuit current, with the smallest
/// current the supply can deliver when given. `None` when neither is.
fn supply_source(project: &Project) -> Option<SourceModel> {
    let props = &project.info().properties;
    let voltage = supply_voltage(project);
    if let (Some(kva), Some(uk)) = (
        number(props, SUPPLY_TRANSFORMER_KVA),
        number(props, SUPPLY_UK_PERCENT),
    ) {
        let mut t = Transformer::new(kva, uk, voltage);
        t.pk_w = number(props, SUPPLY_TRANSFORMER_PK_W);
        if let (Some(max), Some(min)) = (
            number(props, SUPPLY_NETWORK_SK_MVA_MAX),
            number(props, SUPPLY_NETWORK_SK_MVA_MIN),
        ) {
            t.network_sk_mva = Some((max, min));
        }
        return SourceModel::from_transformer(&t);
    }
    let max = supply_ik_ka(project)?;
    let min = number(props, SUPPLY_MIN_SHORT_CIRCUIT_KA).filter(|m| *m > 0.0 && *m <= max);
    SourceModel::from_short_circuit(max, min, voltage)
}

/// The largest three-phase short-circuit current the supply can deliver at
/// its terminals, kA.
fn supply_max_ka(project: &Project) -> Option<f64> {
    let source = supply_source(project)?;
    let network = Network::new(supply_voltage(project), source);
    network.fault(network.root()).map(|f| f.ik3_max_a / 1000.0)
}

/// Voltage drop of every wire with a length, a cross-section, a load and a
/// net of known voltage. Neutral, earth and DC return carry the same
/// current as the line they pair with and are counted there.
fn voltage_drop(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let limit = number(&project.info().properties, VOLTAGE_DROP_PCT)
        .filter(|l| *l > 0.0)
        .unwrap_or(crate::calc::drop::DEFAULT_LIMIT_PERCENT);
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
    let Some(supply) = supply_max_ka(project) else {
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
    let source = supply_source(project);
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
        let (Some(source), Some(rated)) = (
            source.clone(),
            device_number(project, device, "rated_current_a"),
        ) else {
            continue;
        };
        let trip = device_text(project, device, TRIP_CURVE)
            .and_then(|t| Trip::parse(&t))
            .or_else(|| {
                device_number(project, device, TRIP_FACTOR)
                    .filter(|f| *f > 0.0)
                    .map(Trip::Factor)
            });
        let Some(trip) = trip else {
            continue;
        };
        let network = Network::new(voltage, source);
        let mut worst: Option<(f64, WireId)> = None;
        for wire in wires.iter().filter_map(|id| project.wire(*id)) {
            let (Some(length), Some(section)) =
                (wire_length(project, wire), wire_section(project, wire))
            else {
                continue;
            };
            let pe_section = protective_section(project, wire).unwrap_or(section);
            let Some(branch) = Branch::cable(
                section,
                length,
                1,
                wire_insulation(project, wire),
                Some(pe_section),
            ) else {
                continue;
            };
            let mut net = network.clone();
            let end = net.add_node(net.root(), "wire", branch);
            let Some(ik) = net.fault(end).and_then(|f| f.ik1_min_a) else {
                continue;
            };
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

/// `Ib ≤ In ≤ Iz` for the wires behind each protective device whose
/// installation is described (`install_method`, with `ambient_c` and
/// `grouped_circuits` when they differ from the reference). A wire without
/// an installation method says nothing: the rating depends on it, and a
/// default would be a guess.
fn cable_rating(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let mut done: BTreeSet<DeviceId> = BTreeSet::new();
    for symbol in project.symbols().filter(|s| protects_against_overload(s)) {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        if !done.insert(device) {
            continue;
        }
        let Some(rated) = device_number(project, device, "rated_current_a").filter(|a| *a > 0.0)
        else {
            continue;
        };
        let tag = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        let wires: BTreeSet<WireId> = cx
            .by_device
            .get(&device)
            .into_iter()
            .flatten()
            .flat_map(|s| protected_wires(project, netlist, s))
            .collect();
        for wire in wires.iter().filter_map(|id| project.wire(*id)) {
            let Some(section) = wire_section(project, wire) else {
                continue;
            };
            let Some(method) = wire_text(project, wire, INSTALL_METHOD)
                .as_deref()
                .and_then(Method::parse)
            else {
                continue;
            };
            let loaded = match wire_loaded(project, netlist, wire) {
                Some(l) => l,
                None => continue,
            };
            let circuit = Circuit {
                design_current: number(&wire.properties, LOAD_CURRENT_A).unwrap_or(0.0),
                device_rating: rated,
                section_mm2: section,
                insulation: wire_insulation(project, wire),
                method,
                loaded,
                ambient_c: wire_number(project, wire, AMBIENT_C).unwrap_or(30.0),
                grouped_circuits: wire_number(project, wire, GROUPED_CIRCUITS)
                    .filter(|n| *n >= 1.0)
                    .map_or(1, |n| n as u32),
            };
            for finding in check_overload(&circuit) {
                let mut finding = finding;
                finding.message = format!("{tag}: {}", finding.message);
                let mut at = marks(cx, device);
                at.push(AnyId::Wire(wire.id));
                out.push(problem(ProblemKind::CableRating, finding, at));
            }
        }
    }
}

/// A number on the wire, else on its cable.
fn wire_number(project: &Project, wire: &Wire, key: &str) -> Option<f64> {
    number(&wire.properties, key).or_else(|| {
        let (cable, _) = project.core_of_wire(wire.id)?;
        number(&project.cable(cable)?.properties, key)
    })
}

fn wire_text(project: &Project, wire: &Wire, key: &str) -> Option<String> {
    text(&wire.properties, key).map(str::to_owned).or_else(|| {
        let (cable, _) = project.core_of_wire(wire.id)?;
        text(&project.cable(cable)?.properties, key).map(str::to_owned)
    })
}

/// Two loaded conductors for single-phase and DC nets, three for a 400 V
/// line net. `None` when the net or its potential is not known.
fn wire_loaded(project: &Project, netlist: &Netlist, wire: &Wire) -> Option<Loaded> {
    let net = netlist.net_of_wire(wire)?;
    let potential = project.potential(netlist.net(net).potential()?)?;
    let voltage = potential.voltage?;
    match potential.kind {
        PotentialKind::Line if voltage >= 380.0 => Some(Loaded::Three),
        PotentialKind::Line | PotentialKind::DcPositive | PotentialKind::Signal => {
            Some(Loaded::Two)
        }
        _ => None,
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
