//! "Languages" (File menu, `project.languages`): the language reports,
//! generated pages and PDFs print in, and the project's translation table.
//!
//! The app's menus stay English on purpose; only what is printed changes.
//! The language and the table are project properties (`i18n.language`,
//! `i18n.table`), so a project sent to a colleague prints the same. Texts
//! with no translation print in English and are listed here, where each
//! can be typed in; typing writes the entry into the project (one undo
//! step), clearing it brings the shipped text back.

use std::collections::BTreeMap;

use eframe::egui;
use novo_checks::standards;
use novo_core::{Project, Value};
use novo_reports::designer::{builtin, store};
use novo_reports::i18n::{
    self, ENGLISH, LANGUAGE_KEY, ProjectTable, TABLE_KEY, Translator, strings_of_output,
    strings_of_template,
};
use novo_theme::widgets;

use super::{Level, NovoApp};

/// Where a text's translation comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    Project,
    Shipped,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Row {
    pub english: String,
    pub text: String,
    pub source: Source,
}

#[derive(Default)]
pub(super) struct LanguagesWindow {
    only_missing: bool,
    buffers: BTreeMap<String, String>,
    new_code: String,
    new_name: String,
    error: Option<String>,
}

/// Every text the project prints that a language needs an entry for: the
/// standard reports' titles and headers, and the texts of the templates
/// the project can use (its own and the built-in ones).
pub(super) fn printed_texts(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |list: Vec<String>| {
        for s in list {
            if !out.contains(&s) {
                out.push(s);
            }
        }
    };
    for output in novo_reports::generate_all(project) {
        add(strings_of_output(&output));
    }
    let project_templates = store::project_templates(project).unwrap_or_default();
    for t in project_templates.iter().chain(&builtin::all()) {
        add(strings_of_template(t));
    }
    out.sort();
    out
}

/// The table rows for `translator`: one per printed text.
pub(super) fn rows(project: &Project, translator: &Translator) -> Vec<Row> {
    let table = ProjectTable::of_project(project);
    printed_texts(project)
        .into_iter()
        .map(|english| {
            let own = table
                .language(translator.code())
                .and_then(|c| c.get(&english).map(str::to_owned));
            let (text, source) = match own {
                Some(t) => (t, Source::Project),
                None => match translator.translate(&english) {
                    Some(t) => (t, Source::Shipped),
                    None => (String::new(), Source::Missing),
                },
            };
            Row {
                english,
                text,
                source,
            }
        })
        .collect()
}

/// The missing texts as tab-separated lines (English, empty), to hand to a
/// translator.
pub(super) fn missing_as_text(rows: &[Row]) -> String {
    rows.iter()
        .filter(|r| r.source == Source::Missing)
        .map(|r| format!("{}\t", r.english))
        .collect::<Vec<_>>()
        .join("\n")
}

impl NovoApp {
    pub(super) fn toggle_languages(&mut self) {
        if self.wb.languages.take().is_none() && self.is_open() {
            self.wb.languages = Some(LanguagesWindow::default());
        }
    }

    /// The translator for the open project's language; English without a
    /// project.
    pub(super) fn report_translator(&self) -> Translator {
        self.backend
            .document()
            .map_or_else(Translator::english, |d| i18n::translator_of(d.project()))
    }

