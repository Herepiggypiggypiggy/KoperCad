//! Previewing and applying a PXF import.
//!
//! [`plan_pxf`] compares a [`PxfFile`] with the project and says what would
//! be created, what is already there and what is not carried; nothing changes
//! until [`PxfPlan::apply`], which creates everything as one undo step. An
//! import only adds: a device whose tag exists keeps what the project says.
//!
//! What is created: plant sections (`=A1`) and locations (`+CAB1`), one
//! page per EPLAN page (named by its label and title; generated report
//! pages are left out), one device per tag (EPLAN draws a device many times;
//! KoperCAD has one device with many drawings), terminal strips with their
//! terminals, cables with their cores, and the parts behind part numbers.
//! When the file embeds its symbol library, the circuit-diagram pages are
//! drawn too (see `draw`): symbols, interruption points and wires. Free
//! texts are not carried yet and are counted in the report.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    Cable, CableCore, ChangeSet, DeviceId, DeviceKind, DeviceTag, Document, Endpoint, Part, PartId,
    Placement, Point, Project, Rotation, SymbolInstance, SymbolRole, TerminalId, TextAnchor,
    TextContent, TextPlacement, Wire,
};

use super::draw::{Drawing, Role, draw};
use super::model::*;
use super::text::MlText;
use crate::eplan::nets::{NetComparison, compare};
use crate::eplan::symbols::SymbolMapper;
use crate::report::Report;
use crate::sheets::ApplyError;

/// Choices for reading the file.
#[derive(Debug, Clone)]
pub struct PxfOptions {
    /// Language of titles and type texts (`en_US`); another language is used
    /// when this one is missing.
    pub lang: String,
    /// Draw the circuit-diagram pages (symbols and wires) as well as
    /// creating the devices. On unless the person asks for the data only.
    pub draw: bool,
}

