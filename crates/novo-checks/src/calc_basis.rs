//! What the calculations know about each wire, for tools that propose values.
//!
//! The Autopilot (novo-reports) sizes wires the way the Calculations window
//! judges them, so both read the same project properties through the same
//! defaults. This module hands those facts out per wire: the protective
//! device that limits the current in it, the installation conditions, the
//! voltage of its net and the drop limit. Nothing here decides a size.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{DeviceId, Endpoint, NetId, Netlist, PotentialKind, Project, SymbolRole, WireId};

use crate::Context;
use crate::calc::Insulation;
use crate::calc::cable::Method;
use crate::calc::drop::DEFAULT_LIMIT_PERCENT;
use crate::calc::motor::full_load_current;
use crate::calc_checks::{
    AMBIENT_C, GROUPED_CIRCUITS, LOAD_CURRENT_A, SETTING_MAX_A, VOLTAGE_DROP_PCT, device_number,
    installation_method_text, number, protected_wires, supply_voltage, wire_insulation,
    wire_number,
};
use crate::checks::protects_against_overload;
use crate::standards::keys;

/// The protective device that limits the current in a wire.
#[derive(Debug, Clone, PartialEq)]
pub struct Protecting {
    pub device: DeviceId,
    pub tag: String,
    /// Rated current `In` of the device, A.
    pub rated_a: f64,
}

/// What kind of circuit a wire is in, from the devices on its net.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Circuit {
    /// A pin of a motor, a main contactor, a three-pole protective device or
    /// an overload relay is on the net.
    Power,
    /// Only operators, coils, contacts, lamps and single-pole protective
    /// devices are on the net.
    Control,
    /// No device pin on the net (a feed to a terminal).
    Unknown,
}

/// A device the current of a wire can be read from.
#[derive(Debug, Clone, PartialEq)]
pub struct Rated {
    pub tag: String,
    pub amps: f64,
    /// `rated`, `set to`, `motor nameplate` or `motor, estimated from kW`.
    pub what: &'static str,
}

/// Everything the sizing of one wire rests on.
#[derive(Debug, Clone, PartialEq)]
pub struct WireBasis {
    /// The device with the highest rating that protects the wire (a wire on
    /// the load side of one device and the line side of a smaller one is
    /// limited by the larger).
    pub protecting: Option<Protecting>,
    /// Rated devices reached from the wire's net through unrated fuses and
    /// terminals: where `protecting` is empty these say how much current the
    /// wire can see.
    pub neighbours: Vec<Rated>,
    pub circuit: Circuit,
    /// `load_current_a` stated on the wire or its cable, A.
    pub load_a: Option<f64>,
    pub insulation: Insulation,
    /// The installation method the wire or the project states.
    pub method: Option<Method>,
    pub ambient_c: f64,
    pub grouped_circuits: u32,
    /// Kind and voltage of the potential the wire's net carries.
    pub kind: Option<PotentialKind>,
    pub potential_voltage: Option<f64>,
}

/// The voltage-drop limit in percent: the project's, else the default.
pub fn drop_limit_percent(project: &Project) -> f64 {
    number(&project.info().properties, VOLTAGE_DROP_PCT)
        .filter(|l| *l > 0.0)
        .unwrap_or(DEFAULT_LIMIT_PERCENT)
}

/// Nominal supply voltage of the project, V.
pub fn supply_voltage_v(project: &Project) -> f64 {
    supply_voltage(project)
}

/// The facts for every wire of the project.
pub fn wire_basis(project: &Project, netlist: &Netlist) -> BTreeMap<WireId, WireBasis> {
    let cx = Context::new(project);
    let mut protecting: BTreeMap<WireId, Protecting> = BTreeMap::new();
    let mut done: BTreeSet<DeviceId> = BTreeSet::new();
    for symbol in project.symbols().filter(|s| protects_against_overload(s)) {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        if !done.insert(device) {
            continue;
        }
        let Some(rated) = device_number(project, device, "rated_current_a")
            .or_else(|| device_number(project, device, SETTING_MAX_A))
            .filter(|a| *a > 0.0)
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
        for wire in wires {
            let better = protecting.get(&wire).is_none_or(|p| rated > p.rated_a);
            if better {
                protecting.insert(
                    wire,
                    Protecting {
                        device,
                        tag: tag.clone(),
                        rated_a: rated,
                    },
                );
            }
        }
    }
    let default_method = match project
        .info()
        .properties
        .get(keys::DEFAULT_INSTALLATION_METHOD)
    {
        Some(novo_core::Value::Text(t)) => Some(t.clone()),
        _ => None,
    };
    let supply = supply_voltage(project);
    let mut by_net: BTreeMap<NetId, (Circuit, Vec<Rated>)> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for wire in project.wires() {
        let (circuit, neighbours) = netlist
            .net_of_wire(wire)
            .map(|n| {
                by_net
                    .entry(n)
                    .or_insert_with(|| net_facts(project, netlist, n, supply))
                    .clone()
            })
            .unwrap_or((Circuit::Unknown, Vec::new()));
        let potential = netlist
            .net_of_wire(wire)
            .and_then(|n| netlist.net(n).potential())
            .and_then(|p| project.potential(p));
        out.insert(
            wire.id,
            WireBasis {
                protecting: protecting.remove(&wire.id),
                neighbours,
                circuit,
                load_a: wire_number(project, wire, LOAD_CURRENT_A).filter(|a| *a > 0.0),
                insulation: wire_insulation(project, wire),
                method: installation_method_text(project, wire)
                    .or_else(|| default_method.clone())
                    .as_deref()
                    .and_then(Method::parse),
                ambient_c: wire_number(project, wire, AMBIENT_C).unwrap_or(30.0),
                grouped_circuits: wire_number(project, wire, GROUPED_CIRCUITS)
                    .filter(|n| *n >= 1.0)
                    .map_or(1, |n| n as u32),
                kind: potential.map(|p| p.kind),
                potential_voltage: potential.and_then(|p| p.voltage).filter(|v| *v > 0.0),
            },
        );
    }
    out
}

