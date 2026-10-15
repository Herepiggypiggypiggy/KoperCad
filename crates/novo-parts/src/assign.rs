//! Device-to-part assignment, as commands.
//!
//! Everything here returns a [`Command`] instead of editing the project, so
//! the caller runs it through `Document::execute` and the whole assignment
//! (copying the catalog part into the project and assigning it to every
//! selected device) is one undo step. Scripts (WS-28) and the AI assistant
//! (WS-23) use the same functions.

use novo_core::{
    AnyId, Cable, CableCore, CableId, Command, Device, DeviceId, DeviceKind, Part, PartId, Project,
    Record, Terminal, TerminalId, TerminalKind, Value,
};

use crate::catalog::{Catalog, CatalogPart, PartKey};

/// How a part is assigned to a device that may already have parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssignMode {
    /// The part becomes the device's only part (the usual case: picking the
    /// contactor for `-K1`).
    #[default]
    Replace,
    /// The part is added next to the existing ones (an auxiliary contact
    /// block on a contactor).
    Add,
}

/// The project part equal to a catalog part (same manufacturer and order
/// number), if the project already has one.
pub fn find_project_part(project: &Project, key: &PartKey) -> Option<PartId> {
    project
        .parts()
        .find(|p| &PartKey::of_model(p) == key)
        .map(|p| p.id)
}

/// Allocates project parts for a batch of commands. Ids handed out by
/// `Insert` must be unused, so a batch that inserts several parts counts
/// them up from the project's next id.
pub(crate) struct PartAllocator<'a> {
    project: &'a Project,
    next: u64,
    added: Vec<(PartKey, PartId)>,
}

impl<'a> PartAllocator<'a> {
    pub(crate) fn new(project: &'a Project) -> Self {
        PartAllocator {
            project,
            next: project.next_id(),
            added: Vec::new(),
        }
    }

    /// The project part for `part`, inserting a copy (via `commands`) if the
    /// project does not have one yet.
    pub(crate) fn part_id(&mut self, part: &CatalogPart, commands: &mut Vec<Command>) -> PartId {
        let key = part.key();
        if let Some(id) = find_project_part(self.project, &key) {
            return id;
        }
        if let Some((_, id)) = self.added.iter().find(|(k, _)| *k == key) {
            return *id;
        }
        let id = PartId::from(self.next);
        self.next += 1;
        commands.push(Command::Insert {
            record: Record::Part(part.to_model(id)),
        });
        self.added.push((key, id));
        id
    }
}

/// Assigns a catalog part to devices as one command (one undo step).
///
/// The part is copied into the project once, or reused if the project
/// already has it. Returns `None` if there is nothing to change.
///
/// A terminal block ([`is_terminal_block`]) given to a terminal strip becomes the
/// strip's part for the terminal kind it suits ([`terminal_kind_of`]), so
/// every terminal of that kind is built with it and the bill of materials
/// orders one per terminal. It never lands in the strip device's own part
/// list, which is for end plates and accessories.
pub fn assign_part(
    project: &Project,
    devices: &[DeviceId],
    part: &CatalogPart,
    mode: AssignMode,
) -> Option<Command> {
    let mut commands = Vec::new();
    let mut parts = PartAllocator::new(project);
    let part_id = parts.part_id(part, &mut commands);
    let mut changed = 0;
    for &device in devices {
        let Some(current) = project.device(device) else {
            continue;
        };
        if current.kind == DeviceKind::TerminalStrip && is_terminal_block(part) {
            let kind = terminal_kind_of(part);
            if novo_terminals::strip_style(project, device, kind).part != Some(part_id) {
                commands.push(Command::SetProperty {
                    target: device.into(),
                    key: novo_terminals::strip_part_key(kind),
                    value: Some(Value::Int(
                        i64::try_from(part_id.raw()).expect("part ids fit in i64"),
                    )),
                });
                changed += 1;
            }
            continue;
        }
        let mut next = current.clone();
        match mode {
            AssignMode::Replace => next.parts = vec![part_id],
            AssignMode::Add => {
                if !next.parts.contains(&part_id) {
                    next.parts.push(part_id);
                }
            }
        }
        if next != *current {
            commands.push(Command::Replace {
                record: Record::Device(next),
            });
            changed += 1;
        }
    }
    if changed == 0 {
        return None;
    }
    Some(Command::Batch {
        label: format!("Assign {part} to {}", count(changed, "device")),
        commands,
    })
}