impl Default for PxfOptions {
    fn default() -> Self {
        Self {
            lang: "en_US".to_string(),
            draw: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NewPage {
    name: String,
    /// Index in the file's pages.
    source: usize,
}

/// A location without a plant section: the panel the devices sit in.
fn is_panel_tag(tag: &DeviceTag) -> bool {
    !tag.location.is_empty() && tag.function.is_empty()
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

/// A free text on a drawn page.
#[derive(Debug, Clone)]
struct NewText {
    page: usize,
    text: String,
    at: Point,
    height: Option<f64>,
    rotation: Rotation,
}

/// What an import would do.
#[derive(Debug, Clone)]
pub struct PxfPlan {
    structures: Vec<NewStructure>,
    pages: Vec<NewPage>,
    drawing: Drawing,
    /// Free texts of the drawn pages.
    texts: Vec<NewText>,
    devices: Vec<NewDevice>,
    cables: Vec<NewCable>,
    /// Tags the file has that the project already holds.
    pub existing: Vec<String>,
    /// Name and description for a project that is still empty.
    project_info: Option<(String, String)>,
    report: Report,
    revision: u64,
}

/// EPLAN's text angle (radians, counter-clockwise) as the nearest quarter turn.
fn quarter_turns(angle: f64) -> Rotation {
    match (angle / std::f64::consts::FRAC_PI_2)
        .round()
        .rem_euclid(4.0) as u8
    {
        1 => Rotation::R90,
        2 => Rotation::R180,
        3 => Rotation::R270,
        _ => Rotation::R0,
    }
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
    for (pi, p) in file.pages.iter().enumerate() {
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
            pages.push(NewPage { name, source: pi });
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
    let created: BTreeSet<usize> = pages.iter().map(|p| p.source).collect();
    let drawing = if opts.draw {
        draw(file, &SymbolMapper::new(novo_symbols::Pack::Iec), &created)
    } else {
        Drawing::default()
    };
    let texts: Vec<NewText> = if opts.draw {
        file.texts
            .iter()
            .filter_map(|t| {
                let (page, (x, y)) = (t.page?, t.position?);
                let text = t.text.get(&opts.lang).trim().to_string();
                (created.contains(&page) && !text.is_empty()).then(|| NewText {
                    page,
                    text,
                    at: Point::new(x, super::draw::PAGE_HEIGHT - y),
                    height: t.height,
                    rotation: quarter_turns(t.angle),
                })
            })
            .collect()
    } else {
        Vec::new()
    };
    report.moved("pages", pages.len());
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
    let st = &drawing.stats;
    if opts.draw {
        report.moved("symbols drawn", drawing.symbols.len());
        report.moved("wires", st.wires);
        report.lost(
            "symbol drawings",
            st.unplaced,
            "the file's library does not explain them, so they are not placed",
        );
        report.moved("free texts", texts.len());
        report.lost(
            "free texts",
            file.texts.len() - texts.len(),
            "empty, or on a page that is not drawn",
        );
        report.note(format!(
            "Drawn: {} circuit-diagram pages, {} symbols from KoperCAD's library, {} as labelled boxes, {} interruption points, {} nets joined by EPLAN's rules into {} wires. Symbol artwork is inferred (a box where no KoperCAD symbol fits); positions, pin counts, tags and which points are one net are read from the file. Wire routes are plain right-angle runs, not EPLAN's lines.",
            st.pages, st.library_symbols, st.box_symbols, st.interruptions, st.nets, st.wires
        ));
        if !st.box_functions.is_empty() {
            let list: Vec<String> = st
                .box_functions
                .iter()
                .map(|(f, n)| format!("{f} x{n}"))
                .collect();
            report.note(format!("Drawn as boxes: {}.", list.join(", ")));
        }
        for ((function, letters, pins, ports), n) in &st.box_kinds {
            report.note(format!(
                "Box kind: function \"{function}\"; tag letters \"{letters}\"; pins [{}]; {ports} connection points in the library symbol; x{n}.",
                pins.join("|")
            ));
        }
        if st.loose > 0 {
            report.note(format!(
                "{} placements found their library symbol only loosely (same function, another variant); their connection points may be off.",
                st.loose
            ));
        }
        for (what, n) in &st.issues {
            report.note(format!("Net check: {n} {what}."));
        }
    } else {
        report.lost(
            "symbol drawings",
            placements,
            "drawing was switched off; pages are created empty",
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
    }
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

    let mut plan = PxfPlan {
        structures,
        pages,
        drawing,
        texts,
        devices,
        cables,
        existing,
        project_info,
        report,
        revision: project.revision(),
    };
    if !plan.drawing.symbols.is_empty()
        && let Ok(Some((_, _, Some(c)))) = plan.apply_checked(&mut Document::new(project.clone()))
    {
        let agree = c
            .compared
            .saturating_sub(c.import_only.len() + c.project_only.len());
        plan.report.note(if c.agrees() {
            format!(
                "Net check: KoperCAD's own nets of the drawn pages equal the nets EPLAN's rules give, over {} connection points.",
                c.compared
            )
        } else {
            format!(
                "Net check: KoperCAD's own nets agree with EPLAN's rules over {agree} of {} connection points; {} pairs EPLAN joins are apart and {} pairs KoperCAD joins (same-name terminals, names used more than twice) EPLAN keeps apart.",
                c.compared,
                c.import_only.len(),
                c.project_only.len()
            )
        });
    }
    plan
}

impl PxfPlan {
    /// How many records the import would create (terminals and cores included).
    pub fn creates(&self) -> usize {
        self.structures.len()
            + self.pages.len()
            + self.drawing.symbols.len()
            + self.drawing.wires.len()
            + self.texts.len()
            + self.drawing.junctions.len()
            + self.drawing.stats.box_symbols
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
        Ok(self.apply_checked(doc)?.map(|(n, c, _)| (n, c)))
    }

    /// Like [`apply_with_changes`](Self::apply_with_changes), and also says
    /// whether KoperCAD's own nets of what was drawn agree with the nets
    /// EPLAN's rules gave (`None` when nothing was drawn).
    pub fn apply_checked(
        &self,
        doc: &mut Document,
    ) -> Result<Option<(usize, ChangeSet, Option<NetComparison>)>, ApplyError> {
        if doc.project().revision() != self.revision {
            return Err(ApplyError::Outdated);
        }
        if self.is_empty() {
            return Ok(None);
        }
        let mut check = None;
        let (created, changes) = doc.transact("Import EPLAN project", |tx| {
            let mut created = 0usize;
            if let Some((name, description)) = &self.project_info {
                tx.set_info(|i| {
                    i.name = name.clone();
                    i.description = description.clone();
                })?;
            }
            for s in &self.structures {
                // A location that holds pages or devices is a panel.
                tx.add_structure(s.tag.clone(), "", is_panel_tag(&s.tag))?;
                created += 1;
            }
            let mut page_ids = BTreeMap::new();
            for p in &self.pages {
                page_ids.insert(p.source, tx.add_page(p.name.clone())?);
                created += 1;
            }
            let mut device_ids: BTreeMap<String, DeviceId> = BTreeMap::new();
            let mut terminal_ids: BTreeMap<(String, String), TerminalId> = BTreeMap::new();
            let mut parts: BTreeMap<(String, String), PartId> = tx
                .project()
                .parts()
                .map(|p| ((p.manufacturer.clone(), p.order_number.clone()), p.id))
                .collect();
            for d in &self.devices {
                let id = tx.add_device_of_kind(d.tag.clone(), d.description.clone(), d.kind)?;
                device_ids.insert(d.tag.to_string(), id);
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
                    let t = tx.add_terminal(id, name.clone())?;
                    terminal_ids.insert((d.tag.to_string(), name.clone()), t);
                    created += 1;
                }
            }
            for c in &self.cables {
                let cores = c.cores.iter().map(|n| CableCore::new(n.clone())).collect();
                let id = tx.add_cable(c.tag.clone(), cores)?;
                tx.update::<Cable>(id, |x| x.description = c.description.clone())?;
                created += 1;
            }
            created += self.draw_pages(tx, &page_ids, &device_ids, &terminal_ids, &mut check)?;
            Ok(created)
        })?;
        Ok(Some((created, changes, check)))
    }

    /// Places the drawn symbols and their wires; returns how many records.
    fn draw_pages(
        &self,
        tx: &mut novo_core::Tx<'_>,
        page_ids: &BTreeMap<usize, novo_core::PageId>,
        device_ids: &BTreeMap<String, DeviceId>,
        terminal_ids: &BTreeMap<(String, String), TerminalId>,
        check: &mut Option<NetComparison>,
    ) -> Result<usize, novo_core::ModelError> {
        let mut created = 0usize;
        let mut main_taken: BTreeSet<DeviceId> = BTreeSet::new();
        let mut placed: Vec<Option<novo_core::SymbolId>> = Vec::new();
        for s in &self.drawing.symbols {
            let Some(&page) = page_ids.get(&s.page) else {
                placed.push(None);
                continue;
            };
            let role = match &s.role {
                Role::Device { tag, coil } => {
                    let tag = canonical(tag);
                    let id = device_ids.get(&tag).copied().or_else(|| {
                        let t = DeviceTag::parse(&tag).ok()?;
                        tx.project().devices_by_tag(&t).first().copied()
                    });
                    match id {
                        Some(device) => SymbolRole::Device {
                            device,
                            main: *coil && main_taken.insert(device),
                        },
                        None => SymbolRole::Unassigned,
                    }
                }
                Role::Terminal { strip, name } => {
                    match terminal_ids.get(&(canonical(strip), name.clone())) {
                        Some(&t) => SymbolRole::Terminal { terminals: vec![t] },
                        None => SymbolRole::Unassigned,
                    }
                }
                Role::Interruption(name) => SymbolRole::Interruption { name: name.clone() },
                Role::Plain => SymbolRole::Unassigned,
            };
            let id = tx.place_symbol(
                page,
                s.key.as_str(),
                Placement::at(s.origin),
                s.pins.clone(),
                role,
            )?;
            if !s.bridges.is_empty() || !s.pass_through.is_empty() {
                tx.update::<SymbolInstance>(id, |x| {
                    x.bridges = s.bridges.clone();
                    x.pass_through = s.pass_through.clone();
                })?;
            }
            if !s.label.is_empty() {
                // Boxes carry no artwork of their own: name the function inside.
                tx.add_text(
                    page,
                    TextContent::Literal {
                        text: s.label.clone(),
                    },
                    // The placeholder body is centred on the origin.
                    TextPlacement {
                        anchor: TextAnchor::MiddleCenter,
                        ..Default::default()
                    },
                    Some(id.into()),
                )?;
                created += 1;
            }
            placed.push(Some(id));
            created += 1;
        }
        for (pg, at) in &self.drawing.junctions {
            let Some(&page) = page_ids.get(pg) else {
                continue;
            };
            tx.add_junction(page, *at)?;
            created += 1;
        }
        for t in &self.texts {
            let Some(&page) = page_ids.get(&t.page) else {
                continue;
            };
            let id = tx.add_text(
                page,
                TextContent::Literal {
                    text: t.text.clone(),
                },
                TextPlacement {
                    offset: t.at,
                    rotation: t.rotation,
                    ..Default::default()
                },
                None,
            )?;
            if let Some(h) = t.height {
                tx.set_text(id, None, Some(h))?;
            }
            created += 1;
        }
        for w in &self.drawing.wires {
            let (Some(Some(a)), Some(Some(b))) = (placed.get(w.from.0), placed.get(w.to.0)) else {
                continue;
            };
            let id = tx.connect(Endpoint::pin(*a, w.from.1), Endpoint::pin(*b, w.to.1))?;
            if !w.route.is_empty() {
                tx.update::<Wire>(id, |x| x.route = w.route.clone())?;
            }
            created += 1;
        }
        if !placed.is_empty() {
            let netlist = tx.project().netlist();
            *check = Some(compare(&self.drawing.recon, &netlist, |k| {
                let &(si, pin) = self.drawing.keys.get(k)?;
                Some(Endpoint::pin((*placed.get(si)?)?, pin))
            }));
        }
        Ok(created)
    }
}
