//! The shared property sheet of a selection: several devices at once.
//!
//! With two or more devices selected the inspector shows one sheet for all
//! of them (`novo_parts::PropertySheet`): a field every device has the same
//! value for shows that value, a field they differ in says "multiple
//! values", and typing a value applies it to every selected device as one
//! undo step. The Part row assigns one catalog part to the whole selection.
//!
//! Where the user finds it: select two or more devices on the drawing and
//! the Inspector opens with the shared sheet; Edit > Properties of the
//! selection (or Ctrl+K "properties") opens the same sheet in a window, for
//! any selection (a single device, terminals, cables).

use std::collections::BTreeMap;

use eframe::egui;
use novo_core::{AnyId, DeviceId};
use novo_parts::{PartKey, PropertySheet, Row, RowKey, RowValue};

use super::inspector::{INSPECTOR_WIDTH, committed};
use super::part_picker::{
    Candidate, PartPicker, assign_devices_command, device_candidates, picker_ui, selected_devices,
};
use super::{Level, NovoApp};

/// Height of the menu bar the card keeps clear of, in points.
const MENU_BAR_HEIGHT: f32 = 48.0;

/// The text shown in a field whose devices differ.
pub(crate) const MIXED_HINT: &str = "multiple values";

/// What the user did in the sheet this frame.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum SheetAction {
    /// A field edited: row and the text typed.
    Set(RowKey, String),
    /// A part chosen for the whole selection.
    AssignPart(PartKey),
    /// A property removed from every selected object.
    Remove(String),
}

/// What the sheet remembers between frames.
#[derive(Debug, Default)]
pub(crate) struct SelectionSheet {
    /// The window (Edit > Properties of the selection) is open.
    pub(super) window: bool,
    pub(super) filter: String,
    /// Text being typed per field, until it is committed.
    edits: BTreeMap<RowKey, String>,
    picker: PartPicker,
    new_name: String,
    new_value: String,
}

/// The text a field shows: what is being typed, else the shared value.
/// A field whose devices differ shows nothing (the hint says so).
pub(super) fn shown_text<'a>(row: &'a Row, typed: Option<&'a String>) -> &'a str {
    match (typed, &row.value) {
        (Some(t), _) => t,
        (None, RowValue::Same(v)) => v,
        (None, RowValue::Mixed(_)) => "",
    }
}

/// The sheet's rows, drawn. Returns nothing: the user's edits are pushed on
/// `actions` and applied by the caller.
pub(super) fn sheet_ui(
    ui: &mut egui::Ui,
    sheet: &PropertySheet,
    state: &mut SelectionSheet,
    candidates: &[Candidate],
    actions: &mut Vec<SheetAction>,
) {
    ui.add(
        egui::TextEdit::singleline(&mut state.filter)
            .hint_text("Search fields")
            .desired_width(f32::INFINITY),
    );
    ui.add_space(4.0);
    let rows = sheet.filter(&state.filter);
    if rows.is_empty() {
        ui.weak("No field matches");
    }
    let mut group = None;
    for row in rows {
        if group != Some(row.group) {
            group = Some(row.group);
            ui.add_space(4.0);
            ui.label(egui::RichText::new(row.group.label()).strong());
        }
        row_ui(ui, row, state, candidates, actions);
    }
    ui.add_space(8.0);
    ui.label(egui::RichText::new("Add a property to all").strong());
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.new_name)
                .hint_text("Name")
                .desired_width(90.0),
        );
        ui.add(
            egui::TextEdit::singleline(&mut state.new_value)
                .hint_text("Value")
                .desired_width(90.0),
        );
        let ready = !state.new_name.trim().is_empty();
        if ui
            .add_enabled(ready, egui::Button::new("Add"))
            .on_hover_text("Set this property on every selected object")
            .clicked()
        {
            let name = std::mem::take(&mut state.new_name);
            let value = std::mem::take(&mut state.new_value);
            actions.push(SheetAction::Set(RowKey::Property(name), value));
        }
    });
}