/// Whether a catalog part is a terminal block: letter code `X` and
/// connection points (`connections` in its technical data). End covers,
/// end brackets, markers and other strip accessories are coded `X` too but
/// connect nothing, so they stay in the strip device's own part list.
///
/// A catalog pasted without a `connections` column says nothing either
/// way; such a part is a terminal block unless its description or keywords
/// name an accessory ("end cover", "end bracket", "marker", ...) before any
/// "with" or "for", so "terminal block with marker groove" stays a block.
/// `connections` of 0 in any form (`0`, `0,0`, text "0") is an accessory.
pub fn is_terminal_block(part: &CatalogPart) -> bool {
    if !part.fits("X") {
        return false;
    }
    match part.data.get("connections") {
        Some(Value::Int(n)) => *n != 0,
        Some(Value::Float(f)) => *f != 0.0,
        Some(Value::Text(t)) if t.trim().is_empty() => !names_an_accessory(part),
        Some(Value::Text(t)) => crate::sheet::parse_number(t) != Some(0.0),
        Some(_) => true,
        None => !names_an_accessory(part),
    }
}

/// Words that mark a strip accessory rather than a terminal block.
const ACCESSORY_WORDS: &[&str] = &[
    "cover", "bracket", "plate", "stop", "marker", "marking", "bridge", "jumper",
];

fn names_an_accessory(part: &CatalogPart) -> bool {
    let head_names_one = |text: &str| {
        let text = text.to_lowercase();
        words(&text)
            .take_while(|w| !QUALIFIER_WORDS.contains(w))
            .any(|w| ACCESSORY_WORDS.contains(&w))
    };
    let head = part.description.split([',', ';', '(']).next().unwrap_or("");
    head_names_one(head) || part.keywords.iter().any(|k| head_names_one(k))
}

/// Words after which a description lists features rather than naming the
/// part: "terminal block with marker groove" is a block, "end cover for
/// UT 2,5" a cover.
const QUALIFIER_WORDS: &[&str] = &["with", "for", "mit", "für", "fur", "met", "voor"];

/// The terminal kind a terminal block is for: a protective conductor block
/// is for PE terminals, any other block for feed-through terminals.
///
/// A block is a PE block when its type number has `PE` as a word or a
/// one-letter prefix to it (`UT 2,5-PE`, Weidmüller `WPE 2.5`), or its
/// description or keywords name it (`PE`, protective) before any "with"
/// or "for", or say green-yellow or earth conductor (WAGO `2002-1207`, "gn/ye").
pub fn terminal_kind_of(part: &CatalogPart) -> TerminalKind {
    let type_number = part.type_number.to_lowercase();
    let pe_type = words(&type_number).any(|w| {
        w == "pe" || (w.len() == 3 && w.ends_with("pe") && w.chars().all(char::is_alphabetic))
    });
    // PE words name the block only in the description's head, before a
    // comma or "with"/"for" ("terminal block with protective cover" is not
    // a PE block); the colour counts anywhere.
    let says_pe = |text: &str| {
        let text = text.to_lowercase();
        let head = text.split([',', ';', '(']).next().unwrap_or("");
        words(head)
            .take_while(|w| !QUALIFIER_WORDS.contains(w))
            .any(|w| PE_WORDS.contains(&w))
            || PE_PHRASES.iter().any(|p| text.contains(p))
    };
    if pe_type || says_pe(&part.description) || part.keywords.iter().any(|k| says_pe(k)) {
        TerminalKind::ProtectiveEarth
    } else {
        TerminalKind::Feedthrough
    }
}

