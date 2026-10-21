//! Macros in the drawing: `macro.save` stores the selection in the user's
//! macro library, `macro.place` picks one from it and puts it on the
//! current page with the next free tags (docs/design/macros.md). The
//! engine is `novo_workflow::macro_place` over `novo_parts::MacroLibrary`;
//! the library window with rename, delete, import and export is
//! `macro.browser`.
//!
//! The Place window previews before it changes anything: the tags the
//! devices will take, wire numbers kept or cleared, and clashes. A placed
//! macro lands to the right of what is on the page and is selected, so it
//! can be moved with the usual keys.

use eframe::egui;
use novo_core::{AnyId, BlockDefinition, Value};
use novo_parts::{MacroEntry, MacroFile, MacroLibrary};
use novo_workflow::macro_place::{self, PartRef, PlaceOptions, Report, WireNumbers};

use super::{Level, NovoApp};

/// Gap between the drawing and a placed macro, in mm.
const PLACE_GAP: f64 = 20.0;

/// What the Save macro window holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SaveDraft {
    pub name: String,
    pub description: String,
    pub error: Option<String>,
    /// The name is taken: the window offers to replace that macro.
    pub exists: bool,
    focused: bool,
}

/// What the Place macro preview was computed for, and its result.
struct Shown {
    key: (String, u64, bool, bool, Vec<(String, Value)>),
    definition: BlockDefinition,
    report: Result<Report, String>,
}

/// State of the Place macro window.
pub(crate) struct PlaceWindow {
    pub(super) open: bool,
    library: Option<MacroLibrary>,
    started: bool,
    query: String,
    selected: Option<String>,
    keep_numbers: bool,
    /// The parts the picked macro carries, and whether to assign them.
    parts: Vec<PartRef>,
    carry_parts: bool,
    values: Vec<(String, Value)>,
    shown: Option<Shown>,
    status: String,
}

impl PlaceWindow {
    pub(crate) fn new(library: Option<MacroLibrary>) -> Self {
        PlaceWindow {
            open: false,
            library,
            started: false,
            query: String::new(),
            selected: None,
            keep_numbers: false,
            parts: Vec::new(),
            carry_parts: true,
            values: Vec::new(),
            shown: None,
            status: String::new(),
        }
    }

    fn entries(&self) -> Vec<MacroEntry> {
        self.library
            .as_ref()
            .map(|l| l.search(&self.query))
            .unwrap_or_default()
    }

    fn options(&self) -> PlaceOptions {
        PlaceOptions {
            parts: if self.carry_parts {
                self.parts.clone()
            } else {
                Vec::new()
            },
            values: self.values.clone(),
            wire_numbers: if self.keep_numbers {
                WireNumbers::Keep
            } else {
                WireNumbers::Clear
            },
        }
    }

    /// Picks a macro and loads its parameters with their defaults.
    pub(super) fn select(&mut self, path: &str) {
        self.selected = Some(path.to_owned());
        self.shown = None;
        let file = self.library.as_ref().and_then(|l| l.load(path).ok());
        self.parts = file
            .as_ref()
            .and_then(|f| f.extra.get("parts"))
            .map(macro_place::parts_from_json)
            .unwrap_or_default();
        self.values = file
            .map(|f| {
                f.definition
                    .parameters
                    .iter()
                    .map(|p| (p.name.clone(), p.default.clone()))
                    .collect()
            })
            .unwrap_or_default();
    }
}

impl PlaceWindow {
    /// The preview line of the picked macro, once it is computed.
    #[cfg(test)]
    pub(super) fn preview_text(&self) -> Option<String> {
        match &self.shown.as_ref()?.report {
            Ok(report) => Some(describe(report)),
            Err(reason) => Some(reason.clone()),
        }
    }
}

impl Default for PlaceWindow {
    fn default() -> Self {
        PlaceWindow::new(MacroLibrary::user())
    }
}

/// The preview line: what a placement will do.
pub(crate) fn describe(report: &Report) -> String {
    let tags: Vec<String> = report.tags.iter().map(|t| t.to_string()).collect();
    let mut text = if tags.is_empty() {
        "No devices".to_owned()
    } else {
        format!("Takes {}", tags.join(", "))
    };
    if !report.kept_wire_numbers.is_empty() {
        text += &format!(" · wire numbers {}", report.kept_wire_numbers.join(", "));
    } else if report.cleared_wires > 0 {
        text += &format!(" · {} wires without a number", report.cleared_wires);
    }
    if !report.clashes.is_empty() {
        text += &format!(" · already used: {}", report.clashes.join(", "));
    }
    if report.parts_assigned > 0 {
        let n = report.parts_assigned;
        text += &format!(" · {n} {} assigned", if n == 1 { "part" } else { "parts" });
    }
    text
}

