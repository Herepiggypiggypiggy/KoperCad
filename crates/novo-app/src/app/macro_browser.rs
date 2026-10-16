//! The macro browser: the user's library of reusable circuits (`.kcmacro`
//! files), with search, folders, rename, move, trash and restore, and import
//! and export so a team can share them. The engine is
//! `novo_parts::MacroLibrary`; this file is the window over it.
//!
//! Reach it with `macro.browser` (Ctrl+K: "macro", "macros", "bibliotheek"),
//! or `macro.import` to pick a file straight away. Placing a macro belongs to
//! the `macro.place` window of the Macros thread, which the Place button
//! opens with the macro's name.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_parts::{
    Imported, MacroEntry, MacroLibrary, MacroUpdate, OnConflict, Thumbnail, macro_updates,
};

use super::{Level, NovoApp};

/// State of the browser window.
pub(crate) struct MacroBrowser {
    pub(super) open: bool,
    /// The library shown; the per-user one unless a test or a company
    /// folder says otherwise.
    pub(super) library: Option<MacroLibrary>,
    /// The person's own library and the company folder (Preferences).
    user_library: Option<MacroLibrary>,
    company_library: Option<MacroLibrary>,
    /// Which of the two `library` is.
    pub(super) company: bool,
    /// The picture of the selected macro, with what it was built from.
    thumb: Option<(ThumbKey, Option<Thumbnail>)>,
    started: bool,
    pub(super) query: String,
    /// Library path of the selected macro.
    pub(super) selected: Option<String>,
    /// The new name while renaming the selection.
    pub(super) renaming: Option<String>,
    /// What the last action did, shown under the list.
    pub(super) status: String,
}

/// What a cached picture belongs to; any change rebuilds it.
type ThumbKey = (std::path::PathBuf, String, u32);

impl Default for MacroBrowser {
    fn default() -> Self {
        MacroBrowser {
            open: false,
            library: MacroLibrary::user(),
            user_library: MacroLibrary::user(),
            company_library: None,
            company: false,
            thumb: None,
            started: false,
            query: String::new(),
            selected: None,
            renaming: None,
            status: String::new(),
        }
    }
}

impl MacroBrowser {
    /// A browser over a given library (tests, a company folder).
    #[cfg(test)]
    pub(crate) fn with_library(library: MacroLibrary) -> Self {
        MacroBrowser {
            user_library: Some(library.clone()),
            library: Some(library),
            ..Default::default()
        }
    }

    /// Follows the Preferences setting: the company folder, if any. When it
    /// is cleared while shown, the browser goes back to the person's own.
    pub(super) fn set_company_folder(&mut self, folder: Option<&Path>) {
        let same = self.company_library.as_ref().map(MacroLibrary::root) == folder;
        if same {
            return;
        }
        self.company_library = folder.map(|f| MacroLibrary::new(f, "company"));
        if self.company_library.is_none() {
            self.company = false;
        }
        self.pick_library();
    }

    pub(super) fn show_company(&mut self, company: bool) {
        self.company = company && self.company_library.is_some();
        self.pick_library();
    }

    fn pick_library(&mut self) {
        self.library = if self.company {
            self.company_library.clone()
        } else {
            self.user_library.clone()
        };
        self.selected = None;
        self.renaming = None;
        self.thumb = None;
    }

    pub(super) fn has_company(&self) -> bool {
        self.company_library.is_some()
    }

    /// The picture of the selected macro, built when the pick, its revision
    /// or the library changed. `None` when nothing is selected or the file
    /// cannot be read.
    pub(super) fn thumbnail(&mut self) -> Option<&Thumbnail> {
        let (lib, sel) = (self.library.as_ref()?, self.selected.as_ref()?);
        let revision = lib.search("").iter().find(|e| &e.path == sel)?.revision;
        let key = (lib.root().to_path_buf(), sel.clone(), revision);
        if self.thumb.as_ref().is_none_or(|(k, _)| *k != key) {
            let built = lib
                .load(sel)
                .ok()
                .map(|f| novo_parts::thumbnail(&f.definition));
            self.thumb = Some((key, built));
        }
        self.thumb.as_ref().and_then(|(_, t)| t.as_ref())
    }

