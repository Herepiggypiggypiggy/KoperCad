//! Previewing and applying an AutomationML import.
//!
//! [`plan_import`] compares a [`Plant`] with the project and says what would
//! be created and what is already there; nothing changes until
//! [`AmlPlan::apply`], which creates everything as one undo step. An import
//! only adds: a device whose tag exists keeps what the project says.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    Cable, CableCore, ChangeSet, DeviceId, DeviceTag, Document, MountingPlate, PageId, Part,
    PartId, Project, Terminal, TerminalId, Value,
};

use super::draw::{draw, is_box};
use super::read::{Plant, PlantPage};
use crate::report::Report;
use crate::sheets::ApplyError;

/// What an import would do.
#[derive(Debug, Clone)]
pub struct AmlPlan {
    plant: Plant,
    new_structures: Vec<usize>,
    new_devices: Vec<usize>,
    new_cables: Vec<usize>,
    /// Pages of the file's drawing profile that the project does not have.
    new_pages: Vec<usize>,
    /// Drawn symbols that would be placed (those of new devices).
    new_symbols: usize,
    /// Nets that would get at least one wire (two or more pins on symbols
    /// that would be placed).
    new_nets: usize,
    /// Symbols that would be drawn as boxes, counted by kind.
    box_kinds: BTreeMap<String, usize>,
    /// New devices whose footprint neither the file nor the project's
    /// catalog gives.
    no_footprint: usize,
    /// Tags the file has that the project already holds.
    pub existing: Vec<String>,
    revision: u64,
}

/// Compares `plant` with `project`.
pub fn plan_import(project: &Project, plant: &Plant) -> AmlPlan {
    let have_structure: BTreeSet<String> =
        project.structures().map(|s| s.tag.to_string()).collect();
    let have_device: BTreeSet<String> = project
        .devices()
        .map(|d| d.tag.to_string())
        .chain(project.cables().map(|c| c.tag.to_string()))
        .collect();
    let mut existing = Vec::new();
    let new_structures = plant
        .structures
        .iter()
        .enumerate()
        .filter(|(_, s)| !have_structure.contains(&s.tag.to_string()))
        .map(|(i, _)| i)
        .collect();
    let new_devices: Vec<usize> = plant
        .devices
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            let tag = d.tag.to_string();
            let known = have_device.contains(&tag);
            if known {
                existing.push(tag);
            }
            !known
        })
        .map(|(i, _)| i)
        .collect();
    let new_cables = plant
        .cables
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let tag = c.tag.to_string();
            let known = have_device.contains(&tag);
            if known {
                existing.push(tag);
            }
            !known
        })
        .map(|(i, _)| i)
        .collect();
    let have_page: BTreeSet<String> = project
        .pages()
        .map(|p| page_key(&p.structure, &p.name))
        .collect();
    let new_pages: Vec<usize> = plant
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| {
            let (structure, name) = page_parts(p);
            !have_page.contains(&page_key(&structure, &name))
        })
        .map(|(i, _)| i)
        .collect();
    let new_set: BTreeSet<usize> = new_devices_set(&new_devices);
    let new_page_names: BTreeSet<&str> = new_pages
        .iter()
        .map(|&i| plant.pages[i].name.as_str())
        .collect();
    let new_symbols = plant
        .symbols
        .iter()
        .filter(|s| match s.device {
            Some(d) => new_set.contains(&d),
            None => new_page_names.contains(s.page.as_str()),
        })
        .count();
    let placed = |s: usize| {
        plant.symbols.get(s).is_some_and(|s| match s.device {
            Some(d) => new_set.contains(&d),
            None => new_page_names.contains(s.page.as_str()),
        })
    };
    let new_nets = plant
        .nets
        .iter()
        .filter(|n| n.pins.iter().filter(|(s, _)| placed(*s)).count() >= 2)
        .count();
    let mut box_kinds: BTreeMap<String, usize> = BTreeMap::new();
    for s in plant.symbols.iter().filter(|s| match s.device {
        Some(d) => new_set.contains(&d),
        None => new_page_names.contains(s.page.as_str()),
    }) {
        if !s.pins.is_empty() && is_box(s) {
            *box_kinds
                .entry(format!("{} ({} pins)", s.kind, s.pins.len()))
                .or_default() += 1;
        }
    }
    let catalog_size: BTreeSet<(String, String)> = project
        .parts()
        .filter(|p| {
            ["width_mm", "height_mm"]
                .iter()
                .all(|k| p.properties.contains_key(*k))
        })
        .map(|p| (p.manufacturer.clone(), p.order_number.clone()))
        .collect();
    let no_footprint = new_devices
        .iter()
        .filter(|&&i| {
            let d = &plant.devices[i];
            let [w, h, _] = d.size_mm;
            !(w.is_some() && h.is_some())
                && !catalog_size.contains(&(d.manufacturer.clone(), d.order_number.clone()))
        })
        .count();
    AmlPlan {
        plant: plant.clone(),
        new_structures,
        new_devices,
        new_cables,
        new_pages,
        new_symbols,
        new_nets,
        box_kinds,
        no_footprint,
        existing,
        revision: project.revision(),
    }
}

