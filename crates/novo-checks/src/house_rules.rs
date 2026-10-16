//! The remaining portable check rules of the EPLAN project check (findings
//! 9.3: C02, C03, C05, C06, C07, C10, C11, C14, C15, C17, F01, F04, H01),
//! written from the one-sentence rules, never from EPLAN content. Each rule
//! reads the model only; none edits it. Sources and the user-visible places
//! are listed in `docs/design/eplan-nets.md`.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use novo_core::{
    AnyId, DeviceId, DeviceKind, DeviceTag, Endpoint, Netlist, Point, PotentialKind, SymbolId,
    SymbolInstance, SymbolRole, TerminalKind, TextContent, Value,
};

use crate::contacts::{ContactKind, contact_limit_property};
use crate::{Context, Problem, ProblemKind};

/// Tolerance (mm) for "a line ends on something": the findings' 0.2 mm.
const TOUCH_MM: f64 = 0.2;

/// Every (symbol, pin) a wire ends on.
pub(crate) fn wired_pins(cx: &Context<'_>) -> HashSet<(SymbolId, u16)> {
    cx.project
        .wires()
        .flat_map(|w| w.ends())
        .filter_map(|end| end.as_pin().map(|p| (p.symbol, p.pin)))
        .collect()
}

fn tag_text(tag: &DeviceTag) -> String {
    tag.to_string()
}

/// C02 and C17: a terminal or plug pin no wire reaches, and a plug pin wired
/// on one half only.
///
/// Plug halves are two `Connector` devices; the first names the second in
/// its `plug.mate` property (the tag of the other half). A plug without a
/// mate is judged alone.
pub(crate) fn terminals_and_plugs(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let wired = wired_pins(cx);

    // C02: terminals.
    for terminal in project.terminals() {
        let Some(symbols) = cx.by_terminal.get(&terminal.id) else {
            continue;
        };
        let used = symbols.iter().any(|s| {
            (0..s.pins.len())
                .filter_map(|p| u16::try_from(p).ok())
                .any(|p| s.terminal_of_pin(p) == Some(terminal.id) && wired.contains(&(s.id, p)))
        });
        if !used {
            let name = project
                .terminal_designation(terminal.id)
                .unwrap_or_else(|| terminal.name.clone());
            let mut at: Vec<AnyId> = symbols.iter().map(|s| s.id.into()).collect();
            at.push(terminal.id.into());
            out.push(Problem::new(
                ProblemKind::UnwiredTerminal,
                at,
                format!("Terminal {name} has no wire on any side"),
            ));
        }
    }

    // Plug pins per connector device: pin name -> wired?
    let mut pins: BTreeMap<DeviceId, BTreeMap<String, bool>> = BTreeMap::new();
    for device in project
        .devices()
        .filter(|d| d.kind == DeviceKind::Connector)
    {
        let entry = pins.entry(device.id).or_default();
        for symbol in cx.by_device.get(&device.id).into_iter().flatten() {
            for (index, pin) in symbol.pins.iter().enumerate() {
                let index = u16::try_from(index).unwrap_or(u16::MAX);
                let is_wired = wired.contains(&(symbol.id, index));
                let slot = entry.entry(pin.name.clone()).or_insert(false);
                *slot |= is_wired;
            }
        }
    }
    let mate_of = |id: DeviceId| -> Option<DeviceId> {
        let device = project.device(id)?;
        let Some(Value::Text(mate)) = device.properties.get("plug.mate") else {
            return None;
        };
        let wanted = DeviceTag::parse(mate).ok()?;
        project
            .devices()
            .find(|d| d.kind == DeviceKind::Connector && d.tag == wanted && d.id != id)
            .map(|d| d.id)
    };
    let symbol_ids = |id: DeviceId| -> Vec<AnyId> {
        cx.by_device
            .get(&id)
            .into_iter()
            .flatten()
            .map(|s| s.id.into())
            .collect()
    };
    let mut done: BTreeSet<DeviceId> = BTreeSet::new();
    for (&id, halves) in &pins {
        if done.contains(&id) {
            continue;
        }
        let Some(device) = project.device(id) else {
            continue;
        };
        let mate = mate_of(id).filter(|m| pins.contains_key(m));
        let Some(mate) = mate else {
            for (name, _) in halves.iter().filter(|(_, wired)| !**wired) {
                out.push(Problem::new(
                    ProblemKind::UnwiredTerminal,
                    symbol_ids(id),
                    format!("Plug pin {}:{name} has no wire", tag_text(&device.tag)),
                ));
            }
            continue;
        };
        done.insert(id);
        done.insert(mate);
        let Some(other) = project.device(mate) else {
            continue;
        };
        let other_pins = &pins[&mate];
        let names: BTreeSet<&String> = halves.keys().chain(other_pins.keys()).collect();
        let mut at = symbol_ids(id);
        at.extend(symbol_ids(mate));
        for name in names {
            let here = halves.get(name).copied().unwrap_or(false);
            let there = other_pins.get(name).copied().unwrap_or(false);
            let (a, b) = (tag_text(&device.tag), tag_text(&other.tag));
            if !here && !there {
                out.push(Problem::new(
                    ProblemKind::UnwiredTerminal,
                    at.clone(),
                    format!("Plug pin {name} has no wire on {a} or {b}"),
                ));
            } else if here != there {
                let (with, without) = if here { (&a, &b) } else { (&b, &a) };
                out.push(Problem::new(
                    ProblemKind::PlugPinOneSided,
                    at.clone(),
                    format!("Plug pin {name} is wired on {with} but not on {without}"),
                ));
            }
        }
    }
}

