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
use novo_core::{AnyId, BlockDefinition, Record, Value};
use novo_parts::{MacroEntry, MacroFile, MacroLibrary};
use novo_workflow::macro_place::{self, PartRef, PlaceOptions, Report, WireNumbers};

use super::{Level, NovoApp, count_words, supply_feeds};

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
    /// What the command field searches, read again every few seconds.
    palette: (
        Option<std::time::Instant>,
        Vec<novo_workflow::palette::MacroEntry>,
    ),
    query: String,
    pub(super) selected: Option<String>,
    keep_numbers: bool,
    /// The parts the picked macro carries, and whether to assign them.
    parts: Vec<PartRef>,
    carry_parts: bool,
    values: Vec<(String, Value)>,
    shown: Option<Shown>,
    status: String,
}

impl PlaceWindow {
    /// The macros as the command field searches them. The starters are put
    /// in the library the first time; the list is read from disk at most
    /// every few seconds, not on every keystroke.
    pub(crate) fn palette_entries(&mut self) -> Vec<novo_workflow::palette::MacroEntry> {
        const EVERY: std::time::Duration = std::time::Duration::from_secs(5);
        if self.palette.0.is_some_and(|at| at.elapsed() < EVERY) {
            return self.palette.1.clone();
        }
        let Some(library) = &self.library else {
            return Vec::new();
        };
        if !self.started {
            self.started = true;
            if let Err(e) = library.install_starters() {
                self.status = format!("Starter macros: {e}");
            }
        }
        let entries = library
            .search("")
            .into_iter()
            .map(|e| novo_workflow::palette::MacroEntry {
                words: format!("{} {}", e.tags.join(" "), e.description).to_lowercase(),
                detail: e.description,
                name: e.name,
            })
            .collect();
        self.palette = (Some(std::time::Instant::now()), entries);
        self.palette.1.clone()
    }