/// How many unrated devices, contacts and terminals a search passes.
const MAX_HOPS: u8 = 8;

/// Whether a symbol is part of a main (power) circuit.
fn is_power_symbol(key: &str) -> bool {
    let name = key.rsplit('/').next().unwrap_or_default();
    key.contains("/motors/")
        || key.contains("/contactors/main")
        || name.contains("_3p")
        || name.contains("_4p")
        || name.starts_with("overload_relay")
        || name.starts_with("motor_protection_breaker")
}

/// The current a device's data gives, A, with what it means.
fn device_current(
    project: &Project,
    device: DeviceId,
    key: &str,
    supply: f64,
) -> Option<(f64, &'static str)> {
    if key.contains("/motors/") {
        if let Some(a) = project
            .rated_current_a(device)
            .or_else(|| device_number(project, device, keys::FLC_A))
            .filter(|a| *a > 0.0)
        {
            return Some((a, "motor nameplate"));
        }
        let kw = device_number(project, device, "power_kw")
            .or_else(|| device_number(project, device, "rated_power_kw"))
            .filter(|p| *p > 0.0)?;
        // Efficiency and power factor of 0.8 are on the low side for a
        // standard motor, so the current is on the high side.
        return full_load_current(kw, supply, 0.8, 0.8).map(|a| (a, "motor, estimated from kW"));
    }
    if let Some(a) = project.rated_current_a(device).filter(|a| *a > 0.0) {
        return Some((a, "rated"));
    }
    device_number(project, device, SETTING_MAX_A)
        .filter(|a| *a > 0.0)
        .map(|a| (a, "set to"))
}

/// Circuit kind of a net and the rated devices reachable from it through
/// unrated fuses, contacts and terminals (at most [`MAX_HOPS`] hops).
fn net_facts(
    project: &Project,
    netlist: &Netlist,
    start: NetId,
    supply: f64,
) -> (Circuit, Vec<Rated>) {
    let mut circuit = Circuit::Unknown;
    let mut found: Vec<Rated> = Vec::new();
    let mut seen: BTreeSet<NetId> = BTreeSet::new();
    let mut queue = vec![(start, 0u8)];
    while let Some((net, depth)) = queue.pop() {
        if !seen.insert(net) {
            continue;
        }
        for end in &netlist.net(net).endpoints {
            let Endpoint::Pin { symbol, .. } = end else {
                continue;
            };
            let Some(sym) = project.symbol(*symbol) else {
                continue;
            };
            let key = sym.symbol.0.as_str();
            let mut hop: Vec<u16> = Vec::new();
            match &sym.role {
                SymbolRole::Device { device, .. } => {
                    if net == start {
                        if is_power_symbol(key) {
                            circuit = Circuit::Power;
                        } else if circuit == Circuit::Unknown {
                            circuit = Circuit::Control;
                        }
                    }
                    if !is_power_symbol(key) && !key.contains("/protection/") {
                        // A contact or an operator passes the control current on.
                        if (key.contains("/contacts/") || key.contains("/operators/"))
                            && depth < MAX_HOPS
                            && let Endpoint::Pin { pin, .. } = end
                            && let Some(partner) = partner_pin(sym, *pin)
                        {
                            hop.push(partner);
                        }
                    } else if let Some((amps, what)) = device_current(project, *device, key, supply)
                    {
                        let tag = project
                            .device(*device)
                            .map(|d| d.tag.to_string())
                            .unwrap_or_default();
                        if !found.iter().any(|r| r.tag == tag) {
                            found.push(Rated { tag, amps, what });
                        }
                    } else if key.contains("/protection/") && depth < MAX_HOPS {
                        // An unrated fuse passes the current on to its other pole pin.
                        if let Endpoint::Pin { pin, .. } = end
                            && let Some(partner) = partner_pin(sym, *pin)
                        {
                            hop.push(partner);
                        }
                    }
                }
                SymbolRole::Terminal { .. } if depth < MAX_HOPS => {
                    if let Endpoint::Pin { pin, .. } = end {
                        hop.extend((0..sym.pins.len() as u16).filter(|i| i != pin));
                    }
                }
                _ => {}
            }
            for pin in hop {
                if let Some(next) = netlist.net_of(Endpoint::pin(*symbol, pin)) {
                    queue.push((next, depth + 1));
                }
            }
        }
    }
    (circuit, found)
}

/// The other pin of a pole: 1 and 2, 3 and 4, 5 and 6.
fn partner_pin(sym: &novo_core::SymbolInstance, pin: u16) -> Option<u16> {
    let n: u32 = sym.pins.get(usize::from(pin))?.name.parse().ok()?;
    let want = if n % 2 == 1 { n + 1 } else { n - 1 };
    let index = sym.pins.iter().position(|p| p.name.parse() == Ok(want))?;
    u16::try_from(index).ok()
}