fn row_ui(
    ui: &mut egui::Ui,
    row: &Row,
    state: &mut SelectionSheet,
    candidates: &[Candidate],
    actions: &mut Vec<SheetAction>,
) {
    if row.key == RowKey::Part {
        part_row_ui(ui, row, state, candidates, actions);
        return;
    }
    ui.horizontal(|ui| {
        let label = match row.unit {
            Some(unit) => format!("{} ({unit})", row.label),
            None => row.label.clone(),
        };
        ui.add_sized([96.0, 18.0], egui::Label::new(label).truncate());
        if !row.editable {
            let text = match &row.value {
                RowValue::Same(v) => v.clone(),
                RowValue::Mixed(_) => MIXED_HINT.to_owned(),
            };
            ui.add(egui::Label::new(egui::RichText::new(text).weak()).truncate());
            return;
        }
        let remove = matches!(row.key, RowKey::Property(_));
        let mut buffer = shown_text(row, state.edits.get(&row.key)).to_owned();
        let response = ui.add(
            egui::TextEdit::singleline(&mut buffer)
                .hint_text(MIXED_HINT)
                .id_salt(("selection_sheet", &row.key))
                .desired_width(if remove { 150.0 } else { f32::INFINITY }),
        );
        if let RowKey::Property(name) = &row.key
            && ui
                .small_button("\u{d7}")
                .on_hover_text("Remove this property from all")
                .clicked()
        {
            actions.push(SheetAction::Remove(name.clone()));
        }
        if response.changed() {
            state.edits.insert(row.key.clone(), buffer);
        }
        if committed(ui, &response) {
            if let Some(text) = state.edits.remove(&row.key) {
                actions.push(SheetAction::Set(row.key.clone(), text));
            }
        } else if !response.has_focus() && !response.changed() {
            // Left without a change, or never touched: show the model again.
            state.edits.remove(&row.key);
        }
    });
}

fn part_row_ui(
    ui: &mut egui::Ui,
    row: &Row,
    state: &mut SelectionSheet,
    candidates: &[Candidate],
    actions: &mut Vec<SheetAction>,
) {
    ui.horizontal(|ui| {
        ui.add_sized([96.0, 18.0], egui::Label::new(row.label.as_str()));
        let text = match &row.value {
            RowValue::Same(v) if v.is_empty() => "none".to_owned(),
            RowValue::Same(v) => v.clone(),
            RowValue::Mixed(_) => MIXED_HINT.to_owned(),
        };
        ui.add(egui::Label::new(egui::RichText::new(text).weak()).truncate());
    });
    let label = if state.picker.open {
        "Close the list"
    } else {
        "Choose a part for all"
    };
    if ui.button(label).clicked() {
        state.picker.toggle();
    }
    if state.picker.open
        && let Some(key) = picker_ui(
            ui,
            &mut state.picker,
            candidates,
            "No part matches. Try the order number.",
        )
    {
        state.picker.close();
        actions.push(SheetAction::AssignPart(key));
    }
}

impl NovoApp {
    /// The command `edit.properties`: the window with the shared sheet.
    pub(super) fn open_selection_sheet(&mut self) {
        let selection = self.selection();
        let any = self.backend.document().is_some_and(|doc| {
            !PropertySheet::build(doc.project(), &selection)
                .targets()
                .is_empty()
        });
        if !any {
            self.message(Level::Info, "Select a device on the drawing first");
            return;
        }
        if self.multi_selected_devices().len() >= 2 {
            self.message(
                Level::Info,
                "The Inspector shows the shared properties of the selected devices",
            );
            return;
        }
        self.wb.selection_sheet.window = true;
    }

    /// The sheet of the current selection, when it has anything to show.
    fn selection_property_sheet(&self) -> Option<PropertySheet> {
        let selection = self.selection();
        let doc = self.backend.document()?;
        let sheet = PropertySheet::build(doc.project(), &selection);
        (!sheet.is_empty()).then_some(sheet)
    }

    /// The part candidates for a selection of devices: what suits the first
    /// device, narrowed by the search.
    fn selection_candidates(&mut self, devices: &[DeviceId]) -> Vec<Candidate> {
        if !self.wb.selection_sheet.picker.open {
            return Vec::new();
        }
        if let Some(problem) = self.wb.part_catalog.ensure() {
            self.message(Level::Error, problem);
        }
        let (Some(catalog), Some(doc), Some(&first)) = (
            self.wb.part_catalog.get(),
            self.backend.document(),
            devices.first(),
        ) else {
            return Vec::new();
        };
        let scope = format!("selection {devices:?}");
        self.wb.selection_sheet.picker.rows(&scope, |query| {
            device_candidates(catalog, doc.project(), first, query)
        })
    }

    /// The inspector while two or more devices are selected. Returns whether
    /// it was drawn.
    pub(super) fn multi_inspector(
        &mut self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        devices: &[DeviceId],
    ) -> bool {
        let Some(sheet) = self.selection_property_sheet() else {
            return false;
        };
        let candidates = self.selection_candidates(devices);
        let mut state = std::mem::take(&mut self.wb.selection_sheet);
        let mut actions = Vec::new();
        let max_height = (canvas.height() - 40.0 - 72.0).max(120.0);
        egui::Area::new(egui::Id::new("inspector"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(canvas.right_top() + egui::vec2(-12.0, 40.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(INSPECTOR_WIDTH);
                    ui.add(
                        egui::Label::new(egui::RichText::new(sheet.title()).heading()).truncate(),
                    );
                    ui.weak("Typing a value sets it on all of them");
                    egui::ScrollArea::vertical()
                        .id_salt("selection_sheet_scroll")
                        .max_height(max_height)
                        .show(ui, |ui| {
                            sheet_ui(ui, &sheet, &mut state, &candidates, &mut actions)
                        });
                });
            });
        self.wb.selection_sheet = state;
        self.apply_sheet_actions(actions);
        true
    }

