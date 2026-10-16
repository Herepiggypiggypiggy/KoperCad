//! The calculation sheet: the numbers behind the calculation problems.
//!
//! [`calc_checks`](crate::calc_checks) turns the engines of [`crate::calc`]
//! into problems and says nothing when an input is missing. This module
//! reads the same project properties and returns the whole answer instead:
//! per protected circuit a [`CircuitReport`] (cable rating, voltage drop,
//! breaking capacity, short circuit, disconnection and protective conductor,
//! each with its value, limit and standard clause), the smallest passing
//! section when a circuit fails, a sized starter per motor, and the panel's
//! short-circuit rating. What is missing is named, never defaulted into a
//! pass.
//!
//! The ampacity tables are planning values until the user loads their
//! licensed copy ([`TableSet::disclosure`] says which are in use).

use std::collections::BTreeSet;

use novo_core::{AnyId, DeviceId, Endpoint, Netlist, PotentialKind, Project, SymbolRole, WireId};

use crate::Context;
use crate::calc::Insulation;
use crate::calc::cable::Method;
use crate::calc::circuit::{
    CableData, CircuitDesign, CircuitReport, Device, evaluate, smallest_passing_section,
};
use crate::calc::curves::{Characteristic, McbCurve};
use crate::calc::drop::{DEFAULT_LIMIT_PERCENT, Supply};
use crate::calc::network::Network;
use crate::calc::sccr::{self, Component, DefaultTable, Kind, Panel, SccrResult};
use crate::calc::short_circuit::Trip;
use crate::calc::starter::{
    Catalog, Coordination, MotorSpec, NoProposal, Proposal, Route, Scheme, SizingInput,
    size_starter,
};
use crate::calc::tables::TableSet;
use crate::calc_checks::{
    AMBIENT_C, BREAKING_CAPACITY_KA, GROUPED_CIRCUITS, LET_THROUGH_I2T, LOAD_CURRENT_A, TRIP_CURVE,
    TRIP_FACTOR, VOLTAGE_DROP_PCT, device_number, device_text, installation_method_text, number,
    protected_wires, protective_section, supply_max_ka, supply_source, supply_voltage,
    wire_insulation, wire_length, wire_number, wire_section,
};
use crate::checks::protects_against_overload;
use crate::standards::keys;

/// The project and device property keys the calculations read, for the
/// screens that edit them.
pub mod inputs {
    pub use crate::calc_checks::{
        AC3_CURRENT_A, AMBIENT_C, BREAKING_CAPACITY_KA, GROUPED_CIRCUITS, INSTALL_METHOD,
        INSTANTANEOUS_TRIP_A, LENGTH_M, LET_THROUGH_I2T, LOAD_CURRENT_A, MAX_SCPD_A, SETTING_MAX_A,
        SETTING_MIN_A, SUPPLY_MIN_SHORT_CIRCUIT_KA, SUPPLY_NETWORK_SK_MVA_MAX,
        SUPPLY_NETWORK_SK_MVA_MIN, SUPPLY_SHORT_CIRCUIT_KA, SUPPLY_TRANSFORMER_KVA,
        SUPPLY_TRANSFORMER_PK_W, SUPPLY_UK_PERCENT, SUPPLY_VOLTAGE_V, TRIP_CURVE, TRIP_FACTOR,
        VOLTAGE_DROP_PCT,
    };
}

/// The standard clause each report row rests on, by row title.
pub fn clause(title: &str) -> &'static str {
    match title {
        "Overload" => "IEC 60364-4-43 §433.1",
        "Voltage drop" => "IEC 60364-5-52 Annex G",
        "Breaking capacity" => "IEC 60364-4-43 §434.5.1",
        "Short-circuit (maximum)" | "Short-circuit (minimum)" => "IEC 60364-4-43 §434.5.2",
        "Disconnection" => "IEC 60364-4-41 §411.4.4",
        "Protective conductor" => "IEC 60364-5-54 §543.1.2",
        _ => "",
    }
}

