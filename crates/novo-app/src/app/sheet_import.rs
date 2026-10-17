//! Spreadsheet import (WS-24): File > Import spreadsheet reads an `.xlsx`
//! or CSV file made by the spreadsheet export, lists every row that would
//! change with a tick per row, holds back changes that clash with later
//! edits, lists the rows that cannot be used with their line and reason,
//! and applies the ticked changes as one undo step. Nothing is written to
//! the project until Apply (docs/design/spreadsheets.md).

use std::path::Path;

use eframe::egui;
use novo_interop::sheets::{
    AddedRow, ImportPlan, Resolution, TableKind, Workbook, plan_import_with, read_file,
    rejected_workbook, xlsx_bytes,
};
use novo_parts::Catalog;

use super::{Level, NovoApp};

/// Which rows the dialog lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Filter {
    #[default]
    All,
    /// Rows with a change that can be taken as is.
    Changed,
    /// Devices the sheet adds.
    New,
    /// Rows with a change the project also made since the export.
    Conflicts,
    /// Rows that cannot be used.
    Rejected,
}

impl Filter {
    const ALL: [Filter; 5] = [
        Filter::All,
        Filter::Changed,
        Filter::New,
        Filter::Conflicts,
        Filter::Rejected,
    ];

    fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Changed => "Changed",
            Filter::New => "New",
            Filter::Conflicts => "Conflicts",
            Filter::Rejected => "Rejected",
        }
    }
}

/// The changes of one sheet row, as indices into [`ImportPlan::changes`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RowGroup {
    pub table: TableKind,
    pub line: usize,
    pub item: String,
    pub changes: Vec<usize>,
}

/// Groups the plan's changes by sheet row, in file order.
pub(super) fn row_groups(plan: &ImportPlan) -> Vec<RowGroup> {
    let mut groups: Vec<RowGroup> = Vec::new();
    for (i, change) in plan.changes.iter().enumerate() {
        match groups
            .iter_mut()
            .find(|g| g.table == change.table && g.line == change.line)
        {
            Some(group) => group.changes.push(i),
            None => groups.push(RowGroup {
                table: change.table,
                line: change.line,
                item: change.item.clone(),
                changes: vec![i],
            }),
        }
    }
    groups
}

/// The import preview while it is open.
pub(super) struct SheetImport {
    file_name: String,
    plan: ImportPlan,
    /// The workbook the plan was made from, kept to write the rejected rows
    /// back out.
    book: Workbook,
    groups: Vec<RowGroup>,
    filter: Filter,
}

impl SheetImport {
    pub(super) fn new(file_name: String, plan: ImportPlan, book: Workbook) -> SheetImport {
        let groups = row_groups(&plan);
        SheetImport {
            file_name,
            plan,
            book,
            groups,
            filter: Filter::All,
        }
    }

    /// The row's changes that are not conflicts: what its tick controls.
    fn plain<'a>(&'a self, group: &'a RowGroup) -> impl Iterator<Item = usize> + 'a {
        group
            .changes
            .iter()
            .copied()
            .filter(|&i| self.plan.changes[i].conflict.is_none())
    }

    fn has_plain(&self, group: &RowGroup) -> bool {
        self.plain(group).next().is_some()
    }

    fn has_conflict(&self, group: &RowGroup) -> bool {
        group
            .changes
            .iter()
            .any(|&i| self.plan.changes[i].conflict.is_some())
    }

    /// Whether the row's tick is on: all its plain changes are selected.
    fn ticked(&self, group: &RowGroup) -> bool {
        self.has_plain(group) && self.plain(group).all(|i| self.plan.changes[i].selected)
    }

    fn tick(&mut self, group: usize, value: bool) {
        let indices: Vec<usize> = self.plain(&self.groups[group]).collect();
        for i in indices {
            self.plan.select(i, value);
        }
    }

    fn shows(&self, group: &RowGroup) -> bool {
        match self.filter {
            Filter::All => true,
            Filter::Changed => self.has_plain(group),
            Filter::Conflicts => self.has_conflict(group),
            Filter::New | Filter::Rejected => false,
        }
    }