/// C03: an open wire end that lies on a pin, a junction or another wire
/// without being connected. It looks joined on the page and is not.
pub(crate) fn stray_wire_ends(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let near = |a: Point, b: Point| (a.x - b.x).hypot(a.y - b.y) <= TOUCH_MM;
    let free: Vec<_> = project
        .wires()
        .flat_map(|w| {
            w.ends().into_iter().filter_map(move |end| match end {
                Endpoint::Free { at } => Some((w, at)),
                _ => None,
            })
        })
        .collect();
    if free.is_empty() {
        return;
    }
    // Segments per wire, in page coordinates.
    let segments = |wire: &novo_core::Wire| -> Vec<(Point, Point)> {
        let mut points = Vec::new();
        points.extend(project.endpoint_position(wire.from));
        points.extend(wire.route.iter().copied());
        points.extend(project.endpoint_position(wire.to));
        points.windows(2).map(|w| (w[0], w[1])).collect()
    };
    for (wire, at) in free {
        let mut on: Option<String> = None;
        'search: {
            for symbol in project.symbols().filter(|s| s.page == wire.page) {
                for (index, pin) in symbol.pins.iter().enumerate() {
                    if near(symbol.placement.apply(pin.offset), at) {
                        let index = u16::try_from(index).unwrap_or(u16::MAX);
                        on = Some(cx.endpoint_label(Endpoint::pin(symbol.id, index)));
                        break 'search;
                    }
                }
            }
            for junction in project.junctions().filter(|j| j.page == wire.page) {
                if near(junction.position, at) {
                    on = Some("a junction".to_owned());
                    break 'search;
                }
            }
            for other in project
                .wires()
                .filter(|o| o.page == wire.page && o.id != wire.id)
            {
                if segments(other)
                    .into_iter()
                    .any(|(a, b)| distance_to_segment(at, a, b) <= TOUCH_MM)
                {
                    on = Some("another wire".to_owned());
                    break 'search;
                }
            }
        }
        if let Some(on) = on {
            out.push(Problem::new(
                ProblemKind::StrayWireEnd,
                vec![wire.id.into()],
                format!(
                    "An open wire end at {:.1}, {:.1} mm lies on {on} but is not connected to it",
                    at.x, at.y
                ),
            ));
        }
    }
}

fn distance_to_segment(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0)
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

/// Contact pin number `13` as (position 1, function 3). Only names of two or
/// more digits carry both.
fn contact_number(name: &str) -> Option<(u32, u32)> {
    if name.len() < 2 || !name.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let (position, function) = name.split_at(name.len() - 1);
    Some((position.parse().ok()?, function.parse().ok()?))
}