/// Words that name a protective conductor block (English, German, Dutch).
const PE_WORDS: &[&str] = &[
    "pe",
    "protective",
    "schutzleiter",
    "schutzleiterklemme",
    "aarde",
];

/// Phrases for green-yellow, the protective conductor colour.
const PE_PHRASES: &[&str] = &[
    "green-yellow",
    "green/yellow",
    "green yellow",
    "gn/ye",
    "gn-ye",
    "gnye",
    "grün-gelb",
    "grün/gelb",
    "groen-geel",
    "groen/geel",
    "earth conductor",
];

fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
}

/// Assigns a cable part to a cable as one command. The cable takes the
/// part's cores (`BN,BK,GY,GNYE`) and cross-section: existing cores are
/// renamed in order, keeping their wires, and missing cores are added as
/// spares. Cores the part does not have are kept, so no wire loses its
/// core; the design checks (WS-14) flag a cable with too many cores.
pub fn assign_cable_part(project: &Project, cable: CableId, part: &CatalogPart) -> Option<Command> {
    let current = project.cable(cable)?;
    let mut commands = Vec::new();
    let mut parts = PartAllocator::new(project);
    let part_id = parts.part_id(part, &mut commands);
    let mut next: Cable = current.clone();
    next.part = Some(part_id);
    let section = match part.data.get(crate::catalog::keys::CORE_CROSS_SECTION_MM2) {
        Some(novo_core::Value::Float(v)) => Some(*v),
        Some(novo_core::Value::Int(v)) => Some(*v as f64),
        _ => None,
    };
    let mut names = part.cores();
    let mut unmatched: Vec<usize> = Vec::new();
    // Cores whose name the part already has keep it (and their wire).
    for (i, core) in next.cores.iter().enumerate() {
        match names
            .iter()
            .position(|n| n.eq_ignore_ascii_case(&core.name))
        {
            Some(at) => {
                names.remove(at);
            }
            None => unmatched.push(i),
        }
    }
    // The protective conductor stays the protective conductor.
    unmatched.retain(|&i| {
        if !is_protective(&next.cores[i].name) {
            return true;
        }
        match names.iter().position(|n| is_protective(n)) {
            Some(at) => {
                next.cores[i].name = names.remove(at);
                false
            }
            None => true,
        }
    });
    // The rest take the remaining names in order; extra names are spares.
    let mut names = names.into_iter();
    for i in unmatched {
        if let Some(name) = names.next() {
            next.cores[i].name = name;
        }
    }
    next.cores.extend(names.map(CableCore::new));
    if let Some(section) = section {
        for core in &mut next.cores {
            core.cross_section_mm2 = Some(section);
        }
    }
    if next == *current {
        return None;
    }
    commands.push(Command::Replace {
        record: Record::Cable(next),
    });
    Some(Command::Batch {
        label: format!("Assign {part} to {}", current.tag),
        commands,
    })
}

/// Assigns a catalog part to terminals as their own article
/// (`Terminal.part`), as one command. The part is copied into the project
/// once, or reused. Returns `None` if there is nothing to change.
pub fn assign_terminal_part(
    project: &Project,
    terminals: &[TerminalId],
    part: &CatalogPart,
) -> Option<Command> {
    let mut commands = Vec::new();
    let mut parts = PartAllocator::new(project);
    let part_id = parts.part_id(part, &mut commands);
    let changed: Vec<Command> = terminals
        .iter()
        .filter_map(|&id| project.terminal(id))
        .filter(|t| t.part != Some(part_id))
        .map(|t| Command::Replace {
            record: Record::Terminal(Terminal {
                part: Some(part_id),
                ..t.clone()
            }),
        })
        .collect();
    if changed.is_empty() {
        return None;
    }
    let label = format!("Assign {part} to {}", count(changed.len(), "terminal"));
    commands.extend(changed);
    Some(Command::Batch { label, commands })
}

