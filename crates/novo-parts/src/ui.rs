//! The egui properties panel and bulk editor (WORKFLOW.md J12).
//!
//! The app shell owns the window and the document; these widgets only read
//! the project and return the [`Command`] the user asked for, which the app
//! runs with `Document::execute` (so every edit is one undo step and goes
//! through the same door as scripts):
//!
//! ```no_run
//! # fn frame(ui: &mut egui::Ui, doc: &mut novo_core::Document,
//! #          selection: &[novo_core::AnyId], catalog: &novo_parts::Catalog,
//! #          panel: &mut novo_parts::ui::PropertiesPanel) {
//! if let Some(command) = panel.show(ui, doc.project(), selection, catalog) {
//!     let _ = doc.execute(command);
//! }
//! # }
//! ```
//!
//! Ctrl+2 calls [`PropertiesPanel::focus_filter`]: type part of a property
//! name, Enter jumps to the first editable match, type the value, Enter
//! commits. Escape leaves a field without changing it.

use egui::{Color32, Id, Key, RichText, Ui};
use novo_core::{AnyId, Command, Project};

use crate::assign::{AssignMode, assign_cable_part, assign_part};
use crate::catalog::{Catalog, CatalogPart};
use crate::sheet::{EditError, Group, PropertySheet, Row, RowKey, RowValue};
use crate::table::{BulkTable, TableColumn, apply_table, set_cell};

const ERROR: Color32 = Color32::from_rgb(0xE0, 0x5A, 0x4F);

/// A field being typed into.
#[derive(Debug, Clone, PartialEq)]
struct Editing {
    key: RowKey,
    text: String,
}

/// State of the properties panel. Keep one per window.
#[derive(Debug, Default)]
pub struct PropertiesPanel {
    filter: String,
    focus_filter: bool,
    focus_row: Option<RowKey>,
    editing: Option<Editing>,
    error: Option<(RowKey, String)>,
    part_query: String,
    new_name: String,
    new_value: String,
    cache: Option<(u64, Vec<AnyId>, PropertySheet)>,
}

impl PropertiesPanel {
    pub fn new() -> Self {
        Self::default()
    }

    /// Puts the keyboard in the filter box (Ctrl+2).
    pub fn focus_filter(&mut self) {
        self.focus_filter = true;
    }

    /// The current filter text.
    pub fn filter(&self) -> &str {
        &self.filter
    }

    fn sheet(&mut self, project: &Project, selection: &[AnyId]) -> &PropertySheet {
        let fresh = matches!(
            &self.cache,
            Some((rev, sel, _)) if *rev == project.revision() && sel == selection
        );
        if !fresh {
            let sheet = PropertySheet::build(project, selection);
            // A new selection drops edits and errors that belonged to the old one.
            if self
                .cache
                .as_ref()
                .is_none_or(|(_, sel, _)| sel != selection)
            {
                self.editing = None;
                self.error = None;
                self.part_query.clear();
            }
            self.cache = Some((project.revision(), selection.to_vec(), sheet));
        }
        &self.cache.as_ref().expect("cache set above").2
    }

