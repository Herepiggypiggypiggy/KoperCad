//! The individual checks. Each appends its problems in any order; `check`
//! sorts them afterwards.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{
    AnyId, DeviceId, DeviceKind, DeviceTag, Endpoint, Netlist, PageId, Point, SameName, SymbolId,
    SymbolInstance, SymbolRole, TerminalKind, Value,
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
/// A spare point (`Pin::optional`, from the symbol library) may stay free;
/// it is still reported when it touches another point without a wire,
/// since that looks connected but is not.
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
    // Per free pin: its name, whether it is a spare point, where it is.
    type FreePin<'a> = (&'a str, bool, GridKey);
    let mut free: Vec<(&SymbolInstance, Vec<FreePin<'_>>)> = Vec::new();
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
                pins.push((pin.name.as_str(), pin.optional, grid_key(symbol.page, at)));
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
        .flat_map(|(_, pins)| pins.iter().map(|&(_, _, key)| (key, 0)))
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
            pins.into_iter().partition(|(_, _, key)| points[key] > 1);
        // A spare point left free is fine.
        let alone: Vec<_> = alone
            .into_iter()
            .filter(|(_, optional, _)| !optional)
            .collect();
        if touching.is_empty() && alone.is_empty() {
            continue;
        }
        let names = |list: Vec<(&str, bool, _)>| {
            list.into_iter()
                .map(|(name, _, _)| name)
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
/// Marks what defines them, as the netlist names it (`Net::sources`): feed
/// symbols and source pins with a feed, wires with a potential, supply
/// terminals and, under the terminal-name rule, terminals named like a
/// potential (marked through their symbols on the net).
pub(crate) fn potential_conflicts(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    for net in netlist.potential_conflicts() {
        let names: Vec<&str> = net
            .defined
            .iter()
            .filter_map(|&p| project.potential(p).map(|p| p.name.as_str()))
            .collect();
        let mut symbols = BTreeSet::new();
        let mut wires = BTreeSet::new();
        for source in &net.sources {
            match source.by {
                AnyId::Symbol(s) => {
                    symbols.insert(s);
                }
                AnyId::Wire(w) => {
                    wires.insert(w);
                }
                // A terminal is drawn through its symbols; mark those on
                // this net.
                AnyId::Terminal(t) => symbols.extend(
                    project
                        .symbols_of_terminal(t)
                        .into_iter()
                        .map(|s| s.id)
                        .filter(|s| {
                            net.endpoints
                                .iter()
                                .any(|e| e.as_pin().is_some_and(|p| p.symbol == *s))
                        }),
                ),
                _ => {}
            }
        }
        let mut at: Vec<AnyId> = symbols.into_iter().map(AnyId::from).collect();
        at.extend(wires.into_iter().map(AnyId::from));
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
/// A terminal strip is judged by its terminals: their article is their own
/// part or the strip's style for their kind
/// ([`novo_terminals::terminal_part`], the rule the strip editor, the bill
/// of materials and `novo_parts::missing_parts` share), never the strip
/// device's own part list. A strip is flagged once, naming its terminals
/// without an article.
pub(crate) fn missing_parts(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for device in project.devices() {
        if device.kind == DeviceKind::TerminalStrip {
            let bare: Vec<String> = project
                .terminals_of_strip(device.id)
                .iter()
                .filter(|t| novo_terminals::terminal_part(project, t.id).is_none())
                .map(|t| t.name.clone())
                .collect();
            if !bare.is_empty() {
                out.push(Problem::new(
                    ProblemKind::MissingPart,
                    vec![device_mark(cx, device.id)],
                    format!("{} terminals {} have no part", device.tag, bare.join(", ")),
                ));
            }
        } else if device.parts.is_empty() {
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

/// Current-carrying capacity of copper conductors with PVC insulation, in
/// amperes, by cross-section in mm²: IEC 60204-1 (Table 6), installation
/// method B2 (cable in conduit or a wiring duct, the usual case in a
/// cabinet), 40 °C ambient. Smaller sections than listed scale down from
/// the smallest; larger ones are not checked.
const AMPACITY_B2: [(f64, f64); 10] = [
    (0.75, 7.6),
    (1.0, 9.6),
    (1.5, 12.2),
    (2.5, 16.5),
    (4.0, 23.0),
    (6.0, 29.0),
    (10.0, 40.0),
    (16.0, 53.0),
    (25.0, 67.0),
    (35.0, 83.0),
];

/// What a conductor of `mm2` carries (see [`AMPACITY_B2`]): the value for
/// the largest listed section not above it. `None` above the table.
fn ampacity(mm2: f64) -> Option<f64> {
    let (smallest, smallest_a) = AMPACITY_B2[0];
    let (largest, _) = AMPACITY_B2[AMPACITY_B2.len() - 1];
    if mm2.is_nan() || mm2 <= 0.0 || mm2 > largest + EPS {
        return None;
    }
    if mm2 < smallest - EPS {
        return Some(smallest_a * mm2 / smallest);
    }
    AMPACITY_B2
        .iter()
        .rev()
        .find(|(section, _)| *section <= mm2 + EPS)
        .map(|&(_, amps)| amps)
}

/// Whether a symbol is an overcurrent protective device, by its library key
/// (`iec/protection/...`): fuses, breakers (MCB, RCBO, motor protection) and
/// overload relays. Residual-current devices, surge arresters and
/// magnetic-only motor circuit protectors do not protect a wire against
/// overload.
fn protects_against_overload(symbol: &SymbolInstance) -> bool {
    let key = symbol.symbol.0.as_str();
    let Some(name) = key
        .rsplit('/')
        .next()
        .filter(|_| key.contains("/protection/"))
    else {
        return false;
    };
    [
        "fuse",
        "mcb",
        "rcbo",
        "motor_protection_breaker",
        "overload_relay",
    ]
    .iter()
    .any(|kind| name.starts_with(kind))
}

/// Wires that end on an overcurrent protective device and are thinner than
/// its rated current needs ([`Project::rated_current_a`]: the device's own
/// `rated_current_a`, else its part's). A wire's cross-section is its own,
/// else its cable core's; wires without one are not checked. One problem
/// per device, marking the device's symbols and every such wire.
pub(crate) fn undersized_wires(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let section = |wire: &novo_core::Wire| {
        wire.cross_section_mm2.or_else(|| {
            let (cable, core) = project.core_of_wire(wire.id)?;
            project.cable(cable)?.cores.get(core)?.cross_section_mm2
        })
    };
    type Found = (BTreeSet<SymbolId>, BTreeSet<novo_core::WireId>, Option<f64>);
    let mut by_device: BTreeMap<DeviceId, Found> = BTreeMap::new();
    for symbol in project.symbols().filter(|s| protects_against_overload(s)) {
        let SymbolRole::Device { device, .. } = symbol.role else {
            continue;
        };
        let Some(rated) = project.rated_current_a(device).filter(|a| *a > 0.0) else {
            continue;
        };
        for pin in 0..symbol.pins.len() {
            let Ok(pin) = u16::try_from(pin) else { break };
            for wire in project.wires_at(Endpoint::pin(symbol.id, pin)) {
                let Some(wire) = project.wire(wire) else {
                    continue;
                };
                let Some(mm2) = section(wire) else { continue };
                let Some(amps) = ampacity(mm2) else { continue };
                if amps + EPS < rated {
                    let entry = by_device.entry(device).or_default();
                    entry.0.insert(symbol.id);
                    entry.1.insert(wire.id);
                    entry.2 = Some(entry.2.map_or(mm2, |t| t.min(mm2)));
                }
            }
        }
    }
    for (device, (symbols, wires, thinnest)) in by_device {
        let Some(d) = project.device(device) else {
            continue;
        };
        let rated = project.rated_current_a(device).unwrap_or_default();
        let thinnest = thinnest.unwrap_or_default();
        let carries = ampacity(thinnest).unwrap_or_default();
        let mut at: Vec<AnyId> = symbols.into_iter().map(AnyId::from).collect();
        at.extend(wires.into_iter().map(AnyId::from));
        out.push(Problem::new(
            ProblemKind::UndersizedWire,
            at,
            format!(
                "{} is rated {} A, but a {} mm² wire on it carries about {} A",
                d.tag,
                number(rated),
                number(thinnest),
                number(carries)
            ),
        ));
    }
}

/// `16`, `0.75`, `12.2`: at most two decimals, no trailing zeros.
fn number(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

/// Terminals whose article does not suit their kind, read with the catalog's
/// PE marker (`PE` in the keywords or type number, `UT 2,5-PE`;
/// [`novo_parts::terminal_kind_of`]) from the article the strip editor, the
/// bill of materials and the missing-parts check all use
/// ([`novo_terminals::terminal_part`]). Articles without a type number are
/// not judged. One problem per strip and mismatch, naming the terminals.
pub(crate) fn wrong_terminal_blocks(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for strip in project
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip)
    {
        // (PE terminal on a plain block, plain terminal on a PE block)
        let mut wrong: [Vec<&novo_core::Terminal>; 2] = [Vec::new(), Vec::new()];
        for terminal in project.terminals_of_strip(strip.id) {
            let Some(part) =
                novo_terminals::terminal_part(project, terminal.id).and_then(|p| project.part(p))
            else {
                continue;
            };
            let catalog = novo_parts::CatalogPart::from_model(part);
            // A bare order number says nothing about the block: only judge
            // parts whose catalog data could carry the PE marker.
            if catalog.type_number.trim().is_empty() && catalog.keywords.is_empty() {
                continue;
            }
            let block_is_pe =
                novo_parts::terminal_kind_of(&catalog) == TerminalKind::ProtectiveEarth;
            let wants_pe = terminal.kind == TerminalKind::ProtectiveEarth;
            if wants_pe && !block_is_pe {
                wrong[0].push(terminal);
            } else if !wants_pe && block_is_pe {
                wrong[1].push(terminal);
            }
        }
        for (i, terminals) in wrong.into_iter().enumerate() {
            if terminals.is_empty() {
                continue;
            }
            let names: Vec<&str> = terminals.iter().map(|t| t.name.as_str()).collect();
            let one = names.len() == 1;
            let what = match (i, one) {
                (0, true) => "is a PE terminal on a block that is not a PE block",
                (0, false) => "are PE terminals on a block that is not a PE block",
                (_, true) => "is built with a PE block but is not a PE terminal",
                (_, false) => "are built with a PE block but are not PE terminals",
            };
            let mut at: Vec<AnyId> = terminals
                .iter()
                .flat_map(|t| project.symbols_of_terminal(t.id))
                .map(|s| AnyId::from(s.id))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            if at.is_empty() {
                at.extend(terminals.iter().map(|t| AnyId::from(t.id)));
            }
            out.push(Problem::new(
                ProblemKind::WrongTerminalBlock,
                at,
                format!("{}:{} {what}", strip.tag, names.join(", ")),
            ));
        }
    }
}