    pub(crate) fn new(library: Option<MacroLibrary>) -> Self {
        PlaceWindow {
            open: false,
            library,
            started: false,
            palette: (None, Vec::new()),
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

/// Shown in the Place window under the preview of a macro with PLC channel
/// symbols: the symbols alone give no addresses, so the I/O list and PLC
/// overview stay empty.
const PLC_CARD_NOTE: &str =
    "Symbols only: no addresses or I/O list until you add a PLC card (Draw > PLC > Add PLC card)";

/// Whether the macro draws PLC input or output channels (not just a supply
/// or CPU), which need a registered card to show in the I/O list.
pub(crate) fn draws_plc_channels(definition: &BlockDefinition) -> bool {
    definition.members.iter().any(|m| {
        matches!(m, Record::Symbol(s) if
            s.symbol.0.starts_with("iec/plc/")
                && (s.symbol.0.ends_with("_input") || s.symbol.0.ends_with("_output")))
    })
}

/// How many of the placed symbols have their origin outside the sheet.
fn off_sheet_count(project: &novo_core::Project, page: novo_core::PageId, ids: &[AnyId]) -> usize {
    let Some(page) = project.page(page) else {
        return 0;
    };
    let (w, h) = page.format.size_mm();
    ids.iter()
        .filter_map(|id| match id {
            AnyId::Symbol(s) => project.symbol(*s),
            _ => None,
        })
        .filter(|s| {
            let o = s.placement.origin;
            o.x < 0.0 || o.y < 0.0 || o.x > w || o.y > h
        })
        .count()
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
        let modal = super::panel::modal(ctx, "macro_save", "Save as macro", [380.0, 0.0], |ui| {
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
        // A circuit picked by its whole name is already chosen: the preview
        // shows.
        if let Some(name) = query.map(str::trim) {
            let found = win
                .entries()
                .into_iter()
                .find(|e| e.name.eq_ignore_ascii_case(name));
            if let Some(entry) = found {
                win.select(&entry.path);
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
        let page = self.macro_target_page();
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
                                if draws_plc_channels(&shown.definition) {
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(PLC_CARD_NOTE)
                                                .color(crate::theme::ui_palette().warning),
                                        )
                                        .wrap(),
                                    );
                                }
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

    /// The page a macro goes on: the open page, or the first drawing page
    /// when the open page is generated from the model.
    pub(super) fn macro_target_page(&self) -> Option<novo_core::PageId> {
        let current = self.current_page?;
        let project = self.backend.document()?.project();
        let generated =
            |p: &novo_core::Page| matches!(p.kind, novo_core::PageKind::Generated { .. });
        match project.page(current) {
            Some(p) if !generated(p) => Some(current),
            _ => project
                .pages_in_order()
                .into_iter()
                .find(|p| !generated(p))
                .map(|p| p.id),
        }
    }

    /// Places what the window previewed, as one undo step, and selects it.
    pub(super) fn place_picked_macro(&mut self) {
        let Some(page) = self.macro_target_page() else {
            self.message(
                Level::Error,
                "Could not place: the project has no drawing page, only generated pages. Add a page first.",
            );
            return;
        };
        // A macro never lands on a generated page (contents, lists): the
        // first drawing page takes it, and the canvas goes there.
        let moved = self.current_page != Some(page);
        let Some(shown) = self.wb.macro_place.shown.take() else {
            return;
        };
        let options = self.wb.macro_place.options();
        let name = shown.definition.name.clone();
        let plc_note = draws_plc_channels(&shown.definition);
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let origin = macro_place::free_origin(doc.project(), page, PLACE_GAP);
        // The supply circuit in a drawing with no supply: feed its open points.
        let feed_supply =
            name == supply_feeds::SUPPLY_MACRO && !supply_feeds::has_supply(doc.project());
        let mut fed = 0;
        let mut joined = 0;
        let result = doc.transact("Place macro", |tx| {
            let placed = macro_place::place_macro(tx, shown.definition, page, origin, &options)?;
            if feed_supply {
                fed = supply_feeds::feed_open_points(tx, page, placed.instance)?;
            }
            // The supply and the circuit it feeds, in either order.
            joined = supply_feeds::join_starter(tx, page, placed.instance)?;
            Ok(placed)
        });
        match result {
            Ok((placed, changes)) => {
                let ids: Vec<AnyId> = doc
                    .project()
                    .block_instance(placed.instance)
                    .map(|i| i.members.iter().map(|m| m.actual).collect())
                    .unwrap_or_default();
                // Symbols that landed beyond the sheet edge print nowhere.
                let off_sheet = off_sheet_count(doc.project(), page, &ids);
                self.changed_outside_canvas(&changes);
                if moved {
                    self.show_page(page);
                }
                if let Some(view) = self.canvas.schematic() {
                    view.select_ids(&ids);
                }
                let text = if plc_note {
                    format!(
                        "Placed {name} (symbols only: use Draw > PLC > Add PLC card for addresses) · {}",
                        describe(&placed.report)
                    )
                } else if fed > 0 && joined > 0 {
                    format!(
                        "Placed {name}, L1 L2 L3 N PE fed · {} joined to the starter's open entries · {}",
                        count_words::wires(joined),
                        describe(&placed.report)
                    )
                } else if fed > 0 {
                    format!(
                        "Placed {name}, L1 L2 L3 N PE fed · now wire its outputs to the starter · {}",
                        describe(&placed.report)
                    )
                } else if joined > 0 {
                    format!(
                        "Placed {name}, {} joined to the Three-phase supply · {}",
                        count_words::wires(joined),
                        describe(&placed.report)
                    )
                } else {
                    format!("Placed {name} · {}", describe(&placed.report))
                };
                if off_sheet > 0 {
                    self.message(
                        Level::Warning,
                        format!(
                            "{name}: {off_sheet} {} outside the page edge; move {} onto the sheet or use a bigger page",
                            if off_sheet == 1 { "symbol is" } else { "symbols are" },
                            if off_sheet == 1 { "it" } else { "them" }
                        ),
                    );
                }
                self.message(Level::Info, text);
                self.wb.macro_place.open = false;
            }
            Err(e) => {
                self.wb.macro_place.status = format!("Could not place: {e}");
                self.message(Level::Error, format!("Could not place {name}: {e}"));
            }
        }
    }
}
