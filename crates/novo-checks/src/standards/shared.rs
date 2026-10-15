//! Rule logic the three standards share. Each function takes the rule ids it
//! reports under, so IEC 60204-1, NFPA 79 and UL 508A call the same code with
//! their own ids, clauses and limits.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, DeviceId, Endpoint, NetId, PotentialKind, SymbolId, SymbolInstance, SymbolRole, Value,
    WireId,
};

use super::ampacity::{self, covers, grounding_conductor, next_standard_rating};
use super::netclass::{Circuit, Current};
use super::symbols::{Class, Colour, class_of, is_break_contact, lamp_colour};
use super::{Env, Sink, StandardSet, as_bool, as_f64, device_property, keys};

// ----- small readers -----------------------------------------------------

/// A wire's cross-section: its own, else its cable core's.
pub(super) fn wire_section(e: &Env<'_>, wire: WireId) -> Option<f64> {
    let project = e.project;
    let w = project.wire(wire)?;
    w.cross_section_mm2.filter(|s| *s > 0.0).or_else(|| {
        let (cable, core) = project.core_of_wire(wire)?;
        project
            .cable(cable)?
            .cores
            .get(core)?
            .cross_section_mm2
            .filter(|s| *s > 0.0)
    })
}

/// What the colour of a wire says.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Read {
    Unset,
    Unreadable(String),
    Known(Colour),
}

/// A wire's colour: its own, else the name of its cable core when that is a
/// colour code.
pub(super) fn wire_colour(e: &Env<'_>, wire: WireId) -> Read {
    let project = e.project;
    let Some(w) = project.wire(wire) else {
        return Read::Unset;
    };
    if let Some(text) = w.color.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        return Colour::parse(text).map_or_else(|| Read::Unreadable(text.to_owned()), Read::Known);
    }
    if let Some((cable, core)) = project.core_of_wire(wire)
        && let Some(core) = project.cable(cable).and_then(|c| c.cores.get(core))
        && let Some(colour) = Colour::parse(&core.name)
    {
        return Read::Known(colour);
    }
    Read::Unset
}

fn device_text<'a>(e: &'a Env<'_>, device: DeviceId, key: &str) -> Option<&'a str> {
    match device_property(e.project, device, key)? {
        Value::Text(t) => Some(t.as_str()),
        _ => None,
    }
}

fn device_number(e: &Env<'_>, device: DeviceId, key: &str) -> Option<f64> {
    device_property(e.project, device, key).and_then(as_f64)
}

fn device_flag(e: &Env<'_>, device: DeviceId, key: &str) -> Option<bool> {
    device_property(e.project, device, key).and_then(as_bool)
}

fn device_colour(e: &Env<'_>, device: DeviceId, key: &str) -> Option<Result<Colour, String>> {
    let text = device_text(e, device, key)?;
    Some(Colour::parse(text).ok_or_else(|| text.to_owned()))
}

fn symbols_of(e: &Env<'_>, device: DeviceId) -> Vec<SymbolId> {
    e.cx.by_device
        .get(&device)
        .map(|v| v.iter().map(|s| s.id).collect())
        .unwrap_or_default()
}

fn at_symbols(e: &Env<'_>, device: DeviceId) -> Vec<AnyId> {
    let mut at: Vec<AnyId> = symbols_of(e, device)
        .into_iter()
        .map(AnyId::Symbol)
        .collect();
    if at.is_empty() {
        at.push(AnyId::Device(device));
    }
    at
}

/// The device a symbol draws, with its symbols' ids, per class. Each device
/// once, in id order.
fn devices_of_class<'a>(
    e: &Env<'a>,
    pred: impl Fn(Class) -> bool,
) -> BTreeMap<DeviceId, Vec<&'a SymbolInstance>> {
    let mut out: BTreeMap<DeviceId, Vec<&'a SymbolInstance>> = BTreeMap::new();
    for s in e.project.symbols() {
        if let SymbolRole::Device { device, .. } = s.role
            && pred(class_of(s))
        {
            out.entry(device).or_default().push(s);
        }
    }
    out
}

fn symbol_device(s: &SymbolInstance) -> Option<DeviceId> {
    s.device()
}

