//! What the import adds to editing: new devices from rows without an Id,
//! and parts from the Order number column.
//!
//! Both need project-wide ids (a new device, a new part), so they are
//! resolved when the command is built, with one counter, in the order the
//! commands run.

use std::collections::BTreeMap;

use novo_core::{
    Cable, CableId, Command, Device, DeviceId, DeviceKind, DeviceTag, Part, PartId, Project,
    Record, Terminal, TerminalId, Value,
};
use novo_parts::{Catalog, CatalogPart, normalize_code};

use super::fields::{Field, Slot, parse};
use super::grid::Cell;
use super::new_rows::{NewCable, NewCore, NewTerminal, StripRef};
use super::plan::ApplyError;

/// A part named in the sheet: its order number, the manufacturer when the
/// row gives one, and the catalog entry that matches, if any.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PartRef {
    pub order_number: String,
    pub manufacturer: String,
    pub catalog: Option<Box<CatalogPart>>,
}

impl PartRef {
    pub(crate) fn new(catalog: &Catalog, order_number: &str, manufacturer: &str) -> PartRef {
        let found = if manufacturer.is_empty() {
            catalog.find_order_number(order_number)
        } else {
            catalog.find(manufacturer, order_number)
        };
        PartRef {
            order_number: order_number.to_owned(),
            manufacturer: manufacturer.to_owned(),
            catalog: found.cloned().map(Box::new),
        }
    }
}

/// A device the sheet adds: a Devices row without an Id whose tag no device
/// has. Selected by default; the preview lets the user leave it out.
#[derive(Debug, Clone)]
pub struct NewDevice {
    /// The row in the sheet (CSV: the line).
    pub line: usize,
    pub tag: DeviceTag,
    pub description: String,
    pub kind: DeviceKind,
    /// The properties the row sets, by key.
    pub properties: Vec<(String, Value)>,
    pub selected: bool,
    pub(crate) part: Option<PartRef>,
}

impl NewDevice {
    /// The order number the row asks for, if any.
    pub fn order_number(&self) -> Option<&str> {
        self.part.as_ref().map(|p| p.order_number.as_str())
    }
}

/// Reads a Devices row that has no Id and a tag nobody has as a new
/// device. `None` when the row is not that (the caller reports it as it
/// always did); `Some(Err)` when it is meant as a new device but unusable.
pub(crate) fn new_device(
    project: &Project,
    catalog: &Catalog,
    columns: &[Option<Field>],
    cells: &[Cell],
    claimed: &[NewDevice],
) -> Option<Result<NewDevice, Vec<String>>> {
    let at = |slot: &Slot| {
        columns
            .iter()
            .position(|f| f.as_ref().is_some_and(|f| &f.slot == slot))
            .and_then(|i| cells.get(i))
            .map(|c| c.as_text().trim().to_owned())
            .unwrap_or_default()
    };
    let tag_text = at(&Slot::DeviceTag);
    let tag_text = tag_text.strip_prefix('\'').unwrap_or(&tag_text);
    let tag = DeviceTag::parse(tag_text).ok()?;
    if tag_text.is_empty() || !project.devices_by_tag(&tag).is_empty() {
        return None;
    }
    let mut errors = Vec::new();
    if tag.levels(novo_core::Aspect::Product).is_empty() {
        errors.push(format!("Tag: {tag} has no device part (-K1)"));
    }
    if tag.validate().is_err() {
        errors.push(format!("Tag: '{tag_text}' is not a valid tag"));
    }
    if let Some(first) = claimed.iter().find(|n| n.tag == tag) {
        errors.push(format!(
            "{tag} is already added on row {}; the first row counts",
            first.line
        ));
    }
    let kind = match normalize_code(&at(&Slot::DeviceKind)).as_str() {
        "" | "general" | "device" => DeviceKind::General,
        "terminalstrip" | "strip" => DeviceKind::TerminalStrip,
        "connector" | "plug" => DeviceKind::Connector,
        "plc" => DeviceKind::Plc,
        _ => {
            errors.push(format!(
                "Kind: '{}' is not a device kind",
                at(&Slot::DeviceKind)
            ));
            DeviceKind::General
        }
    };
    let mut properties = Vec::new();
    for (field, cell) in columns.iter().zip(cells) {
        let Some(field) = field else { continue };
        if let Slot::Property(key) = &field.slot {
            match parse(field, cell, None) {
                Ok(p) => {
                    if let Some(v) = p.value() {
                        properties.push((key.clone(), v));
                    }
                }
                Err(e) => errors.push(e),
            }
        }
    }
    let order = at(&Slot::DeviceOrderNumber);
    let part =
        (!order.is_empty()).then(|| PartRef::new(catalog, &order, &at(&Slot::DeviceManufacturer)));
    if !errors.is_empty() {
        return Some(Err(errors));
    }
    Some(Ok(NewDevice {
        line: 0,
        tag,
        description: at(&Slot::DeviceDescription),
        kind,
        properties,
        selected: true,
        part,
    }))
}