/// One protective device with the cable behind it, judged.
#[derive(Debug, Clone)]
pub struct CircuitEntry {
    pub device: DeviceId,
    pub tag: String,
    /// The wire the figures are for (the worst of those with enough data).
    pub wire: Option<WireId>,
    pub wire_label: String,
    /// How many wires behind the device had the data to be judged.
    pub judged_wires: usize,
    /// Inputs that are missing or assumed, named by property key.
    pub missing: Vec<String>,
    /// `None` while an essential input is missing.
    pub report: Option<CircuitReport>,
    /// The smallest listed section that passes everything, when the circuit
    /// fails. `Some(None)` when no listed section does.
    pub smallest_section_mm2: Option<Option<f64>>,
    pub section_mm2: Option<f64>,
}

/// A motor's starter sized from its current.
#[derive(Debug, Clone)]
pub struct StarterEntry {
    pub motor: DeviceId,
    pub tag: String,
    pub missing: Vec<String>,
    /// Defaults used for the cable route, one line each.
    pub notes: Vec<String>,
    pub result: Option<Result<Proposal, NoProposal>>,
}

/// The panel's short-circuit current rating.
#[derive(Debug, Clone)]
pub struct PanelEntry {
    pub result: SccrResult,
    /// Components counted.
    pub components: usize,
    /// Available at the supply, kA.
    pub available_ka: Option<f64>,
    /// The rating the project marks on the panel, kA.
    pub marked_ka: Option<f64>,
    /// Whether the calculated rating covers the available current.
    pub fit: Option<bool>,
}

/// Everything the Calculations window shows.
#[derive(Debug, Clone)]
pub struct CalcSheet {
    /// One line about the supply, `None` when it is not stated.
    pub supply: Option<String>,
    pub voltage_v: f64,
    pub drop_limit_percent: f64,
    /// Project inputs that are missing, with the property key.
    pub missing: Vec<String>,
    pub circuits: Vec<CircuitEntry>,
    pub starters: Vec<StarterEntry>,
    pub panel: PanelEntry,
    /// Which ampacity tables the numbers were read from.
    pub tables: String,
    pub planning_values: bool,
}

impl CalcSheet {
    /// Circuits with a failing row.
    pub fn failing(&self) -> usize {
        self.circuits
            .iter()
            .filter(|c| c.report.as_ref().is_some_and(|r| r.has_failure()))
            .count()
    }
}

/// What to size starters for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StarterChoice {
    pub scheme: Scheme,
    pub coordination: Coordination,
}

impl Default for StarterChoice {
    fn default() -> Self {
        StarterChoice {
            scheme: Scheme::FuseContactorRelay,
            coordination: Coordination::Type1,
        }
    }
}

/// Builds the sheet for the project.
pub fn calc_sheet(
    project: &Project,
    netlist: &Netlist,
    starters: StarterChoice,
    tables: &TableSet,
) -> CalcSheet {
    let cx = Context::new(project);
    let props = &project.info().properties;
    let voltage = supply_voltage(project);
    let drop_limit = number(props, VOLTAGE_DROP_PCT)
        .filter(|l| *l > 0.0)
        .unwrap_or(DEFAULT_LIMIT_PERCENT);
    let source = supply_source(project);
    let network = source.clone().map(|s| Network::new(voltage, s));
    let mut missing = Vec::new();
    if network.is_none() {
        missing.push(
            "supply.short_circuit_ka, or supply.transformer_kva with supply.uk_percent".to_owned(),
        );
    }
    let supply = source.as_ref().and_then(|_| {
        let max = supply_max_ka(project)?;
        Some(format!(
            "{} V, {:.1} kA maximum prospective short-circuit current",
            crate::calc::num(voltage),
            max
        ))
    });

    let mut circuits = Vec::new();
    let mut done: BTreeSet<DeviceId> = BTreeSet::new();
    for symbol in project.symbols().filter(|s| protects_against_overload(s)) {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        if !done.insert(device) {
            continue;
        }
        circuits.push(circuit_entry(
            &cx,
            netlist,
            network.as_ref(),
            device,
            voltage,
            drop_limit,
            tables,
        ));
    }
    circuits.sort_by(|a, b| a.tag.cmp(&b.tag));

    let starters = motor_starters(&cx, network.as_ref(), voltage, starters, tables);
    let panel = panel_entry(&cx);

    CalcSheet {
        supply,
        voltage_v: voltage,
        drop_limit_percent: drop_limit,
        missing,
        circuits,
        starters,
        panel,
        tables: tables.disclosure(),
        planning_values: !tables.fully_licensed(),
    }
}