    /// Newer library revisions of the macros the project was built from,
    /// from both libraries.
    pub(super) fn updates_for(&self, project: &novo_core::Project) -> Vec<MacroUpdate> {
        let sources: Vec<_> = project
            .block_definitions()
            .filter_map(|d| d.source.as_ref().map(|s| (d.name.as_str(), s)))
            .collect();
        [&self.user_library, &self.company_library]
            .into_iter()
            .flatten()
            .flat_map(|lib| macro_updates(lib, sources.iter().copied()))
            .collect()
    }

    /// Copies the selected macro into the person's own library (a company
    /// macro is placed from there); the copy gets a free name.
    pub(super) fn copy_selected_to_user(&mut self) {
        let (Some(from), Some(to), Some(sel)) = (&self.library, &self.user_library, &self.selected)
        else {
            return;
        };
        self.status = match to.copy_from(from, sel, "", OnConflict::KeepBoth) {
            Ok(_) => "Copied to your macros".into(),
            Err(e) => format!("Copy failed: {e}"),
        };
    }

    /// Opens the window; the first time it also puts the starter macros in
    /// the library (it never overwrites anything).
    pub(super) fn show(&mut self) {
        self.open = true;
        if !self.started {
            self.started = true;
            if let Some(Err(e)) = self.library.as_ref().map(|l| l.install_starters()) {
                self.status = format!("Starter macros: {e}");
            }
        }
    }

    pub(super) fn entries(&self) -> Vec<MacroEntry> {
        self.library
            .as_ref()
            .map(|l| l.search(&self.query))
            .unwrap_or_default()
    }

    /// Imports a `.kcmacro` or `.kcmacropack`; macros whose name is taken
    /// are kept as copies, never replaced.
    pub(super) fn import(&mut self, path: &Path) {
        let Some(lib) = &self.library else { return };
        self.status = match lib.import(path, "", OnConflict::KeepBoth) {
            Ok(done) => {
                let added = done
                    .iter()
                    .filter(|d| matches!(d, Imported::Added(_)))
                    .count();
                if let Some(Imported::Added(p)) = done.first() {
                    self.selected = Some(p.clone());
                }
                format!("Imported {added} macro(s)")
            }
            Err(e) => format!("Import failed: {e}"),
        };
    }

    pub(super) fn export_selected(&mut self, dest: &Path) {
        let (Some(lib), Some(sel)) = (&self.library, &self.selected) else {
            return;
        };
        self.status = match lib.export(sel, dest) {
            Ok(()) => format!("Exported to {}", dest.display()),
            Err(e) => format!("Export failed: {e}"),
        };
    }

    pub(super) fn rename_selected(&mut self, name: &str) {
        let (Some(lib), Some(sel)) = (&self.library, self.selected.clone()) else {
            return;
        };
        match lib.rename(&sel, name) {
            Ok(path) => {
                self.selected = Some(path);
                self.status = format!("Renamed to {name}");
            }
            Err(e) => self.status = format!("Rename failed: {e}"),
        }
    }

    pub(super) fn move_selected(&mut self, folder: &str) {
        let (Some(lib), Some(sel)) = (&self.library, self.selected.clone()) else {
            return;
        };
        match lib.move_to(&sel, folder) {
            Ok(path) => {
                self.selected = Some(path);
                self.status = "Moved".into();
            }
            Err(e) => self.status = format!("Move failed: {e}"),
        }
    }

    /// Deleting is moving to the trash, so it can be undone from the list.
    pub(super) fn trash_selected(&mut self) {
        let (Some(lib), Some(sel)) = (&self.library, self.selected.take()) else {
            return;
        };
        self.status = match lib.trash(&sel) {
            Ok(_) => "Moved to the trash".into(),
            Err(e) => format!("Delete failed: {e}"),
        };
    }

    pub(super) fn restore(&mut self, trash_path: &str) {
        let Some(lib) = &self.library else { return };
        match lib.restore(trash_path, "") {
            Ok(path) => {
                self.selected = Some(path);
                self.status = "Restored".into();
            }
            Err(e) => self.status = format!("Restore failed: {e}"),
        }
    }
}