    /// The window opened by `edit.properties`. For two or more devices the
    /// Inspector shows the same sheet, so the window stays shut then.
    pub(super) fn selection_sheet_window(&mut self, ctx: &egui::Context) {
        if !self.wb.selection_sheet.window {
            return;
        }
        let selection = self.selection();
        let devices = self
            .backend
            .document()
            .map(|d| selected_devices(d.project(), &selection))
            .unwrap_or_default();
        if devices.len() >= 2 {
            // The Inspector draws the sheet for several devices.
            self.wb.selection_sheet.window = false;
            return;
        }
        let Some(sheet) = self.selection_property_sheet() else {
            self.wb.selection_sheet.window = false;
            return;
        };
        let candidates = self.selection_candidates(&devices);
        let mut state = std::mem::take(&mut self.wb.selection_sheet);
        let mut actions = Vec::new();
        let mut open = true;
        // Below the menu bar, inside the window: the default place is the
        // top left corner, which is the menu bar.
        let content = ctx.content_rect();
        let area =
            egui::Rect::from_min_max(content.min + egui::vec2(0.0, MENU_BAR_HEIGHT), content.max);
        egui::Window::new("Properties of the selection")
            .open(&mut open)
            .default_size([340.0, 420.0])
            .default_pos(area.min + egui::vec2(16.0, 16.0))
            .constrain_to(area)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(sheet.title()).strong());
                ui.weak("Typing a value sets it on every selected object");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    sheet_ui(ui, &sheet, &mut state, &candidates, &mut actions)
                });
            });
        state.window = open;
        self.wb.selection_sheet = state;
        self.apply_sheet_actions(actions);
    }

    fn apply_sheet_actions(&mut self, actions: Vec<SheetAction>) {
        for action in actions {
            match action {
                SheetAction::Set(key, text) => self.set_selection_field(&key, &text),
                SheetAction::AssignPart(key) => self.assign_selection_part(&key),
                SheetAction::Remove(name) => self.remove_selection_property(&name),
            }
        }
    }

    /// Sets one field on every selected object: one command, one undo step.
    pub(super) fn set_selection_field(&mut self, key: &RowKey, text: &str) {
        let selection = self.selection();
        let Some(doc) = self.backend.document() else {
            return;
        };
        let sheet = PropertySheet::build(doc.project(), &selection);
        let result = match key {
            // A name typed in "Add a property" has no row yet.
            RowKey::Property(name) if sheet.row(key).is_none() => {
                sheet.add_property(doc.project(), name, text)
            }
            _ => sheet.edit(doc.project(), key, text),
        };
        match result {
            Ok(Some(command)) => {
                if let Some(Err(e)) = self.execute(command) {
                    self.message(Level::Error, format!("Could not change it: {e}"));
                }
            }
            Ok(None) => {}
            Err(e) => self.message(Level::Warning, e.to_string()),
        }
    }

    /// Gives the whole selection one catalog part: one undo step.
    pub(super) fn assign_selection_part(&mut self, key: &PartKey) {
        let selection = self.selection();
        let Some((command, title, count)) = self.backend.document().and_then(|d| {
            let catalog = self.wb.part_catalog.get()?;
            let devices = selected_devices(d.project(), &selection);
            let command = assign_devices_command(d.project(), catalog, &devices, key)?;
            let part = catalog.get(key)?;
            Some((
                command,
                format!("{} {}", part.manufacturer, part.order_number),
                devices.len(),
            ))
        }) else {
            return;
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(Level::Info, format!("Part set on {count}: {title}")),
            Some(Err(e)) => self.message(Level::Error, format!("Could not set the part: {e}")),
            None => {}
        }
    }

    /// Removes a named property from every selected object: one undo step.
    pub(super) fn remove_selection_property(&mut self, name: &str) {
        let selection = self.selection();
        let Some(doc) = self.backend.document() else {
            return;
        };
        let sheet = PropertySheet::build(doc.project(), &selection);
        if let Some(command) = sheet.remove_property(doc.project(), name) {
            self.execute(command);
        }
    }

    /// The devices of the selection when there are several: the inspector
    /// then shows the shared sheet.
    pub(super) fn multi_selected_devices(&self) -> Vec<DeviceId> {
        let selection: Vec<AnyId> = self.selection();
        self.backend
            .document()
            .map(|d| selected_devices(d.project(), &selection))
            .filter(|devices| devices.len() >= 2)
            .unwrap_or_default()
    }
}
