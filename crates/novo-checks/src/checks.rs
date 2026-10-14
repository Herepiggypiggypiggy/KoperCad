//! The individual checks. Each appends its problems in any order; `check`
//! sorts them afterwards.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{
    AnyId, DeviceId, DeviceTag, Endpoint, Netlist, PageId, Point, SameName, SymbolId,
    SymbolInstance, SymbolRole, Value,
};

use crate::contacts::{ContactKind, contact_limit_property};
use crate::{Context, Problem, ProblemKind};

/// Positions closer than this (mm) are one point.
const EPS: f64 = 1e-6;

/// A page and a position on it snapped to a micrometre grid.
type GridKey = (PageId, i64, i64);

/// A position snapped to a micrometre grid, for finding coincident points.
fn grid_key(page: PageId, p: Point) -> GridKey {
    // Saturating float-to-int casts; drawings are far inside i64 range.
    (
        page,
        (p.x * 1000.0).round() as i64,
        (p.y * 1000.0).round() as i64,
    )
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Connection points no wire ends on. A point that lies on another pin or a
/// junction without a wire is reported too, with its own wording: touching
/// is not a connection in the model (the editor draws a wire when pins
/// touch, DESIGN.md §2), so it looks connected but is not.
///
/// The model has no "optional connection point" flag yet, so every pin is
/// expected to be wired.
pub(crate) fn unconnected_pins(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    // Free pins first; most projects have few, so the position lookup below
    // only tracks their points. Wired pins come from one sorted pass over
    // the wires, searched per symbol, which is much faster than a map lookup
    // per pin on large pages.
    let mut wired: Vec<(SymbolId, u16)> = project
        .wires()
        .flat_map(|w| w.ends())
        .filter_map(|end| end.as_pin().map(|p| (p.symbol, p.pin)))
        .collect();
    wired.sort_unstable();
    wired.dedup();
    let mut free: Vec<(&SymbolInstance, Vec<(&str, GridKey)>)> = Vec::new();
    for symbol in project.symbols() {
        let start = wired.partition_point(|w| w.0 < symbol.id);
        let end = start + wired[start..].partition_point(|w| w.0 == symbol.id);
        let wired_here = &wired[start..end];
        let mut pins = Vec::new();
        for (index, pin) in symbol.pins.iter().enumerate() {
            let Ok(index) = u16::try_from(index) else {
                break;
            };
            if wired_here.binary_search(&(symbol.id, index)).is_err() {
                let at = symbol.placement.apply(pin.offset);
                pins.push((pin.name.as_str(), grid_key(symbol.page, at)));
            }
        }
        if !pins.is_empty() {
            free.push((symbol, pins));
        }
    }
    if free.is_empty() {
        return;
    }
    // How many connection points (pins, junctions) sit on each free pin's
    // point; more than one means it touches something.
    let mut points: HashMap<GridKey, u32> = free
        .iter()
        .flat_map(|(_, pins)| pins.iter().map(|&(_, key)| (key, 0)))
        .collect();
    let mut count = |key| {
        if let Some(n) = points.get_mut(&key) {
            *n += 1;
        }
    };
    for symbol in project.symbols() {
        for pin in &symbol.pins {
            count(grid_key(symbol.page, symbol.placement.apply(pin.offset)));
        }
    }
    for junction in project.junctions() {
        count(grid_key(junction.page, junction.position));
    }
    for (symbol, pins) in free {
        let (touching, alone): (Vec<_>, Vec<_>) =
            pins.into_iter().partition(|(_, key)| points[key] > 1);
        let names = |list: Vec<(&str, _)>| {
            list.into_iter()
                .map(|(name, _)| name)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let mut parts = Vec::new();
        if !alone.is_empty() {
            parts.push(format!("{} not connected", names(alone)));
        }
        if !touching.is_empty() {
            let verb = if touching.len() == 1 {
                "touches"
            } else {
                "touch"
            };
            parts.push(format!(
                "{} {verb} another point but no wire joins them",
                names(touching)
            ));
        }
        out.push(Problem::new(
            ProblemKind::UnconnectedPin,
            vec![symbol.id.into()],
            format!("{}: {}", cx.symbol_label(symbol), parts.join("; ")),
        ));
    }
}

/// What to mark for a device: its first drawn function (main first), or the
/// device itself when it is not drawn. Terminal strips are drawn through
/// their terminals.
fn device_mark(cx: &Context<'_>, device: DeviceId) -> AnyId {
    if let Some(symbol) = cx.by_device.get(&device).and_then(|s| s.first()) {
        return symbol.id.into();
    }
    cx.project
        .terminals_of_strip(device)
        .iter()
        .find_map(|t| cx.by_terminal.get(&t.id)?.first())
        .map_or(device.into(), |s| s.id.into())
}

/// One tag on several devices or cables. Empty tags are ignored. Same
/// result as `Project::duplicate_tags`, found by sorting on the device code
/// first, which is several times faster on large projects.
pub(crate) fn duplicate_tags(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    fn code(tag: &DeviceTag) -> &str {
        tag.product.last().map_or("", String::as_str)
    }
    let mut tags: Vec<(&str, &DeviceTag, AnyId)> = project
        .devices()
        .map(|d| (code(&d.tag), &d.tag, AnyId::from(d.id)))
        .chain(
            project
                .cables()
                .map(|c| (code(&c.tag), &c.tag, AnyId::from(c.id))),
        )
        .filter(|(_, tag, _)| !tag.is_empty())
        .collect();
    tags.sort_unstable_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(b.1)).then(a.2.cmp(&b.2)));
    let mut duplicates: Vec<(&DeviceTag, Vec<AnyId>)> = Vec::new();
    for group in tags.chunk_by(|a, b| a.1 == b.1) {
        if group.len() > 1 {
            duplicates.push((group[0].1, group.iter().map(|g| g.2).collect()));
        }
    }
    for (tag, ids) in duplicates {
        let devices = ids
            .iter()
            .filter(|id| matches!(id, AnyId::Device(_)))
            .count();
        let cables = ids.len() - devices;
        let users = match (devices, cables) {
            (d, 0) => plural(d, "device", "devices"),
            (0, c) => plural(c, "cable", "cables"),
            (d, c) => format!(
                "{} and {}",
                plural(d, "device", "devices"),
                plural(c, "cable", "cables")
            ),
        };
        let at = ids
            .iter()
            .map(|&id| match id {
                AnyId::Device(d) => device_mark(cx, d),
                other => other,
            })
            .collect();
        out.push(Problem::new(
            ProblemKind::DuplicateTag,
            at,
            format!("{tag} is used by {users}"),
        ));
    }
}

