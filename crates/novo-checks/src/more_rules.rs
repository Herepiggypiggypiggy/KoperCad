//! The last four portable rules of the EPLAN project check (findings 9.3:
//! C08, C12, C16, C18), written from the one-sentence rules. Each needs a
//! convention the model does not carry on its own, so each reads a device or
//! project property and stays quiet without it (C18 is information only).
//! The places and properties are listed in `docs/design/eplan-nets.md`.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, DeviceId, DeviceKind, Endpoint, Netlist, PotentialKind, SymbolInstance, SymbolRole,
    Value,
};

use crate::contacts::{ContactKind, contact_limit_property};
use crate::{Context, Problem, ProblemKind};

/// C08: no larger relay than needed. A device that declares the relay sizes
/// its family comes in (`relay.sizes`, text such as `2,4,8`) and the contacts
/// it has (`contacts.no`, `.nc`, `.co`) is judged by the auxiliary contacts
/// drawn: when a smaller size than the declared one would hold them, say so.
pub(crate) fn relay_size(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    for device in project.devices() {
        let Some(Value::Text(sizes)) = device.properties.get("relay.sizes") else {
            continue;
        };
        let mut sizes: Vec<i64> = sizes
            .split([',', ';', ' '])
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        sizes.sort_unstable();
        let declared: i64 = [
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
        .sum();
        let Some(symbols) = cx.by_device.get(&device.id) else {
            continue;
        };
        let used = symbols
            .iter()
            .filter(|s| {
                matches!(
                    ContactKind::of_key(&s.symbol),
                    ContactKind::NormallyOpen
                        | ContactKind::NormallyClosed
                        | ContactKind::Changeover
                ) && !matches!(s.role, SymbolRole::Device { main: true, .. })
            })
            .map(|s| s.pins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>())
            .collect::<BTreeSet<_>>()
            .len() as i64;
        if declared == 0 || used == 0 {
            continue;
        }
        if let Some(&fits) = sizes.iter().find(|&&s| s >= used)
            && fits < declared
        {
            out.push(Problem::new(
                ProblemKind::RelaySize,
                vec![device.id.into()],
                format!(
                    "{} has {declared} auxiliary contacts but only {used} are drawn; a relay with {fits} would do",
                    device.tag
                ),
            ));
        }
    }
}

/// What a coil pin is wired to.
#[derive(Clone, Copy, PartialEq)]
enum CoilEnd {
    Unwired,
    /// A pole of a multi-pole breaker, fuse or switch: a coil between two
    /// such poles is a 400 V coil, which is fine.
    Pole,
    /// A motor or power contacts: the coil is in the main circuit.
    Load,
    Other,
}

/// A contactor coil has one pole. Wired into the main circuit, between a
/// three-phase feed and a motor or power contacts, or alone on one pole of
/// a three-pole device with nothing on its other side, it takes one phase
/// and L2 and L3 run past it: the power contacts belong in the main circuit,
/// the coil in the control circuit. A coil between two poles of one feed
/// device is a 400 V coil and is left alone.
pub(crate) fn coil_on_power_feed(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let class = |o: &SymbolInstance| {
        let name = o.symbol.0.rsplit('/').next().unwrap_or("");
        if name.starts_with("motor_3ph") || o.symbol.0.contains("contactors/") {
            CoilEnd::Load
        } else if o.pass_through.len() >= 3 {
            CoilEnd::Pole
        } else {
            CoilEnd::Other
        }
    };
    let mut seen: BTreeSet<DeviceId> = BTreeSet::new();
    for coil in project
        .symbols()
        .filter(|s| s.symbol.0.ends_with("coils/contactor"))
    {
        let ends: Vec<CoilEnd> = (0..coil.pins.len())
            .filter_map(|pin| u16::try_from(pin).ok())
            .map(|pin| {
                let other = project
                    .wires_at(Endpoint::pin(coil.id, pin))
                    .filter_map(|w| project.wire(w))
                    .flat_map(|w| w.ends())
                    .find_map(|e| match e {
                        Endpoint::Pin { symbol, .. } if symbol != coil.id => {
                            project.symbol(symbol).map(&class)
                        }
                        _ => None,
                    });
                let wired = project
                    .wires_at(Endpoint::pin(coil.id, pin))
                    .next()
                    .is_some();
                other.unwrap_or(if wired {
                    CoilEnd::Other
                } else {
                    CoilEnd::Unwired
                })
            })
            .collect();
        let feeds = ends.contains(&CoilEnd::Load)
            || (ends.contains(&CoilEnd::Pole) && ends.contains(&CoilEnd::Unwired));
        let device = coil.device();
        if feeds && device.is_none_or(|d| seen.insert(d)) {
            let tag = device
                .and_then(|d| project.device(d))
                .map_or_else(|| "A coil".to_owned(), |d| d.tag.to_string());
            let mut at: Vec<AnyId> = vec![coil.id.into()];
            at.extend(device.map(AnyId::from));
            out.push(Problem::new(
                ProblemKind::CoilOnPowerFeed,
                at,
                format!(
                    "{tag} is a coil with one pole in a three-phase feed, so L2 and L3 run past it; \
                     use the 3-pole power contacts here (search `contactor`) and put the coil in the control circuit"
                ),
            ));
        }
    }
}

/// What a cable text such as `4G1.5` or `3x2.5` says.
#[derive(Debug, PartialEq)]
struct CableSpec {
    cores: usize,
    /// `G` (one core is the green-yellow protective conductor) or `x`.
    with_pe: bool,
    mm2: f64,
}

fn parse_spec(text: &str) -> Option<CableSpec> {
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len() {
        if !chars[i].is_ascii_digit()
            || (i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '.'))
        {
            continue;
        }
        let mut j = i;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        let cores: usize = chars[i..j].iter().collect::<String>().parse().ok()?;
        while j < chars.len() && chars[j] == ' ' {
            j += 1;
        }
        let Some(&sep) = chars.get(j) else { break };
        let with_pe = match sep {
            'G' | 'g' => true,
            'X' | 'x' | '×' => false,
            _ => continue,
        };
        j += 1;
        while j < chars.len() && chars[j] == ' ' {
            j += 1;
        }
        let start = j;
        while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == '.' || chars[j] == ',') {
            j += 1;
        }
        let mm2: f64 = chars[start..j]
            .iter()
            .map(|&c| if c == ',' { '.' } else { c })
            .collect::<String>()
            .parse()
            .ok()?;
        if cores > 0 && mm2 > 0.0 {
            return Some(CableSpec {
                cores,
                with_pe,
                mm2,
            });
        }
    }
    None
}