fn is_aux_contact(kind: ContactKind) -> bool {
    matches!(
        kind,
        ContactKind::NormallyOpen | ContactKind::NormallyClosed | ContactKind::Changeover
    )
}

fn aux_contacts<'a>(
    cx: &'a Context<'_>,
) -> impl Iterator<Item = (DeviceId, &'a SymbolInstance, ContactKind)> {
    cx.by_device.iter().flat_map(|(&device, symbols)| {
        symbols.iter().filter_map(move |&s| {
            let kind = ContactKind::of_key(&s.symbol);
            (is_aux_contact(kind) && !matches!(s.role, SymbolRole::Device { main: true, .. }))
                .then_some((device, s, kind))
        })
    })
}

/// C05: a normally open contact is numbered `x3 x4`, a normally closed one
/// `x1 x2`, a changeover `x1 x2 x4`; the delayed and overload forms use
/// `x5 x6` (closed) and `x7 x8` (open), as on the `95 96 97 98` of a relay.
pub(crate) fn contact_types(cx: &Context<'_>, out: &mut Vec<Problem>) {
    for (_, symbol, kind) in aux_contacts(cx) {
        let allowed: &[u32] = match kind {
            ContactKind::NormallyOpen => &[3, 4, 7, 8],
            ContactKind::NormallyClosed => &[1, 2, 5, 6],
            _ => &[1, 2, 4, 5, 6, 8],
        };
        let wrong: Vec<&str> = symbol
            .pins
            .iter()
            .filter(|p| contact_number(&p.name).is_some_and(|(_, f)| !allowed.contains(&f)))
            .map(|p| p.name.as_str())
            .collect();
        if !wrong.is_empty() {
            let uses = allowed
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            out.push(Problem::new(
                ProblemKind::ContactType,
                vec![symbol.id.into()],
                format!(
                    "{}: {} contact has pin {} but its numbers end in {uses}",
                    cx.symbol_label(symbol),
                    kind.label(),
                    wrong.join(", ")
                ),
            ));
        }
    }
}

/// C06: the contact positions used (the tens of `13`, `24`) fit the number of
/// contacts the device declares (`contacts.no`, `.nc`, `.co`).
pub(crate) fn contact_positions(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for (device_id, symbol, _) in aux_contacts(cx) {
        let Some(device) = project.device(device_id) else {
            continue;
        };
        let limits: Vec<i64> = [
            ContactKind::NormallyOpen,
            ContactKind::NormallyClosed,
            ContactKind::Changeover,
        ]
        .into_iter()
        .filter_map(contact_limit_property)
        .filter_map(|key| match device.properties.get(key) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        })
        .collect();
        let total = u32::try_from(limits.iter().sum::<i64>().max(0)).unwrap_or(u32::MAX);
        // Nothing declared, or none available (the too-many-contacts check
        // reports that): there is no range to judge against.
        if limits.is_empty() || total == 0 {
            continue;
        }
        let beyond: Vec<&str> = symbol
            .pins
            .iter()
            .filter(|p| contact_number(&p.name).is_some_and(|(pos, _)| pos > total))
            .map(|p| p.name.as_str())
            .collect();
        if !beyond.is_empty() {
            out.push(Problem::new(
                ProblemKind::ContactPosition,
                vec![symbol.id.into()],
                format!(
                    "{}: contact {} is beyond the {total} auxiliary contacts the device has",
                    tag_text(&device.tag),
                    beyond.join(", ")
                ),
            ));
        }
    }
}

