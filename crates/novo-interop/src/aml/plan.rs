//! Previewing and applying an AutomationML import.
//!
//! [`plan_import`] compares a [`Plant`] with the project and says what would
//! be created and what is already there; nothing changes until
//! [`AmlPlan::apply`], which creates everything as one undo step. An import
//! only adds: a device whose tag exists keeps what the project says.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    Cable, CableCore, ChangeSet, DeviceId, Document, MountingPlate, PageId, Part, PartId, Project,
    Terminal, TerminalId,
};

use super::draw::draw;
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
    let have_page: BTreeSet<String> = project.pages().map(|p| p.name.clone()).collect();
    let new_pages: Vec<usize> = plant
        .pages
        .iter()
        .enumerate()
        .filter(|(_, p)| !have_page.contains(&page_name(p)))
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
    AmlPlan {
        plant: plant.clone(),
        new_structures,
        new_devices,
        new_cables,
        new_pages,
        new_symbols,
        new_nets,
        existing,
        revision: project.revision(),
    }
}

fn new_devices_set(v: &[usize]) -> BTreeSet<usize> {
    v.iter().copied().collect()
}

/// The project page name for a page of the file: its label and title.
fn page_name(p: &PlantPage) -> String {
    if p.title.is_empty() {
        p.name.clone()
    } else {
        format!("{} {}", p.name, p.title)
    }
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
                .map(|p| (p.name.clone(), p.id))
                .collect();
            for (n, p) in plant.pages.iter().enumerate() {
                let name = page_name(p);
                let id = match existing.get(&name) {
                    Some(&id) => id,
                    None => {
                        created += 1;
                        let id = tx.add_page(name.clone())?;
                        existing.insert(name, id);
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
                    let id = match existing.get(&s.page) {
                        Some(&id) => id,
                        None => {
                            created += 1;
                            let id = tx.add_page(s.page.clone())?;
                            existing.insert(s.page.clone(), id);
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