fn is_pe_core(name: &str) -> bool {
    let n = name
        .trim()
        .to_ascii_uppercase()
        .replace(['/', '-', ' '], "");
    matches!(n.as_str(), "PE" | "GNYE" | "GNGE" | "GEGN")
}

/// C12: core count, cross-section and use of a cable agree with its text:
/// `4G1.5` is four cores of 1.5 mm² with a protective core, `4x1.5` four
/// without. Only a cable whose description holds such a text is judged.
pub(crate) fn cable_text(cx: &Context<'_>, out: &mut Vec<Problem>) {
    for cable in cx.project.cables() {
        let part_text = cable
            .part
            .and_then(|p| cx.project.part(p))
            .map(|p| p.description.as_str())
            .unwrap_or("");
        let Some(spec) = parse_spec(&cable.description).or_else(|| parse_spec(part_text)) else {
            continue;
        };
        if cable.cores.is_empty() {
            continue;
        }
        let live: Vec<_> = cable.cores.iter().filter(|c| !c.shield).collect();
        let mut wrong = Vec::new();
        if live.len() != spec.cores {
            wrong.push(format!(
                "it has {} cores but the text says {}",
                live.len(),
                spec.cores
            ));
        }
        let off: Vec<&str> = live
            .iter()
            .filter(|c| {
                c.cross_section_mm2
                    .is_some_and(|s| (s - spec.mm2).abs() > 0.005)
            })
            .map(|c| c.name.as_str())
            .collect();
        if !off.is_empty() {
            wrong.push(format!(
                "core {} is not {} mm\u{b2}",
                off.join(", "),
                spec.mm2
            ));
        }
        let pe = live.iter().filter(|c| is_pe_core(&c.name)).count();
        if spec.with_pe && pe == 0 {
            wrong.push("the text has a protective core (G) but no core is green-yellow".into());
        } else if !spec.with_pe && pe > 0 {
            wrong.push("the text has no protective core (x) but a core is green-yellow".into());
        }
        if !wrong.is_empty() {
            out.push(Problem::new(
                ProblemKind::CableText,
                vec![cable.id.into()],
                format!("Cable {}: {}", cable.tag, wrong.join("; ")),
            ));
        }
    }
}