    fn shows_new(&self) -> bool {
        matches!(self.filter, Filter::All | Filter::New)
    }

    fn counts(&self) -> [usize; 5] {
        let changed = self.groups.iter().filter(|g| self.has_plain(g)).count();
        let conflicts = self.groups.iter().filter(|g| self.has_conflict(g)).count();
        [
            self.groups.len() + self.plan.added_rows().len(),
            changed,
            self.plan.added_rows().len(),
            conflicts,
            self.plan.rejected.len(),
        ]
    }

    /// Ticks or unticks every listed row's plain changes.
    fn set_listed(&mut self, value: bool) {
        for g in 0..self.groups.len() {
            if self.shows(&self.groups[g]) {
                self.tick(g, value);
            }
        }
        if self.shows_new() {
            for i in 0..self.plan.added_rows().len() {
                self.plan.select_added(i, value);
            }
        }
    }
}

impl NovoApp {
    /// File > Import spreadsheet: asks for the file, then opens the preview.
    pub(super) fn import_sheet(&mut self) {
        if !self.is_open() {
            return;
        }
        let picked = crate::dialogs::FileDialog::new()
            .set_title("Import spreadsheet")
            .add_filter("Spreadsheet", &["xlsx", "xlsm", "csv", "tsv", "txt"])
            .pick_file();
        if let Some(path) = picked {
            self.import_sheet_from(&path);
        }
    }

