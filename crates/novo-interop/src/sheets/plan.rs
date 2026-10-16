//! Import: what a workbook would change, before anything changes.
//!
//! [`plan_import`] compares every row of the sheets with the project and
//! returns an [`ImportPlan`]: the cell changes (each with the value it
//! replaces), the rows it refused and why, and the conflicts. Nothing
//! touches the project until [`ImportPlan::apply`], which makes the
//! selected changes one undo step.
//!
//! # Matching
//!
//! A row is matched by its **Id**, never by its tag or name, so renaming
//! a device, terminal or cable in the sheet renames it in the project.
//! A row without an Id (a CSV from somewhere else) falls back to the tag
//! (devices, cables) or strip and name (terminals) when that is unique; wires have
//! nothing unique to fall back to. A row whose Id is gone from the
//! project (deleted since the export) is rejected, not recreated: this
//! import edits what exists and never adds or deletes.
//!
//! # Conflicts
//!
//! An `.xlsx` export carries a hidden baseline: the values at export time.
//! For each cell, with `sheet` the value in the file, `project` the value
//! now and `base` the value at export:
//!
//! - `sheet == project`: nothing to do.
//! - `sheet == base`: the sheet did not touch it; the project's own later
//!   edit stays.
//! - `project == base`: the sheet changed it alone: a plain change.
//! - otherwise both changed it differently: a **conflict**, left out until
//!   the user takes the sheet's value ([`ImportPlan::resolve`]).
//!
//! Without a baseline (CSV, or a rebuilt workbook) every difference is a
//! plain change, and the plan says so.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, CableId, Command, DeviceId, DeviceTag, Document, ModelError, Project, Record,
    TerminalId, WireId,
};
use novo_parts::{Catalog, display_value, parse_number};

use super::create::{Ids, NewDevice, PartRef, new_device};
use super::fields::{Field, Parsed, Slot, apply, fields, parse, property_of_header, read};
use super::new_rows::{
    AddedRow, CoreTarget, NewCable, NewCore, NewTerminal, PendingCore, StripRef,
    core_target_by_tag, new_cable, new_terminal, pending_core, place_cores,
};

/// A row's key in the baseline: its Id, and on the cores sheet the core.
type BaseKey = (u64, usize);
use super::grid::{Cell, Table, TableKind, Workbook};

/// One cell the import would change.
#[derive(Debug, Clone)]
pub struct Change {
    pub id: AnyId,
    /// On the cores sheet, the 0-based index of the core in its cable.
    pub core: Option<usize>,
    pub table: TableKind,
    /// The row in the sheet (CSV: the line).
    pub line: usize,
    /// Which item the row is, as the project knows it now (`-K1`).
    pub item: String,
    pub header: String,
    /// What the project holds now, as the preview shows it.
    pub before: String,
    /// What the sheet says.
    pub after: String,
    /// Set when the project changed this cell since the export too: the
    /// value the export held. Conflicts start unselected.
    pub conflict: Option<String>,
    pub selected: bool,
    field: Field,
    parsed: Parsed,
}

/// A row the import did not use, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub table: TableKind,
    pub line: usize,
    /// The row as the sheet names it (its tag, or its Id), when it has one.
    pub item: String,
    pub reasons: Vec<String>,
}

/// Which side wins a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// Take the value in the sheet.
    Sheet,
    /// Keep what the project has.
    Project,
}

/// Why a plan could not be applied.
#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("the project changed after this preview was made; preview the import again")]
    Outdated,
    #[error(transparent)]
    Model(#[from] ModelError),
}

/// The preview of an import.
#[derive(Debug, Clone)]
pub struct ImportPlan {
    pub changes: Vec<Change>,
    /// Devices the sheet adds (rows without an Id and with a new tag).
    pub new_devices: Vec<NewDevice>,
    /// Terminals the sheet adds (Terminals rows without an Id).
    pub new_terminals: Vec<NewTerminal>,
    /// Cables the sheet adds (Cables rows without an Id).
    pub new_cables: Vec<NewCable>,
    /// Cores the sheet adds, on existing and on new cables.
    pub new_cores: Vec<NewCore>,
    pub rejected: Vec<Rejected>,
    /// Things worth knowing that block nothing: ignored columns and
    /// sheets, rows matched by tag, a missing baseline.
    pub notes: Vec<String>,
    /// Things that would be wrong after the import, such as two devices
    /// ending up with one tag.
    pub warnings: Vec<String>,
    /// Rows read, blank rows excluded.
    pub rows_read: usize,
    /// Rows that matched and already agree with the project.
    pub rows_unchanged: usize,
    /// The project revision the workbook was exported from, if it says.
    pub exported_revision: Option<u64>,
    /// The revision this plan was made against.
    pub revision: u64,
    /// Whether the workbook carried a baseline (so conflicts are detected).
    pub has_baseline: bool,
}

impl ImportPlan {
    /// The changes that conflict with a later edit in the project.
    pub fn conflicts(&self) -> impl Iterator<Item = &Change> {
        self.changes.iter().filter(|c| c.conflict.is_some())
    }