/// A short label for a net in messages: its potentials, else a wire number.
pub(super) fn net_label(e: &Env<'_>, net: NetId) -> String {
    let n = e.nets.netlist.net(net);
    let names: Vec<&str> = n
        .potentials
        .iter()
        .filter_map(|p| e.project.potential(*p))
        .map(|p| p.name.as_str())
        .collect();
    if !names.is_empty() {
        return names.join("/");
    }
    n.wires
        .iter()
        .filter_map(|w| e.project.wire(*w)?.number.clone())
        .next()
        .map_or_else(|| "an unnamed net".to_owned(), |n| format!("wire {n}"))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn plural_have(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn number(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        let t = format!("{v:.2}");
        t.trim_end_matches('0').to_owned()
    }
}

/// The nets with a live source: Line and positive dc feeds.
fn live_sources(e: &Env<'_>) -> Vec<NetId> {
    e.nets
        .sources()
        .filter(|n| {
            e.nets
                .netlist
                .net(*n)
                .potentials
                .iter()
                .filter_map(|p| e.project.potential(*p))
                .any(|p| matches!(p.kind, PotentialKind::Line | PotentialKind::DcPositive))
        })
        .collect()
}

fn is_drive_pin_on(e: &Env<'_>, net: NetId, except: SymbolId) -> bool {
    e.nets
        .netlist
        .net(net)
        .endpoints
        .iter()
        .filter_map(|end| end.as_pin())
        .filter(|p| p.symbol != except)
        .filter_map(|p| e.project.symbol(p.symbol))
        .any(|s| matches!(class_of(s), Class::Drive | Class::SoftStarter))
}

// ----- disconnecting device ---------------------------------------------

/// A breaker's suitability for isolation, as stated on the device or part.
fn breaker_isolating(e: &Env<'_>, s: &SymbolInstance) -> Option<bool> {
    device_flag(e, symbol_device(s)?, "isolating")
}

/// Every power load reaches the supply only through a disconnecting device.
pub(super) fn supply_disconnect(
    e: &Env<'_>,
    s: &mut Sink,
    set: StandardSet,
    id: &'static str,
    lock_id: &'static str,
) {
    debug_assert!(e.on(set));
    let project = e.project;
    let loads = devices_of_class(e, Class::is_power_load);
    if loads.is_empty() {
        return;
    }
    let starts = live_sources(e);
    if starts.is_empty() {
        s.unknown(
            id,
            e.anchor(),
            "No supply feed (a Line potential defined on a net) is drawn, so the path from the supply to the loads cannot be followed".to_owned(),
        );
        return;
    }
    let isolating = |sym: SymbolId, strict: bool| -> bool {
        let Some(symbol) = project.symbol(sym) else {
            return false;
        };
        match class_of(symbol) {
            c if c.is_isolator() => true,
            Class::Breaker => match breaker_isolating(e, symbol) {
                Some(yes) => yes,
                None => !strict,
            },
            _ => false,
        }
    };
    let lenient = e.nets.reach(&starts, |sym| isolating(sym, false));
    let strict = e.nets.reach(&starts, |sym| isolating(sym, true));
    let open = e.nets.reach(&starts, |_| false);
    let mut isolators: BTreeSet<DeviceId> = BTreeSet::new();
    for (device, symbols) in &loads {
        let nets: Vec<NetId> = symbols
            .iter()
            .flat_map(|sym| e.nets.live_nets_of(sym))
            .collect();
        let tag = e.tag(*device);
        if lenient.any(&nets) {
            s.fail(
                id,
                at_symbols(e, *device),
                format!("{tag} can be reached from the supply without passing a disconnecting device"),
            );
        } else if strict.any(&nets) {
            s.unknown(
                id,
                at_symbols(e, *device),
                format!("{tag} is reached through a circuit breaker only; state `isolating = true` on it if it is suitable for isolation"),
            );
        } else if open.any(&nets) {
            s.pass(id);
            for net in &nets {
                for sym in e.nets.upstream(*net).poles.keys() {
                    if let Some(symbol) = project.symbol(*sym)
                        && isolating(*sym, true)
                        && let Some(d) = symbol_device(symbol)
                    {
                        isolators.insert(d);
                    }
                }
            }
        } else {
            s.unknown(
                id,
                at_symbols(e, *device),
                format!("{tag} is not reached by any supply feed through switched poles"),
            );
        }
    }
    for device in isolators {
        let tag = e.tag(device);
        match device_flag(e, device, "lockable") {
            Some(true) => s.pass(lock_id),
            Some(false) => s.fail(
                lock_id,
                at_symbols(e, device),
                format!("{tag} cannot be locked in the OFF position"),
            ),
            None => s.unknown(
                lock_id,
                at_symbols(e, device),
                format!("{tag}: state whether it can be locked in OFF (`lockable`)"),
            ),
        }
    }
}

// ----- conductor colours ---------------------------------------------------

/// The colours a standard allows per kind of conductor.
pub(super) struct Scheme {
    pub(super) pe: Vec<Colour>,
    pub(super) neutral: Vec<Colour>,
    pub(super) ac_power: Vec<Colour>,
    pub(super) dc_power: Vec<Colour>,
    pub(super) ac_control: Vec<Colour>,
    pub(super) dc_control: Vec<Colour>,
    /// Circuits that stay live with the supply off.
    pub(super) excepted: Vec<Colour>,
}

impl Scheme {
    pub(super) fn iec60204(e: &Env<'_>) -> Scheme {
        Scheme::build(
            e,
            StandardSet::Iec60204,
            Scheme {
                pe: vec![Colour::GreenYellow],
                neutral: vec![Colour::Blue],
                ac_power: vec![Colour::Black],
                dc_power: vec![Colour::Black],
                ac_control: vec![Colour::Red],
                dc_control: vec![Colour::Blue],
                excepted: vec![Colour::Orange],
            },
        )
    }

    pub(super) fn nfpa79(e: &Env<'_>) -> Scheme {
        Scheme::build(
            e,
            StandardSet::Nfpa79,
            Scheme {
                pe: vec![Colour::Green, Colour::GreenYellow],
                neutral: vec![Colour::White, Colour::Grey, Colour::WhiteBlue],
                ac_power: vec![Colour::Black],
                dc_power: vec![Colour::Blue],
                ac_control: vec![Colour::Red],
                dc_control: vec![Colour::Blue],
                excepted: vec![Colour::Yellow, Colour::Orange],
            },
        )
    }

    fn build(e: &Env<'_>, set: StandardSet, mut base: Scheme) -> Scheme {
        let p = &e.params;
        for (slot, name) in [
            (&mut base.pe, "pe"),
            (&mut base.neutral, "neutral"),
            (&mut base.ac_power, "ac_power"),
            (&mut base.dc_power, "dc_power"),
            (&mut base.ac_control, "ac_control"),
            (&mut base.dc_control, "dc_control"),
            (&mut base.excepted, "excepted"),
        ] {
            if let Some(colours) = p.colours(set, name) {
                *slot = colours;
            }
        }
        base
    }
}

fn list(colours: &[Colour]) -> String {
    colours
        .iter()
        .map(|c| c.name())
        .collect::<Vec<_>>()
        .join(" or ")
}

/// Conductor colours: protective, neutral and the power and control
/// circuits. Failures are grouped per net and colour; wires without a colour
/// are one "unknown" per rule.
pub(super) fn colours(
    e: &Env<'_>,
    s: &mut Sink,
    scheme: &Scheme,
    pe_id: &'static str,
    neutral_id: &'static str,
    conductor_id: &'static str,
) {
    let project = e.project;
    let mut unset: BTreeMap<&'static str, Vec<AnyId>> = BTreeMap::new();
    let mut unreadable: BTreeMap<&'static str, Vec<(AnyId, String)>> = BTreeMap::new();
    let mut unclassified: Vec<AnyId> = Vec::new();
    // (rule, net, colour) -> wires
    let mut wrong: BTreeMap<(&'static str, NetId, Colour), Vec<WireId>> = BTreeMap::new();
    let mut reserved: BTreeMap<(NetId, Colour), Vec<WireId>> = BTreeMap::new();
    let mut no_current: Vec<AnyId> = Vec::new();
    for wire in project.wires() {
        let Some(net) = e.nets.netlist.net_of_wire(wire) else {
            continue;
        };
        let circuit = e.nets.circuit(net);
        let current = e.nets.current(net);
        let (rule, allowed): (&'static str, Option<Vec<Colour>>) = match circuit {
            Circuit::ProtectiveEarth => (pe_id, Some(scheme.pe.clone())),
            Circuit::Neutral => (neutral_id, Some(scheme.neutral.clone())),
            Circuit::Power | Circuit::Control => {
                let (ac, dc) = if circuit == Circuit::Power {
                    (&scheme.ac_power, &scheme.dc_power)
                } else {
                    (&scheme.ac_control, &scheme.dc_control)
                };
                let allowed = match current {
                    Current::Ac => Some(ac.clone()),
                    Current::Dc => Some(dc.clone()),
                    Current::Unknown => (ac == dc).then(|| ac.clone()),
                };
                (conductor_id, allowed)
            }
            Circuit::Unknown => (conductor_id, None),
        };
        let read = wire_colour(e, wire.id);
        // Green-yellow belongs to the protective conductor alone.
        if let Read::Known(Colour::GreenYellow) = read
            && circuit != Circuit::ProtectiveEarth
        {
            if circuit == Circuit::Unknown {
                s.unknown(
                    pe_id,
                    vec![AnyId::Wire(wire.id)],
                    "A green-yellow wire is in a circuit that could not be classified; give its net a PE potential if it is a protective conductor".to_owned(),
                );
            } else {
                reserved
                    .entry((net, Colour::GreenYellow))
                    .or_default()
                    .push(wire.id);
            }
            continue;
        }
        let Some(allowed) = allowed else {
            if circuit == Circuit::Unknown {
                unclassified.push(AnyId::Wire(wire.id));
            } else {
                no_current.push(AnyId::Wire(wire.id));
            }
            continue;
        };
        match read {
            Read::Unset => {
                unset.entry(rule).or_default().push(AnyId::Wire(wire.id));
            }
            Read::Unreadable(text) => unreadable
                .entry(rule)
                .or_default()
                .push((AnyId::Wire(wire.id), text)),
            Read::Known(colour) => {
                let excepted = rule == conductor_id && scheme.excepted.contains(&colour);
                if allowed.contains(&colour) || excepted {
                    s.pass(rule);
                } else {
                    wrong.entry((rule, net, colour)).or_default().push(wire.id);
                }
            }
        }
    }
    for ((rule, net, colour), wires) in wrong {
        let circuit = e.nets.circuit(net);
        let label = net_label(e, net);
        let wanted = match circuit {
            Circuit::ProtectiveEarth => list(&scheme.pe),
            Circuit::Neutral => list(&scheme.neutral),
            _ => {
                let (ac, dc) = if circuit == Circuit::Power {
                    (&scheme.ac_power, &scheme.dc_power)
                } else {
                    (&scheme.ac_control, &scheme.dc_control)
                };
                match e.nets.current(net) {
                    Current::Dc => list(dc),
                    _ => list(ac),
                }
            }
        };
        let what = match circuit {
            Circuit::ProtectiveEarth => "protective-earth",
            Circuit::Neutral => "neutral",
            Circuit::Power => "power",
            Circuit::Control => "control",
            Circuit::Unknown => "",
        };
        s.fail(
            rule,
            wires.iter().map(|w| AnyId::Wire(*w)).collect(),
            format!(
                "{} on {what} net {label} {} {}, which should be {wanted}",
                plural(wires.len(), "wire", "wires"),
                if wires.len() == 1 { "is" } else { "are" },
                colour.name()
            ),
        );
    }
    for ((net, colour), wires) in reserved {
        s.fail(
            pe_id,
            wires.iter().map(|w| AnyId::Wire(*w)).collect(),
            format!(
                "{} on net {} {} {}, a colour reserved for the protective conductor",
                plural(wires.len(), "wire", "wires"),
                net_label(e, net),
                if wires.len() == 1 { "is" } else { "are" },
                colour.name()
            ),
        );
    }
    for (rule, wires) in unset {
        s.unknown(
            rule,
            wires.clone(),
            format!(
                "{} no colour set",
                plural_have(wires.len(), "wire has", "wires have")
            ),
        );
    }
    for (rule, wires) in unreadable {
        let names: BTreeSet<String> = wires.iter().map(|(_, t)| t.clone()).collect();
        s.unknown(
            rule,
            wires.iter().map(|(a, _)| *a).collect(),
            format!(
                "{} a colour that is not a known code or name ({})",
                plural_have(wires.len(), "wire carries", "wires carry"),
                names.into_iter().collect::<Vec<_>>().join(", ")
            ),
        );
    }
    if !no_current.is_empty() {
        s.unknown(
            conductor_id,
            no_current.clone(),
            format!(
                "{} in a power or control circuit whose current type (ac or dc) is not known and whose colours differ between the two; assign a potential to the circuit",
                plural_have(no_current.len(), "wire is", "wires are")
            ),
        );
    }
    if !unclassified.is_empty() {
        s.unknown(
            conductor_id,
            unclassified.clone(),
            format!(
                "{} in a circuit that could not be classified as power or control; assign potentials or connect their loads",
                plural_have(unclassified.len(), "wire is", "wires are")
            ),
        );
    }
}

// ----- control circuits ----------------------------------------------------

fn net_marks(e: &Env<'_>, net: NetId) -> Vec<AnyId> {
    let n = e.nets.netlist.net(net);
    let mut at: Vec<AnyId> = n.wires.iter().take(24).map(|w| AnyId::Wire(*w)).collect();
    if at.is_empty() {
        at = n.potentials.iter().map(|p| AnyId::Potential(*p)).collect();
    }
    at
}

fn net_voltage(e: &Env<'_>, net: NetId) -> Option<f64> {
    e.nets
        .netlist
        .net(net)
        .potentials
        .iter()
        .filter_map(|p| e.project.potential(*p)?.voltage)
        .reduce(f64::max)
}

/// Control circuit voltage against the limits for ac and dc.
pub(super) fn control_voltage(
    e: &Env<'_>,
    s: &mut Sink,
    set: StandardSet,
    id: &'static str,
    ac_default: f64,
    dc_default: Option<f64>,
) {
    let ac_limit = e.params.limit(set, "max_control_ac_v", ac_default);
    let dc_limit = e
        .params
        .number(&format!("standards.{}.max_control_dc_v", set.id()))
        .or(dc_default);
    let mut no_voltage: Vec<AnyId> = Vec::new();
    let mut no_limit: Vec<AnyId> = Vec::new();
    let mut ambiguous: Vec<AnyId> = Vec::new();
    for net in e.nets.netlist.nets() {
        if e.nets.circuit(net.id) != Circuit::Control || net.potentials.is_empty() {
            continue;
        }
        // A return conductor (0 V, neutral) has no voltage of its own.
        let carries_voltage = net
            .potentials
            .iter()
            .filter_map(|p| e.project.potential(*p))
            .any(|p| matches!(p.kind, PotentialKind::Line | PotentialKind::DcPositive));
        if !carries_voltage {
            continue;
        }
        let Some(v) = net_voltage(e, net.id) else {
            no_voltage.extend(net_marks(e, net.id));
            continue;
        };
        let label = net_label(e, net.id);
        let check = |s: &mut Sink, limit: f64, kind: &str| {
            if v > limit + 1e-9 {
                s.fail(
                    id,
                    net_marks(e, net.id),
                    format!(
                        "Control circuit {label} is {} V {kind}, above the limit of {} V",
                        number(v),
                        number(limit)
                    ),
                );
            } else {
                s.pass(id);
            }
        };
        match e.nets.current(net.id) {
            Current::Ac => check(s, ac_limit, "ac"),
            Current::Dc => match dc_limit {
                Some(limit) => check(s, limit, "dc"),
                None => no_limit.extend(net_marks(e, net.id)),
            },
            Current::Unknown => match dc_limit {
                Some(dc) if v > ac_limit.max(dc) + 1e-9 => check(s, ac_limit.max(dc), "ac/dc"),
                Some(dc) if v <= ac_limit.min(dc) + 1e-9 => s.pass(id),
                _ => ambiguous.extend(net_marks(e, net.id)),
            },
        }
    }
    if !no_voltage.is_empty() {
        s.unknown(
            id,
            no_voltage,
            "Control circuits carry a potential without a voltage; set the voltage on the potential".to_owned(),
        );
    }
    if !no_limit.is_empty() {
        s.unknown(
            id,
            no_limit,
            format!(
                "No dc control voltage limit is built in for this standard; set `standards.{}.max_control_dc_v` from your copy",
                set.id()
            ),
        );
    }
    if !ambiguous.is_empty() {
        s.unknown(
            id,
            ambiguous,
            "Control circuits whose voltage lies between the ac and dc limits and whose current type is not known".to_owned(),
        );
    }
}

/// Control circuits are not tapped from the power supply potentials.
pub(super) fn control_supply(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    for net in e.nets.netlist.nets() {
        if e.nets.circuit(net.id) != Circuit::Control || e.nets.current(net.id) != Current::Ac {
            continue;
        }
        let Some(p) = net.potential() else { continue };
        let on_power = e
            .nets
            .netlist
            .nets_with_potential(p)
            .any(|other| e.nets.circuit(other.id) == Circuit::Power);
        if on_power {
            s.fail(
                id,
                net_marks(e, net.id),
                format!(
                    "Control circuit {} uses a potential that also feeds power circuits; feed control circuits from a control transformer with its own source",
                    net_label(e, net.id)
                ),
            );
        } else {
            s.pass(id);
        }
    }
}

// ----- protection ----------------------------------------------------------

/// Short-circuit protection of power loads, protection of control circuits
/// and motor overload protection, under one standard's ids.
pub(super) fn protection(
    e: &Env<'_>,
    s: &mut Sink,
    short_id: &'static str,
    control_id: &'static str,
    overload_id: &'static str,
) {
    let project = e.project;
    let starts = live_sources(e);
    let protector = |sym: SymbolId| {
        project
            .symbol(sym)
            .is_some_and(|x| class_of(x).is_short_circuit_protector())
    };
    let loads = devices_of_class(e, Class::is_power_load);
    if !loads.is_empty() {
        let blocked = e.nets.reach(&starts, protector);
        let open = e.nets.reach(&starts, |_| false);
        for (device, symbols) in &loads {
            let tag = e.tag(*device);
            for sym in symbols {
                let nets = e.nets.live_nets_of(sym);
                if class_of(sym) == Class::Motor && nets.iter().any(|n| is_drive_pin_on(e, *n, sym.id))
                {
                    s.pass(short_id);
                } else if blocked.any(&nets) {
                    s.fail(
                        short_id,
                        vec![AnyId::Symbol(sym.id)],
                        format!("{tag} can be reached from the supply without passing a fuse or circuit breaker"),
                    );
                } else if open.any(&nets) {
                    s.pass(short_id);
                } else if starts.is_empty() {
                    s.unknown(
                        short_id,
                        e.anchor(),
                        "No supply feed (a Line potential defined on a net) is drawn".to_owned(),
                    );
                } else {
                    s.unknown(
                        short_id,
                        vec![AnyId::Symbol(sym.id)],
                        format!("{tag} is not reached by any supply feed through switched poles"),
                    );
                }
            }
        }
    }

    // Control circuits: no path from the supply to a control net that does
    // not cross a fuse or breaker.
    let control_nets: Vec<NetId> = e
        .nets
        .netlist
        .nets()
        .iter()
        .filter(|n| {
            e.nets.circuit(n.id) == Circuit::Control
                && e.nets.has_control_device(n.id)
                && e.nets.is_fed(n.id)
        })
        .map(|n| n.id)
        .collect();
    if !control_nets.is_empty() {
        let blocked = e.nets.reach(&starts, protector);
        let mut bare: BTreeMap<String, Vec<AnyId>> = BTreeMap::new();
        for net in control_nets {
            if blocked.contains(net) {
                bare.entry(net_label(e, net))
                    .or_default()
                    .extend(net_marks(e, net));
            } else {
                s.pass(control_id);
            }
        }
        for (label, at) in bare {
            s.fail(
                control_id,
                at,
                format!("Control circuit {label} is fed without a fuse or circuit breaker between the source and the control devices"),
            );
        }
    }

    // Motor overload.
    let motors = devices_of_class(e, |c| c == Class::Motor);
    if motors.is_empty() {
        return;
    }
    let protectors: Vec<(SymbolId, Vec<NetId>)> = project
        .symbols()
        .filter(|x| class_of(x).is_overload_protector() && class_of(x) != Class::Drive)
        .map(|x| {
            let mut nets: Vec<NetId> = (0..x.pins.len())
                .filter_map(|i| u16::try_from(i).ok())
                .filter_map(|i| e.nets.netlist.net_of(Endpoint::pin(x.id, i)))
                .collect();
            nets.sort();
            nets.dedup();
            (x.id, nets)
        })
        .collect();
    for (device, symbols) in &motors {
        let tag = e.tag(*device);
        if device_flag(e, *device, "thermal_protection") == Some(true) {
            s.pass(overload_id);
            continue;
        }
        for sym in symbols {
            let nets = e.nets.live_nets_of(sym);
            if nets.iter().any(|n| is_drive_pin_on(e, *n, sym.id)) {
                s.pass(overload_id);
                continue;
            }
            if !nets.iter().any(|n| e.nets.is_fed(*n)) {
                s.unknown(
                    overload_id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag} is not reached by any supply feed, so its protection cannot be followed (or state `thermal_protection = true` on the motor)"),
                );
                continue;
            }
            let mut region: BTreeSet<NetId> = BTreeSet::new();
            let mut poles: BTreeSet<SymbolId> = BTreeSet::new();
            for net in &nets {
                let up = e.nets.upstream(*net);
                region.extend(up.nets.iter().copied());
                poles.extend(up.poles.keys().copied());
            }
            let protected = poles.iter().any(|p| {
                project
                    .symbol(*p)
                    .is_some_and(|x| class_of(x).is_overload_protector())
            }) || protectors
                .iter()
                .any(|(_, pnets)| pnets.iter().any(|n| region.contains(n)));
            if protected {
                s.pass(overload_id);
            } else {
                s.fail(
                    overload_id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag} has no overload relay, motor protection breaker or drive in its supply path, and no `thermal_protection` on the motor"),
                );
            }
        }
    }
}