fn is_fuse(project: &Project, device: DeviceId, cx: &Context<'_>) -> bool {
    cx.by_device.get(&device).is_some_and(|symbols| {
        symbols.iter().any(|s| {
            let key = s.symbol.0.as_str();
            key.contains("/protection/")
                && key
                    .rsplit('/')
                    .next()
                    .is_some_and(|n| n.starts_with("fuse"))
        })
    }) && project.device(device).is_some()
}

fn circuit_entry(
    cx: &Context<'_>,
    netlist: &Netlist,
    network: Option<&Network>,
    device: DeviceId,
    voltage: f64,
    drop_limit: f64,
    tables: &TableSet,
) -> CircuitEntry {
    let project = cx.project;
    let tag = project
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default();
    let mut missing = Vec::new();
    let rated = device_number(project, device, "rated_current_a").filter(|a| *a > 0.0);
    if rated.is_none() {
        missing.push("rated_current_a on the device or its part".to_owned());
    }
    if network.is_none() {
        missing.push("supply (project electrical inputs)".to_owned());
    }
    let mut entry = CircuitEntry {
        device,
        tag: tag.clone(),
        wire: None,
        wire_label: String::new(),
        judged_wires: 0,
        missing: Vec::new(),
        report: None,
        smallest_section_mm2: None,
        section_mm2: None,
    };
    let wires: BTreeSet<WireId> = cx
        .by_device
        .get(&device)
        .into_iter()
        .flatten()
        .flat_map(|s| protected_wires(project, netlist, s))
        .collect();
    let any_section = wires
        .iter()
        .filter_map(|w| project.wire(*w))
        .any(|w| wire_section(project, w).is_some());
    let any_length = wires
        .iter()
        .filter_map(|w| project.wire(*w))
        .any(|w| wire_length(project, w).is_some());
    if wires.is_empty() {
        missing.push("a wire on the load side of the device".to_owned());
    } else {
        if !any_section {
            missing.push("cross-section on a wire behind the device".to_owned());
        }
        if !any_length {
            missing.push("length_m on a wire behind the device".to_owned());
        }
    }
    let (Some(rated), Some(network)) = (rated, network) else {
        entry.missing = missing;
        return entry;
    };

    let trip_text = device_text(project, device, TRIP_CURVE);
    let trip = trip_text.as_deref().and_then(Trip::parse).or_else(|| {
        device_number(project, device, TRIP_FACTOR)
            .filter(|f| *f > 0.0)
            .map(Trip::Factor)
    });
    let characteristic = if is_fuse(project, device, cx) {
        Some(Characteristic::FuseGg { in_a: rated })
    } else {
        trip.and_then(McbCurve::from_trip)
            .map(|curve| Characteristic::Mcb { curve, in_a: rated })
    };
    let icu = device_number(project, device, BREAKING_CAPACITY_KA);
    if icu.is_none() {
        missing.push("breaking_capacity_ka on the device or its part".to_owned());
    }
    if characteristic.is_none() && trip.is_none() {
        missing.push("trip_curve (B, C, D) on the device".to_owned());
    }
    let device_data = Device {
        id: None,
        rated_a: rated,
        characteristic,
        trip,
        icu_ka: icu,
        let_through_i2t: device_number(project, device, LET_THROUGH_I2T),
    };

    // The wire with the most failures, then the thinnest, stands for the
    // circuit.
    let mut best: Option<(usize, f64, CircuitEntry)> = None;
    let mut judged = 0usize;
    let mut load_missing = false;
    let mut method_missing = false;
    for wire in wires.iter().filter_map(|w| project.wire(*w)) {
        let (Some(length), Some(section)) =
            (wire_length(project, wire), wire_section(project, wire))
        else {
            continue;
        };
        judged += 1;
        let mut notes = Vec::new();
        let potential = netlist
            .net_of_wire(wire)
            .and_then(|n| netlist.net(n).potential())
            .and_then(|p| project.potential(p));
        let (supply_kind, line_voltage) = match potential {
            Some(p) if p.voltage.is_some_and(|v| v > 0.0) => {
                let v = p.voltage.unwrap_or(voltage);
                match p.kind {
                    PotentialKind::Line if v >= 380.0 => (Supply::ThreePhase, v),
                    PotentialKind::Line | PotentialKind::DcPositive | PotentialKind::Signal => {
                        (Supply::TwoWire, v)
                    }
                    _ => (Supply::ThreePhase, voltage),
                }
            }
            _ => {
                notes.push(format!(
                    "the net of the wire has no voltage: the supply's {} V, three-phase is assumed",
                    crate::calc::num(voltage)
                ));
                (Supply::ThreePhase, voltage)
            }
        };
        let load = number(&wire.properties, LOAD_CURRENT_A).filter(|a| *a > 0.0);
        let load_a = load.unwrap_or_else(|| {
            load_missing = true;
            notes.push(format!(
                "load_current_a is not stated on the wire: the device rating {} A is used as the load, the most the device allows",
                crate::calc::num(rated)
            ));
            rated
        });
        let method = installation_method_text(project, wire)
            .or_else(|| {
                match project
                    .info()
                    .properties
                    .get(keys::DEFAULT_INSTALLATION_METHOD)
                {
                    Some(novo_core::Value::Text(t)) => Some(t.clone()),
                    _ => None,
                }
            })
            .as_deref()
            .and_then(Method::parse)
            .unwrap_or_else(|| {
                method_missing = true;
                notes.push(
                    "installation method is not stated: method C (clipped or on a tray) is assumed"
                        .to_owned(),
                );
                Method::C
            });
        let power_factor =
            wire_number(project, wire, "power_factor").filter(|pf| (0.1..=1.0).contains(pf));
        if power_factor.is_none() {
            notes.push("power factor 1 assumed for the voltage drop: exact for resistive loads; for inductive loads the real drop is larger on thick cables and smaller on thin ones (the reactive part is left out)".to_owned());
        }
        if number(&project.info().properties, VOLTAGE_DROP_PCT).is_none() {
            notes.push(format!(
                "voltage drop limit not stated: {DEFAULT_LIMIT_PERCENT} % assumed"
            ));
        }
        notes.push(
            "feeder impedance upstream of the device is not included: the supply is taken at the device"
                .to_owned(),
        );
        let pe = protective_section(project, wire);
        let design = CircuitDesign {
            name: tag.clone(),
            load_current_a: load_a,
            power_factor: power_factor.unwrap_or(1.0),
            supply: supply_kind,
            voltage: line_voltage,
            device: device_data.clone(),
            overload_setting_a: None,
            cable: CableData {
                section_mm2: section,
                insulation: wire_insulation(project, wire),
                method,
                ambient_c: wire_number(project, wire, AMBIENT_C).unwrap_or(30.0),
                grouped_circuits: wire_number(project, wire, GROUPED_CIRCUITS)
                    .filter(|n| *n >= 1.0)
                    .map_or(1, |n| n as u32),
                length_m: length,
                parallel: 1,
                pe_mm2: pe,
            },
            drop_limit_percent: drop_limit,
            notes,
        };
        let report = evaluate(network, network.root(), &design, tables);
        let failures = report.failures().count();
        let smallest = report
            .has_failure()
            .then(|| smallest_passing_section(network, network.root(), &design, tables))
            .map(|r| r.map(|(s, _)| s));
        let candidate = CircuitEntry {
            wire: Some(wire.id),
            wire_label: wire
                .number
                .as_deref()
                .filter(|n| !n.is_empty())
                .map_or_else(|| "unnumbered wire".to_owned(), |n| format!("wire {n}")),
            report: Some(report),
            smallest_section_mm2: smallest,
            section_mm2: Some(section),
            ..entry.clone()
        };
        let better = best
            .as_ref()
            .is_none_or(|(f, s, _)| failures > *f || (failures == *f && section < *s));
        if better {
            best = Some((failures, section, candidate));
        }
    }
    if load_missing {
        missing.push("load_current_a on the wire (the device rating is used instead)".to_owned());
    }
    if method_missing {
        missing.push("installation_method on the wire (method C is assumed)".to_owned());
    }
    let mut out = best.map_or(entry, |(_, _, e)| e);
    out.judged_wires = judged;
    out.missing = missing;
    out
}