/// Hands out ids for the commands of one import, in the order they run.
pub(crate) struct Ids<'a> {
    project: &'a Project,
    next: u64,
    added: Vec<(PartRef, PartId)>,
    /// The devices this import adds, by tag.
    devices: Vec<(DeviceTag, DeviceId)>,
    /// The next terminal order per strip: after the last one there.
    next_order: BTreeMap<DeviceId, i64>,
    pub inserts: Vec<Command>,
}

impl<'a> Ids<'a> {
    pub(crate) fn new(project: &'a Project) -> Self {
        Ids {
            project,
            next: project.next_id(),
            added: Vec::new(),
            devices: Vec::new(),
            next_order: BTreeMap::new(),
            inserts: Vec::new(),
        }
    }

    pub(crate) fn device(&mut self) -> DeviceId {
        let id = DeviceId::from(self.next);
        self.next += 1;
        id
    }

    /// The project part for `part`: one the project already has, else a
    /// new one (from the catalog entry, or just the order number).
    pub(crate) fn part(&mut self, part: &PartRef) -> PartId {
        let wanted = normalize_code(&part.order_number);
        let manufacturer = normalize_code(&part.manufacturer);
        if let Some(p) = self.project.parts().find(|p| {
            normalize_code(&p.order_number) == wanted
                && (manufacturer.is_empty() || normalize_code(&p.manufacturer) == manufacturer)
        }) {
            return p.id;
        }
        if let Some((_, id)) = self.added.iter().find(|(r, _)| {
            normalize_code(&r.order_number) == wanted
                && normalize_code(&r.manufacturer) == manufacturer
        }) {
            return *id;
        }
        let id = PartId::from(self.next);
        self.next += 1;
        let model: Part = match &part.catalog {
            Some(c) => c.to_model(id),
            None => CatalogPart::new(part.manufacturer.as_str(), part.order_number.as_str())
                .to_model(id),
        };
        self.inserts.push(Command::Insert {
            record: Record::Part(model),
        });
        self.added.push((part.clone(), id));
        id
    }

    /// The command that adds `new`.
    pub(crate) fn insert_device(&mut self, new: &NewDevice) -> Command {
        let parts = new.part.iter().map(|p| self.part(p)).collect();
        let mut properties = novo_core::Properties::default();
        for (k, v) in &new.properties {
            properties.insert(k.clone(), v.clone());
        }
        let id = self.device();
        self.devices.push((new.tag.clone(), id));
        Command::Insert {
            record: Record::Device(Device {
                id,
                tag: new.tag.clone(),
                description: new.description.clone(),
                kind: new.kind,
                parts,
                properties,
            }),
        }
    }
}

impl Ids<'_> {
    /// The command that adds `new` with its `cores` (in core order).
    pub(crate) fn insert_cable(&mut self, new: &NewCable, cores: &[&NewCore]) -> Command {
        let id = CableId::from(self.next);
        self.next += 1;
        let mut properties = novo_core::Properties::default();
        for (k, v) in &new.properties {
            properties.insert(k.clone(), v.clone());
        }
        let mut cable = Cable {
            id,
            tag: new.tag.clone(),
            description: new.description.clone(),
            part: None,
            cores: Vec::new(),
            length_m: new.length_m,
            properties,
        };
        for core in cores {
            super::new_rows::add_core(&mut cable, core);
        }
        Command::Insert {
            record: Record::Cable(cable),
        }
    }

    /// The command that adds `new`. A terminal without an order goes after
    /// the last one of its strip.
    pub(crate) fn insert_terminal(&mut self, new: &NewTerminal) -> Result<Command, ApplyError> {
        let strip = match &new.strip {
            StripRef::Existing(id) => *id,
            StripRef::New(tag) => self
                .devices
                .iter()
                .find(|(t, _)| t == tag)
                .map(|(_, id)| *id)
                .ok_or(ApplyError::Outdated)?,
        };
        let project = self.project;
        let next = self.next_order.entry(strip).or_insert_with(|| {
            project
                .terminals_of_strip(strip)
                .iter()
                .map(|t| t.order + 1)
                .max()
                .unwrap_or(0)
        });
        let order = new.order.unwrap_or(*next);
        *next = (*next).max(order + 1);
        let id = TerminalId::from(self.next);
        self.next += 1;
        let mut properties = novo_core::Properties::default();
        for (k, v) in &new.properties {
            properties.insert(k.clone(), v.clone());
        }
        Ok(Command::Insert {
            record: Record::Terminal(Terminal {
                id,
                strip,
                name: new.name.clone(),
                kind: new.kind,
                level: new.level,
                order,
                jumper: new.jumper,
                part: None,
                feed: None,
                pe_override: Default::default(),
                properties,
            }),
        })
    }
}