// ----- protective earth ----------------------------------------------------

fn motor_pe_pin(sym: &SymbolInstance) -> Option<u16> {
    sym.pin_index("PE")
        .or_else(|| sym.pin_index("PEN"))
        .or_else(|| sym.pin_index("E"))
}

/// Motors are connected to a protective-earth net.
pub(super) fn earth_connection(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    let project = e.project;
    for (device, symbols) in devices_of_class(e, |c| c == Class::Motor) {
        let tag = e.tag(device);
        for sym in symbols {
            let Some(pin) = motor_pe_pin(sym) else {
                s.unknown(
                    id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag}: its symbol has no PE connection point to check"),
                );
                continue;
            };
            let end = Endpoint::pin(sym.id, pin);
            if project.wires_at(end).next().is_none() {
                s.fail(
                    id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag} has its PE connection unconnected"),
                );
                continue;
            }
            let Some(net) = e.nets.netlist.net_of(end) else {
                continue;
            };
            let kinds: Vec<PotentialKind> = e
                .nets
                .netlist
                .net(net)
                .potentials
                .iter()
                .filter_map(|p| project.potential(*p))
                .map(|p| p.kind)
                .collect();
            if e.nets.circuit(net) == Circuit::ProtectiveEarth {
                s.pass(id);
            } else if kinds.is_empty() {
                s.unknown(
                    id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag}: the net on its PE connection has no PE potential; assign PE to it or connect it to a PE terminal"),
                );
            } else {
                s.fail(
                    id,
                    vec![AnyId::Symbol(sym.id)],
                    format!(
                        "{tag}'s PE connection is on net {}, which is not a protective-earth net",
                        net_label(e, net)
                    ),
                );
            }
        }
    }
}