    /// How many changes would be made.
    pub fn selected(&self) -> usize {
        self.changes.iter().filter(|c| c.selected).count()
            + self.new_devices.iter().filter(|n| n.selected).count()
            + self
                .new_terminals
                .iter()
                .filter(|t| self.terminal_on(t))
                .count()
            + self.new_cables.iter().filter(|c| c.selected).count()
            + self.new_cores.iter().filter(|c| self.core_on(c)).count()
    }

    /// Whether a new terminal is added: selected, and its strip too when
    /// the strip is a new device.
    fn terminal_on(&self, t: &NewTerminal) -> bool {
        t.selected
            && match &t.strip {
                StripRef::Existing(_) => true,
                StripRef::New(tag) => self.new_devices.iter().any(|d| d.selected && &d.tag == tag),
            }
    }

    /// Whether a new core is added: selected, and its cable too when the
    /// cable is new.
    fn core_on(&self, c: &NewCore) -> bool {
        c.selected
            && match &c.target {
                CoreTarget::Existing(_) => true,
                CoreTarget::New(tag) => self.new_cables.iter().any(|n| n.selected && &n.tag == tag),
            }
    }

    /// Every row the sheet adds (devices, terminals, cables, cores), in
    /// that order, for a preview list. [`ImportPlan::select_added`] takes
    /// the index in this list.
    pub fn added_rows(&self) -> Vec<AddedRow> {
        let mut rows = Vec::new();
        for n in &self.new_devices {
            rows.push(AddedRow {
                table: TableKind::Devices,
                line: n.line,
                label: format!("device {}", n.tag),
                selected: n.selected,
                left_out_with: None,
            });
        }
        for t in &self.new_terminals {
            rows.push(AddedRow {
                table: TableKind::Terminals,
                line: t.line,
                label: format!("terminal {}:{}", t.strip_tag, t.name),
                selected: t.selected,
                left_out_with: (t.selected && !self.terminal_on(t))
                    .then(|| t.strip_tag.to_string()),
            });
        }
        for c in &self.new_cables {
            rows.push(AddedRow {
                table: TableKind::Cables,
                line: c.line,
                label: format!("cable {}", c.tag),
                selected: c.selected,
                left_out_with: None,
            });
        }
        for c in &self.new_cores {
            rows.push(AddedRow {
                table: TableKind::Cores,
                line: c.line,
                label: format!("core {} ({}) of {}", c.no, c.name, c.cable_tag),
                selected: c.selected,
                left_out_with: (c.selected && !self.core_on(c)).then(|| c.cable_tag.to_string()),
            });
        }
        rows
    }

    /// Selects or deselects one added row; `index` is its place in
    /// [`ImportPlan::added_rows`].
    pub fn select_added(&mut self, index: usize, selected: bool) {
        let mut i = index;
        if i < self.new_devices.len() {
            self.new_devices[i].selected = selected;
            return;
        }
        i -= self.new_devices.len();
        if i < self.new_terminals.len() {
            self.new_terminals[i].selected = selected;
            return;
        }
        i -= self.new_terminals.len();
        if i < self.new_cables.len() {
            self.new_cables[i].selected = selected;
            return;
        }
        i -= self.new_cables.len();
        if let Some(core) = self.new_cores.get_mut(i) {
            core.selected = selected;
        }
    }

    /// Selects or deselects one new device.
    pub fn select_new(&mut self, index: usize, selected: bool) {
        if let Some(new) = self.new_devices.get_mut(index) {
            new.selected = selected;
        }
    }

    /// Selects or deselects one change.
    pub fn select(&mut self, index: usize, selected: bool) {
        if let Some(change) = self.changes.get_mut(index) {
            change.selected = selected;
        }
    }

    /// Settles every conflict the same way.
    pub fn resolve(&mut self, resolution: Resolution) {
        for change in self.changes.iter_mut().filter(|c| c.conflict.is_some()) {
            change.selected = resolution == Resolution::Sheet;
        }
    }

    /// Whether applying would do anything.
    pub fn is_empty(&self) -> bool {
        self.selected() == 0
    }

    /// One line for the confirmation dialog.
    pub fn summary(&self) -> String {
        let items: BTreeSet<(AnyId, Option<usize>)> = self
            .changes
            .iter()
            .filter(|c| c.selected)
            .map(|c| (c.id, c.core))
            .collect();
        let conflicts = self.conflicts().count();
        let changes = self.changes.iter().filter(|c| c.selected).count();
        let added = self.new_devices.iter().filter(|n| n.selected).count();
        let mut s = format!(
            "{changes} change{} in {} row{}",
            plural(changes),
            items.len(),
            plural(items.len()),
        );
        if added > 0 {
            s.push_str(&format!(", {added} new device{}", plural(added)));
        }
        let terminals = self
            .new_terminals
            .iter()
            .filter(|t| self.terminal_on(t))
            .count();
        if terminals > 0 {
            s.push_str(&format!(", {terminals} new terminal{}", plural(terminals)));
        }
        let cables = self.new_cables.iter().filter(|c| c.selected).count();
        if cables > 0 {
            s.push_str(&format!(", {cables} new cable{}", plural(cables)));
        }
        let cores = self.new_cores.iter().filter(|c| self.core_on(c)).count();
        if cores > 0 {
            s.push_str(&format!(", {cores} new core{}", plural(cores)));
        }
        if conflicts > 0 {
            s.push_str(&format!(
                ", {conflicts} conflict{} with later edits",
                plural(conflicts)
            ));
        }
        if !self.rejected.is_empty() {
            s.push_str(&format!(
                ", {} row{} rejected",
                self.rejected.len(),
                plural(self.rejected.len())
            ));
        }
        s
    }