    /// Reads `path` against the open project and opens the preview. A file
    /// that cannot be read, or has nothing to import, is a message, not a
    /// dialog.
    pub fn import_sheet_from(&mut self, path: &Path) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.message(Level::Error, format!("Could not read {name}: {e}"));
                return;
            }
        };
        let book = match read_file(&name, &bytes) {
            Ok(book) => book,
            Err(e) => {
                self.message(Level::Error, format!("Could not import {name}: {e}"));
                return;
            }
        };
        let catalog = Catalog::load_default().unwrap_or_default();
        let plan = plan_import_with(doc.project(), &book, &catalog);
        if plan.changes.is_empty() && plan.added_rows().is_empty() && plan.rejected.is_empty() {
            let text = format!(
                "{name}: nothing to import, {} rows match",
                plan.rows_unchanged
            );
            self.message(Level::Info, text);
            return;
        }
        self.wb.sheet_import = Some(SheetImport::new(name, plan, book));
    }

    /// Writes the rejected rows to `<project> rejected rows.xlsx` next to the
    /// project (a free name, or our own earlier file).
    pub(super) fn save_rejected_rows(
        &mut self,
        plan: &ImportPlan,
        book: &Workbook,
    ) -> Option<std::path::PathBuf> {
        let Some(rejected) = rejected_workbook(plan, book) else {
            self.message(Level::Info, "No rejected rows to save");
            return None;
        };
        let bytes = match xlsx_bytes(&rejected) {
            Ok(bytes) => bytes,
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not save the rejected rows: {e}"),
                );
                return None;
            }
        };
        let (dir, stem) = self.export_base()?;
        let path = super::sheets::ours_or_free(
            &dir,
            &format!("{stem} rejected rows"),
            "xlsx",
            &self.wb.sheet_exports,
        );
        match super::pdf::write_replacing(&path, &bytes) {
            Ok(()) => {
                self.wb
                    .sheet_exports
                    .insert(path.clone(), super::sheets::hash(&bytes));
                self.message(
                    Level::Info,
                    format!(
                        "Saved {} rejected rows: {}",
                        plan.rejected.len(),
                        path.display()
                    ),
                );
                self.wb.last_export = Some(path.clone());
                Some(path)
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not write {}: {e}", path.display()),
                );
                None
            }
        }
    }

    /// The preview as a modal: a tick per row, filters, conflicts, rejected
    /// rows with their line and reason. Enter applies, Esc cancels.
    pub(super) fn sheet_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut import) = self.wb.sheet_import.take() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("sheet_import")).show(ctx, |ui| {
            ui.set_width(600.0);
            ui.heading(format!("Import {}", import.file_name));
            ui.weak(import.plan.summary());
            if !import.plan.has_baseline {
                ui.weak("This file has no baseline, so later edits in the project are not checked");
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let counts = import.counts();
                for (filter, count) in Filter::ALL.into_iter().zip(counts) {
                    let text = format!("{} {count}", filter.label());
                    if ui.selectable_label(import.filter == filter, text).clicked() {
                        import.filter = filter;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button("None").clicked() {
                        import.set_listed(false);
                    }
                    if ui.small_button("All").clicked() {
                        import.set_listed(true);
                    }
                });
            });
            if import.plan.conflicts().next().is_some() {
                ui.horizontal(|ui| {
                    ui.colored_label(ui.visuals().warn_fg_color, "Conflicts:");
                    if ui.small_button("Take the sheet").clicked() {
                        import.plan.resolve(Resolution::Sheet);
                    }
                    if ui.small_button("Keep the project").clicked() {
                        import.plan.resolve(Resolution::Project);
                    }
                });
            }
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for g in 0..import.groups.len() {
                        if import.shows(&import.groups[g]) {
                            row(ui, &mut import, g);
                        }
                    }
                    if import.shows_new() {
                        for i in 0..import.plan.new_devices.len() {
                            new_row(ui, &mut import.plan, i);
                        }
                        // Terminals, cables and cores the sheet adds.
                        let skip = import.plan.new_devices.len();
                        for (k, added) in
                            import.plan.added_rows().into_iter().enumerate().skip(skip)
                        {
                            added_row(ui, &mut import.plan, k, &added);
                        }
                    }
                    if matches!(import.filter, Filter::All | Filter::Rejected) {
                        for r in &import.plan.rejected {
                            ui.horizontal_wrapped(|ui| {
                                ui.colored_label(
                                    ui.visuals().error_fg_color,
                                    format!("{} line {}", r.table.name(), r.line),
                                );
                                if !r.item.is_empty() {
                                    ui.strong(&r.item);
                                }
                                ui.label(r.reasons.join("; "));
                            });
                        }
                    }
                    for text in &import.plan.notes {
                        ui.weak(text);
                    }
                });
            // Warnings stay in view next to Apply: confirm with them read.
            for text in &import.plan.warnings {
                ui.colored_label(ui.visuals().warn_fg_color, format!("Warning: {text}"));
            }
            ui.separator();
            let n = import.plan.selected();
            ui.horizontal(|ui| {
                let label = format!("Apply {n} change{} (Enter)", if n == 1 { "" } else { "s" });
                let apply = ui.add_enabled(n > 0, egui::Button::new(label)).clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                let rejected = import.plan.rejected.len();
                let save = ui
                    .add_enabled(rejected > 0, egui::Button::new("Save rejected rows..."))
                    .on_hover_text(
                        "Write the rows that cannot be used to a workbook, with the reason \
                         in a last column. Fix them there and import that file.",
                    )
                    .clicked();
                (apply, cancel, save)
            })
            .inner
        });
        let (apply, cancel, save) = modal.inner;
        if save {
            self.save_rejected_rows(&import.plan, &import.book);
        }
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if apply || (enter && !import.plan.is_empty()) {
            self.apply_sheet_import(&import.plan);
        } else if cancel || modal.should_close() {
            // Dropped: the project was never touched.
        } else {
            self.wb.sheet_import = Some(import);
        }
    }

    /// Applies the selected changes as one undo step.
    fn apply_sheet_import(&mut self, plan: &ImportPlan) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        match plan.command(doc.project()) {
            Ok(Some(command)) => match self.execute(command) {
                Some(Ok(_)) => {
                    let n = plan.selected();
                    let s = if n == 1 { "" } else { "s" };
                    self.message(
                        Level::Info,
                        format!("Imported {n} change{s} from spreadsheet"),
                    );
                }
                Some(Err(e)) => {
                    self.message(Level::Error, format!("Could not import spreadsheet: {e}"))
                }
                None => {}
            },
            Ok(None) => self.message(Level::Info, "Nothing to import"),
            Err(e) => self.message(Level::Error, format!("Could not import spreadsheet: {e}")),
        }
    }
}