fn new_devices_set(v: &[usize]) -> BTreeSet<usize> {
    v.iter().copied().collect()
}

/// A page of the file as the project names it: the structure the label
/// starts with (`=A1+CAB1`) and the page name after the last `/` (`05`), so
/// a cross reference reads `/05.5` on its own structure and `=A1+CAB1/05.5`
/// from another. A label that is not of that shape stays whole, structure
/// empty. The title is the page title, never part of the name.
fn page_parts(p: &PlantPage) -> (DeviceTag, String) {
    if let Some((head, tail)) = p.name.rsplit_once('/')
        && !tail.trim().is_empty()
        && head.starts_with(['=', '+'])
        && let Ok(structure) = DeviceTag::parse(head)
        && structure.product.is_empty()
    {
        return (structure, tail.trim().to_string());
    }
    (DeviceTag::default(), p.name.clone())
}

/// What identifies a project page across imports: structure and name.
fn page_key(structure: &DeviceTag, name: &str) -> String {
    format!("{structure}/{name}")
}

impl AmlPlan {
    /// How many records the import would create (terminals included).
    pub fn creates(&self) -> usize {
        self.new_structures.len()
            + self.new_cables.len()
            + self.new_pages.len()
            + self.new_symbols
            + self
                .new_devices
                .iter()
                .map(|&i| 1 + self.plant.devices[i].terminals.len())
                .sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.creates() == 0
    }

    /// The report of the read, plus what already exists.
    pub fn report(&self) -> Report {
        let mut r = self.plant.report.clone();
        r.notes.retain(|n| !n.starts_with("Already"));
        let boxes: usize = self.box_kinds.values().sum();
        if boxes > 0 {
            let kinds: Vec<String> = self
                .box_kinds
                .iter()
                .map(|(k, n)| format!("{k} x{n}"))
                .collect();
            r.guess(
                "symbols drawn as labelled boxes",
                boxes,
                format!(
                    "no library symbol of that kind and pin count fits ({}); the pins are where the file has them",
                    kinds.join(", ")
                ),
            );
        }
        if self.new_nets > 0 && self.plant.segments.is_empty() {
            r.guess(
                "wire routes",
                self.new_nets,
                "the file lists the nets but not how the wires run, so they are drawn as plain right-angle runs between the pins",
            );
        }
        r.guess(
            "devices without a footprint",
            self.no_footprint,
            "neither the file nor the catalog gives a size, so the cabinet view uses its default footprint until a parts import fills it in",
        );
        if !self.existing.is_empty() {
            r.note(format!(
                "Already in the project and left as they are: {}.",
                self.existing.join(", ")
            ));
        }
        r
    }

    pub fn summary(&self) -> String {
        let p = &self.plant;
        let terminals: usize = self
            .new_devices
            .iter()
            .map(|&i| p.devices[i].terminals.len())
            .sum();
        let mut text = format!(
            "Would add {} devices ({} terminals), {} cables and {} sections or locations; {} already exist.",
            self.new_devices.len(),
            terminals,
            self.new_cables.len(),
            self.new_structures.len(),
            self.existing.len()
        );
        if self.new_symbols > 0 {
            text.push_str(&format!(
                " Would draw {} symbols on {} new pages and wire {} nets (plus interruption points for nets that cross pages).",
                self.new_symbols,
                self.new_pages.len(),
                self.new_nets
            ));
        }
        text
    }

    /// Creates everything as one undo step; returns how many records.
    pub fn apply(&self, doc: &mut Document) -> Result<usize, ApplyError> {
        Ok(self.apply_with_changes(doc)?.map_or(0, |(n, _)| n))
    }