/// The article a terminal is built with: its own `Terminal.part`, else
/// its strip's style for its kind. This is [`novo_terminals::terminal_part`],
/// the one answer the strip editor, the bill of materials and
/// [`missing_parts`] share. The strip device's own parts (end plates,
/// accessories) are never a terminal's article.
pub fn terminal_part(project: &Project, terminal: TerminalId) -> Option<PartId> {
    novo_terminals::terminal_part(project, terminal)
}

/// Every terminal kind, for the strip styles.
const TERMINAL_KINDS: [TerminalKind; 5] = [
    TerminalKind::Feedthrough,
    TerminalKind::ProtectiveEarth,
    TerminalKind::Neutral,
    TerminalKind::Fuse,
    TerminalKind::Disconnect,
];

/// Fails to compile when a terminal kind is added, so it is added to
/// [`TERMINAL_KINDS`] too.
const fn _every_kind_listed(kind: TerminalKind) {
    match kind {
        TerminalKind::Feedthrough
        | TerminalKind::ProtectiveEarth
        | TerminalKind::Neutral
        | TerminalKind::Fuse
        | TerminalKind::Disconnect => {}
    }
}

/// Parts the terminal strip styles use (novo-terminals keeps them as
/// properties, which the project's references do not see).
fn style_parts(project: &Project) -> Vec<PartId> {
    let strips = project
        .devices()
        .filter(|d| d.kind == DeviceKind::TerminalStrip)
        .flat_map(|d| {
            TERMINAL_KINDS
                .iter()
                .filter_map(move |&k| novo_terminals::strip_style(project, d.id, k).part)
        });
    let terminals = project
        .terminals()
        .filter_map(|t| novo_terminals::style_of(project, t.id).and_then(|s| s.part));
    strips.chain(terminals).collect()
}

/// Removes parts from devices (all of them) as one command.
pub fn clear_parts(project: &Project, devices: &[DeviceId]) -> Option<Command> {
    let commands: Vec<Command> = devices
        .iter()
        .filter_map(|&id| project.device(id))
        .filter(|d| !d.parts.is_empty())
        .map(|d| Command::Replace {
            record: Record::Device(Device {
                parts: Vec::new(),
                ..d.clone()
            }),
        })
        .collect();
    if commands.is_empty() {
        return None;
    }
    Some(Command::Batch {
        label: format!("Remove part from {}", count(commands.len(), "device")),
        commands,
    })
}

/// Project parts nothing uses any more (no device, cable or terminal, and
/// no terminal strip style), oldest first.
pub fn unused_parts(project: &Project) -> Vec<PartId> {
    let styles = style_parts(project);
    project
        .parts()
        .filter(|p| project.referrers(p.id).next().is_none() && !styles.contains(&p.id))
        .map(|p| p.id)
        .collect()
}

/// A command that deletes the unused project parts, or `None`.
pub fn remove_unused_parts(project: &Project) -> Option<Command> {
    let commands: Vec<Command> = unused_parts(project)
        .into_iter()
        .map(|id| Command::Remove { id: id.into() })
        .collect();
    if commands.is_empty() {
        return None;
    }
    Some(Command::Batch {
        label: format!("Remove {}", count(commands.len(), "unused part")),
        commands,
    })
}