/// One row: its tick, where it is and what it is, then each cell change.
fn row(ui: &mut egui::Ui, import: &mut SheetImport, g: usize) {
    let group = import.groups[g].clone();
    ui.horizontal_wrapped(|ui| {
        let mut ticked = import.ticked(&group);
        let plain = import.has_plain(&group);
        if ui
            .add_enabled(plain, egui::Checkbox::new(&mut ticked, ""))
            .changed()
        {
            import.tick(g, ticked);
        }
        ui.weak(format!("{} line {}", group.table.name(), group.line));
        ui.strong(&group.item);
    });
    ui.indent(("sheet_row", g), |ui| {
        for &i in &group.changes {
            let (header, before, after, conflict, mut selected) = {
                let c = &import.plan.changes[i];
                (
                    c.header.clone(),
                    c.before.clone(),
                    c.after.clone(),
                    c.conflict.clone(),
                    c.selected,
                )
            };
            let shown = |v: &str| {
                if v.is_empty() {
                    "(empty)".to_owned()
                } else {
                    v.to_owned()
                }
            };
            let text = format!("{header}: {} → {}", shown(&before), shown(&after));
            match conflict {
                None => {
                    ui.label(text);
                }
                Some(exported) => {
                    ui.horizontal_wrapped(|ui| {
                        if ui.checkbox(&mut selected, text).changed() {
                            import.plan.select(i, selected);
                        }
                        ui.colored_label(
                            ui.visuals().warn_fg_color,
                            format!(
                                "also changed in the project since the export (was {})",
                                shown(&exported)
                            ),
                        );
                    });
                }
            }
        }
    });
    ui.add_space(2.0);
}

/// One terminal, cable or core the sheet adds: its tick, line and what it is.
fn added_row(ui: &mut egui::Ui, plan: &mut ImportPlan, index: usize, added: &AddedRow) {
    let mut selected = added.selected;
    ui.horizontal_wrapped(|ui| {
        if ui.checkbox(&mut selected, "").changed() {
            plan.select_added(index, selected);
        }
        ui.weak(format!("{} line {}", added.table.name(), added.line));
        ui.strong(format!("new {}", added.label));
        if let Some(parent) = &added.left_out_with {
            ui.weak(format!("(left out: {parent} is not added)"));
        }
    });
}

/// One device the sheet adds: its tick, line, tag and the values it sets.
fn new_row(ui: &mut egui::Ui, plan: &mut ImportPlan, i: usize) {
    let (line, tag, description, kind, order, properties, mut selected) = {
        let n = &plan.new_devices[i];
        (
            n.line,
            n.tag.to_string(),
            n.description.clone(),
            format!("{:?}", n.kind),
            n.order_number().map(str::to_owned),
            n.properties
                .iter()
                .map(|(k, v)| format!("{k}: {}", novo_parts::display_value(v)))
                .collect::<Vec<_>>(),
            n.selected,
        )
    };
    ui.horizontal_wrapped(|ui| {
        if ui.checkbox(&mut selected, "").changed() {
            plan.select_new(i, selected);
        }
        ui.weak(format!("Devices line {line}"));
        ui.strong(format!("new {tag}"));
        ui.label(kind);
    });
    ui.indent(("sheet_new", i), |ui| {
        if !description.is_empty() {
            ui.label(format!("Description: {description}"));
        }
        if let Some(order) = order {
            ui.label(format!("Order number: {order}"));
        }
        for p in properties {
            ui.label(p);
        }
    });
    ui.add_space(2.0);
}

#[cfg(test)]
mod tests {
    use novo_core::{Command, DeviceTag};
    use novo_interop::sheets::{Cell, export_project, xlsx_bytes};

    use super::super::gate1_tests::Drive;
    use super::*;
    use crate::actions::Action;

    fn tag(text: &str) -> DeviceTag {
        DeviceTag::parse(text).unwrap()
    }