    /// The command the selected changes make: one record replacement per
    /// changed object, in one batch, so a single undo takes it all back.
    /// `Ok(None)` when nothing is selected. Fails when the project is not
    /// the one this plan was made against.
    pub fn command(&self, project: &Project) -> Result<Option<Command>, ApplyError> {
        if project.revision() != self.revision {
            return Err(ApplyError::Outdated);
        }
        let mut ids = Ids::new(project);
        let mut records: BTreeMap<AnyId, Record> = BTreeMap::new();
        for change in self.changes.iter().filter(|c| c.selected) {
            let record = match records.get_mut(&change.id) {
                Some(r) => r,
                None => {
                    let Some(record) = project.record(change.id) else {
                        return Err(ApplyError::Outdated);
                    };
                    records.entry(change.id).or_insert(record)
                }
            };
            apply(record, change.core, &change.field, &change.parsed);
            if let (Record::Device(d), Parsed::Part(part)) = (&mut *record, &change.parsed) {
                // The sheet names one part; it replaces the first.
                match part {
                    Some(part) => {
                        let id = ids.part(part);
                        d.parts.retain(|&p| p != id);
                        d.parts.insert(0, id);
                        d.parts.truncate(1);
                    }
                    None => d.parts.clear(),
                }
            }
        }
        let mut added: Vec<Command> = self
            .new_devices
            .iter()
            .filter(|n| n.selected)
            .map(|n| ids.insert_device(n))
            .collect();
        // New cores on cables the project has: after the edits to the cable.
        for core in self.new_cores.iter().filter(|c| self.core_on(c)) {
            let CoreTarget::Existing(cable) = core.target else {
                continue;
            };
            let id = AnyId::Cable(cable);
            let record = match records.get_mut(&id) {
                Some(r) => r,
                None => {
                    let Some(record) = project.record(id) else {
                        return Err(ApplyError::Outdated);
                    };
                    records.entry(id).or_insert(record)
                }
            };
            if let Record::Cable(c) = record {
                if c.cores.len() + 1 != core.no {
                    return Err(ApplyError::Outdated);
                }
                super::new_rows::add_core(c, core);
            }
        }
        for cable in self.new_cables.iter().filter(|c| c.selected) {
            let cores: Vec<&NewCore> = self
                .new_cores
                .iter()
                .filter(|c| c.selected && c.target == CoreTarget::New(cable.tag.clone()))
                .collect();
            added.push(ids.insert_cable(cable, &cores));
        }
        for terminal in self.new_terminals.iter().filter(|t| self.terminal_on(t)) {
            added.push(ids.insert_terminal(terminal)?);
        }
        if records.is_empty() && added.is_empty() {
            return Ok(None);
        }
        let mut commands = std::mem::take(&mut ids.inserts);
        commands.extend(added);
        commands.extend(
            records
                .into_values()
                .map(|record| Command::Replace { record }),
        );
        Ok(Some(Command::Batch {
            label: "Import from spreadsheet".into(),
            commands,
        }))
    }

    /// Applies the selected changes to the document as one undo step.
    /// Returns how many objects changed.
    pub fn apply(&self, doc: &mut Document) -> Result<usize, ApplyError> {
        let Some(command) = self.command(doc.project())? else {
            return Ok(0);
        };
        let count = match &command {
            Command::Batch { commands, .. } => commands
                .iter()
                .filter(|c| {
                    !matches!(
                        c,
                        Command::Insert {
                            record: Record::Part(_)
                        }
                    )
                })
                .count(),
            _ => 1,
        };
        doc.execute(command)?;
        Ok(count)
    }