/// C07: one physical contact (`13-14`) drawn twice for one device.
pub(crate) fn contacts_drawn_twice(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let mut seen: BTreeMap<(DeviceId, Vec<&str>), Vec<&SymbolInstance>> = BTreeMap::new();
    for (device, symbol, _) in aux_contacts(cx) {
        let mut names: Vec<&str> = symbol
            .pins
            .iter()
            .map(|p| p.name.as_str())
            .filter(|n| contact_number(n).is_some())
            .collect();
        if names.is_empty() {
            continue;
        }
        names.sort_unstable();
        seen.entry((device, names)).or_default().push(symbol);
    }
    for ((device, names), symbols) in seen {
        if symbols.len() > 1 {
            let tag = cx
                .project
                .device(device)
                .map_or_else(String::new, |d| tag_text(&d.tag));
            out.push(Problem::new(
                ProblemKind::ContactDrawnTwice,
                symbols.iter().map(|s| s.id.into()).collect(),
                format!(
                    "{tag}: contact {} is drawn {} times",
                    names.join("-"),
                    symbols.len()
                ),
            ));
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum VoltageClass {
    ExtraLow,
    Low,
}

/// C10: extra-low voltage (up to 50 V AC or 120 V DC) and low voltage on
/// the pages of one function code. Houses that keep the voltage classes
/// apart give them separate function codes.
pub(crate) fn voltage_classes(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    // function code -> class -> first wire seen
    let mut by_function: BTreeMap<String, BTreeMap<VoltageClass, AnyId>> = BTreeMap::new();
    for wire in project.wires() {
        let Some(page) = project.page(wire.page) else {
            continue;
        };
        if page.structure.function.is_empty() {
            continue;
        }
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
        let Some(volts) = potential.voltage else {
            continue;
        };
        let limit = match potential.kind {
            PotentialKind::DcPositive | PotentialKind::DcNegative => 120.0,
            PotentialKind::Line
            | PotentialKind::Neutral
            | PotentialKind::Pen
            | PotentialKind::Signal => 50.0,
            PotentialKind::ProtectiveEarth => continue,
        };
        let class = if volts <= limit {
            VoltageClass::ExtraLow
        } else {
            VoltageClass::Low
        };
        let code: String = page
            .structure
            .function
            .iter()
            .map(|f| format!("={f}"))
            .collect();
        by_function
            .entry(code)
            .or_default()
            .entry(class)
            .or_insert_with(|| wire.id.into());
    }
    for (code, classes) in by_function {
        if classes.len() > 1 {
            out.push(Problem::new(
                ProblemKind::VoltageClassMix,
                classes.into_values().collect(),
                format!("Extra-low and low voltage are both used under {code}"),
            ));
        }
    }
}

/// C11: a protective-earth or neutral terminal whose net carries no such
/// potential, in a project that defines one. The conductor is broken
/// somewhere between the terminal and its source.
pub(crate) fn conductor_continuity(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let defines = |kinds: &[PotentialKind]| project.potentials().any(|p| kinds.contains(&p.kind));
    let has_pe = defines(&[PotentialKind::ProtectiveEarth, PotentialKind::Pen]);
    let has_n = defines(&[PotentialKind::Neutral, PotentialKind::Pen]);
    for terminal in project.terminals() {
        let (wanted, what): (&[PotentialKind], _) = match terminal.kind {
            TerminalKind::ProtectiveEarth if has_pe => (
                &[PotentialKind::ProtectiveEarth, PotentialKind::Pen],
                "protective earth",
            ),
            TerminalKind::Neutral if has_n => {
                (&[PotentialKind::Neutral, PotentialKind::Pen], "neutral")
            }
            _ => continue,
        };
        let Some(symbols) = cx.by_terminal.get(&terminal.id) else {
            continue;
        };
        let mut wired = false;
        let mut reached = false;
        for symbol in symbols {
            for pin in 0..symbol.pins.len() {
                let Ok(pin) = u16::try_from(pin) else { break };
                if symbol.terminal_of_pin(pin) != Some(terminal.id) {
                    continue;
                }
                let Some(net) = netlist.net_of(Endpoint::pin(symbol.id, pin)) else {
                    continue;
                };
                let net = netlist.net(net);
                wired |= !net.wires.is_empty();
                reached |= net
                    .potentials
                    .iter()
                    .chain(&net.reachable)
                    .filter_map(|&p| project.potential(p))
                    .any(|p| wanted.contains(&p.kind));
            }
        }
        if wired && !reached {
            let name = project
                .terminal_designation(terminal.id)
                .unwrap_or_else(|| terminal.name.clone());
            let mut at: Vec<AnyId> = symbols.iter().map(|s| s.id.into()).collect();
            at.push(terminal.id.into());
            out.push(Problem::new(
                ProblemKind::ConductorContinuity,
                at,
                format!("Terminal {name} is a {what} terminal but is not connected to the {what} conductor"),
            ));
        }
    }
}

/// C14: the three windings of a motor (`U1`, `V1`, `W1`) must be able to
/// receive three different phases: separate nets, and where the nets carry a
/// potential, three different line potentials.
pub(crate) fn motor_phases(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    for symbol in project.symbols() {
        let ends: Vec<Endpoint> = ["U1", "V1", "W1"]
            .into_iter()
            .filter_map(|n| symbol.pin_index(n))
            .map(|p| Endpoint::pin(symbol.id, p))
            .collect();
        if ends.len() != 3 {
            continue;
        }
        let nets: Vec<_> = ends
            .iter()
            .filter_map(|&e| netlist.net_of(e))
            .filter(|&n| !netlist.net(n).wires.is_empty())
            .collect();
        if nets.len() != 3 {
            continue; // an unwired winding is the unconnected-point check's
        }
        let label = cx.symbol_label(symbol);
        let mut message = None;
        if nets[0] == nets[1] || nets[1] == nets[2] || nets[0] == nets[2] {
            message = Some(format!(
                "{label}: two windings are on one net: a short between phases"
            ));
        } else {
            let potentials: Vec<_> = nets
                .iter()
                .map(|&n| {
                    netlist
                        .net(n)
                        .potential()
                        .and_then(|p| project.potential(p))
                })
                .collect();
            if potentials.iter().flatten().any(|p| {
                matches!(
                    p.kind,
                    PotentialKind::Neutral | PotentialKind::ProtectiveEarth | PotentialKind::Pen
                )
            }) {
                message = Some(format!(
                    "{label}: a winding is wired to neutral or earth, not to a phase"
                ));
            } else {
                let ids: Vec<_> = nets.iter().map(|&n| netlist.net(n).potential()).collect();
                for (i, a) in ids.iter().enumerate() {
                    if a.is_some() && ids[i + 1..].contains(a) {
                        message = Some(format!("{label}: two windings get the same phase"));
                    }
                }
            }
        }
        if let Some(message) = message {
            out.push(Problem::new(
                ProblemKind::MotorPhases,
                vec![symbol.id.into()],
                message,
            ));
        }
    }
}

/// C15: a coil (`A1`, `A2`) between a control voltage and its return: not
/// both sides on one net, not both on the same potential, and neither side
/// on protective earth.
pub(crate) fn coil_supply(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    for symbol in project.symbols() {
        if !matches!(symbol.role, SymbolRole::Device { main: true, .. })
            || ContactKind::of_key(&symbol.symbol) != ContactKind::Other
        {
            continue;
        }
        let (Some(a1), Some(a2)) = (symbol.pin_index("A1"), symbol.pin_index("A2")) else {
            continue;
        };
        let (a1, a2) = (Endpoint::pin(symbol.id, a1), Endpoint::pin(symbol.id, a2));
        let (Some(n1), Some(n2)) = (netlist.net_of(a1), netlist.net_of(a2)) else {
            continue;
        };
        if netlist.net(n1).wires.is_empty() || netlist.net(n2).wires.is_empty() {
            continue;
        }
        let label = cx.symbol_label(symbol);
        let message = if n1 == n2 {
            Some(format!(
                "{label}: both coil sides are on one net, the coil is shorted"
            ))
        } else {
            let p1 = netlist.net(n1).potential();
            let p2 = netlist.net(n2).potential();
            let earth = [p1, p2].into_iter().flatten().any(|p| {
                project.potential(p).is_some_and(|p| {
                    matches!(p.kind, PotentialKind::ProtectiveEarth | PotentialKind::Pen)
                })
            });
            if p1.is_some() && p1 == p2 {
                Some(format!("{label}: both coil sides have the same potential"))
            } else if earth {
                Some(format!("{label}: a coil side is on protective earth"))
            } else {
                None
            }
        };
        if let Some(message) = message {
            out.push(Problem::new(
                ProblemKind::CoilSupply,
                vec![symbol.id.into()],
                message,
            ));
        }
    }
}

/// F01, opt-in with the project property `tag.rule` = `page-column`: a
/// device's name is its type letters, the two-digit page and the one-digit
/// column of its main symbol (`K031`), `.1`, `.2` for repeats.
pub(crate) fn tag_rule(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    if !matches!(project.info().properties.get("tag.rule"), Some(Value::Text(t)) if t == "page-column")
    {
        return;
    }
    for device in project.devices().filter(|d| d.kind == DeviceKind::General) {
        let Some(name) = device.tag.product.last() else {
            continue;
        };
        let Some(symbol) = cx.by_device.get(&device.id).and_then(|s| s.first()) else {
            continue;
        };
        let Some(page) = project.page(symbol.page) else {
            continue;
        };
        let Ok(number) = page.name.trim().parse::<u32>() else {
            continue;
        };
        let letters: String = name.chars().take_while(|c| c.is_alphabetic()).collect();
        if letters.is_empty() {
            continue;
        }
        let column = page.format.column_at(symbol.placement.origin.x);
        let expected = format!("{letters}{number:02}{column}");
        let ok = name.strip_prefix(&expected).is_some_and(|rest| {
            rest.is_empty()
                || rest
                    .strip_prefix('.')
                    .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        });
        if !ok {
            out.push(Problem::new(
                ProblemKind::TagRule,
                vec![device.id.into(), symbol.id.into()],
                format!(
                    "{} does not follow the page and column rule: on page {}, column {column} it would be -{expected}",
                    tag_text(&device.tag),
                    page.name
                ),
            ));
        }
    }
}

/// F04: a typed-in text on a device symbol that reads like a tag but is not
/// the device's tag (or its short form).
pub(crate) fn displayed_tags(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for text in project.texts() {
        let (TextContent::Literal { text: shown }, Some(AnyId::Symbol(symbol))) =
            (&text.content, text.attached_to)
        else {
            continue;
        };
        let shown = shown.trim();
        if !shown.starts_with(['-', '=', '+']) {
            continue;
        }
        let Some(symbol) = project.symbol(symbol) else {
            continue;
        };
        let SymbolRole::Device { device, .. } = &symbol.role else {
            continue;
        };
        let Some(device) = project.device(*device) else {
            continue;
        };
        let full = tag_text(&device.tag);
        let short = format!("-{}", device.tag.product.join("-"));
        if shown != full && shown != short {
            out.push(Problem::new(
                ProblemKind::DisplayedTag,
                vec![text.id.into(), symbol.id.into()],
                format!("The text {shown} on {full} reads as a tag but is not the device's tag"),
            ));
        }
    }
}

/// H01: a function or location code used on a device or a page that the
/// project's structure list does not know. Opt-in with the project property
/// `structure.list` = `complete`: a list that only describes the cabinets
/// (the usual case while drawing) is not a list of every code.
pub(crate) fn structure_codes(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    if !matches!(project.info().properties.get("structure.list"), Some(Value::Text(t)) if t == "complete")
    {
        return;
    }
    let mut functions: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut locations: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut any = false;
    for s in project.structures() {
        any = true;
        for n in 1..=s.tag.function.len() {
            functions.insert(s.tag.function[..n].to_vec());
        }
        for n in 1..=s.tag.location.len() {
            locations.insert(s.tag.location[..n].to_vec());
        }
    }
    if !any {
        return;
    }
    let mut unknown: BTreeMap<String, Vec<AnyId>> = BTreeMap::new();
    let mut note = |tag: &DeviceTag, id: AnyId| {
        if !tag.function.is_empty() && !functions.contains(&tag.function) {
            let code: String = tag.function.iter().map(|f| format!("={f}")).collect();
            unknown.entry(code).or_default().push(id);
        }
        if !tag.location.is_empty() && !locations.contains(&tag.location) {
            let code: String = tag.location.iter().map(|l| format!("+{l}")).collect();
            unknown.entry(code).or_default().push(id);
        }
    };
    for device in project.devices() {
        note(&device.tag, device.id.into());
    }
    for page in project.pages() {
        note(&page.structure, page.id.into());
    }
    for (code, at) in unknown {
        out.push(Problem::new(
            ProblemKind::StructureCode,
            at,
            format!("{code} is not in the project's structure list"),
        ));
    }
}
