//! Device-to-part assignment, as commands.
//!
//! Everything here returns a [`Command`] instead of editing the project, so
//! the caller runs it through `Document::execute` and the whole assignment
//! (copying the catalog part into the project and assigning it to every
//! selected device) is one undo step. Scripts (WS-28) and the AI assistant
//! (WS-23) use the same functions.

use novo_core::{
    AnyId, Cable, CableCore, CableId, Command, Device, DeviceId, DeviceKind, Part, PartId, Project,
    Record,
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

/// Project parts no device or cable uses any more, oldest first.
pub fn unused_parts(project: &Project) -> Vec<PartId> {
    project
        .parts()
        .filter(|p| project.referrers(p.id).next().is_none())
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

/// Devices and cables that have no part yet, in tag order. This is the
/// WS-07 done criterion for the Gate 1 project ("every device carries a
/// real part") and the input for the design check "missing parts" (WS-14).
pub fn missing_parts(project: &Project) -> Vec<AnyId> {
    let mut devices: Vec<&Device> = project.devices().filter(|d| d.parts.is_empty()).collect();
    devices.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    let mut cables: Vec<&Cable> = project.cables().filter(|c| c.part.is_none()).collect();
    cables.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    devices
        .into_iter()
        .map(|d| AnyId::from(d.id))
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
/// fits gets no proposal.
pub fn propose_parts<'c>(
    catalog: &'c Catalog,
    project: &Project,
    devices: Option<&[DeviceId]>,
) -> Vec<PartProposal<'c>> {
    let mut list: Vec<&Device> = match devices {
        Some(ids) => ids.iter().filter_map(|&id| project.device(id)).collect(),
        None => project.devices().collect(),
    };
    list.retain(|d| d.parts.is_empty());
    list.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    list.into_iter()
        .filter_map(|device| {
            let code = match device.kind {
                DeviceKind::TerminalStrip => "X",
                _ => device.tag.letter_code(),
            };
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
/// `1 device and 1 cable without a part`, or `None` when all have one.
pub fn missing_parts_summary(project: &Project) -> Option<String> {
    let missing = missing_parts(project);
    let devices = missing
        .iter()
        .filter(|id| matches!(id, AnyId::Device(_)))
        .count();
    let cables = missing.len() - devices;
    let what = match (devices, cables) {
        (0, 0) => return None,
        (d, 0) => count(d, "device"),
        (0, c) => count(c, "cable"),
        (d, c) => format!("{} and {}", count(d, "device"), count(c, "cable")),
    };
    Some(format!("{what} without a part"))
}