    /// An open project with devices -K1 and -Q1, and the workbook file
    /// exported from it with `Description` of the first row set to `text`.
    fn drive_with_sheet(dir: &Path, text: &str) -> (Drive, std::path::PathBuf) {
        let mut d = Drive::new(&dir.join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        for t in ["-K1", "-Q1"] {
            d.app
                .execute(Command::AddDevice {
                    tag: tag(t),
                    description: "old".into(),
                    kind: Default::default(),
                })
                .unwrap()
                .unwrap();
        }
        let mut book = export_project(d.app.document().unwrap().project());
        let col = book.tables[0].column("Description").unwrap();
        book.tables[0].rows[0].cells[col] = Cell::text(text);
        let path = dir.join("devices.xlsx");
        std::fs::write(&path, xlsx_bytes(&book).unwrap()).unwrap();
        (d, path)
    }

    fn description_of(d: &Drive, t: &str) -> String {
        let doc = d.app.document().unwrap();
        let device = doc.project().devices().find(|x| x.tag == tag(t)).unwrap();
        device.description.clone()
    }

    #[test]
    fn the_preview_lists_the_row_and_apply_is_one_undo_step() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, path) = drive_with_sheet(dir.path(), "new text");
        d.app.import_sheet_from(&path);
        d.frame();
        let import = d.app.wb.sheet_import.as_ref().expect("the preview opens");
        assert_eq!(import.groups.len(), 1);
        assert_eq!(import.counts(), [1, 1, 0, 0, 0]);
        assert!(import.ticked(&import.groups[0]));
        assert!(d.app.modal_open());
        // Nothing changed before Apply.
        assert_eq!(description_of(&d, "-K1"), "old");