    /// Like [`apply`](Self::apply), and hands back the changes so a caller
    /// that shows the project (the app) can refresh what it draws. `None`
    /// when there was nothing to add.
    pub fn apply_with_changes(
        &self,
        doc: &mut Document,
    ) -> Result<Option<(usize, ChangeSet)>, ApplyError> {
        if doc.project().revision() != self.revision {
            return Err(ApplyError::Outdated);
        }
        if self.is_empty() {
            return Ok(None);
        }
        let plant = &self.plant;
        let (created, changes) = doc.transact("Import AutomationML", |tx| {
            let mut created = 0usize;
            for &i in &self.new_structures {
                let s = &plant.structures[i];
                let id = tx.add_structure(s.tag.clone(), s.description.clone(), s.enclosure)?;
                if let Some((width, height)) = s.plate {
                    tx.set_plate(
                        id,
                        Some(MountingPlate {
                            width,
                            height,
                            margin: 0.0,
                        }),
                    )?;
                }
                created += 1;
            }
            let mut parts: BTreeMap<(String, String), PartId> = tx
                .project()
                .parts()
                .map(|p| ((p.manufacturer.clone(), p.order_number.clone()), p.id))
                .collect();
            let mut device_ids: BTreeMap<usize, DeviceId> = BTreeMap::new();
            let mut terminal_ids: BTreeMap<(usize, String), TerminalId> = BTreeMap::new();
            for &i in &self.new_devices {
                let d = &plant.devices[i];
                let id = tx.add_device_of_kind(d.tag.clone(), d.description.clone(), d.kind)?;
                device_ids.insert(i, id);
                created += 1;
                if !d.order_number.is_empty() {
                    let key = (d.manufacturer.clone(), d.order_number.clone());
                    let part = match parts.get(&key) {
                        Some(p) => *p,
                        None => {
                            let p = tx.add_part(&d.manufacturer, &d.order_number)?;
                            if !d.type_number.is_empty() {
                                tx.update::<Part>(p, |part| {
                                    part.type_number = d.type_number.clone()
                                })?;
                            }
                            parts.insert(key, p);
                            p
                        }
                    };
                    // The file's size for the part, unless the catalog already
                    // has one (a user's own data wins over the file's).
                    let [w, h, depth] = d.size_mm;
                    if w.is_some() || h.is_some() || depth.is_some() {
                        tx.update::<Part>(part, |p| {
                            for (key, v) in [("width_mm", w), ("height_mm", h), ("depth_mm", depth)]
                            {
                                if let Some(v) = v {
                                    p.properties
                                        .entry(key.to_string())
                                        .or_insert(Value::Float((v * 100.0).round() / 100.0));
                                }
                            }
                        })?;
                    }
                    tx.assign_part(id, part)?;
                }
                for (k, v) in &d.properties {
                    tx.set_property(id, k.clone(), Some(v.clone()))?;
                }
                for t in &d.terminals {
                    let tid = tx.add_terminal(id, t.name.clone())?;
                    tx.update::<Terminal>(tid, |x| {
                        x.level = t.level;
                        x.jumper = t.jumper;
                    })?;
                    terminal_ids.insert((i, t.name.clone()), tid);
                    created += 1;
                }
            }
            for &i in &self.new_cables {
                let c = &plant.cables[i];
                let cores = c
                    .cores
                    .iter()
                    .map(|k| CableCore {
                        cross_section_mm2: k.cross_section_mm2,
                        shield: k.shield,
                        ..CableCore::new(k.name.clone())
                    })
                    .collect();
                let id = tx.add_cable(c.tag.clone(), cores)?;
                tx.update::<Cable>(id, |x| {
                    x.description = c.description.clone();
                    x.length_m = c.length_m;
                })?;
                created += 1;
            }
            // Pages, then what is drawn on them.
            let mut pages: BTreeMap<String, (usize, PageId)> = BTreeMap::new();
            let mut existing: BTreeMap<String, PageId> = tx
                .project()
                .pages()
                .map(|p| (page_key(&p.structure, &p.name), p.id))
                .collect();
            for (n, p) in plant.pages.iter().enumerate() {
                let (structure, name) = page_parts(p);
                let key = page_key(&structure, &name);
                let id = match existing.get(&key) {
                    Some(&id) => id,
                    None => {
                        created += 1;
                        let id = tx.add_page(name)?;
                        let title = p.title.clone();
                        tx.update::<novo_core::Page>(id, |x| {
                            x.structure = structure;
                            x.title = title;
                        })?;
                        existing.insert(key, id);
                        id
                    }
                };
                pages.insert(p.name.clone(), (n, id));
            }
            // A symbol may name a page the file does not describe.
            let mut extra = plant.pages.len();
            for s in plant
                .symbols
                .iter()
                .filter(|s| s.device.is_none_or(|d| device_ids.contains_key(&d)))
            {
                if !pages.contains_key(&s.page) {
                    let id = match existing.get(&page_key(&DeviceTag::default(), &s.page)) {
                        Some(&id) => id,
                        None => {
                            created += 1;
                            let id = tx.add_page(s.page.clone())?;
                            existing.insert(page_key(&DeviceTag::default(), &s.page), id);
                            id
                        }
                    };
                    pages.insert(s.page.clone(), (extra, id));
                    extra += 1;
                }
            }
            let new_page_names: BTreeSet<String> = self
                .new_pages
                .iter()
                .map(|&i| plant.pages[i].name.clone())
                .collect();
            created += draw(
                tx,
                plant,
                &device_ids,
                &terminal_ids,
                &pages,
                &new_page_names,
            )?
            .records;
            Ok(created)
        })?;
        Ok(Some((created, changes)))
    }
}