    /// Draws the panel for `selection` and returns the edit the user made
    /// this frame, if any.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        selection: &[AnyId],
        catalog: &Catalog,
    ) -> Option<Command> {
        let sheet = self.sheet(project, selection).clone();

        ui.horizontal(|ui| {
            ui.strong("Properties");
            if !sheet.is_empty() {
                ui.label(RichText::new(sheet.title()).monospace());
            }
        });
        let filter = ui.add(
            egui::TextEdit::singleline(&mut self.filter)
                .hint_text("Find a property (Ctrl+2)")
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut self.focus_filter) {
            filter.request_focus();
        }
        if filter.lost_focus()
            && ui.input(|i| i.key_pressed(Key::Enter))
            && let Some(row) = sheet.best_match(&self.filter)
        {
            self.focus_row = Some(row.key.clone());
        }
        if filter.lost_focus() && ui.input(|i| i.key_pressed(Key::Escape)) {
            self.filter.clear();
        }
        ui.separator();

        if sheet.is_empty() {
            ui.weak("Select something to see its properties.");
            return None;
        }

        egui::ScrollArea::vertical()
            .id_salt("novo-properties")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let mut out = None;
                if self.filter.trim().is_empty() {
                    for group in Group::ALL {
                        let rows: Vec<&Row> =
                            sheet.rows().iter().filter(|r| r.group == group).collect();
                        if rows.is_empty() && group != Group::Properties {
                            continue;
                        }
                        let body = |ui: &mut Ui, this: &mut Self| {
                            let mut out = None;
                            for row in &rows {
                                out = out.or(this.row(ui, project, &sheet, catalog, row));
                            }
                            if group == Group::Properties {
                                out = out.or(this.add_property_row(ui, project, &sheet));
                            }
                            out
                        };
                        let result = if group == Group::Common {
                            body(ui, self)
                        } else {
                            egui::CollapsingHeader::new(group.label())
                                .id_salt(("novo-properties-group", group.label()))
                                .default_open(matches!(group, Group::Part | Group::Properties))
                                .show(ui, |ui| body(ui, self))
                                .body_returned
                                .flatten()
                        };
                        out = out.or(result);
                    }
                } else {
                    let rows = sheet.filter(&self.filter);
                    if rows.is_empty() {
                        ui.weak(format!("No property matches \"{}\".", self.filter.trim()));
                    }
                    for row in rows {
                        out = out.or(self.row(ui, project, &sheet, catalog, row));
                    }
                }
                out
            })
            .inner
    }

    fn row(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        sheet: &PropertySheet,
        catalog: &Catalog,
        row: &Row,
    ) -> Option<Command> {
        let mut out = None;
        ui.horizontal(|ui| {
            let label = ui.add_sized(
                [ui.available_width() * 0.4, 18.0],
                egui::Label::new(&row.label).truncate(),
            );
            if !self.filter.trim().is_empty() && row.group != Group::Common {
                label.on_hover_text(row.group.label());
            }
            if row.key == RowKey::Part {
                out = self.part_row(ui, project, sheet, catalog, row);
            } else if !row.editable {
                let text = match &row.value {
                    RowValue::Same(v) => v.clone(),
                    mixed => mixed.to_string(),
                };
                ui.label(with_unit(&text, row.unit));
            } else if !row.choices.is_empty() {
                out = self.choice_row(ui, project, sheet, row);
            } else {
                out = self.text_row(ui, project, sheet, row);
            }
            if let RowKey::Property(name) = &row.key
                && ui
                    .small_button("×")
                    .on_hover_text("Remove this property")
                    .clicked()
            {
                out = sheet.remove_property(project, name);
            }
        });
        if let Some((key, message)) = &self.error
            && *key == row.key
        {
            ui.colored_label(ERROR, message);
        }
        out
    }

    fn text_row(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        sheet: &PropertySheet,
        row: &Row,
    ) -> Option<Command> {
        let id = Id::new(("novo-property", row.key.to_string()));
        let editing = self.editing.as_ref().filter(|e| e.key == row.key).cloned();
        let mut text = match &editing {
            Some(e) => e.text.clone(),
            None => row.value.same().unwrap_or_default().to_owned(),
        };
        let hint = match &row.value {
            RowValue::Mixed(_) => row.value.to_string(),
            RowValue::Same(_) => row.unit.unwrap_or_default().to_owned(),
        };
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(id)
                .hint_text(hint)
                .desired_width(ui.available_width() - 24.0),
        );
        if self.focus_row.as_ref() == Some(&row.key) {
            self.focus_row = None;
            response.request_focus();
            // Jumping to a field selects its value, so typing replaces it.
            select_all(ui, id, &text);
        }
        if response.gained_focus() || (response.has_focus() && editing.is_none()) {
            self.editing = Some(Editing {
                key: row.key.clone(),
                text: text.clone(),
            });
        }
        if response.changed()
            && let Some(e) = &mut self.editing
            && e.key == row.key
        {
            e.text = text.clone();
            // Refuse a bad value while typing (J4), before anything changes.
            self.error = match sheet.validate(project, &row.key, &text) {
                Err(err) => Some((row.key.clone(), err.to_string())),
                Ok(()) => None,
            };
        }
        if let Some(unit) = row.unit {
            ui.weak(unit);
        }
        if response.lost_focus() && self.editing.as_ref().is_some_and(|e| e.key == row.key) {
            let cancelled = ui.input(|i| i.key_pressed(Key::Escape));
            self.editing = None;
            if cancelled {
                self.error = None;
                return None;
            }
            return self.commit(sheet.edit(project, &row.key, &text), &row.key);
        }
        None
    }

    fn choice_row(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        sheet: &PropertySheet,
        row: &Row,
    ) -> Option<Command> {
        let current = row.value.same().unwrap_or_default().to_owned();
        let mut picked = None;
        egui::ComboBox::from_id_salt(("novo-property-choice", row.key.to_string()))
            .selected_text(match &row.value {
                RowValue::Same(v) => humanize_choice(v),
                mixed => mixed.to_string(),
            })
            .width(ui.available_width() - 24.0)
            .show_ui(ui, |ui| {
                for &choice in row.choices {
                    if ui
                        .selectable_label(choice == current, humanize_choice(choice))
                        .clicked()
                    {
                        picked = Some(choice);
                    }
                }
            });
        let choice = picked.filter(|&c| c != current)?;
        self.commit(sheet.edit(project, &row.key, choice), &row.key)
    }

    fn part_row(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        sheet: &PropertySheet,
        catalog: &Catalog,
        row: &Row,
    ) -> Option<Command> {
        let mut out = None;
        ui.vertical(|ui| {
            match &row.value {
                RowValue::Same(v) if v.is_empty() => {
                    ui.colored_label(ERROR, "No part yet");
                }
                value => {
                    ui.label(value.to_string());
                }
            }
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.part_query)
                    .id(Id::new("novo-property-part-search"))
                    .hint_text("Search the catalog: order no, type, mpcb, 24vdc…")
                    .desired_width(ui.available_width() - 24.0),
            );
            if self.focus_row.as_ref() == Some(&RowKey::Part) {
                self.focus_row = None;
                response.request_focus();
            }
            let hits = part_hits(catalog, project, sheet, &self.part_query);
            let enter = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let mut chosen: Option<&CatalogPart> = None;
            if enter && !self.part_query.trim().is_empty() {
                chosen = hits.first().copied();
            }
            if response.has_focus() || !self.part_query.trim().is_empty() {
                for part in &hits {
                    let text = format!("{part}  {}", part.description);
                    if ui
                        .selectable_label(false, RichText::new(text).small())
                        .clicked()
                    {
                        chosen = Some(part);
                    }
                }
                if hits.is_empty() && !self.part_query.trim().is_empty() {
                    ui.weak("No catalog part matches.");
                }
            }
            if let Some(part) = chosen {
                out = assign_to_targets(project, sheet, part);
                self.part_query.clear();
            }
        });
        out
    }

    fn add_property_row(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        sheet: &PropertySheet,
    ) -> Option<Command> {
        let key = RowKey::Property(String::new());
        let mut out = None;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 40.0) / 2.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.new_name)
                    .hint_text("New property")
                    .desired_width(width),
            );
            let value = ui.add(
                egui::TextEdit::singleline(&mut self.new_value)
                    .hint_text("Value")
                    .desired_width(width),
            );
            let enter = value.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if (ui.small_button("Add").clicked() || enter) && !self.new_name.trim().is_empty() {
                out = self.commit(
                    sheet.add_property(project, &self.new_name, &self.new_value),
                    &key,
                );
                if out.is_some() {
                    self.new_name.clear();
                    self.new_value.clear();
                }
            }
        });
        if let Some((k, message)) = &self.error
            && *k == key
        {
            ui.colored_label(ERROR, message);
        }
        out
    }

    fn commit(
        &mut self,
        result: Result<Option<Command>, EditError>,
        key: &RowKey,
    ) -> Option<Command> {
        match result {
            Ok(command) => {
                self.error = None;
                command
            }
            Err(err) => {
                self.error = Some((key.clone(), err.to_string()));
                None
            }
        }
    }
}

