//! The macro editor: define, change and remove the parameters of a macro in
//! the library (`macro.edit`, Draw > Macros > Edit macro parameters, or the
//! "Edit parameters..." button of the macro browser).
//!
//! A parameter has a name, a type and a default. Texts and device properties
//! of the macro name it as `{name}`; the Place macro window then asks for it
//! with the default filled in. Saving writes the library file again, which
//! raises its revision, so projects holding an older copy are offered the
//! update (Macros > "Newer in the library"). The engine is
//! `novo_parts::{ParamDraft, validate_parameters, MacroLibrary::set_parameters}`.
//! Company macros are read-only here; copy one to your own library first.

use eframe::egui;
use novo_parts::{MacroLibrary, ParamDraft, ParamKind, param_drafts, validate_parameters};

use super::{Level, NovoApp};

/// State of the macro editor window.
#[derive(Default)]
pub(crate) struct MacroEditor {
    pub(super) open: bool,
    /// Library path of the macro being edited.
    pub(super) path: Option<String>,
    pub(super) name: String,
    pub(super) drafts: Vec<ParamDraft>,
    /// What the macro had when loaded, to tell whether anything changed.
    loaded: Vec<ParamDraft>,
    /// The company library is shown: nothing can be saved.
    pub(super) read_only: bool,
    pub(super) error: Option<String>,
    /// `{name}` references no parameter answers (warning, not an error).
    pub(super) dangling: Vec<String>,
    pub(super) status: String,
}

impl MacroEditor {
    /// Loads the parameters of the macro at `path`.
    pub(super) fn load(&mut self, library: &MacroLibrary, path: &str, read_only: bool) {
        self.read_only = read_only;
        self.error = None;
        self.status.clear();
        match library.load(path) {
            Ok(file) => {
                self.path = Some(path.to_owned());
                self.name = file.name.clone();
                self.drafts = param_drafts(&file.definition.parameters);
                self.loaded = self.drafts.clone();
                self.refresh_dangling(library);
            }
            Err(e) => {
                self.path = None;
                self.name.clear();
                self.drafts.clear();
                self.loaded.clear();
                self.dangling.clear();
                self.error = Some(format!("Could not read the macro: {e}"));
            }
        }
    }

    pub(super) fn changed(&self) -> bool {
        self.drafts != self.loaded
    }

    pub(super) fn add(&mut self) {
        let mut n = self.drafts.len() + 1;
        let name = loop {
            let candidate = format!("param{n}");
            if self.drafts.iter().all(|d| d.name != candidate) {
                break candidate;
            }
            n += 1;
        };
        self.drafts.push(ParamDraft {
            name,
            ..ParamDraft::new()
        });
    }

    pub(super) fn remove(&mut self, index: usize) {
        if index < self.drafts.len() {
            self.drafts.remove(index);
        }
    }

    /// References in the macro's texts that the current list does not answer.
    fn refresh_dangling(&mut self, library: &MacroLibrary) {
        self.dangling.clear();
        let (Some(path), Ok(list)) = (&self.path, validate_parameters(&self.drafts)) else {
            return;
        };
        if let Ok(file) = library.load(path) {
            self.dangling = file.unknown_references(&list);
        }
    }

    /// Checks and writes the list. Returns whether it was saved.
    pub(super) fn save(&mut self, library: &MacroLibrary) -> bool {
        if self.read_only {
            self.error = Some("Company macros are read-only. Copy it to your macros first.".into());
            return false;
        }
        let Some(path) = self.path.clone() else {
            return false;
        };
        let list = match validate_parameters(&self.drafts) {
            Ok(list) => list,
            Err(reason) => {
                self.error = Some(reason);
                return false;
            }
        };
        match library.set_parameters(&path, list) {
            Ok(()) => {
                self.error = None;
                self.status = format!("Saved {} parameter(s) in {}", self.drafts.len(), self.name);
                self.loaded = self.drafts.clone();
                self.refresh_dangling(library);
                true
            }
            Err(e) => {
                self.error = Some(format!("Could not save: {e}"));
                false
            }
        }
    }
}

impl NovoApp {
    /// `macro.edit`: opens the editor on the macro named by `argument`, else
    /// on the one selected in the browser, else on the first in the library.
    pub(super) fn open_macro_editor(&mut self, argument: Option<&str>) {
        self.wb.macros.show();
        let b = &self.wb.macros;
        let Some(lib) = b.library.clone() else {
            self.message(Level::Warning, "No macro library on this machine");
            return;
        };
        let entries = lib.search("");
        let wanted = argument.map(str::trim).filter(|a| !a.is_empty());
        let path = wanted
            .and_then(|w| {
                entries
                    .iter()
                    .find(|e| e.name.eq_ignore_ascii_case(w))
                    .or_else(|| entries.iter().find(|e| e.path.eq_ignore_ascii_case(w)))
            })
            .map(|e| e.path.clone())
            .or_else(|| b.selected.clone())
            .or_else(|| entries.first().map(|e| e.path.clone()));
        let read_only = b.company;
        let Some(path) = path else {
            self.message(Level::Info, "Save a macro first, then edit its parameters");
            return;
        };
        self.wb.macro_editor.load(&lib, &path, read_only);
        self.wb.macro_editor.open = true;
    }