        let depth = d.app.document().unwrap().undo_depth();
        let import = d.app.wb.sheet_import.take().unwrap();
        d.app.apply_sheet_import(&import.plan);
        assert_eq!(description_of(&d, "-K1"), "new text");
        assert_eq!(d.app.document().unwrap().undo_depth(), depth + 1);
        d.app.run(&d.ctx.clone(), Action::Undo);
        assert_eq!(description_of(&d, "-K1"), "old");
    }

    #[test]
    fn an_unticked_row_is_not_applied() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, path) = drive_with_sheet(dir.path(), "new text");
        d.app.import_sheet_from(&path);
        let mut import = d.app.wb.sheet_import.take().unwrap();
        import.tick(0, false);
        assert!(!import.ticked(&import.groups[0]));
        assert!(import.plan.is_empty());
        d.app.apply_sheet_import(&import.plan);
        assert_eq!(description_of(&d, "-K1"), "old");
    }

    #[test]
    fn a_project_edit_after_the_export_is_a_conflict_that_starts_unticked() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, path) = drive_with_sheet(dir.path(), "from the sheet");
        let k1 = d
            .app
            .document()
            .unwrap()
            .project()
            .devices()
            .find(|x| x.tag == tag("-K1"))
            .unwrap()
            .id;
        d.app
            .backend
            .document_mut()
            .unwrap()
            .transact("Edit", |tx| {
                tx.update::<novo_core::Device>(k1, |x| x.description = "in project".into())
            })
            .unwrap();
        d.app.import_sheet_from(&path);
        let import = d.app.wb.sheet_import.as_ref().unwrap();
        assert_eq!(import.counts(), [1, 0, 0, 1, 0]);
        assert_eq!(import.plan.selected(), 0);
    }

    #[test]
    fn a_row_without_an_id_is_a_new_device_with_its_own_tick() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, _) = drive_with_sheet(dir.path(), "x");
        let mut book = export_project(d.app.document().unwrap().project());
        let table = &mut book.tables[0];
        let mut cells: Vec<Cell> = table.headers.iter().map(|_| Cell::text("")).collect();
        cells[table.column("Tag").unwrap()] = Cell::text("-M1");
        cells[table.column("Description").unwrap()] = Cell::text("Pump motor");
        table.push(cells);
        let path = dir.path().join("new.xlsx");
        std::fs::write(&path, xlsx_bytes(&book).unwrap()).unwrap();
        d.app.import_sheet_from(&path);
        let import = d.app.wb.sheet_import.as_mut().expect("the preview opens");
        assert_eq!(import.counts()[2], 1, "one new device");
        assert!(import.plan.new_devices[0].selected);
        import.filter = Filter::New;
        import.set_listed(false);
        assert!(import.plan.is_empty());
        import.set_listed(true);
        let import = d.app.wb.sheet_import.take().unwrap();
        d.app.apply_sheet_import(&import.plan);
        assert_eq!(description_of(&d, "-M1"), "Pump motor");
        d.app.run(&d.ctx.clone(), Action::Undo);
        assert!(
            d.app
                .document()
                .unwrap()
                .project()
                .devices()
                .all(|x| x.tag != tag("-M1"))
        );
    }

    #[test]
    fn a_new_cable_row_is_listed_counted_and_can_be_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, _) = drive_with_sheet(dir.path(), "x");
        let mut book = export_project(d.app.document().unwrap().project());
        let mut cables = novo_interop::sheets::Table::new(
            TableKind::Cables,
            vec!["Id".into(), "Tag".into(), "Description".into()],
        );
        cables.push(vec![
            Cell::text(""),
            Cell::text("-W9"),
            Cell::text("New cable"),
        ]);
        book.tables.push(cables);
        let path = dir.path().join("cable.xlsx");
        std::fs::write(&path, xlsx_bytes(&book).unwrap()).unwrap();
        d.app.import_sheet_from(&path);
        let import = d.app.wb.sheet_import.as_mut().expect("the preview opens");
        assert_eq!(import.counts()[2], 1, "one added row");
        assert_eq!(import.plan.selected(), 1);
        import.filter = Filter::New;
        import.set_listed(false);
        assert!(import.plan.is_empty());
        import.set_listed(true);
        let import = d.app.wb.sheet_import.take().unwrap();
        d.app.apply_sheet_import(&import.plan);
        assert!(
            d.app
                .document()
                .unwrap()
                .project()
                .cables()
                .any(|c| c.tag == tag("-W9"))
        );
    }

    #[test]
    fn a_missing_or_unknown_file_is_a_message_not_a_dialog() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, _) = drive_with_sheet(dir.path(), "x");
        d.app.import_sheet_from(&dir.path().join("nope.xlsx"));
        d.app.import_sheet_from(&dir.path().join("notes.docx"));
        assert!(d.app.wb.sheet_import.is_none());
        assert!(
            d.app
                .messages()
                .iter()
                .any(|m| m.text.contains("Could not"))
        );
    }

    #[test]
    fn the_rejected_rows_can_be_saved_as_a_workbook() {
        let dir = tempfile::tempdir().unwrap();
        let (mut d, _) = drive_with_sheet(dir.path(), "x");
        assert!(d.app.save_to(&dir.path().join("Plant.novo")));
        let mut book = export_project(d.app.document().unwrap().project());
        let table = &mut book.tables[0];
        let id = table.column("Id").unwrap();
        table.rows[0].cells[id] = Cell::text("not an id");
        let path = dir.path().join("bad.xlsx");
        std::fs::write(&path, xlsx_bytes(&book).unwrap()).unwrap();
        d.app.import_sheet_from(&path);
        let import = d.app.wb.sheet_import.take().expect("the preview opens");
        assert_eq!(import.plan.rejected.len(), 1);
        let saved = d
            .app
            .save_rejected_rows(&import.plan, &import.book)
            .expect("saved");
        assert_eq!(saved, dir.path().join("Plant rejected rows.xlsx"));
        let back = read_file("r.xlsx", &std::fs::read(&saved).unwrap()).unwrap();
        let t = &back.tables[0];
        assert_eq!(t.rows.len(), 1);
        assert_eq!(t.headers.last().unwrap(), novo_interop::sheets::WHY_HEADER);
        // The dialog draws the button, enabled for a plan with rejects.
        d.app.wb.sheet_import = Some(import);
        d.frame();
        assert!(d.app.wb.sheet_import.is_some());
    }
}