    /// The rejected rows as readable text, one row per line.
    pub fn rejected_report(&self) -> String {
        let mut out = String::new();
        for r in &self.rejected {
            out.push_str(&format!(
                "{} row {}{}: {}\n",
                r.table.name(),
                r.line,
                if r.item.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", r.item)
                },
                r.reasons.join("; ")
            ));
        }
        out
    }

    /// The rejected rows as a table to save next to the project: sheet,
    /// row, item and reasons.
    pub fn rejected_table(&self) -> Vec<Vec<String>> {
        let mut rows = vec![vec![
            "Sheet".to_owned(),
            "Row".into(),
            "Item".into(),
            "Why it was rejected".into(),
        ]];
        for r in &self.rejected {
            rows.push(vec![
                r.table.name().to_owned(),
                r.line.to_string(),
                r.item.clone(),
                r.reasons.join("; "),
            ]);
        }
        rows
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn any_id(kind: TableKind, raw: u64) -> AnyId {
    match kind {
        TableKind::Devices => DeviceId(raw).into(),
        TableKind::Terminals => TerminalId(raw).into(),
        TableKind::Wires => WireId(raw).into(),
        TableKind::Cables | TableKind::Cores => CableId(raw).into(),
    }
}

/// The row's Id cell, if it holds a usable one.
fn parse_id(cell: &Cell) -> Result<Option<u64>, String> {
    let text = cell.as_text();
    if text.is_empty() {
        return Ok(None);
    }
    // `12`, `#12` or `Device#12`.
    let text = text
        .trim_start_matches(|c: char| c.is_alphabetic())
        .trim_start_matches('#');
    match parse_number(text) {
        Some(n) if n >= 0.0 && n.fract() == 0.0 && n < 1e15 => Ok(Some(n as u64)),
        _ => Err(format!("Id: '{text}' is not an id")),
    }
}

/// What the project calls the item, for messages.
fn item_name(project: &Project, id: AnyId, core: Option<usize>) -> String {
    if let (AnyId::Cable(c), Some(i)) = (id, core)
        && let Some(cable) = project.cable(c)
    {
        let name = cable.cores.get(i).map_or("?", |c| c.name.as_str());
        return format!("{} core {} ({})", cable.tag, i + 1, name);
    }
    match id {
        AnyId::Device(d) => project.device(d).map(|d| d.tag.to_string()),
        AnyId::Cable(c) => project.cable(c).map(|c| c.tag.to_string()),
        AnyId::Terminal(t) => project.terminal_designation(t),
        AnyId::Wire(w) => project.wire(w).map(|w| {
            let page = project
                .page(w.page)
                .map(|p| p.name.clone())
                .unwrap_or_default();
            match &w.number {
                Some(n) => format!("wire {n} on page {page}"),
                None => format!("wire {} on page {page}", w.id.raw()),
            }
        }),
        _ => None,
    }
    .unwrap_or_else(|| id.to_string())
}

/// Finds the row's item by its names when it has no Id.
fn find_by_key(project: &Project, table: &Table, cells: &[Cell]) -> Result<AnyId, String> {
    let cell = |header: &str| {
        table
            .column(header)
            .and_then(|c| cells.get(c))
            .map(Cell::as_text)
            .unwrap_or_default()
    };
    let unique = |found: Vec<AnyId>, what: &str| match found.as_slice() {
        [one] => Ok(*one),
        [] => Err(format!("no Id, and nothing has {what}")),
        many => Err(format!("no Id, and {} items have {what}", many.len())),
    };
    match table.kind {
        TableKind::Devices | TableKind::Cables => {
            let text = cell("Tag");
            let text = text.strip_prefix('\'').unwrap_or(&text).trim().to_owned();
            if text.is_empty() {
                return Err("no Id and no Tag".into());
            }
            let tag =
                DeviceTag::parse(&text).map_err(|_| format!("no Id, and '{text}' is not a tag"))?;
            let found: Vec<AnyId> = if table.kind == TableKind::Devices {
                project
                    .devices_by_tag(&tag)
                    .into_iter()
                    .map(Into::into)
                    .collect()
            } else {
                project
                    .cables()
                    .filter(|c| c.tag == tag)
                    .map(|c| c.id.into())
                    .collect()
            };
            unique(found, &format!("the tag {text}"))
        }
        TableKind::Terminals => {
            let (strip, name) = (cell("Strip"), cell("Name"));
            if strip.is_empty() || name.is_empty() {
                return Err("no Id and no Strip and Name".into());
            }
            let strip = strip.strip_prefix('\'').unwrap_or(&strip).to_owned();
            let found: Vec<AnyId> = project
                .terminals()
                .filter(|t| {
                    t.name == name
                        && project
                            .device(t.strip)
                            .is_some_and(|d| d.tag.to_string() == strip)
                })
                .map(|t| t.id.into())
                .collect();
            unique(found, &format!("the name {strip}:{name}"))
        }
        TableKind::Wires => Err("a wire row needs its Id".into()),
        TableKind::Cores => Err("a core row needs the cable's Id and the Core No".into()),
    }
}

/// The value of `field` in the project now, in comparison form.
fn project_canon(
    project: &Project,
    id: AnyId,
    core: Option<usize>,
    field: &Field,
    like: Option<&novo_core::Value>,
) -> String {
    let cell = read(project, id, core, field);
    parse(field, &cell, like)
        .map(|p| p.canon())
        .unwrap_or_else(|_| cell.as_text())
}

fn current_value(project: &Project, id: AnyId, key: &str) -> Option<novo_core::Value> {
    let mut record = project.record(id)?;
    record.properties_mut()?.get(key).cloned()
}

/// Header of a column, matched against the project's columns. A
/// `properties.<key>` column nobody has yet is a new property.
fn match_field(known: &[Field], header: &str) -> Option<Field> {
    let mut h = header.trim();
    // `Number` is what a terminal's name is called on a terminal diagram.
    if h.eq_ignore_ascii_case("number") && known.iter().any(|f| f.slot == Slot::TerminalName) {
        h = "Name";
    }
    if let Some(f) = known.iter().find(|f| f.header.eq_ignore_ascii_case(h)) {
        return Some(f.clone());
    }
    property_of_header(h).map(|key| Field {
        header: format!("{}{key}", super::fields::PROPERTY_PREFIX),
        slot: Slot::Property(key),
    })
}

/// Compares a workbook with the project. See the [module](self) for how
/// rows are matched and conflicts found.
pub fn plan_import(project: &Project, workbook: &Workbook) -> ImportPlan {
    plan_import_with(project, workbook, &Catalog::new())
}

/// [`plan_import`] with a parts catalog: an order number the project does
/// not have is looked up there (manufacturer, type, data) before a bare
/// part is made from the order number alone.
pub fn plan_import_with(project: &Project, workbook: &Workbook, catalog: &Catalog) -> ImportPlan {
    let mut plan = ImportPlan {
        changes: Vec::new(),
        new_devices: Vec::new(),
        new_terminals: Vec::new(),
        new_cables: Vec::new(),
        new_cores: Vec::new(),
        rejected: Vec::new(),
        notes: Vec::new(),
        warnings: Vec::new(),
        rows_read: 0,
        rows_unchanged: 0,
        exported_revision: workbook.meta.revision,
        revision: project.revision(),
        has_baseline: !workbook.baseline.is_empty(),
    };
    for name in &workbook.ignored {
        plan.notes.push(format!(
            "Sheet '{name}' is not a KoperCAD table and was left alone"
        ));
    }
    if let Some(rev) = workbook.meta.revision
        && rev != project.revision()
    {
        plan.notes.push(
            "The project has changed since this file was exported; \
             cells edited in both places are listed as conflicts"
                .into(),
        );
    }
    if workbook.baseline.is_empty() {
        plan.notes.push(
            "This file has no baseline (CSV, or the hidden sheets were removed): every \
             difference is treated as an edit in the file and conflicts cannot be detected"
                .into(),
        );
    }
    // Devices before terminals, cables before cores: new rows name the
    // new rows before them.
    let mut tables: Vec<&Table> = workbook.tables.iter().collect();
    tables.sort_by_key(|t| t.kind);
    for table in tables {
        plan_table(project, catalog, workbook, table, &mut plan);
    }
    warn_duplicate_tags(project, &mut plan);
    warn_core_clashes(project, &mut plan);
    warn_lone_jumpers(project, &mut plan);
    plan
}

fn plan_table(
    project: &Project,
    catalog: &Catalog,
    workbook: &Workbook,
    table: &Table,
    plan: &mut ImportPlan,
) {
    let kind = table.kind;
    let known = fields(project, kind);
    let mut columns: Vec<Option<Field>> = Vec::with_capacity(table.headers.len());
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for header in &table.headers {
        let field = match_field(&known, header);
        let field = match field {
            Some(f) if !seen.insert(f.header.to_lowercase()) => {
                plan.notes.push(format!(
                    "{}: column '{header}' appears twice; the first one is used",
                    kind.name()
                ));
                None
            }
            Some(f) => Some(f),
            None => {
                if !header.trim().is_empty() {
                    plan.notes.push(format!(
                        "{}: column '{header}' is not one KoperCAD knows and was ignored",
                        kind.name()
                    ));
                }
                None
            }
        };
        columns.push(field);
    }
    let id_col = columns
        .iter()
        .position(|f| f.as_ref().is_some_and(Field::is_id));
    if id_col.is_none() {
        plan.notes.push(format!(
            "{}: no Id column; rows are matched by {}",
            kind.name(),
            match kind {
                TableKind::Terminals => "strip and name",
                TableKind::Wires => "nothing (wires need an Id)",
                TableKind::Cores => "nothing (cores need the cable's Id and the Core No)",
                _ => "tag",
            }
        ));
    }

    // The baseline rows by Id, with their own column order.
    let base_table = workbook.baseline_of(kind);
    let mut base_rows: BTreeMap<BaseKey, BTreeMap<String, &Cell>> = BTreeMap::new();
    if let Some(base) = base_table
        && let Some(base_id) = base.column("Id")
    {
        for row in &base.rows {
            let Some(Ok(Some(id))) = row.cells.get(base_id).map(parse_id) else {
                continue;
            };
            let no = match kind {
                TableKind::Cores => match base.column("Core No").and_then(|c| row.cells.get(c)) {
                    Some(c) => match core_no(c) {
                        Some(n) => n,
                        None => continue,
                    },
                    None => continue,
                },
                _ => 0,
            };
            let by_header = base
                .headers
                .iter()
                .zip(&row.cells)
                .map(|(h, c)| (h.trim().to_lowercase(), c))
                .collect();
            base_rows.insert((id, no), by_header);
        }
    }

    let core_col = columns
        .iter()
        .position(|f| f.as_ref().is_some_and(|f| f.slot == Slot::CoreNo));
    let mut first_line: BTreeMap<(AnyId, Option<usize>), usize> = BTreeMap::new();
    // Wires asked for by rows already accepted, and the cable they would sit on:
    // the project alone cannot tell that two rows want one free wire.
    let mut claims: BTreeMap<novo_core::WireId, (CableId, usize)> = BTreeMap::new();
    let mut matched_by_key = 0usize;
    let mut pending_cores: Vec<PendingCore> = Vec::new();
    for row in &table.rows {
        if row.cells.iter().all(Cell::is_blank) {
            continue;
        }
        plan.rows_read += 1;
        let reject = |plan: &mut ImportPlan, item: String, reasons: Vec<String>| {
            plan.rejected.push(Rejected {
                table: kind,
                line: row.line,
                item,
                reasons,
            });
        };
        let cell_at = |c: usize| row.cells.get(c).cloned().unwrap_or_default();

        // Which item is this row?
        let by_id = match id_col.map(|c| parse_id(&cell_at(c))) {
            Some(Err(e)) => {
                reject(plan, String::new(), vec![e]);
                continue;
            }
            Some(Ok(Some(raw))) => Some(raw),
            _ => None,
        };
        let id = match by_id {
            Some(raw) => {
                let id = any_id(kind, raw);
                if !project.contains(id) {
                    reject(
                        plan,
                        format!("Id {raw}"),
                        vec![format!(
                            "the {} with Id {raw} no longer exists in the project (deleted after the export?)",
                            kind.noun()
                        )],
                    );
                    continue;
                }
                id
            }
            None => match find_by_key(project, table, &row.cells) {
                Ok(id) => {
                    matched_by_key += 1;
                    id
                }
                Err(why) => {
                    // A row added to an export (the sheet has an Id column) is a new
                    // device, terminal, cable or core; in a sheet without one an
                    // unknown name is more likely a typo.
                    if id_col.is_some() {
                        let added = match kind {
                            TableKind::Devices => new_device(
                                project,
                                catalog,
                                &columns,
                                &row.cells,
                                &plan.new_devices,
                            )
                            .map(|r| {
                                r.map(|mut new| {
                                    new.line = row.line;
                                    plan.new_devices.push(new);
                                })
                            }),
                            TableKind::Terminals => new_terminal(
                                project,
                                &columns,
                                &row.cells,
                                &plan.new_devices,
                                &plan.new_terminals,
                            )
                            .map(|r| {
                                r.map(|mut new| {
                                    new.line = row.line;
                                    plan.new_terminals.push(new);
                                })
                            }),
                            TableKind::Cables => {
                                new_cable(project, &columns, &row.cells, &plan.new_cables).map(
                                    |r| {
                                        r.map(|mut new| {
                                            new.line = row.line;
                                            plan.new_cables.push(new);
                                        })
                                    },
                                )
                            }
                            TableKind::Cores => core_without_id(
                                project, &columns, &row.cells, core_col, row.line, plan,
                            )
                            .map(|r| r.map(|p| pending_cores.push(p))),
                            TableKind::Wires => None,
                        };
                        match added {
                            Some(Ok(())) => continue,
                            Some(Err(reasons)) => {
                                reject(plan, String::new(), reasons);
                                continue;
                            }
                            None => {}
                        }
                    }
                    reject(plan, String::new(), vec![why]);
                    continue;
                }
            },
        };
        // On the cores sheet the second half of the key: which core.
        let core = if kind == TableKind::Cores {
            let n = core_col.map(cell_at);
            let count = match id {
                AnyId::Cable(c) => project.cable(c).map_or(0, |c| c.cores.len()),
                _ => 0,
            };
            match n.as_ref().and_then(core_no) {
                Some(n) if n >= 1 && n <= count => Some(n - 1),
                Some(n) if n > count => {
                    // One past the end adds a core; the rows are numbered together below.
                    let tag = match id {
                        AnyId::Cable(c) => project.cable(c).map(|c| c.tag.clone()),
                        _ => None,
                    };
                    let (AnyId::Cable(cable), Some(tag)) = (id, tag) else {
                        continue;
                    };
                    match pending_core(
                        &columns,
                        &row.cells,
                        row.line,
                        CoreTarget::Existing(cable),
                        tag,
                        Some(n),
                    ) {
                        Ok(p) => pending_cores.push(p),
                        Err(reasons) => reject(plan, item_name(project, id, None), reasons),
                    }
                    continue;
                }
                Some(n) => {
                    reject(
                        plan,
                        item_name(project, id, None),
                        vec![format!(
                            "{} has no core {n}; cores are numbered from 1",
                            item_name(project, id, None),
                        )],
                    );
                    continue;
                }
                None => {
                    reject(
                        plan,
                        item_name(project, id, None),
                        vec!["Core No: needs the core's position (1, 2, ...)".into()],
                    );
                    continue;
                }
            }
        } else {
            None
        };
        let item = item_name(project, id, core);
        if let Some(&first) = first_line.get(&(id, core)) {
            reject(
                plan,
                item,
                vec![format!(
                    "this {} is already on row {first}; the first row counts",
                    kind.noun()
                )],
            );
            continue;
        }
        first_line.insert((id, core), row.line);

        let base = by_id.and_then(|raw| base_rows.get(&(raw, core.map_or(0, |i| i + 1))));
        // The key of a core is its position. If the cores were reordered
        // after the export, the position no longer means the same core:
        // a row that edits something is refused rather than applied to
        // the wrong core.
        let reordered = match (core, base, id) {
            (Some(i), Some(b), AnyId::Cable(c)) => project.cable(c).and_then(|cable| {
                let was = b.get("core").map(|c| c.as_text()).unwrap_or_default();
                let here = cable.cores.get(i).map_or("", |c| c.name.as_str());
                (was != here && cable.cores.iter().any(|c| c.name == was)).then(|| {
                    format!(
                        "core {} was '{was}' in the export but the cores of this cable have \
                         been reordered since; export again",
                        i + 1
                    )
                })
            }),
            _ => None,
        };
        let mut errors = Vec::new();
        let mut changes = Vec::new();
        let mut row_claims = Vec::new();
        for (c, field) in columns.iter().enumerate() {
            let Some(field) = field else { continue };
            if !field.editable() {
                continue;
            }
            let cell = cell_at(c);
            let like = match &field.slot {
                Slot::Property(key) => current_value(project, id, key),
                _ => None,
            };
            let mut parsed = match parse(field, &cell, like.as_ref()) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            if let Parsed::Part(Some(part)) = &mut parsed {
                let manufacturer = table
                    .column("Manufacturer")
                    .and_then(|c| row.cells.get(c))
                    .map(|c| c.as_text().trim().to_owned())
                    .unwrap_or_default();
                *part = PartRef::new(catalog, &part.order_number, &manufacturer);
            }
            let now = project_canon(project, id, core, field, like.as_ref());
            let sheet = parsed.canon();
            if sheet == now {
                continue;
            }
            if matches!(parsed, Parsed::Part(_))
                && matches!(id, AnyId::Device(d) if project.device(d).is_some_and(|d| d.parts.len() > 1))
            {
                errors.push(format!(
                    "{}: the device has several parts; change them in the part panel",
                    field.header
                ));
                continue;
            }
            let base_canon = base
                .and_then(|b| b.get(&field.header.to_lowercase()))
                .and_then(|cell| parse(field, cell, like.as_ref()).ok())
                .map(|p| p.canon());
            let conflict = match base_canon {
                Some(b) if b == sheet => continue, // the sheet did not touch it
                Some(b) if b != now => Some(b),    // both changed it
                _ => None,
            };
            let before = match (&field.slot, &like) {
                (Slot::Property(_), Some(v)) => display_value(v),
                _ => read(project, id, core, field).as_text(),
            };
            if field.slot == Slot::CoreWireId
                && let Parsed::Number(Some(n)) = &parsed
            {
                let wire = novo_core::WireId::from(*n as u64);
                match (project.wire(wire), project.core_of_wire(wire), id) {
                    (None, ..) => {
                        errors.push(format!("{}: wire {n} does not exist", field.header));
                        continue;
                    }
                    (_, Some((other, _)), AnyId::Cable(this)) if other != this => {
                        let tag = project.cable(other).map(|c| c.tag.to_string());
                        errors.push(format!(
                            "{}: wire {n} is already on cable {}; free it there first",
                            field.header,
                            tag.unwrap_or_default()
                        ));
                        continue;
                    }
                    _ => {}
                }
                if let AnyId::Cable(this) = id {
                    match claims.get(&wire) {
                        Some(&(other, line)) if other != this => {
                            let tag = project.cable(other).map(|c| c.tag.to_string());
                            errors.push(format!(
                                "{}: wire {n} is already given to cable {} on row {line}; \
                                 a wire sits on one cable",
                                field.header,
                                tag.unwrap_or_default()
                            ));
                            continue;
                        }
                        _ => row_claims.push((wire, this)),
                    }
                }
            }
            changes.push(Change {
                id,
                core,
                table: kind,
                line: row.line,
                item: item.clone(),
                header: field.header.clone(),
                before,
                after: parsed.shown(),
                selected: conflict.is_none(),
                conflict,
                field: field.clone(),
                parsed,
            });
        }
        if !changes.is_empty()
            && let Some(why) = reordered
        {
            errors.push(why);
        }
        if !errors.is_empty() {
            reject(plan, item, errors);
        } else if changes.is_empty() {
            plan.rows_unchanged += 1;
        } else {
            for (wire, cable) in row_claims {
                claims.entry(wire).or_insert((cable, row.line));
            }
            plan.changes.extend(changes);
        }
    }
    if kind == TableKind::Cores {
        place_cores(project, pending_cores, &claims, plan);
    }
    if matched_by_key > 0 {
        plan.notes.push(format!(
            "{}: {matched_by_key} row{} had no Id and {} matched by name",
            kind.name(),
            plural(matched_by_key),
            if matched_by_key == 1 { "was" } else { "were" },
        ));
    }
}

/// A tag two devices would share after the import is worth a warning (the
/// design checks flag it too, but not at the moment of choosing).
fn warn_duplicate_tags(project: &Project, plan: &mut ImportPlan) {
    for (kind, noun) in [(TableKind::Devices, "device"), (TableKind::Cables, "cable")] {
        let mut tags: BTreeMap<AnyId, String> = if kind == TableKind::Devices {
            project
                .devices()
                .map(|d| (d.id.into(), d.tag.to_string()))
                .collect()
        } else {
            project
                .cables()
                .map(|c| (c.id.into(), c.tag.to_string()))
                .collect()
        };
        let mut touched: BTreeSet<String> = BTreeSet::new();
        for change in plan.changes.iter().filter(|c| {
            c.selected
                && c.table == kind
                && matches!(c.field.slot, Slot::DeviceTag | Slot::CableTag)
        }) {
            tags.insert(change.id, change.after.clone());
            touched.insert(change.after.clone());
        }
        for tag in touched {
            let n = tags.values().filter(|t| **t == tag).count();
            if n > 1 {
                plan.warnings
                    .push(format!("{n} {noun}s would have the tag {tag}"));
            }
        }
    }
}

/// A Cores row without an Id that names a cable by its tag (an existing
/// cable or one the sheet adds): a core to add. `None` when it names no
/// cable. `Some(Err)` when it is meant as a core but unusable.
fn core_without_id(
    project: &Project,
    columns: &[Option<Field>],
    cells: &[Cell],
    core_col: Option<usize>,
    line: usize,
    plan: &ImportPlan,
) -> Option<Result<PendingCore, Vec<String>>> {
    let (target, tag) = core_target_by_tag(project, columns, cells, &plan.new_cables)?;
    let cell = core_col
        .and_then(|c| cells.get(c))
        .cloned()
        .unwrap_or_default();
    let explicit = if cell.is_blank() {
        None
    } else {
        match core_no(&cell) {
            Some(n) if n >= 1 => Some(n),
            _ => {
                return Some(Err(vec![
                    "Core No: needs the core's position (1, 2, ...)".into(),
                ]));
            }
        }
    };
    if let (CoreTarget::Existing(id), Some(n)) = (&target, explicit) {
        let count = project.cable(*id).map_or(0, |c| c.cores.len());
        if n <= count {
            return Some(Err(vec![format!(
                "{tag} has a core {n} already; a row that edits a core needs the cable's Id                  (leave Core No empty to add a core)"
            )]));
        }
    }
    Some(pending_core(columns, cells, line, target, tag, explicit))
}

fn core_no(cell: &Cell) -> Option<usize> {
    parse_number(&cell.as_text())
        .filter(|n| *n >= 0.0 && n.fract() == 0.0 && *n < 1e9)
        .map(|n| n as usize)
}

/// Two cores of one cable that would share a wire or a name after the
/// import: the sheet asked for it, but it is almost surely a slip.
fn warn_core_clashes(project: &Project, plan: &mut ImportPlan) {
    let mut cables: BTreeSet<CableId> = BTreeSet::new();
    for change in plan
        .changes
        .iter()
        .filter(|c| c.selected && c.core.is_some())
    {
        if let AnyId::Cable(c) = change.id {
            cables.insert(c);
        }
    }
    for core in plan.new_cores.iter().filter(|c| c.selected) {
        if let CoreTarget::Existing(c) = core.target {
            cables.insert(c);
        }
    }
    for cable in cables {
        let Some(mut record) = project.record(cable.into()) else {
            continue;
        };
        for change in plan
            .changes
            .iter()
            .filter(|c| c.selected && c.id == AnyId::Cable(cable))
        {
            apply(&mut record, change.core, &change.field, &change.parsed);
        }
        let Record::Cable(mut after) = record else {
            continue;
        };
        for core in plan
            .new_cores
            .iter()
            .filter(|c| c.selected && c.target == CoreTarget::Existing(cable))
        {
            super::new_rows::add_core(&mut after, core);
        }
        let mut names: BTreeMap<&str, usize> = BTreeMap::new();
        for core in &after.cores {
            *names.entry(core.name.as_str()).or_default() += 1;
        }
        for (name, n) in names.into_iter().filter(|(_, n)| *n > 1) {
            plan.warnings
                .push(format!("{n} cores of {} would be called {name}", after.tag));
        }
        // A wire asked for by two cores of one cable: the later row wins.
        let mut wanted: BTreeMap<WireId, usize> = BTreeMap::new();
        for change in plan.changes.iter().filter(|c| {
            c.selected && c.id == AnyId::Cable(cable) && c.field.slot == Slot::CoreWireId
        }) {
            if let Parsed::Number(Some(n)) = &change.parsed {
                *wanted.entry(WireId::from(*n as u64)).or_default() += 1;
            }
        }
        for (wire, n) in wanted.into_iter().filter(|(_, n)| *n > 1) {
            plan.warnings.push(format!(
                "{n} cores of {} were given wire {}; only the last one keeps it",
                after.tag,
                wire.raw()
            ));
        }
    }
}

/// A jumper group of one terminal bridges nothing: after the import a group
/// number that only one terminal of a strip carries is probably a slip.
fn warn_lone_jumpers(project: &Project, plan: &mut ImportPlan) {
    let mut jumpers: BTreeMap<TerminalId, Option<u32>> =
        project.terminals().map(|t| (t.id, t.jumper)).collect();
    let mut touched: BTreeSet<TerminalId> = BTreeSet::new();
    for change in plan
        .changes
        .iter()
        .filter(|c| c.selected && c.field.slot == Slot::TerminalJumper)
    {
        if let (AnyId::Terminal(t), Parsed::Number(n)) = (change.id, &change.parsed) {
            jumpers.insert(t, n.map(|n| n as u32));
            touched.insert(t);
        }
    }
    let mut said: BTreeSet<(DeviceId, u32)> = BTreeSet::new();
    for t in touched {
        let (Some(terminal), Some(Some(group))) = (project.terminal(t), jumpers.get(&t)) else {
            continue;
        };
        let strip = terminal.strip;
        let members = project
            .terminals_of_strip(strip)
            .into_iter()
            .filter(|o| jumpers.get(&o.id) == Some(&Some(*group)))
            .count();
        if members == 1 && said.insert((strip, *group)) {
            let tag = project
                .device(strip)
                .map(|d| d.tag.to_string())
                .unwrap_or_default();
            plan.warnings.push(format!(
                "jumper group {group} on {tag} would have only one terminal ({}): it bridges nothing",
                terminal.name
            ));
        }
    }
}