/// Terminals sharing a name and level on one strip while the project's
/// same-name rule is off. Under `Join` they are one point by intent, and
/// `Refuse` never lets them exist.
pub(crate) fn duplicate_terminals(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    if project.info().rules.terminals.same_name != SameName::Off {
        return;
    }
    for group in project.terminal_designation_groups() {
        let Some(name) = group.first().and_then(|&t| project.terminal_designation(t)) else {
            continue;
        };
        let at = group
            .iter()
            .map(|&t| {
                cx.by_terminal
                    .get(&t)
                    .and_then(|s| s.first())
                    .map_or(t.into(), |s| s.id.into())
            })
            .collect();
        out.push(Problem::new(
            ProblemKind::DuplicateTerminal,
            at,
            format!(
                "{name} is on its strip {} times as separate terminals",
                group.len()
            ),
        ));
    }
}

const LIMITED: [ContactKind; 4] = [
    ContactKind::Power,
    ContactKind::NormallyOpen,
    ContactKind::NormallyClosed,
    ContactKind::Changeover,
];

/// Main functions and contacts of each device: no main function, several
/// main functions, and more contacts of a kind than the device has.
pub(crate) fn device_functions(cx: &Context<'_>, out: &mut Vec<Problem>) {
    for device in cx.project.devices() {
        let Some(symbols) = cx.by_device.get(&device.id) else {
            continue;
        };
        let is_main =
            |s: &&&SymbolInstance| matches!(s.role, SymbolRole::Device { main: true, .. });
        let mains: Vec<&SymbolInstance> = symbols.iter().filter(is_main).copied().collect();
        let tag = &device.tag;
        if mains.is_empty() {
            out.push(Problem::new(
                ProblemKind::NoMainFunction,
                symbols.iter().map(|s| s.id.into()).collect(),
                format!("{tag} is drawn without its main function (coil)"),
            ));
        } else if mains.len() > 1 {
            out.push(Problem::new(
                ProblemKind::SeveralMainFunctions,
                mains.iter().map(|s| s.id.into()).collect(),
                format!("{tag} has {} main functions", mains.len()),
            ));
        }
        for kind in LIMITED {
            let Some(Value::Int(limit)) =
                contact_limit_property(kind).and_then(|key| device.properties.get(key))
            else {
                continue;
            };
            let available = usize::try_from(*limit).unwrap_or(0);
            let drawn: Vec<AnyId> = symbols
                .iter()
                .filter(|s| !is_main(s) && ContactKind::of_key(&s.symbol) == kind)
                .map(|s| s.id.into())
                .collect();
            if drawn.len() > available {
                out.push(Problem::new(
                    ProblemKind::TooManyContacts,
                    drawn.clone(),
                    format!(
                        "{tag}: {} {} contacts drawn, {available} available",
                        drawn.len(),
                        kind.label()
                    ),
                ));
            }
        }
    }
}