impl NovoApp {
    /// `macro.import`: pick a file and import it, then show the browser.
    pub(super) fn macro_import(&mut self) {
        self.wb.macros.show();
        let picked = rfd::FileDialog::new()
            .set_title("Import macros")
            .add_filter("KoperCAD macros", &["kcmacro", "kcmacropack"])
            .pick_file();
        if let Some(path) = picked {
            self.wb.macros.import(&path);
            let status = self.wb.macros.status.clone();
            self.message(Level::Info, status);
        }
    }

    /// Renames the project's definition `name` to `name (rev N)`, so the
    /// library's newer macro of that name can be placed without the
    /// duplicate-name refusal. Placed circuits keep the old definition.
    pub(super) fn set_old_macro_aside(&mut self, name: &str, revision: u32) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(id) = doc.project().block_definition_by_name(name).map(|d| d.id) else {
            return;
        };
        let mut new_name = format!("{name} (rev {revision})");
        let mut n = 2;
        while doc.project().block_definition_by_name(&new_name).is_some() {
            new_name = format!("{name} (rev {revision}) {n}");
            n += 1;
        }
        let renamed = new_name.clone();
        let result = doc.transact("Set old macro copy aside", |tx| {
            tx.update::<novo_core::BlockDefinition>(id, |d| d.name = renamed)
        });
        match result {
            Ok((_, changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(
                    Level::Info,
                    format!("{name} is now {new_name}; place the macro again for the new revision"),
                );
            }
            Err(e) => self.message(Level::Warning, format!("Could not rename {name}: {e}")),
        }
    }