/// Protective conductor against the phase conductors at each power load
/// (metric steps).
pub(super) fn pe_size_metric(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    let project = e.project;
    let mut missing: Vec<AnyId> = Vec::new();
    for (device, symbols) in devices_of_class(e, |c| c.is_power_load() || c == Class::Transformer) {
        let tag = e.tag(device);
        let mut phase: Vec<(WireId, f64)> = Vec::new();
        let mut pe: Vec<(WireId, f64)> = Vec::new();
        let mut gaps: Vec<AnyId> = Vec::new();
        for sym in &symbols {
            for pin in 0..sym.pins.len() {
                let Ok(pin) = u16::try_from(pin) else { break };
                for w in project.wires_at(Endpoint::pin(sym.id, pin)) {
                    let Some(net) = project.wire(w).and_then(|w| e.nets.netlist.net_of_wire(w))
                    else {
                        continue;
                    };
                    let bucket = match e.nets.circuit(net) {
                        Circuit::ProtectiveEarth => &mut pe,
                        Circuit::Power => &mut phase,
                        _ => continue,
                    };
                    match wire_section(e, w) {
                        Some(mm2) => bucket.push((w, mm2)),
                        None => gaps.push(AnyId::Wire(w)),
                    }
                }
            }
        }
        if pe.is_empty() && gaps.is_empty() || phase.is_empty() && gaps.is_empty() {
            continue;
        }
        if !gaps.is_empty() {
            missing.extend(gaps);
            continue;
        }
        let largest = phase.iter().map(|(_, a)| *a).fold(0.0, f64::max);
        let smallest = pe.iter().map(|(_, a)| *a).fold(f64::INFINITY, f64::min);
        let required = if largest <= 16.0 {
            largest
        } else if largest <= 35.0 {
            16.0
        } else {
            largest / 2.0
        };
        if smallest + 1e-9 < required {
            let mut at: Vec<AnyId> = pe
                .iter()
                .filter(|(_, a)| *a < required)
                .map(|(w, _)| AnyId::Wire(*w))
                .collect();
            at.extend(symbols.iter().map(|x| AnyId::Symbol(x.id)));
            s.fail(
                id,
                at,
                format!(
                    "{tag}: the protective conductor is {} mm², phase conductors up to {} mm² need at least {} mm²",
                    number(smallest),
                    number(largest),
                    number(required)
                ),
            );
        } else {
            s.pass(id);
        }
    }
    if !missing.is_empty() {
        s.unknown(
            id,
            missing.clone(),
            format!(
                "{} at power loads no cross-section set, so the protective conductor cannot be compared with the phase conductors",
                plural_have(missing.len(), "wire has", "wires have")
            ),
        );
    }
}