    pub(super) fn macro_editor_window(&mut self, ctx: &egui::Context) {
        if !self.wb.macro_editor.open {
            return;
        }
        let Some(lib) = self.wb.macros.library.clone() else {
            self.wb.macro_editor.open = false;
            return;
        };
        let entries = lib.search("");
        let mut open = true;
        let mut pick: Option<String> = None;
        let mut save = false;
        let mut remove: Option<usize> = None;
        let mut add = false;
        let mut revert = false;
        let window = super::panel::dialog(ctx, "Edit macro parameters", [560.0, 0.0])
            .id(super::panel::keep_id(ctx, egui::Id::new("macro_editor")))
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                let ed = &mut self.wb.macro_editor;
                ui.weak(
                    "A parameter is asked for when the macro is placed. Use {name} in the macro's texts and device properties where its value goes.",
                );
                ui.horizontal(|ui| {
                    ui.label("Macro");
                    egui::ComboBox::from_id_salt("macro_editor_pick")
                        .selected_text(if ed.name.is_empty() { "Pick a macro" } else { &ed.name })
                        .show_ui(ui, |ui| {
                            for e in &entries {
                                let on = ed.path.as_deref() == Some(e.path.as_str());
                                if ui.selectable_label(on, &e.name).clicked() {
                                    pick = Some(e.path.clone());
                                }
                            }
                        });
                });
                if ed.read_only {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        "Company macro: read-only. Copy it to your macros to change it.",
                    );
                }
                ui.separator();
                novo_theme::widgets::section_label(ui, "Parameters");
                if ed.drafts.is_empty() {
                    ui.weak("This macro has no parameters.");
                }
                let read_only = ed.read_only;
                egui::Grid::new("macro_editor_grid")
                    .num_columns(5)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        if !ed.drafts.is_empty() {
                            for h in ["Name", "Type", "Default", "Description", ""] {
                                ui.weak(h);
                            }
                            ui.end_row();
                        }
                        for (i, d) in ed.drafts.iter_mut().enumerate() {
                            ui.add_enabled(
                                !read_only,
                                egui::TextEdit::singleline(&mut d.name).desired_width(110.0),
                            );
                            let mut kind = d.kind;
                            ui.add_enabled_ui(!read_only, |ui| {
                                egui::ComboBox::from_id_salt(("macro_param_kind", i))
                                    .selected_text(kind.label())
                                    .width(110.0)
                                    .show_ui(ui, |ui| {
                                        for k in ParamKind::ALL {
                                            ui.selectable_value(&mut kind, k, k.label());
                                        }
                                    });
                            });
                            if kind != d.kind {
                                d.set_kind(kind);
                            }
                            if d.kind == ParamKind::Switch {
                                let mut on = d.default == "yes";
                                if ui.add_enabled(!read_only, egui::Checkbox::new(&mut on, "")).changed() {
                                    d.default = if on { "yes" } else { "no" }.into();
                                }
                            } else {
                                ui.add_enabled(
                                    !read_only,
                                    egui::TextEdit::singleline(&mut d.default).desired_width(80.0),
                                );
                            }
                            ui.add_enabled(
                                !read_only,
                                egui::TextEdit::singleline(&mut d.description).desired_width(160.0),
                            );
                            if ui
                                .add_enabled(!read_only, egui::Button::new("Remove"))
                                .clicked()
                            {
                                remove = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                ui.horizontal(|ui| {
                    if ui.add_enabled(!ed.read_only, egui::Button::new("Add parameter")).clicked() {
                        add = true;
                    }
                    let changed = ed.changed();
                    if ui
                        .add_enabled(changed && !ed.read_only, egui::Button::new("Save"))
                        .clicked()
                    {
                        save = true;
                    }
                    if ui.add_enabled(changed, egui::Button::new("Revert")).clicked() {
                        revert = true;
                    }
                });
                if !ed.dangling.is_empty() {
                    let names: Vec<String> =
                        ed.dangling.iter().map(|n| format!("{{{n}}}")).collect();
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        format!(
                            "The macro uses {} but no parameter has that name; it stays literal text when placed.",
                            names.join(", ")
                        ),
                    );
                }
                if let Some(error) = &ed.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
                if !ed.status.is_empty() {
                    ui.weak(&ed.status);
                }
            });
        let _ = window;
        let read_only = self.wb.macros.company;
        let ed = &mut self.wb.macro_editor;
        if let Some(path) = pick {
            ed.load(&lib, &path, read_only);
        }
        if add {
            ed.add();
        }
        if let Some(i) = remove {
            ed.remove(i);
        }
        if revert && let Some(path) = ed.path.clone() {
            ed.load(&lib, &path, read_only);
        }
        if save {
            ed.save(&lib);
        } else if add || remove.is_some() {
            ed.error = None;
            ed.refresh_dangling(&lib);
        }
        ed.open = open && ed.open;
    }
}