/// Devices, terminals and cables that have no part yet, in that order and
/// each in tag order. This is the WS-07 done criterion for the Gate 1
/// project ("every device carries a real part") and the input for the
/// design check "missing parts" (WS-14).
///
/// A terminal has a part when [`terminal_part`] finds one: its own or its
/// strip's style. A terminal strip device is not listed itself: its
/// terminals carry the articles.
pub fn missing_parts(project: &Project) -> Vec<AnyId> {
    // A terminal strip is covered through its terminals, listed below.
    let mut devices: Vec<&Device> = project
        .devices()
        .filter(|d| d.parts.is_empty() && d.kind != DeviceKind::TerminalStrip)
        .collect();
    devices.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    let mut terminals: Vec<(String, TerminalId)> = project
        .terminals()
        .filter(|t| terminal_part(project, t.id).is_none())
        .map(|t| {
            let name = project
                .terminal_designation(t.id)
                .unwrap_or_else(|| t.name.clone());
            (name, t.id)
        })
        .collect();
    terminals.sort_by(|a, b| novo_core::natural_cmp(&a.0, &b.0).then(a.1.cmp(&b.1)));
    let mut cables: Vec<&Cable> = project.cables().filter(|c| c.part.is_none()).collect();
    cables.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    devices
        .into_iter()
        .map(|d| AnyId::from(d.id))
        .chain(terminals.into_iter().map(|(_, t)| AnyId::from(t)))
        .chain(cables.into_iter().map(|c| AnyId::from(c.id)))
        .collect()
}

/// Catalog parts that suit a device, best first: parts for its letter code
/// (`K` for `-K1`), narrowed by `query` when given. A terminal strip gets
/// terminal blocks.
pub fn suggest_for_device<'c>(
    catalog: &'c Catalog,
    project: &Project,
    device: DeviceId,
    query: &str,
    limit: usize,
) -> Vec<&'c CatalogPart> {
    let Some(device) = project.device(device) else {
        return Vec::new();
    };
    let code = match device.kind {
        DeviceKind::TerminalStrip => "X",
        _ => device.tag.letter_code(),
    };
    let mut hits = catalog.suggest(code, query, limit);
    if hits.is_empty() && !query.trim().is_empty() {
        hits = catalog.search(query, limit);
    }
    hits
}

/// Terminal blocks for a terminal, best first: blocks for its kind (a PE
/// block for a PE terminal) before the others, each group in the catalog's
/// order, narrowed by `query` when given.
pub fn suggest_for_terminal<'c>(
    catalog: &'c Catalog,
    project: &Project,
    terminal: TerminalId,
    query: &str,
    limit: usize,
) -> Vec<&'c CatalogPart> {
    let Some(terminal) = project.terminal(terminal) else {
        return Vec::new();
    };
    let mut blocks: Vec<&CatalogPart> = catalog
        .suggest("X", query, usize::MAX)
        .into_iter()
        .filter(|p| is_terminal_block(p))
        .collect();
    // A stable sort keeps the catalog's ranking within each group.
    blocks.sort_by_key(|p| terminal_kind_of(p) != terminal.kind);
    blocks.truncate(limit);
    blocks
}

/// The project part's catalog entry (by manufacturer and order number).
pub fn catalog_entry<'c>(catalog: &'c Catalog, part: &Part) -> Option<&'c CatalogPart> {
    catalog.get(&PartKey::of_model(part))
}

pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// `PE`, `GNYE`, `GN/YE`, `GN-YE`: names of a protective conductor core.
fn is_protective(name: &str) -> bool {
    matches!(
        crate::search::normalize_code(name).as_str(),
        "pe" | "gnye" | "yegn" | "pen"
    )
}

/// Catalog parts that could be a device's part, for a proposal.
#[derive(Debug, Clone, PartialEq)]
pub struct PartProposal<'c> {
    pub device: DeviceId,
    /// Every fitting part, best first. One candidate means the answer is
    /// certain and [`auto_assign_parts`] takes it without asking.
    pub candidates: Vec<&'c CatalogPart>,
}

/// `rated_power_kw` and `power_kw` name the same quantity.
fn quantity(key: &str) -> &str {
    key.strip_prefix("rated_").unwrap_or(key)
}

fn number(value: &novo_core::Value) -> Option<f64> {
    match value {
        novo_core::Value::Int(i) => Some(*i as f64),
        novo_core::Value::Float(f) => Some(*f),
        _ => None,
    }
}