/// C16, opt-in with the project property `neutral.rule` =
/// `supply-and-external`: neutral goes only to supply and external devices.
/// A device symbol on a neutral net is reported unless its device is a
/// terminal strip or plug, or carries `external` or `supply` = true.
pub(crate) fn neutral_use(cx: &Context<'_>, netlist: &Netlist, out: &mut Vec<Problem>) {
    let project = cx.project;
    if !matches!(project.info().properties.get("neutral.rule"), Some(Value::Text(t)) if t == "supply-and-external")
    {
        return;
    }
    let mut seen: BTreeSet<DeviceId> = BTreeSet::new();
    for net in netlist.nets() {
        let neutral = net
            .potentials
            .iter()
            .chain(&net.carried)
            .filter_map(|&p| project.potential(p))
            .any(|p| matches!(p.kind, PotentialKind::Neutral | PotentialKind::Pen));
        if !neutral {
            continue;
        }
        for end in &net.endpoints {
            let Endpoint::Pin { symbol, .. } = end else {
                continue;
            };
            let Some(symbol) = project.symbol(*symbol) else {
                continue;
            };
            let Some(device) = symbol.device().and_then(|d| project.device(d)) else {
                continue;
            };
            let allowed = device.kind != DeviceKind::General
                || ["external", "supply"]
                    .iter()
                    .any(|k| matches!(device.properties.get(*k), Some(Value::Bool(true))));
            if !allowed && seen.insert(device.id) {
                out.push(Problem::new(
                    ProblemKind::NeutralUse,
                    vec![device.id.into(), symbol.id.into()],
                    format!(
                        "{} is on a neutral conductor but is neither supply nor an external device",
                        device.tag
                    ),
                ));
            }
        }
    }
}

/// C18: an overview of the pins of every plug, wired and free (information).
pub(crate) fn plug_overview(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let wired = super::house_rules::wired_pins(cx);
    for device in cx
        .project
        .devices()
        .filter(|d| d.kind == DeviceKind::Connector)
    {
        let Some(symbols) = cx.by_device.get(&device.id) else {
            continue;
        };
        let mut pins: BTreeMap<&str, bool> = BTreeMap::new();
        for s in symbols {
            for (i, p) in s.pins.iter().enumerate() {
                let w = u16::try_from(i).is_ok_and(|i| wired.contains(&(s.id, i)));
                *pins.entry(p.name.as_str()).or_insert(false) |= w;
            }
        }
        if pins.is_empty() {
            continue;
        }
        let list = |want: bool| -> String {
            pins.iter()
                .filter(|(_, w)| **w == want)
                .map(|(n, _)| *n)
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (used, free) = (list(true), list(false));
        let mut text = format!("Plug {}: {} pins", device.tag, pins.len());
        if !used.is_empty() {
            text.push_str(&format!("; wired {used}"));
        }
        if !free.is_empty() {
            text.push_str(&format!("; free {free}"));
        }
        let mut at: Vec<AnyId> = vec![device.id.into()];
        at.extend(symbols.iter().map(|s| AnyId::from(s.id)));
        out.push(Problem::new(ProblemKind::PlugPins, at, text));
    }
}