/// The nearest upstream overcurrent device of a motor with a rated current.
fn branch_protector(e: &Env<'_>, sym: &SymbolInstance) -> Option<(SymbolId, DeviceId, f64)> {
    let mut poles: Vec<(u32, SymbolId)> = Vec::new();
    for net in e.nets.live_nets_of(sym) {
        let up = e.nets.upstream(net);
        poles.extend(up.poles.iter().map(|(s, d)| (*d, *s)));
    }
    poles.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    poles.dedup_by_key(|p| p.1);
    for (_, id) in poles {
        let Some(x) = e.project.symbol(id) else {
            continue;
        };
        if !class_of(x).is_short_circuit_protector() {
            continue;
        }
        let device = symbol_device(x)?;
        if let Some(rated) = e.project.rated_current_a(device).filter(|a| *a > 0.0) {
            return Some((id, device, rated));
        }
    }
    None
}

/// Equipment grounding conductor of each motor, sized for the nearest
/// overcurrent device above it (AWG steps).
pub(super) fn ground_size_awg(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    let project = e.project;
    let mut unknown: Vec<AnyId> = Vec::new();
    for (device, symbols) in devices_of_class(e, |c| c == Class::Motor) {
        let tag = e.tag(device);
        for sym in symbols {
            let Some(pin) = motor_pe_pin(sym) else { continue };
            let wires: Vec<WireId> = project.wires_at(Endpoint::pin(sym.id, pin)).collect();
            if wires.is_empty() {
                continue;
            }
            let Some((_, protector, rated)) = branch_protector(e, sym) else {
                unknown.push(AnyId::Symbol(sym.id));
                continue;
            };
            let Some(size) = grounding_conductor(rated) else {
                unknown.push(AnyId::Symbol(sym.id));
                continue;
            };
            let sections: Vec<(WireId, Option<f64>)> =
                wires.iter().map(|w| (*w, wire_section(e, *w))).collect();
            if sections.iter().any(|(_, a)| a.is_none()) {
                unknown.extend(
                    sections
                        .iter()
                        .filter(|(_, a)| a.is_none())
                        .map(|(w, _)| AnyId::Wire(*w)),
                );
                continue;
            }
            let thin: Vec<AnyId> = sections
                .iter()
                .filter(|(_, a)| a.is_some_and(|a| !covers(a, size)))
                .map(|(w, _)| AnyId::Wire(*w))
                .collect();
            if thin.is_empty() {
                s.pass(id);
            } else {
                let smallest = sections
                    .iter()
                    .filter_map(|(_, a)| *a)
                    .fold(f64::INFINITY, f64::min);
                let mut at = thin;
                at.push(AnyId::Symbol(sym.id));
                s.fail(
                    id,
                    at,
                    format!(
                        "{tag}: the grounding conductor is {} mm², {} protects its branch at {} A and needs at least {} ({} mm²)",
                        number(smallest),
                        e.tag(protector),
                        number(rated),
                        size.label,
                        number(size.mm2)
                    ),
                );
            }
        }
    }
    if !unknown.is_empty() {
        s.unknown(
            id,
            unknown,
            "A motor's grounding conductor could not be sized: no overcurrent device with a rated current (`rated_current_a`) was found upstream of it, its rating is above the steps in the table, or the wire has no cross-section".to_owned(),
        );
    }
}