impl NovoApp {
    /// `macro.save`: asks for a name and stores the selection as a macro.
    pub(super) fn open_macro_save(&mut self, name: Option<&str>) {
        let selection = self.selection();
        if selection.is_empty() {
            self.message(Level::Info, "Select the circuit to save as a macro first");
            return;
        }
        let hint = name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| self.suggested_macro_name(&selection));
        self.wb.macro_save = Some(SaveDraft {
            name: hint,
            description: String::new(),
            error: None,
            exists: false,
            focused: false,
        });
    }

    /// The description of the first selected device, or `macro`.
    fn suggested_macro_name(&self, selection: &[AnyId]) -> String {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return "macro".into();
        };
        selection
            .iter()
            .filter_map(|id| match id {
                AnyId::Symbol(s) => project.symbol(*s),
                _ => None,
            })
            .filter_map(|s| match s.role {
                novo_core::SymbolRole::Device { device, .. } => project.device(device),
                _ => None,
            })
            .map(|d| d.description.trim().to_owned())
            .find(|d| !d.is_empty())
            .unwrap_or_else(|| "macro".into())
    }

    pub(super) fn macro_save_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.wb.macro_save.take() else {
            return;
        };
        let mut save = false;
        let mut replace = false;
        let modal = egui::Modal::new(egui::Id::new("macro_save")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(
                egui::RichText::new("Save as macro")
                    .size(15.0)
                    .family(novo_theme::fonts::strong_family()),
            );
            ui.weak("Keeps the selected circuit in your library, to place on any page.");
            ui.add_space(8.0);
            ui.label("Name");
            let text = ui.add(
                egui::TextEdit::singleline(&mut draft.name)
                    .hint_text("Motor starter")
                    .desired_width(f32::INFINITY),
            );
            if !draft.focused {
                text.request_focus();
                draft.focused = true;
            }
            if text.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                save = true;
            }
            ui.add_space(6.0);
            ui.label("Description");
            ui.add(egui::TextEdit::singleline(&mut draft.description).desired_width(f32::INFINITY));
            if let Some(error) = &draft.error {
                ui.add_space(4.0);
                novo_theme::widgets::banner(ui, novo_theme::widgets::Level::Error, error, |_| {});
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled_ui(!draft.name.trim().is_empty(), |ui| {
                        novo_theme::widgets::primary_button(ui, "Save")
                    })
                    .inner
                    .clicked()
                {
                    save = true;
                }
                if draft.exists
                    && ui
                        .button(format!("Replace “{}”", draft.name.trim()))
                        .clicked()
                {
                    replace = true;
                }
                ui.weak("Enter saves · Esc cancels");
            });
        });
        if modal.should_close() {
            return;
        }
        if replace {
            self.submit_macro_replace(draft);
        } else if save {
            self.submit_macro_save(draft);
        } else {
            self.wb.macro_save = Some(draft);
        }
    }

    /// Captures the selection and writes it to the library. On a refusal
    /// the window stays, saying why.
    pub(crate) fn submit_macro_save(&mut self, draft: SaveDraft) {
        self.write_macro(draft, false);
    }

    /// Writes the selection over the macro of the same name; its revision
    /// goes up by one.
    pub(crate) fn submit_macro_replace(&mut self, draft: SaveDraft) {
        self.write_macro(draft, true);
    }

    fn write_macro(&mut self, mut draft: SaveDraft, replace: bool) {
        let name = draft.name.trim().to_owned();
        let result = self.save_selection_as_macro(&name, draft.description.trim(), replace);
        match result {
            Ok(path) => self.message(Level::Info, format!("Saved macro {path}")),
            Err((reason, exists)) => {
                draft.error = Some(reason);
                draft.exists = exists;
                self.wb.macro_save = Some(draft);
            }
        }
    }

    /// Saves, or with `replace` writes over the macro of that name. The
    /// error says whether the name was taken.
    fn save_selection_as_macro(
        &mut self,
        name: &str,
        description: &str,
        replace: bool,
    ) -> Result<String, (String, bool)> {
        let plain = |e: String| (e, false);
        let selection = self.selection();
        let project = self
            .backend
            .document()
            .map(|d| d.project())
            .ok_or_else(|| plain("No project is open".into()))?;
        let definition = macro_place::capture(project, name, description, &selection)
            .map_err(|e| plain(e.to_string()))?;
        let library = self
            .wb
            .macro_place
            .library
            .as_ref()
            .ok_or_else(|| plain("There is no macro folder on this computer".into()))?;
        let parts = macro_place::captured_parts(project, &definition);
        let mut file = MacroFile::new(definition).with_author(self.settings.initials.clone());
        // Carried parts make the file format version 2.
        file.set_parts(
            &parts
                .iter()
                .map(|p| novo_parts::MacroPart {
                    device: p.device.raw(),
                    manufacturer: p.manufacturer.clone(),
                    order_number: p.order_number.clone(),
                    type_number: p.type_number.clone(),
                    description: p.description.clone(),
                })
                .collect::<Vec<_>>(),
        );
        if replace {
            // The file `save` refused is the top-level macro of this name;
            // its path comes from the library, not from the raw name, which
            // `save` cleans (a `/` becomes `_`).
            let (entries, _) = library.list();
            let path = entries
                .into_iter()
                .find(|e| e.folder.is_empty() && e.name == file.name)
                .map(|e| e.path)
                .ok_or_else(|| plain(format!("No macro called {name} to replace")))?;
            library
                .replace(&path, &file)
                .map_err(|e| plain(e.to_string()))?;
            return Ok(path);
        }
        library.save("", &file).map_err(|e| {
            let exists = matches!(e, novo_parts::MacroError::Exists(_));
            let text = if exists {
                format!("A macro called {name} already exists")
            } else {
                e.to_string()
            };
            (text, exists)
        })
    }

    /// `macro.place`: opens the window that picks a macro and previews it.
    pub(super) fn open_macro_place(&mut self, query: Option<&str>) {
        let win = &mut self.wb.macro_place;
        win.status.clear();
        if let Some(query) = query {
            win.query = query.trim().to_owned();
        }
        if !win.started {
            win.started = true;
            if let Some(Err(e)) = win.library.as_ref().map(|l| l.install_starters()) {
                win.status = format!("Starter macros: {e}");
            }
        }
        win.open = true;
    }

    pub(super) fn macro_place_window(&mut self, ctx: &egui::Context) {
        if !self.wb.macro_place.open {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            // Esc clears the search first, then closes the window.
            if self.wb.macro_place.query.is_empty() {
                self.wb.macro_place.open = false;
                return;
            }
            self.wb.macro_place.query.clear();
        }
        let page = self.current_page;
        // Recompute the preview when the pick, the options or the drawing
        // changed; the clone behind it is not paid for on every frame.
        if let (Some(project), Some(page)) = (self.backend.document().map(|d| d.project()), page) {
            let win = &mut self.wb.macro_place;
            if let Some(path) = win.selected.clone() {
                let key = (
                    path.clone(),
                    project.revision(),
                    win.keep_numbers,
                    win.carry_parts,
                    win.values.clone(),
                );
                if win.shown.as_ref().is_none_or(|s| s.key != key) {
                    let loaded = win
                        .library
                        .as_ref()
                        .ok_or_else(|| "No macro folder".to_owned())
                        .and_then(|l| l.definition_for_project(&path).map_err(|e| e.to_string()));
                    win.shown = loaded.ok().map(|definition| {
                        let origin = macro_place::free_origin(project, page, PLACE_GAP);
                        let report = macro_place::preview(
                            project,
                            &definition,
                            page,
                            origin,
                            &win.options(),
                        )
                        .map_err(|e| e.to_string());
                        Shown {
                            key,
                            definition,
                            report,
                        }
                    });
                }
            }
        }
        let mut open = true;
        let mut pick: Option<String> = None;
        let mut place = false;
        let can_place = page.is_some();
        let win = &mut self.wb.macro_place;
        let entries = win.entries();
        let window = super::panel::dialog(ctx, "Place macro", [420.0, 440.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut win.query)
                        .hint_text("Search macros")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(180.0)
                    .show(ui, |ui| {
                        if entries.is_empty() {
                            ui.weak("No macros. Select a circuit and use Save as macro.");
                        }
                        for entry in &entries {
                            let selected = win.selected.as_deref() == Some(entry.path.as_str());
                            let clicked = novo_theme::widgets::row(ui, selected, |ui| {
                                ui.label(
                                    egui::RichText::new(&entry.name)
                                        .family(novo_theme::fonts::strong_family()),
                                );
                                if !entry.description.is_empty() {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(&entry.description).weak(),
                                        )
                                        .truncate(),
                                    );
                                }
                            })
                            .clicked();
                            if clicked {
                                pick = Some(entry.path.clone());
                            }
                        }
                    });
                ui.separator();
                match &win.shown {
                    None if win.selected.is_some() => {
                        ui.weak("This macro could not be read.");
                    }
                    None => {
                        ui.weak("Pick a macro to see what it will do.");
                    }
                    Some(shown) => {
                        for parameter in &shown.definition.parameters {
                            if let Some((_, value)) =
                                win.values.iter_mut().find(|(n, _)| *n == parameter.name)
                            {
                                ui.horizontal(|ui| {
                                    ui.label(&parameter.name);
                                    match value {
                                        Value::Float(v) => {
                                            ui.add(egui::DragValue::new(v).speed(0.1));
                                        }
                                        Value::Int(v) => {
                                            ui.add(egui::DragValue::new(v));
                                        }
                                        Value::Bool(v) => {
                                            ui.checkbox(v, "");
                                        }
                                        Value::Text(v) => {
                                            ui.text_edit_singleline(v);
                                        }
                                    }
                                    if !parameter.description.is_empty() {
                                        ui.weak(&parameter.description);
                                    }
                                });
                            }
                        }
                        ui.checkbox(&mut win.keep_numbers, "Keep the macro's wire numbers");
                        if !win.parts.is_empty() {
                            let n = win.parts.len();
                            let label =
                                format!("Assign its {n} {}", if n == 1 { "part" } else { "parts" });
                            ui.checkbox(&mut win.carry_parts, label);
                        }
                        match &shown.report {
                            Ok(report) => {
                                ui.label(describe(report));
                            }
                            Err(reason) => {
                                novo_theme::widgets::banner(
                                    ui,
                                    novo_theme::widgets::Level::Error,
                                    reason,
                                    |_| {},
                                );
                            }
                        }
                    }
                }
                if !win.status.is_empty() {
                    ui.weak(&win.status);
                }
                ui.add_space(6.0);
                let ready = can_place && win.shown.as_ref().is_some_and(|s| s.report.is_ok());
                if ui
                    .add_enabled_ui(ready, |ui| novo_theme::widgets::primary_button(ui, "Place"))
                    .inner
                    .on_disabled_hover_text("Open a page and pick a macro")
                    .clicked()
                {
                    place = true;
                }
            });
        if let Some(window) = window {
            self.anchor("macro.place", window.response.rect, "Place macro");
        }
        if let Some(path) = pick {
            self.wb.macro_place.select(&path);
        }
        if place {
            self.place_picked_macro();
        }
        if !open {
            self.wb.macro_place.open = false;
        }
    }

    /// Places what the window previewed, as one undo step, and selects it.
    pub(super) fn place_picked_macro(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let Some(shown) = self.wb.macro_place.shown.take() else {
            return;
        };
        let options = self.wb.macro_place.options();
        let name = shown.definition.name.clone();
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let origin = macro_place::free_origin(doc.project(), page, PLACE_GAP);
        let result = doc.transact("Place macro", |tx| {
            macro_place::place_macro(tx, shown.definition, page, origin, &options)
        });
        match result {
            Ok((placed, changes)) => {
                let ids: Vec<AnyId> = doc
                    .project()
                    .block_instance(placed.instance)
                    .map(|i| i.members.iter().map(|m| m.actual).collect())
                    .unwrap_or_default();
                self.changed_outside_canvas(&changes);
                if let Some(view) = self.canvas.schematic() {
                    view.select_ids(&ids);
                }
                self.message(
                    Level::Info,
                    format!("Placed {name} · {}", describe(&placed.report)),
                );
                self.wb.macro_place.open = false;
            }
            Err(e) => {
                self.wb.macro_place.status = format!("Could not place: {e}");
                self.message(Level::Error, format!("Could not place {name}: {e}"));
            }
        }
    }
}