fn select_all(ui: &Ui, id: Id, text: &str) {
    use egui::text::{CCursor, CCursorRange};
    let mut state = egui::text_edit::TextEditState::load(ui.ctx(), id).unwrap_or_default();
    let range = CCursorRange::two(CCursor::new(0), CCursor::new(text.chars().count()));
    state.cursor.set_char_range(Some(range));
    state.store(ui.ctx(), id);
}

fn with_unit(text: &str, unit: Option<&str>) -> String {
    match unit {
        Some(unit) if !text.is_empty() => format!("{text} {unit}"),
        _ => text.to_owned(),
    }
}

fn humanize_choice(choice: &str) -> String {
    let words = choice.replace('_', " ");
    let mut chars = words.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Catalog parts for the part search: suggestions for the first target's
/// letter code, or a plain search when the targets are not devices.
fn part_hits<'c>(
    catalog: &'c Catalog,
    project: &Project,
    sheet: &PropertySheet,
    query: &str,
) -> Vec<&'c CatalogPart> {
    const LIMIT: usize = 8;
    match sheet.targets().first() {
        Some(AnyId::Device(device)) => {
            if query.trim().is_empty() {
                crate::assign::suggest_for_device(catalog, project, *device, "", LIMIT)
            } else {
                // Typing searches the whole catalog, so a part for another
                // letter code can still be picked.
                let mut hits =
                    crate::assign::suggest_for_device(catalog, project, *device, query, LIMIT);
                for extra in catalog.search(query, LIMIT) {
                    if hits.len() < LIMIT && !hits.contains(&extra) {
                        hits.push(extra);
                    }
                }
                hits
            }
        }
        Some(AnyId::Cable(_)) => catalog.suggest("W", query, LIMIT),
        _ => catalog.search(query, LIMIT),
    }
}