    /// Writes the current language as a language file; returns the number
    /// of entries.
    pub(super) fn export_language_file(&mut self, path: &std::path::Path) -> Option<usize> {
        let translator = self.report_translator();
        if translator.is_english() {
            self.message(
                Level::Info,
                "English is the source text; pick another language first",
            );
            return None;
        }
        let catalog = translator.catalog();
        match super::pdf::write_replacing(path, catalog.to_json().as_bytes()) {
            Ok(()) => {
                self.message(
                    Level::Info,
                    format!("Wrote {} ({} texts)", path.display(), catalog.entries.len()),
                );
                Some(catalog.entries.len())
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

    /// Reads a language file into the project (one undo step) and makes it
    /// the report language. The project's own entries for that language are
    /// kept; the file only fills in the rest. Entries that would break placeholders are left
    /// out and counted in the message.
    pub(super) fn import_language_file(&mut self, path: &std::path::Path) -> bool {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not read {}: {e}", path.display()),
                );
                return false;
            }
        };
        let catalog = match i18n::Catalog::from_json(&text) {
            Ok(c) => c,
            Err(e) => {
                self.message(Level::Error, format!("{}: {e}", path.display()));
                return false;
            }
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return false;
        };
        let mut table = ProjectTable::of_project(&project);
        let code = catalog.code.clone();
        let total = catalog.entries.len();
        let kept = table.language(&code).map_or(0, |c| c.entries.len());
        let dropped = table.merge_language(catalog);
        // One undo step: the table and the language together.
        let value = table.to_property().map(Value::Text);
        let language = (code != ENGLISH).then(|| Value::Text(code.clone()));
        self.edit_project("Import language file", |tx| {
            standards::set_info_property(tx, TABLE_KEY, value)?;
            standards::set_info_property(tx, LANGUAGE_KEY, language)
        });
        let own = if kept > 0 {
            format!(" The project's {kept} own entries for {code} were kept.")
        } else {
            String::new()
        };
        let note = if dropped.is_empty() {
            format!("Loaded {total} texts for {code}.{own}")
        } else {
            format!(
                "Loaded {} texts for {code}; {} left out because they change a {{placeholder}}: {}.{own}",
                total - dropped.len(),
                dropped.len(),
                dropped
                    .iter()
                    .map(|p| p.key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        self.message(Level::Info, note);
        true
    }

    pub(super) fn set_report_language(&mut self, code: &str) {
        let value = (code != ENGLISH).then(|| Value::Text(code.to_owned()));
        self.edit_project("Set report language", |tx| {
            standards::set_info_property(tx, LANGUAGE_KEY, value)
        });
    }

    /// Writes the project's table (or removes the property when empty).
    pub(super) fn set_translation_table(&mut self, label: &str, table: &ProjectTable) {
        let value = table.to_property().map(Value::Text);
        self.edit_project(label, |tx| {
            standards::set_info_property(tx, TABLE_KEY, value)
        });
    }

    pub(super) fn languages_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.languages.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return;
        };
        let table = ProjectTable::of_project(&project);
        let code = i18n::language_of(&project);
        let translator = Translator::new(&code, &table);
        let list = i18n::languages(&table);
        let table_rows = if translator.is_english() {
            Vec::new()
        } else {
            rows(&project, &translator)
        };
        let mut pick: Option<String> = None;
        let mut edit: Option<(String, String)> = None;
        let mut add_language: Option<(String, String)> = None;
        let mut decimal: Option<char> = None;
        let mut copy: Option<String> = None;
        let mut save_file = false;
        let mut load_file = false;
        let mut open = true;
        super::panel::dialog(ctx, "Languages", [620.0, 560.0])
            .open(&mut open)
            .show(ctx, |ui| {
                super::dialog_ui::hint(
                    ui,
                    "The language of reports, generated pages and PDFs. The menus and \
                     commands stay English.",
                );
                ui.add_space(4.0);
                widgets::section_label(ui, "Report language");
                ui.horizontal_wrapped(|ui| {
                    for l in &list {
                        if widgets::chip(ui, &l.name, l.code == code).clicked() && l.code != code {
                            pick = Some(l.code.clone());
                        }
                    }
                });
                if translator.is_english() {
                    super::dialog_ui::hint(
                        ui,
                        "English prints the texts as the program writes them.",
                    );
                } else {
                    let missing = table_rows
                        .iter()
                        .filter(|r| r.source == Source::Missing)
                        .count();
                    if missing == 0 {
                        ui.label(format!(
                            "All {} texts of this project's reports have a translation.",
                            table_rows.len()
                        ));
                    } else {
                        ui.colored_label(
                            crate::theme::ui_palette().warning,
                            format!(
                                "{missing} of {} texts have no translation and print in English.",
                                table_rows.len()
                            ),
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.label("Decimal sign");
                        for (sign, example) in [('.', "1.5"), (',', "1,5")] {
                            if widgets::chip(ui, example, translator.decimal() == sign).clicked()
                                && translator.decimal() != sign
                            {
                                decimal = Some(sign);
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut win.only_missing, "Only missing");
                        if missing > 0
                            && widgets::ghost_button(ui, "Copy missing as text").clicked()
                        {
                            copy = Some(missing_as_text(&table_rows));
                        }
                    });
                    if let Some(error) = &win.error {
                        ui.colored_label(crate::theme::ui_palette().error, error);
                    }
                    ui.separator();
                    // Leave room for the add-a-language rows under the table.
                    let table_height = (ui.available_height() - 150.0).clamp(120.0, 300.0);
                    egui::ScrollArea::vertical()
                        .max_height(table_height)
                        .show(ui, |ui| {
                            egui::Grid::new("language_rows")
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label(
                                        egui::RichText::new("ENGLISH")
                                            .size(12.0)
                                            .color(crate::theme::ui_palette().text_secondary),
                                    );
                                    ui.label(
                                        egui::RichText::new("TRANSLATION")
                                            .size(12.0)
                                            .color(crate::theme::ui_palette().text_secondary),
                                    );
                                    ui.end_row();
                                    for r in &table_rows {
                                        if win.only_missing && r.source != Source::Missing {
                                            continue;
                                        }
                                        ui.label(&r.english);
                                        let id = egui::Id::new(("language_row", &r.english));
                                        let focused = ui.memory(|m| m.has_focus(id));
                                        let buffer =
                                            win.buffers.entry(r.english.clone()).or_default();
                                        if !focused {
                                            buffer.clone_from(&r.text);
                                        }
                                        let response = ui.add(
                                            egui::TextEdit::singleline(buffer)
                                                .id(id)
                                                .hint_text(&r.english)
                                                .desired_width(260.0),
                                        );
                                        if response.lost_focus() && *buffer != r.text {
                                            edit = Some((r.english.clone(), buffer.clone()));
                                        }
                                        ui.end_row();
                                    }
                                });
                        });
                }
                ui.separator();
                widgets::section_label(ui, "Add a language");
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut win.new_code)
                            .hint_text("code, e.g. de")
                            .desired_width(90.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut win.new_name)
                            .hint_text("name, e.g. Deutsch")
                            .desired_width(140.0),
                    );
                    let code_ok = !win.new_code.trim().is_empty()
                        && !list.iter().any(|l| l.code == win.new_code.trim());
                    let add = ui
                        .add_enabled_ui(code_ok, |ui| widgets::primary_button(ui, "Add"))
                        .inner
                        .on_disabled_hover_text("Type a code that is not in the list yet");
                    if !code_ok {
                        let why = if win.new_code.trim().is_empty() {
                            "Type a new language code to add it."
                        } else {
                            "That code is already in the list."
                        };
                        ui.label(
                            egui::RichText::new(why)
                                .size(12.0)
                                .color(crate::theme::ui_palette().text_secondary),
                        );
                    }
                    if add.clicked() {
                        add_language = Some((
                            win.new_code.trim().to_lowercase(),
                            win.new_name.trim().to_owned(),
                        ));
                    }
                });
                ui.horizontal(|ui| {
                    if !translator.is_english()
                        && widgets::ghost_button(ui, "Save language file...").clicked()
                    {
                        save_file = true;
                    }
                    if widgets::ghost_button(ui, "Load language file...").clicked() {
                        load_file = true;
                    }
                });
                super::dialog_ui::hint(
                    ui,
                    "Texts with no translation print in English, so nothing is left blank. \
                     CSV and Excel exports of a report use the same language for their headers.",
                );
            });
        if let Some(code) = pick {
            self.set_report_language(&code);
        }
        if let Some(sign) = decimal {
            let mut table = table.clone();
            table.set_decimal(translator.code(), sign);
            self.set_translation_table("Set decimal sign", &table);
        }
        if let Some((english, text)) = edit {
            let mut table = table.clone();
            win.buffers.remove(&english);
            let result = if text.trim().is_empty() {
                table.remove(translator.code(), &english);
                Ok(())
            } else {
                table.set(translator.code(), &english, text.trim())
            };
            match result {
                Ok(()) => {
                    win.error = None;
                    self.set_translation_table("Edit translation", &table);
                }
                Err(why) => win.error = Some(format!("\"{english}\": {why}")),
            }
        }
        if let Some((code, name)) = add_language {
            let mut table = table.clone();
            let _ = table.put_language(i18n::Catalog::new(&code, &name));
            self.set_translation_table("Add language", &table);
            self.set_report_language(&code);
            win.new_code.clear();
            win.new_name.clear();
        }
        if save_file {
            let name = format!("{}{}", code, i18n::EXTENSION);
            if let Some(path) = crate::dialogs::FileDialog::new()
                .set_title("Save language file")
                .set_file_name(name)
                .save_file()
            {
                self.export_language_file(&path);
            }
        }
        if load_file
            && let Some(path) = crate::dialogs::FileDialog::new()
                .set_title("Load language file")
                .add_filter("Language file", &["json"])
                .pick_file()
        {
            self.import_language_file(&path);
        }
        if let Some(text) = copy {
            ctx.copy_text(text);
            self.message(Level::Info, "Missing texts copied");
        }
        if open {
            self.wb.languages = Some(win);
        }
    }
}