fn motor_starters(
    cx: &Context<'_>,
    network: Option<&Network>,
    voltage: f64,
    choice: StarterChoice,
    tables: &TableSet,
) -> Vec<StarterEntry> {
    let project = cx.project;
    let catalog = Catalog::generic();
    let mut seen: BTreeSet<DeviceId> = BTreeSet::new();
    let mut out = Vec::new();
    for symbol in project
        .symbols()
        .filter(|s| s.symbol.0.contains("/motors/"))
    {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        if !seen.insert(device) {
            continue;
        }
        let tag = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        let mut missing = Vec::new();
        let mut notes = Vec::new();
        let current = project
            .rated_current_a(device)
            .or_else(|| device_number(project, device, keys::FLC_A))
            .filter(|a| *a > 0.0);
        let power = device_number(project, device, "power_kw").filter(|p| *p > 0.0);
        if current.is_none() && power.is_none() {
            missing.push("rated_current_a (nameplate) or power_kw on the motor".to_owned());
        }
        // The longest stated wire to the motor stands for the route.
        let mut route_wire = None;
        for s in cx.by_device.get(&device).into_iter().flatten() {
            for index in 0..s.pins.len() {
                let Ok(index) = u16::try_from(index) else {
                    break;
                };
                for w in project.wires_at(Endpoint::pin(s.id, index)) {
                    let Some(wire) = project.wire(w) else {
                        continue;
                    };
                    if let Some(l) = wire_length(project, wire)
                        && route_wire
                            .as_ref()
                            .is_none_or(|(len, _): &(f64, WireId)| l > *len)
                    {
                        route_wire = Some((l, wire.id));
                    }
                }
            }
        }
        if route_wire.is_none() {
            missing.push("length_m on a wire to the motor".to_owned());
        }
        if network.is_none() {
            missing.push("supply (project electrical inputs)".to_owned());
        }
        let result = match (network, route_wire) {
            (Some(network), Some((length, wire))) if missing.is_empty() => {
                let motor = MotorSpec {
                    power_kw: power.unwrap_or(0.0),
                    voltage,
                    rated_current_a: current,
                    efficiency: None,
                    cos_phi: None,
                    start_time_s: None,
                };
                let wire = project.wire(wire);
                let method = wire
                    .and_then(|w| installation_method_text(project, w))
                    .or_else(|| {
                        match project
                            .info()
                            .properties
                            .get(keys::DEFAULT_INSTALLATION_METHOD)
                        {
                            Some(novo_core::Value::Text(t)) => Some(t.clone()),
                            _ => None,
                        }
                    })
                    .as_deref()
                    .and_then(Method::parse);
                if method.is_none() {
                    notes.push("installation method not stated: method C assumed".to_owned());
                }
                let ambient = wire.and_then(|w| wire_number(project, w, AMBIENT_C));
                if ambient.is_none() {
                    notes.push("ambient temperature not stated: 30 \u{b0}C assumed".to_owned());
                }
                let grouped = wire
                    .and_then(|w| wire_number(project, w, GROUPED_CIRCUITS))
                    .filter(|n| *n >= 1.0);
                if grouped.is_none() {
                    notes.push("grouped circuits not stated: 1 assumed".to_owned());
                }
                let route = Route {
                    length_m: length,
                    insulation: wire
                        .map(|w| wire_insulation(project, w))
                        .unwrap_or(Insulation::Pvc),
                    method: method.unwrap_or(Method::C),
                    ambient_c: ambient.unwrap_or(30.0),
                    grouped_circuits: grouped.map_or(1, |n| n as u32),
                };
                Some(size_starter(&SizingInput {
                    motor: &motor,
                    scheme: choice.scheme,
                    coordination: choice.coordination,
                    network,
                    at: network.root(),
                    catalog: &catalog,
                    tables,
                    pairs: None,
                    route,
                    name: &tag,
                }))
            }
            _ => None,
        };
        out.push(StarterEntry {
            motor: device,
            tag,
            missing,
            notes,
            result,
        });
    }
    out.sort_by(|a, b| a.tag.cmp(&b.tag));
    out
}