// ----- ampacity ------------------------------------------------------------

/// Conductors against the current they carry, in AWG terms.
pub(super) fn ampacity_rule(e: &Env<'_>, s: &mut Sink, set: StandardSet, id: &'static str) {
    let project = e.project;
    let terminal_c = e.params.limit(set, "terminal_temp_c", 60.0);
    let rate = |mm2: f64| ampacity::ampacity(mm2, terminal_c);
    let mut unknown_wires: Vec<AnyId> = Vec::new();
    let mut unrated: Vec<AnyId> = Vec::new();
    // Protective devices: their load-side wires.
    for symbol in project.symbols() {
        let class = class_of(symbol);
        if !class.is_short_circuit_protector() {
            continue;
        }
        let Some(device) = symbol_device(symbol) else {
            continue;
        };
        let Some(rated) = project.rated_current_a(device).filter(|a| *a > 0.0) else {
            unrated.push(AnyId::Symbol(symbol.id));
            continue;
        };
        let mut thinnest: Option<(f64, f64)> = None;
        let mut bad: Vec<WireId> = Vec::new();
        for (index, pin) in symbol.pins.iter().enumerate() {
            let Ok(index) = u16::try_from(index) else { break };
            let end = Endpoint::pin(symbol.id, index);
            let mut protected: BTreeSet<WireId> = project.wires_at(end).collect();
            let load_side = pin.name.parse::<u32>().is_ok_and(|n| n % 2 == 0);
            if load_side && let Some(net) = e.nets.netlist.net_of(end) {
                protected.extend(e.nets.netlist.net(net).wires.iter().copied());
            }
            for w in protected {
                match wire_section(e, w) {
                    None => unknown_wires.push(AnyId::Wire(w)),
                    Some(mm2) => match rate(mm2) {
                        None => unknown_wires.push(AnyId::Wire(w)),
                        Some(amps) if amps + 1e-9 < rated => {
                            bad.push(w);
                            if thinnest.is_none_or(|(m, _)| mm2 < m) {
                                thinnest = Some((mm2, amps));
                            }
                        }
                        Some(_) => s.pass(id),
                    },
                }
            }
        }
        if let Some((mm2, amps)) = thinnest {
            let mut at: Vec<AnyId> = bad.iter().map(|w| AnyId::Wire(*w)).collect();
            at.push(AnyId::Symbol(symbol.id));
            s.fail(
                id,
                at,
                format!(
                    "{} is rated {} A, but {} on it {} thinner: the thinnest, {} mm² (rated as {}), carries {} A at {} °C terminations",
                    e.tag(device),
                    number(rated),
                    plural(bad.len(), "wire", "wires"),
                    if bad.len() == 1 { "is" } else { "are" },
                    number(mm2),
                    ampacity::awg_for_area(mm2).map_or("-", |a| a.label),
                    number(amps),
                    number(terminal_c)
                ),
            );
        }
    }
    // Motors: 125 % of the full-load current.
    for (device, symbols) in devices_of_class(e, |c| c == Class::Motor) {
        let Some(flc) = device_number(e, device, keys::FLC_A).filter(|a| *a > 0.0) else {
            unrated.extend(at_symbols(e, device));
            continue;
        };
        let need = 1.25 * flc;
        for sym in symbols {
            for (i, pin) in sym.pins.iter().enumerate() {
                if is_ground(&pin.name) {
                    continue;
                }
                let Ok(i) = u16::try_from(i) else { break };
                for w in project.wires_at(Endpoint::pin(sym.id, i)) {
                    match wire_section(e, w).and_then(|a| rate(a).map(|amps| (a, amps))) {
                        None => unknown_wires.push(AnyId::Wire(w)),
                        Some((_, amps)) if amps + 1e-9 >= need => s.pass(id),
                        Some((mm2, amps)) => s.fail(
                            id,
                            vec![AnyId::Wire(w), AnyId::Symbol(sym.id)],
                            format!(
                                "{}: a {} mm² wire carries {} A, a motor of {} A full-load current needs {} A (125 %)",
                                e.tag(device),
                                number(mm2),
                                number(amps),
                                number(flc),
                                number(need)
                            ),
                        ),
                    }
                }
            }
        }
    }
    if !unrated.is_empty() {
        s.unknown(
            id,
            unrated,
            "Protective devices without a rated current (`rated_current_a`) and motors without a full-load current (`flc_a`): their conductors cannot be checked".to_owned(),
        );
    }
    unknown_wires.sort();
    unknown_wires.dedup();
    if !unknown_wires.is_empty() {
        s.unknown(
            id,
            unknown_wires.clone(),
            format!(
                "{} no cross-section, or one thinner than the smallest AWG size in the table",
                plural_have(unknown_wires.len(), "wire has", "wires have")
            ),
        );
    }
}

fn is_ground(name: &str) -> bool {
    matches!(name.to_ascii_uppercase().as_str(), "PE" | "PEN" | "N" | "E")
}

/// How much of a motor's full-load current its branch device may be rated.
fn branch_percent(e: &Env<'_>, protector: DeviceId, class: Class) -> Option<f64> {
    let kind = device_text(e, protector, "protection_kind").map(|t| t.to_ascii_lowercase());
    match kind.as_deref() {
        Some(k) if k.contains("non") && k.contains("delay") => Some(300.0),
        Some(k) if k.contains("time") && k.contains("delay") || k.contains("dual") => Some(175.0),
        Some(k) if k.contains("instant") => Some(800.0),
        Some(k) if k.contains("inverse") => Some(250.0),
        Some(_) => None,
        None => match class {
            Class::Breaker => Some(250.0),
            Class::MotorCircuitProtector => Some(800.0),
            _ => None,
        },
    }
}