fn assign_to_targets(
    project: &Project,
    sheet: &PropertySheet,
    part: &CatalogPart,
) -> Option<Command> {
    let devices: Vec<_> = sheet
        .targets()
        .iter()
        .filter_map(|id| match id {
            AnyId::Device(d) => Some(*d),
            _ => None,
        })
        .collect();
    if !devices.is_empty() {
        return assign_part(project, &devices, part, AssignMode::Replace);
    }
    let cables: Vec<Command> = sheet
        .targets()
        .iter()
        .filter_map(|id| match id {
            AnyId::Cable(c) => assign_cable_part(project, *c, part),
            _ => None,
        })
        .collect();
    match cables.len() {
        0 => None,
        1 => cables.into_iter().next(),
        n => Some(Command::Batch {
            label: format!("Assign {part} to {n} cables"),
            commands: cables,
        }),
    }
}

/// The spreadsheet-style bulk editor: every device as a row, copy to Excel,
/// paste back, or type into cells.
#[derive(Debug, Default)]
pub struct BulkEditor {
    filter: String,
    editing: Option<(usize, usize, String)>,
    message: Option<(bool, String)>,
}

impl BulkEditor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Draws the editor and returns the edit made this frame, if any. Ctrl+V
    /// while no cell is being edited applies the clipboard as a table.
    pub fn show(&mut self, ui: &mut Ui, project: &Project, catalog: &Catalog) -> Option<Command> {
        let mut table = BulkTable::devices(project, None);
        let mut out = None;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.filter)
                    .hint_text("Filter rows")
                    .desired_width(160.0),
            );
            if ui
                .button("Copy table")
                .on_hover_text("Copy every device as a table for Excel")
                .clicked()
            {
                ui.ctx().copy_text(table.to_tsv());
                self.message = Some((false, format!("Copied {} rows.", table.rows.len())));
            }
            ui.weak("Paste a table from Excel with Ctrl+V to apply it.");
        });
        if let Some((error, message)) = &self.message {
            if *error {
                ui.colored_label(ERROR, message);
            } else {
                ui.weak(message);
            }
        }

        let typing = ui.memory(|m| m.focused()).is_some();
        let pasted = ui.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Paste(text) => Some(text.clone()),
                _ => None,
            })
        });
        if let Some(text) = pasted.filter(|_| !typing && ui.ui_contains_pointer()) {
            match apply_table(project, catalog, &text) {
                Ok(outcome) => {
                    let mut message = format!("Changed {} devices.", outcome.changed);
                    for (line, why) in outcome.skipped.iter().take(3) {
                        message.push_str(&format!(" Line {line}: {why}."));
                    }
                    self.message = Some((false, message));
                    out = outcome.command;
                }
                Err(err) => self.message = Some((true, err.to_string())),
            }
        }

        let matcher = crate::search::Matcher::new(&self.filter);
        table
            .rows
            .retain(|r| matcher.matches(r.cells.iter().map(String::as_str)));
        egui::ScrollArea::both()
            .id_salt("novo-bulk-table")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("novo-bulk-grid")
                    .striped(true)
                    .num_columns(table.columns.len())
                    .show(ui, |ui| {
                        for column in &table.columns {
                            ui.strong(column.label());
                        }
                        ui.end_row();
                        for (r, row) in table.rows.iter().enumerate() {
                            for (c, column) in table.columns.iter().enumerate() {
                                if column == &TableColumn::Tag {
                                    ui.monospace(&row.cells[c]);
                                    continue;
                                }
                                if let Some(command) =
                                    self.cell(ui, project, catalog, row, r, c, column)
                                {
                                    out = Some(command);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        out
    }

    #[allow(clippy::too_many_arguments)]
    fn cell(
        &mut self,
        ui: &mut Ui,
        project: &Project,
        catalog: &Catalog,
        row: &crate::table::BulkRow,
        r: usize,
        c: usize,
        column: &TableColumn,
    ) -> Option<Command> {
        let editing = matches!(&self.editing, Some((er, ec, _)) if *er == r && *ec == c);
        let mut text = match &self.editing {
            Some((_, _, t)) if editing => t.clone(),
            _ => row.cells[c].clone(),
        };
        let response = ui.add(
            egui::TextEdit::singleline(&mut text)
                .id(Id::new(("novo-bulk-cell", row.device, c)))
                .desired_width(120.0),
        );
        if response.gained_focus() || (response.changed() && editing) {
            self.editing = Some((r, c, text.clone()));
        }
        if response.lost_focus() && editing {
            self.editing = None;
            if ui.input(|i| i.key_pressed(Key::Escape)) {
                return None;
            }
            match set_cell(project, catalog, row.device, column, &text) {
                Ok(command) => return command,
                Err(err) => self.message = Some((true, err.to_string())),
            }
        }
        None
    }
}
