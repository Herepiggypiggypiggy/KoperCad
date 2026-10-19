//! Previewing and applying a PXF import.
//!
//! [`plan_pxf`] compares a [`PxfFile`] with the project and says what would
//! be created, what is already there and what is not carried; nothing changes
//! until [`PxfPlan::apply`], which creates everything as one undo step. An
//! import only adds: a device whose tag exists keeps what the project says.
//!
//! What is created: plant sections (`=A1`) and locations (`+CAB1`), one empty
//! page per EPLAN page (named by its label and title), one device per tag
//! (EPLAN draws a device many times; KoperCAD has one device with many
//! drawings), terminal strips with their terminals, cables with their cores,
//! and the parts behind part numbers. Symbols, wires and texts are not
//! carried yet and are counted in the report.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    Cable, CableCore, ChangeSet, DeviceKind, DeviceTag, Document, Part, PartId, Project,
};

use super::model::*;
use super::text::MlText;
use crate::report::Report;
use crate::sheets::ApplyError;

/// Choices for reading the file.
#[derive(Debug, Clone)]
pub struct PxfOptions {
    /// Language of titles and type texts (`en_US`); another language is used
    /// when this one is missing.
    pub lang: String,
}

impl Default for PxfOptions {
    fn default() -> Self {
        Self {
            lang: "en_US".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NewStructure {
    tag: DeviceTag,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NewDevice {
    tag: DeviceTag,
    description: String,
    kind: DeviceKind,
    manufacturer: String,
    order_number: String,
    type_number: String,
    /// Terminal names, for a strip.
    terminals: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NewCable {
    tag: DeviceTag,
    description: String,
    cores: Vec<String>,
}

/// What an import would do.
#[derive(Debug, Clone)]
pub struct PxfPlan {
    structures: Vec<NewStructure>,
    pages: Vec<String>,
    devices: Vec<NewDevice>,
    cables: Vec<NewCable>,
    /// Tags the file has that the project already holds.
    pub existing: Vec<String>,
    /// Name and description for a project that is still empty.
    project_info: Option<(String, String)>,
    report: Report,
    revision: u64,
}

fn parse_tag(s: &str) -> Option<DeviceTag> {
    DeviceTag::parse(s).ok().filter(|t| !t.product.is_empty())
}

/// A tag written the way [`DeviceTag`] writes it, so a cable and its cores
/// meet under one spelling.
fn canonical(full: &str) -> String {
    parse_tag(full).map_or_else(|| full.to_string(), |t| t.to_string())
}

/// Compares `file` with `project`.
pub fn plan_pxf(project: &Project, file: &PxfFile, opts: &PxfOptions) -> PxfPlan {
    let have_structure: BTreeSet<String> =
        project.structures().map(|s| s.tag.to_string()).collect();
    let have_device: BTreeSet<String> = project
        .devices()
        .map(|d| d.tag.to_string())
        .chain(project.cables().map(|c| c.tag.to_string()))
        .collect();
    let have_page: BTreeSet<String> = project.pages().map(|p| p.name.clone()).collect();

    let mut existing = Vec::new();
    let mut unreadable = 0usize;

    // Sections and locations, from the pages and from every tag.
    let mut wanted: BTreeSet<String> = BTreeSet::new();
    let mut note_structure = |function: &str, location: &str| {
        let f = function.trim_start_matches('=');
        let l = location.trim_start_matches('+');
        if !f.is_empty() {
            wanted.insert(format!("={f}"));
        }
        if !l.is_empty() {
            wanted.insert(format!("+{l}"));
        }
    };
    for p in &file.pages {
        note_structure(&p.function, &p.location);
    }
    for d in &file.devices {
        note_structure(&d.tag.function, &d.tag.location);
    }
    let structures: Vec<NewStructure> = wanted
        .into_iter()
        .filter(|s| !have_structure.contains(s))
        .filter_map(|s| DeviceTag::parse(&s).ok())
        .map(|tag| NewStructure { tag })
        .collect();

    // Pages: one empty page per EPLAN page.
    let mut pages = Vec::new();
    let mut page_names = BTreeSet::new();
    let mut pages_existing = 0usize;
    let mut report_pages = 0usize;
    for p in &file.pages {
        if p.class() == PageClass::Report {
            report_pages += 1;
            continue;
        }
        let title = p.title.get(&opts.lang);
        let name = if title.is_empty() {
            p.label()
        } else {
            format!("{} {title}", p.label())
        };
        if have_page.contains(&name) || !page_names.insert(name.clone()) {
            pages_existing += 1;
        } else {
            pages.push(name);
        }
    }

    // One logical device per tag, in file order.
    let mut devices: Vec<NewDevice> = Vec::new();
    let mut strip_at: BTreeMap<String, usize> = BTreeMap::new();
    let mut device_at: BTreeMap<String, usize> = BTreeMap::new();
    let mut cable_at: BTreeMap<String, usize> = BTreeMap::new();
    let mut cable_of_id: BTreeMap<ObjId, usize> = BTreeMap::new();
    let mut cables: Vec<NewCable> = Vec::new();
    let mut seen_existing: BTreeSet<String> = BTreeSet::new();
    let mut placements = 0usize;
    let mut repeats = 0usize;

    for d in &file.devices {
        let full = d.tag.full();
        let Some(tag) = parse_tag(&full) else {
            unreadable += 1;
            continue;
        };
        let text = tag.to_string();
        if have_device.contains(&text) {
            if seen_existing.insert(text.clone()) {
                existing.push(text);
            }
            continue;
        }
        placements += 1;
        match d.role {
            DeviceRole::Terminal => {
                let idx = *strip_at.entry(text.clone()).or_insert_with(|| {
                    let (manufacturer, type_number) = split_part_number(&d.part_number);
                    devices.push(NewDevice {
                        tag,
                        description: String::new(),
                        kind: DeviceKind::TerminalStrip,
                        manufacturer: manufacturer.to_string(),
                        order_number: String::new(),
                        type_number: type_number.to_string(),
                        terminals: Vec::new(),
                    });
                    devices.len() - 1
                });
                let name = d.terminal_text.trim().to_string();
                if !name.is_empty() && !devices[idx].terminals.contains(&name) {
                    devices[idx].terminals.push(name);
                }
            }
            DeviceRole::Cable => {
                if let Entry::Vacant(slot) = cable_at.entry(text) {
                    slot.insert(cables.len());
                    if let Some(id) = d.id {
                        cable_of_id.insert(id, cables.len());
                    }
                    cables.push(NewCable {
                        tag,
                        description: MlText::parse(&d.cable_text).get(&opts.lang).to_string(),
                        cores: Vec::new(),
                    });
                } else {
                    repeats += 1;
                }
            }
            DeviceRole::Device => {
                if let Entry::Vacant(slot) = device_at.entry(text) {
                    slot.insert(devices.len());
                    let (manufacturer, type_number) = split_part_number(&d.part_number);
                    devices.push(NewDevice {
                        tag,
                        description: d.type_text.get(&opts.lang).to_string(),
                        kind: DeviceKind::General,
                        manufacturer: manufacturer.to_string(),
                        order_number: if d.part_number.is_empty() {
                            String::new()
                        } else {
                            d.part_number.clone()
                        },
                        type_number: type_number.to_string(),
                        terminals: Vec::new(),
                    });
                } else {
                    repeats += 1;
                }
            }
        }
    }

    // Cores go to their cable by tag, in core-number order.
    let mut orphan_cores = 0usize;
    let mut cores_of_existing = 0usize;
    let mut core_numbers: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    let mut cores_by_id = 0usize;
    for c in &file.cores {
        // The core's own reference to its cable beats the tag, which a core
        // may carry without the function level.
        let by_id = c.cable_ref.and_then(|r| cable_of_id.get(&r)).copied();
        let found = by_id.or_else(|| cable_at.get(&canonical(&c.cable.full())).copied());
        if by_id.is_some() {
            cores_by_id += 1;
        }
        match found {
            Some(i) => {
                let n = c.number.trim().to_string();
                let list = core_numbers.entry(i).or_default();
                if !n.is_empty() && !list.contains(&n) {
                    list.push(n);
                }
            }
            None if have_device.contains(&canonical(&c.cable.full())) => cores_of_existing += 1,
            None => orphan_cores += 1,
        }
    }
    for (i, mut list) in core_numbers {
        list.sort_by(|a, b| match (a.parse::<u64>(), b.parse::<u64>()) {
            (Ok(x), Ok(y)) => x.cmp(&y),
            _ => a.cmp(b),
        });
        cables[i].cores = list;
    }

    // The report: what this plan carries, then what the format part leaves.
    let mut report = Report::default();
    let terminals: usize = devices.iter().map(|d| d.terminals.len()).sum();
    let strips = devices
        .iter()
        .filter(|d| d.kind == DeviceKind::TerminalStrip)
        .count();
    report.moved("sections and locations", structures.len());
    report.moved("pages (empty)", pages.len());
    report.lost(
        "generated report pages",
        report_pages,
        "contents, parts and terminal lists EPLAN generated; KoperCAD generates its own from the data",
    );
    report.moved("devices", devices.len() - strips);
    report.moved("terminal strips", strips);
    report.moved("terminals", terminals);
    report.moved("cables", cables.len());
    report.moved(
        "cable cores",
        cables.iter().map(|c| c.cores.len()).sum::<usize>(),
    );
    let file_report = file.report();
    report.lossy.extend(
        file_report
            .lossy
            .iter()
            .filter(|l| l.what != "other objects")
            .cloned(),
    );
    report.lost(
        "symbol drawings",
        placements,
        "pages are created empty; symbols and wires follow with the symbol mapping and net rebuild",
    );
    report.lost(
        "texts",
        file.texts.len(),
        "free texts need a place on a drawn page",
    );
    report.lost(
        "interruption points",
        file.interruptions.len(),
        "they become cross references once the pages are drawn",
    );
    if let Some(l) = file_report.lossy.iter().find(|l| l.what == "other objects") {
        report.lossy.push(l.clone());
    }
    report.lost(
        "devices with a tag KoperCAD cannot read",
        unreadable,
        "the tag has characters or levels KoperCAD tags do not allow",
    );
    report.lost(
        "cable cores without a cable",
        orphan_cores,
        "the core names a cable that is not in the file",
    );
    if cores_by_id > 0 {
        report.note(format!(
            "{cores_by_id} cable cores were tied to their cable by the reference the file stores (exact), not by tag."
        ));
    }
    if cores_of_existing > 0 {
        report.note(format!(
            "{cores_of_existing} cable cores belong to cables that already exist and were skipped."
        ));
    }
    if repeats > 0 {
        report.note(format!(
            "{repeats} repeated drawings of the same tag became one device each."
        ));
    }
    if pages_existing > 0 {
        report.note(format!(
            "{pages_existing} pages have the name of a page that exists and were skipped."
        ));
    }
    if !existing.is_empty() {
        report.note(format!(
            "Already in the project and left as they are: {}.",
            existing.join(", ")
        ));
    }
    for n in &file.notes {
        report.note(n.clone());
    }
    if file.header.kind == PxfKind::PageMacro {
        report.note("A page macro: its pages are added to the project like a project's.");
    }

    let project_info = project
        .devices()
        .next()
        .is_none()
        .then(|| {
            let t = file.project.title.get(&opts.lang);
            let n = file.project.number.trim();
            let name = match (n.is_empty(), t.is_empty()) {
                (false, false) => format!("{n} {t}"),
                (false, true) => n.to_string(),
                (true, false) => t.to_string(),
                (true, true) => String::new(),
            };
            (name, t.to_string())
        })
        .filter(|(n, _)| !n.is_empty());
    if let Some((n, _)) = &project_info {
        report.note(format!("Project name set from the file: {n}."));
    }

    PxfPlan {
        structures,
        pages,
        devices,
        cables,
        existing,
        project_info,
        report,
        revision: project.revision(),
    }
}

impl PxfPlan {
    /// How many records the import would create (terminals and cores included).
    pub fn creates(&self) -> usize {
        self.structures.len()
            + self.pages.len()
            + self
                .devices
                .iter()
                .map(|d| 1 + d.terminals.len())
                .sum::<usize>()
            + self.cables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.creates() == 0
    }

    pub fn report(&self) -> &Report {
        &self.report
    }

    pub fn summary(&self) -> String {
        self.report.summary()
    }

    /// Adds a note to the report the person reads before applying.
    pub fn note(&mut self, text: impl Into<String>) {
        self.report.note(text);
    }

    /// Creates everything as one undo step; returns how many records.
    pub fn apply(&self, doc: &mut Document) -> Result<usize, ApplyError> {
        Ok(self.apply_with_changes(doc)?.map_or(0, |(n, _)| n))
    }

    /// Like [`apply`](Self::apply), and hands back the changes so the app can
    /// refresh what it draws. `None` when there was nothing to add.
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
        let (created, changes) = doc.transact("Import EPLAN project", |tx| {
            let mut created = 0usize;
            if let Some((name, description)) = &self.project_info {
                tx.set_info(|i| {
                    i.name = name.clone();
                    i.description = description.clone();
                })?;
            }
            for s in &self.structures {
                tx.add_structure(s.tag.clone(), "", false)?;
                created += 1;
            }
            for name in &self.pages {
                tx.add_page(name.clone())?;
                created += 1;
            }
            let mut parts: BTreeMap<(String, String), PartId> = tx
                .project()
                .parts()
                .map(|p| ((p.manufacturer.clone(), p.order_number.clone()), p.id))
                .collect();
            for d in &self.devices {
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
                for name in &d.terminals {
                    tx.add_terminal(id, name.clone())?;
                    created += 1;
                }
            }
            for c in &self.cables {
                let cores = c.cores.iter().map(|n| CableCore::new(n.clone())).collect();
                let id = tx.add_cable(c.tag.clone(), cores)?;
                tx.update::<Cable>(id, |x| x.description = c.description.clone())?;
                created += 1;
            }
            Ok(created)
        })?;
        Ok(Some((created, changes)))
    }
}