/// A motor's branch protective device against its type's allowed percentage
/// of full-load current.
pub(super) fn motor_branch_size(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    for (device, symbols) in devices_of_class(e, |c| c == Class::Motor) {
        let Some(flc) = device_number(e, device, keys::FLC_A).filter(|a| *a > 0.0) else {
            if let Some(sym) = symbols.first() {
                s.unknown(
                    id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{}: state the motor's full-load current (`flc_a`)", e.tag(device)),
                );
            }
            continue;
        };
        for sym in symbols {
            let tag = e.tag(device);
            let Some((psym, protector, rated)) = branch_protector(e, sym) else {
                s.unknown(
                    id,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag}: no overcurrent device with a rated current was found upstream"),
                );
                continue;
            };
            let class = e
                .project
                .symbol(psym)
                .map_or(Class::Other, class_of);
            let Some(percent) = branch_percent(e, protector, class) else {
                s.unknown(
                    id,
                    vec![AnyId::Symbol(sym.id), AnyId::Symbol(psym)],
                    format!(
                        "{}: state its type (`protection_kind`: non_time_delay_fuse, time_delay_fuse, inverse_time_breaker or instantaneous_trip) to size it against {tag}",
                        e.tag(protector)
                    ),
                );
                continue;
            };
            let limit = next_standard_rating(flc * percent / 100.0);
            if rated > limit + 1e-9 {
                s.fail(
                    id,
                    vec![AnyId::Symbol(sym.id), AnyId::Symbol(psym)],
                    format!(
                        "{} is rated {} A, above the {} A allowed for {tag} ({} % of {} A, taken up to the next standard rating)",
                        e.tag(protector),
                        number(rated),
                        number(limit),
                        number(percent),
                        number(flc)
                    ),
                );
            } else {
                s.pass(id);
            }
        }
    }
}

/// An overload relay's setting against the motor's full-load current.
pub(super) fn overload_setting(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    let project = e.project;
    for (device, symbols) in devices_of_class(e, |c| c == Class::Motor) {
        let tag = e.tag(device);
        let Some(flc) = device_number(e, device, keys::FLC_A).filter(|a| *a > 0.0) else {
            continue;
        };
        for sym in symbols {
            let mut overloads: BTreeSet<DeviceId> = BTreeSet::new();
            for net in e.nets.live_nets_of(sym) {
                let up = e.nets.upstream(net);
                for p in up.poles.keys() {
                    if let Some(x) = project.symbol(*p)
                        && matches!(class_of(x), Class::OverloadRelay | Class::MotorProtectionBreaker)
                        && let Some(d) = symbol_device(x)
                    {
                        overloads.insert(d);
                    }
                }
                for x in project.symbols() {
                    if matches!(class_of(x), Class::OverloadRelay)
                        && let Some(d) = symbol_device(x)
                        && (0..x.pins.len())
                            .filter_map(|i| u16::try_from(i).ok())
                            .filter_map(|i| e.nets.netlist.net_of(Endpoint::pin(x.id, i)))
                            .any(|n| up.nets.contains(&n))
                    {
                        overloads.insert(d);
                    }
                }
            }
            let service_factor = device_number(e, device, "service_factor");
            let rise = device_number(e, device, "temp_rise_c");
            let generous = service_factor.is_some_and(|f| f >= 1.15) || rise.is_some_and(|r| r <= 40.0);
            let known = service_factor.is_some() || rise.is_some();
            for relay in overloads {
                let Some(setting) = device_number(e, relay, "setting_a").filter(|a| *a > 0.0)
                else {
                    s.unknown(
                        id,
                        at_symbols(e, relay),
                        format!("{}: state its setting (`setting_a`) to check it against {tag}", e.tag(relay)),
                    );
                    continue;
                };
                let high = 1.25 * flc;
                let low = 1.15 * flc;
                if setting > high + 1e-9 {
                    s.fail(
                        id,
                        at_symbols(e, relay),
                        format!(
                            "{} is set to {} A, above 125 % of {tag}'s {} A ({} A)",
                            e.tag(relay),
                            number(setting),
                            number(flc),
                            number(high)
                        ),
                    );
                } else if setting > low + 1e-9 && !generous {
                    if known {
                        s.fail(
                            id,
                            at_symbols(e, relay),
                            format!(
                                "{} is set to {} A, above 115 % of {tag}'s {} A ({} A); the motor's marking does not allow 125 %",
                                e.tag(relay),
                                number(setting),
                                number(flc),
                                number(low)
                            ),
                        );
                    } else {
                        s.unknown(
                            id,
                            at_symbols(e, relay),
                            format!(
                                "{} is set to {} A, between 115 % and 125 % of {tag}'s {} A: state the motor's `service_factor` or `temp_rise_c` to decide which limit applies",
                                e.tag(relay),
                                number(setting),
                                number(flc)
                            ),
                        );
                    }
                } else {
                    s.pass(id);
                }
            }
        }
    }
}

// ----- operators and indicators --------------------------------------------

/// Emergency stop devices: present, break contact, colours, stop category.
pub(super) fn estop(
    e: &Env<'_>,
    s: &mut Sink,
    present: &'static str,
    contact: &'static str,
    colour: &'static str,
    category: &'static str,
) {
    let devices = {
        let mut d = devices_of_class(e, |c| matches!(c, Class::EmergencyStop | Class::PullCord));
        // A device that states the function is an emergency stop too.
        for (device, symbols) in devices_of_class(e, Class::is_operator) {
            let function = device_text(e, device, "function")
                .map(|t| t.to_ascii_lowercase())
                .unwrap_or_default();
            if function.contains("estop")
                || function.contains("e-stop")
                || function.contains("emergency")
            {
                d.entry(device).or_insert(symbols);
            }
        }
        d
    };
    let has_loads = e.project.symbols().any(|x| class_of(x).is_power_load());
    if devices.is_empty() {
        if has_loads {
            if e.params.flag(keys::ESTOP_REQUIRED) == Some(false) {
                // Declared not required: nothing to find.
            } else {
                s.fail(
                    present,
                    e.anchor(),
                    "No emergency stop device is drawn. Add one, or state `machine.estop_required = false` after the risk assessment".to_owned(),
                );
            }
        }
        return;
    }
    s.pass(present);
    let mut colour_unknown: Vec<AnyId> = Vec::new();
    let mut category_unknown: Vec<AnyId> = Vec::new();
    for (device, symbols) in &devices {
        let tag = e.tag(*device);
        let at = at_symbols(e, *device);
        for sym in symbols {
            match is_break_contact(&sym.symbol.0) {
                Some(true) => s.pass(contact),
                Some(false) => s.fail(
                    contact,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag} opens no contact: its symbol is a make contact, an emergency stop needs a break contact"),
                ),
                None => s.unknown(
                    contact,
                    vec![AnyId::Symbol(sym.id)],
                    format!("{tag}: the symbol does not say whether the contact is a make or a break contact"),
                ),
            }
        }
        if symbols
            .iter()
            .any(|x| class_of(x) == Class::EmergencyStop)
        {
            match (
                device_colour(e, *device, "actuator_colour"),
                device_colour(e, *device, "background_colour"),
            ) {
                (Some(Ok(a)), Some(Ok(b))) => {
                    if a == Colour::Red && b == Colour::Yellow {
                        s.pass(colour);
                    } else {
                        s.fail(
                            colour,
                            at.clone(),
                            format!(
                                "{tag} is {} on {}, an emergency stop is red on yellow",
                                a.name(),
                                b.name()
                            ),
                        );
                    }
                }
                _ => colour_unknown.extend(at.iter().copied()),
            }
        }
        match device_number(e, *device, "stop_category") {
            Some(c) if (c - 0.0).abs() < 1e-9 || (c - 1.0).abs() < 1e-9 => s.pass(category),
            Some(c) => s.fail(
                category,
                at.clone(),
                format!(
                    "{tag} is stop category {}; an emergency stop works as category 0 or 1",
                    number(c)
                ),
            ),
            None => category_unknown.extend(at.iter().copied()),
        }
    }
    if !colour_unknown.is_empty() {
        s.unknown(
            colour,
            colour_unknown,
            "Emergency stop devices without `actuator_colour` and `background_colour` on the device or its part".to_owned(),
        );
    }
    if !category_unknown.is_empty() {
        s.unknown(
            category,
            category_unknown,
            "Emergency stop devices without a `stop_category` (0 or 1) on the device or its part".to_owned(),
        );
    }
}