    pub(super) fn macro_browser_window(&mut self, ctx: &egui::Context) {
        if !self.wb.macros.open {
            return;
        }
        let mut open = true;
        let mut import = false;
        let mut export: Option<PathBuf> = None;
        let mut place: Option<String> = None;
        let mut retire: Option<(String, u32)> = None;
        let company_folder = self.settings.company_macros.clone();
        self.wb.macros.set_company_folder(company_folder.as_deref());
        let updates = self
            .backend
            .document()
            .map(|d| self.wb.macros.updates_for(d.project()))
            .unwrap_or_default();
        let window = super::panel::dialog(ctx, "Macros", [520.0, 0.0])
            .id(egui::Id::new("macro_browser"))
            .open(&mut open)
            .show(ctx, |ui| {
                let b = &mut self.wb.macros;
                let Some(lib) = b.library.clone() else {
                    ui.label("No profile folder is available for macros on this machine.");
                    return;
                };
                novo_theme::widgets::section_label(ui, "Macro library");
                if b.has_company() {
                    ui.horizontal(|ui| {
                        let company = b.company;
                        if ui.selectable_label(!company, "My macros").clicked() {
                            b.show_company(false);
                        }
                        if ui.selectable_label(company, "Company").clicked() {
                            b.show_company(true);
                        }
                    });
                }
                let lib = b.library.clone().unwrap_or(lib);
                ui.horizontal(|ui| {
                    ui.label("Search");
                    ui.text_edit_singleline(&mut b.query);
                });
                ui.weak(format!("Library: {}", lib.root().display()));
                ui.separator();
                let entries = b.entries();
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .id_salt("macro_list")
                    .show(ui, |ui| {
                        if entries.is_empty() {
                            ui.weak("No macros match.");
                        }
                        let mut folder = String::new();
                        for e in &entries {
                            if e.folder != folder {
                                folder = e.folder.clone();
                                let label = if folder.is_empty() {
                                    "Top level"
                                } else {
                                    &folder
                                };
                                novo_theme::widgets::section_label(ui, label);
                            }
                            let selected = b.selected.as_deref() == Some(e.path.as_str());
                            if ui
                                .selectable_label(selected, format!("{}  ({})", e.name, e.members))
                                .on_hover_text(&e.description)
                                .clicked()
                            {
                                b.selected = Some(e.path.clone());
                                b.renaming = None;
                            }
                        }
                    });
                ui.separator();
                let current = entries
                    .iter()
                    .find(|e| b.selected.as_deref() == Some(e.path.as_str()))
                    .cloned();
                if let Some(e) = current {
                    novo_theme::widgets::section_label(ui, "Selected");
                    ui.strong(&e.name);
                    if !e.description.is_empty() {
                        ui.label(&e.description);
                    }
                    ui.weak(format!(
                        "{} symbols and wires, {} parts carried, {} parameters, revision {}{}{}",
                        e.members,
                        e.parts,
                        e.parameters,
                        e.revision,
                        if e.author.is_empty() {
                            String::new()
                        } else {
                            format!(", by {}", e.author)
                        },
                        if e.tags.is_empty() {
                            String::new()
                        } else {
                            format!(" \u{b7} {}", e.tags.join(", "))
                        },
                    ));
                    if let Some(thumb) = b.thumbnail() {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width().min(320.0), 120.0),
                            egui::Sense::hover(),
                        );
                        super::macro_thumb::paint(ui, rect, thumb);
                    }
                    ui.horizontal(|ui| {
                        if let Some(name) = &mut b.renaming {
                            let edit = ui.text_edit_singleline(name);
                            let done = ui.button("Rename").clicked()
                                || (edit.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                            if done {
                                let new = name.clone();
                                b.renaming = None;
                                b.rename_selected(&new);
                            }
                        } else {
                            if ui.button("Rename").clicked() {
                                b.renaming = Some(e.name.clone());
                            }
                            let mut folders = lib.folders();
                            folders.insert(0, String::new());
                            egui::ComboBox::from_id_salt("macro_move")
                                .selected_text("Move to...")
                                .show_ui(ui, |ui| {
                                    for f in folders {
                                        let label = if f.is_empty() {
                                            "Top level".to_string()
                                        } else {
                                            f.clone()
                                        };
                                        if ui.selectable_label(f == e.folder, label).clicked() {
                                            b.move_selected(&f);
                                        }
                                    }
                                });
                            if ui.button("Export...").clicked() {
                                export = rfd::FileDialog::new()
                                    .set_title("Export macro")
                                    .add_filter("KoperCAD macro", &["kcmacro"])
                                    .set_file_name(format!("{}.kcmacro", e.name))
                                    .save_file();
                            }
                            if b.company {
                                if ui
                                    .button("Copy to my macros")
                                    .on_hover_text("Place it from your own library")
                                    .clicked()
                                {
                                    b.copy_selected_to_user();
                                }
                            } else if ui.button("Place").clicked() {
                                place = Some(e.name.clone());
                            }
                            if ui.button("Delete").clicked() {
                                b.trash_selected();
                            }
                        }
                    });
                } else {
                    ui.weak("Select a macro.");
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Import...").clicked() {
                        import = true;
                    }
                    let trashed = lib.trashed();
                    if !trashed.is_empty() {
                        ui.menu_button(format!("Trash ({})", trashed.len()), |ui| {
                            for t in trashed {
                                let name = t.trim_start_matches(".trash/").to_string();
                                if ui.button(format!("Restore {name}")).clicked() {
                                    b.restore(&t);
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                if !updates.is_empty() {
                    ui.separator();
                    novo_theme::widgets::section_label(ui, "Newer in the library");
                    ui.weak(
                        "This project holds older copies. Setting one aside renames it, so placing the macro again brings the new revision; circuits already placed keep the old one.",
                    );
                    for u in &updates {
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{}: revision {} in the project, {} in the library",
                                u.definition, u.project_revision, u.library_revision
                            ));
                            if ui.button("Set old copy aside").clicked() {
                                retire = Some((u.definition.clone(), u.project_revision));
                            }
                        });
                    }
                }
                if !b.status.is_empty() {
                    ui.weak(&b.status);
                }
            });
        if let Some(window) = window {
            self.anchor("macros.window", window.response.rect, "Macros");
        }
        if let Some(dest) = export {
            self.wb.macros.export_selected(&dest);
        }
        if import {
            self.macro_import();
        }
        if let Some(name) = place {
            self.open_macro_place(Some(&name));
        }
        if let Some((name, revision)) = retire {
            self.set_old_macro_aside(&name, revision);
        }
        self.wb.macros.open = open && self.wb.macros.open;
    }
}