/// Interruption points whose name appears only once in the project.
pub(crate) fn lone_interruptions(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let mut by_name: BTreeMap<&str, Vec<&SymbolInstance>> = BTreeMap::new();
    for symbol in cx.project.symbols() {
        if let SymbolRole::Interruption { name } = &symbol.role {
            by_name.entry(name.as_str()).or_default().push(symbol);
        }
    }
    for (name, symbols) in by_name {
        if let [lone] = symbols.as_slice() {
            out.push(Problem::new(
                ProblemKind::LoneInterruption,
                vec![lone.id.into()],
                format!("Interruption point {name} has no partner"),
            ));
        }
    }
}

/// Nets where two or more potentials are defined (`Netlist::potential_conflicts`).
/// Marks what defines them: potential feeds, wires with a potential and,
/// under the terminal-name rule, terminals named like a potential.
pub(crate) fn potential_conflicts(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    let from_names = project.info().rules.potentials.from_terminal_names;
    for net in netlist.potential_conflicts() {
        let names: Vec<&str> = net
            .defined
            .iter()
            .filter_map(|&p| project.potential(p).map(|p| p.name.as_str()))
            .collect();
        let mut symbols = BTreeSet::new();
        for end in &net.endpoints {
            let Some(pin) = end.as_pin() else { continue };
            let Some(symbol) = project.symbol(pin.symbol) else {
                continue;
            };
            let defines = match &symbol.role {
                SymbolRole::PotentialFeed { potential } => net.defined.contains(potential),
                SymbolRole::Terminal { .. } if from_names => symbol
                    .terminal_of_pin(pin.pin)
                    .and_then(|t| project.terminal(t))
                    .is_some_and(|t| names.contains(&t.name.as_str())),
                _ => false,
            };
            if defines {
                symbols.insert(symbol.id);
            }
        }
        let wires = net.wires.iter().filter(|&&w| {
            project
                .wire(w)
                .and_then(|w| w.potential)
                .is_some_and(|p| net.defined.contains(&p))
        });
        let mut at: Vec<AnyId> = symbols.into_iter().map(AnyId::from).collect();
        at.extend(wires.map(|&w| AnyId::from(w)));
        if at.is_empty() {
            at.extend(net.wires.iter().map(|&w| AnyId::from(w)));
        }
        let list = match names.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
            _ => names.join(""),
        };
        out.push(Problem::new(
            ProblemKind::PotentialConflict,
            at,
            format!("{list} are defined on one net: a short"),
        ));
    }
}

/// Wires whose drawn length is zero. Two pins of different symbols at one
/// point are exempt: that is how touching pins connect (DESIGN.md §2).
pub(crate) fn zero_length_wires(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for wire in project.wires() {
        // Two pins of different symbols may coincide: that is how touching
        // pins connect. Skipping them first saves the position lookups for
        // most wires.
        if let (Endpoint::Pin { symbol: a, .. }, Endpoint::Pin { symbol: b, .. }) =
            (wire.from, wire.to)
            && a != b
        {
            continue;
        }
        let (Some(from), Some(to)) = (
            project.endpoint_position(wire.from),
            project.endpoint_position(wire.to),
        ) else {
            continue;
        };
        // Zero length: every point of the route lies on the start.
        let on_start = |p: &Point| p.distance(from) <= EPS;
        if !on_start(&to) || !wire.route.iter().all(on_start) {
            continue;
        }
        out.push(Problem::new(
            ProblemKind::ZeroLengthWire,
            vec![wire.id.into()],
            format!(
                "The wire from {} to {} has zero length",
                cx.endpoint_label(wire.from),
                cx.endpoint_label(wire.to)
            ),
        ));
    }
}

/// Devices and cables without a part. Info only: parts are assigned late
/// (WS-07 proposes them), and DESIGN.md §6 step 7 lists them in Issues.
/// Terminals are not checked: their article usually comes with the strip.
pub(crate) fn missing_parts(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for device in project.devices() {
        if device.parts.is_empty() {
            out.push(Problem::new(
                ProblemKind::MissingPart,
                vec![device_mark(cx, device.id)],
                format!("{} has no part", device.tag),
            ));
        }
    }
    for cable in project.cables() {
        if cable.part.is_none() {
            out.push(Problem::new(
                ProblemKind::MissingPart,
                vec![cable.id.into()],
                format!("{} has no part", cable.tag),
            ));
        }
    }
}
