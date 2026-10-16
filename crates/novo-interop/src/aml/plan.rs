//! Previewing and applying an AutomationML import.
//!
//! [`plan_import`] compares a [`Plant`] with the project and says what would
//! be created and what is already there; nothing changes until
//! [`AmlPlan::apply`], which creates everything as one undo step. An import
//! only adds: a device whose tag exists keeps what the project says.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    Cable, CableCore, ChangeSet, Document, MountingPlate, Part, PartId, Project, Terminal,
};

use super::read::Plant;
use crate::report::Report;
use crate::sheets::ApplyError;

/// What an import would do.
#[derive(Debug, Clone)]
pub struct AmlPlan {
    plant: Plant,
    new_structures: Vec<usize>,
    new_devices: Vec<usize>,
    new_cables: Vec<usize>,
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
    let new_devices = plant
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
    AmlPlan {
        plant: plant.clone(),
        new_structures,
        new_devices,
        new_cables,
        existing,
        revision: project.revision(),
    }
}

impl AmlPlan {
    /// How many records the import would create (terminals included).
    pub fn creates(&self) -> usize {
        self.new_structures.len()
            + self.new_cables.len()
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
        format!(
            "Would add {} devices ({} terminals), {} cables and {} sections or locations; {} already exist.",
            self.new_devices.len(),
            terminals,
            self.new_cables.len(),
            self.new_structures.len(),
            self.existing.len()
        )
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
            for &i in &self.new_devices {
                let d = &plant.devices[i];
                let id = tx.add_device_of_kind(d.tag.clone(), d.description.clone(), d.kind)?;
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
            Ok(created)
        })?;
        Ok(Some((created, changes)))
    }
}