/// The kind of power component a protective or switching symbol is.
fn sccr_kind(key: &str) -> Option<Kind> {
    let name = key.rsplit('/').next().unwrap_or_default();
    if key.contains("/protection/") {
        if name.starts_with("fuse") {
            Some(Kind::FuseHolder)
        } else if name.starts_with("overload_relay") {
            Some(Kind::OverloadRelay)
        } else if name.starts_with("motor_protection_breaker")
            || name.starts_with("motor_circuit_protector")
        {
            Some(Kind::MotorController)
        } else if name.starts_with("mcb") || name.starts_with("rcbo") {
            Some(Kind::CircuitBreaker)
        } else {
            None
        }
    } else if key.contains("/contactors/") && name.starts_with("main") {
        Some(Kind::MotorController)
    } else {
        None
    }
}

fn panel_entry(cx: &Context<'_>) -> PanelEntry {
    let project = cx.project;
    let mut seen: BTreeSet<DeviceId> = BTreeSet::new();
    let mut feeder = Vec::new();
    for symbol in project.symbols() {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        let Some(kind) = sccr_kind(symbol.symbol.0.as_str()) else {
            continue;
        };
        if !seen.insert(device) {
            continue;
        }
        let tag = project
            .device(device)
            .map(|d| d.tag.to_string())
            .unwrap_or_default();
        feeder.push(Component::new(
            &tag,
            kind,
            device_number(project, device, keys::SCCR_KA),
        ));
    }
    let components = feeder.len();
    let panel = Panel {
        feeder,
        branches: Vec::new(),
    };
    let result = sccr::panel_sccr(&panel, &DefaultTable::builtin());
    let available_ka = supply_max_ka(project);
    let fit = available_ka.and_then(|a| sccr::check_available(&result, a));
    PanelEntry {
        result,
        components,
        available_ka,
        marked_ka: number(&project.info().properties, keys::PANEL_SCCR_KA),
        fit,
    }
}

/// Marks to jump to for an entry: the device and its wire.
pub fn marks(entry: &CircuitEntry) -> Vec<AnyId> {
    let mut at = vec![AnyId::Device(entry.device)];
    at.extend(entry.wire.map(AnyId::Wire));
    at
}