/// Whether a catalog part agrees with the device's own sizing values: an
/// equal value for the same quantity (`power_kw` 2.2), or a current inside
/// the part's setting range (`current_a` 8 within 7 to 10 A). A quantity
/// the part does not state does not rule it out.
fn sized_for(part: &CatalogPart, device: &Device) -> bool {
    device.properties.iter().all(|(key, value)| {
        let Some(wanted) = number(value) else {
            return true;
        };
        let q = quantity(key);
        let stated = part
            .data
            .iter()
            .find(|(k, _)| quantity(k) == q)
            .and_then(|(_, v)| number(v));
        if let Some(have) = stated {
            return (have - wanted).abs() <= 1e-9 * wanted.abs().max(1.0);
        }
        if q == "current_a" {
            let min = part.data.get("setting_min_a").and_then(number);
            let max = part.data.get("setting_max_a").and_then(number);
            if let (Some(min), Some(max)) = (min, max) {
                return (min..=max).contains(&wanted);
            }
        }
        true
    })
}

/// Part proposals for devices that have no part yet (all of them when
/// `devices` is `None`), in tag order. A device for which no catalog part
/// fits gets no proposal. Terminal strips get none: a terminal block is the
/// strip's style for a terminal kind, not a part of the strip device.
pub fn propose_parts<'c>(
    catalog: &'c Catalog,
    project: &Project,
    devices: Option<&[DeviceId]>,
) -> Vec<PartProposal<'c>> {
    let mut list: Vec<&Device> = match devices {
        Some(ids) => ids.iter().filter_map(|&id| project.device(id)).collect(),
        None => project.devices().collect(),
    };
    list.retain(|d| d.parts.is_empty() && d.kind != DeviceKind::TerminalStrip);
    list.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    list.into_iter()
        .filter_map(|device| {
            let code = device.tag.letter_code();
            if code.is_empty() {
                return None;
            }
            let candidates: Vec<&CatalogPart> = catalog
                .suggest(code, "", usize::MAX)
                .into_iter()
                .filter(|p| sized_for(p, device))
                .collect();
            (!candidates.is_empty()).then_some(PartProposal {
                device: device.id,
                candidates,
            })
        })
        .collect()
}

/// Assigns the part to every device without one where exactly one catalog
/// part fits (DESIGN.md §2: one correct answer happens on its own), as one
/// command. The rest stay proposals.
pub fn auto_assign_parts(catalog: &Catalog, project: &Project) -> Option<Command> {
    let mut commands = Vec::new();
    let mut parts = PartAllocator::new(project);
    let mut assigned = 0;
    for proposal in propose_parts(catalog, project, None) {
        let [part] = proposal.candidates[..] else {
            continue;
        };
        let Some(device) = project.device(proposal.device) else {
            continue;
        };
        let id = parts.part_id(part, &mut commands);
        commands.push(Command::Replace {
            record: Record::Device(Device {
                parts: vec![id],
                ..device.clone()
            }),
        });
        assigned += 1;
    }
    (assigned > 0).then(|| Command::Batch {
        label: format!("Assign parts to {}", count(assigned, "device")),
        commands,
    })
}

/// The Issues line for missing parts: `2 devices without a part`,
/// `1 device, 4 terminals and 1 cable without a part`, or `None` when all
/// have one.
pub fn missing_parts_summary(project: &Project) -> Option<String> {
    let missing = missing_parts(project);
    let of = |pick: fn(&AnyId) -> bool| missing.iter().filter(|id| pick(id)).count();
    let counts = [
        (of(|id| matches!(id, AnyId::Device(_))), "device"),
        (of(|id| matches!(id, AnyId::Terminal(_))), "terminal"),
        (of(|id| matches!(id, AnyId::Cable(_))), "cable"),
    ];
    let parts: Vec<String> = counts
        .iter()
        .filter(|(n, _)| *n > 0)
        .map(|&(n, noun)| count(n, noun))
        .collect();
    let what = match parts.as_slice() {
        [] => return None,
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    };
    Some(format!("{what} without a part"))
}