fn pushbutton_allowed(function: &str) -> Option<&'static [Colour]> {
    let f = function.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| f.contains(w));
    if has(&["emergency", "estop", "e-stop"]) {
        Some(&[Colour::Red])
    } else if has(&["reset", "acknowledge", "confirm"]) {
        Some(&[Colour::Blue, Colour::Black, Colour::Grey, Colour::White])
    } else if has(&["stop", "off"]) && !has(&["start", "on "]) {
        Some(&[Colour::Red, Colour::Black, Colour::Grey, Colour::White])
    } else if has(&["start", "run", " on", "on "]) || f == "on" {
        Some(&[Colour::Green, Colour::Black, Colour::Grey, Colour::White])
    } else if has(&["abnormal", "intervention", "override"]) {
        Some(&[Colour::Yellow])
    } else if has(&["jog", "inch", "neutral", "other"]) {
        Some(&[Colour::Black, Colour::Grey, Colour::White])
    } else {
        None
    }
}

/// Push-button actuator colours against their function.
pub(super) fn pushbutton_colours(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    let mut gaps: Vec<AnyId> = Vec::new();
    let mut odd: BTreeMap<String, Vec<AnyId>> = BTreeMap::new();
    for (device, _) in devices_of_class(e, |c| c == Class::Pushbutton) {
        let at = at_symbols(e, device);
        let tag = e.tag(device);
        let (Some(function), Some(colour)) = (
            device_text(e, device, "function"),
            device_colour(e, device, "actuator_colour"),
        ) else {
            gaps.extend(at);
            continue;
        };
        let Some(allowed) = pushbutton_allowed(function) else {
            odd.entry(function.to_owned()).or_default().extend(at);
            continue;
        };
        match colour {
            Ok(c) if allowed.contains(&c) => s.pass(id),
            Ok(c) => s.fail(
                id,
                at,
                format!(
                    "{tag} ({function}) is {}; that function takes {}",
                    c.name(),
                    list(allowed)
                ),
            ),
            Err(text) => s.unknown(
                id,
                at,
                format!("{tag}: the actuator colour \"{text}\" is not a known code or name"),
            ),
        }
    }
    if !gaps.is_empty() {
        s.unknown(
            id,
            gaps,
            "Push-buttons without a `function` (start, stop, reset, ...) or an `actuator_colour` on the device or its part".to_owned(),
        );
    }
    for (function, at) in odd {
        s.unknown(
            id,
            at,
            format!("The function \"{function}\" is not one the colour table knows (start, stop, reset, emergency, abnormal, jog)"),
        );
    }
}

fn indicator_allowed(function: &str) -> Option<&'static [Colour]> {
    let f = function.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| f.contains(w));
    if has(&["emergency", "danger", "alarm"]) {
        Some(&[Colour::Red])
    } else if has(&["abnormal", "warning", "caution"]) {
        Some(&[Colour::Yellow])
    } else if has(&["mandatory", "action"]) {
        Some(&[Colour::Blue])
    } else if has(&["normal", "running", "ready", "safe"]) || f == "on" {
        Some(&[Colour::Green])
    } else if has(&["neutral", "information", "info"]) {
        Some(&[Colour::White])
    } else {
        None
    }
}

/// Pilot lamp colours against what they indicate.
pub(super) fn indicator_colours(e: &Env<'_>, s: &mut Sink, id: &'static str) {
    let mut gaps: Vec<AnyId> = Vec::new();
    let mut odd: BTreeMap<String, Vec<AnyId>> = BTreeMap::new();
    for (device, symbols) in devices_of_class(e, |c| c == Class::Lamp) {
        let at = at_symbols(e, device);
        let tag = e.tag(device);
        let colour = symbols
            .iter()
            .find_map(|x| lamp_colour(&x.symbol.0))
            .map(Ok)
            .or_else(|| device_colour(e, device, "lens_colour"));
        let (Some(function), Some(colour)) = (device_text(e, device, "function"), colour) else {
            gaps.extend(at);
            continue;
        };
        let Some(allowed) = indicator_allowed(function) else {
            odd.entry(function.to_owned()).or_default().extend(at);
            continue;
        };
        match colour {
            Ok(c) if allowed.contains(&c) => s.pass(id),
            Ok(c) => s.fail(
                id,
                at,
                format!(
                    "{tag} ({function}) is {}; that meaning takes {}",
                    c.name(),
                    list(allowed)
                ),
            ),
            Err(text) => s.unknown(
                id,
                at,
                format!("{tag}: the lens colour \"{text}\" is not a known code or name"),
            ),
        }
    }
    if !gaps.is_empty() {
        s.unknown(
            id,
            gaps,
            "Lamps without a `function` or a colour (a coloured lamp symbol, or `lens_colour`)".to_owned(),
        );
    }
    for (function, at) in odd {
        s.unknown(
            id,
            at,
            format!("The lamp function \"{function}\" is not one the colour table knows (emergency, abnormal, normal, mandatory, neutral)"),
        );
    }
}

// ----- documentation ---------------------------------------------------------

/// The machine data a nameplate carries is stated in the project.
pub(super) fn nameplate(e: &Env<'_>, s: &mut Sink, id: &'static str, with_sccr: bool) {
    let has_machine = e.project.symbols().any(|x| class_of(x).is_power_load());
    if !has_machine {
        return;
    }
    let mut needed: Vec<(&str, &str)> = vec![
        (keys::RATED_VOLTAGE_V, "rated voltage"),
        (keys::PHASES, "number of phases"),
        (keys::FREQUENCY_HZ, "frequency"),
        (keys::FULL_LOAD_CURRENT_A, "full-load current"),
    ];
    if with_sccr {
        needed.push((keys::PANEL_SCCR_KA, "short-circuit current rating"));
    }
    let missing: Vec<&str> = needed
        .iter()
        .filter(|(key, _)| e.params.value(key).is_none())
        .map(|(_, name)| *name)
        .collect();
    if missing.is_empty() {
        s.pass(id);
    } else {
        s.fail(
            id,
            e.anchor(),
            format!(
                "The nameplate data is incomplete; not stated: {}",
                missing.join(", ")
            ),
        );
    }
}
