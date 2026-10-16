//! File > Import > EPLAN project (PXF): reads an EPLAN project export
//! (`.epj`) or page macro (`.emp`), shows what would be added, what exists
//! and what is not carried (counted), and applies it as one undo step when
//! the person confirms (docs/design/eplan-pxf.md).

use std::path::Path;

use eframe::egui;
use novo_interop::eplan::pxf::{PxfFile, PxfOptions, PxfPlan, plan_pxf, read_pxf, survey_pxf};

use super::{Level, NovoApp, projects_dir};

/// An EPLAN import waiting for the person to confirm.
pub(super) struct EplanImport {
    file: String,
    plan: PxfPlan,
    /// What was read, kept so another language can be tried without
    /// reading the file again.
    data: PxfFile,
    /// Language of titles and type texts the plan uses (`en_US`).
    lang: String,
    /// Languages the file's multi-language texts carry.
    langs: Vec<String>,
    /// What `kopercad-cli eplan check` says: the exchange file's header and
    /// whether the export script that made it is current.
    check: Option<(bool, String)>,
    /// What `kopercad-cli eplan survey` says: object kinds, element names
    /// and counts, never values.
    survey: Option<String>,
}

/// The languages (`en_US`, `de_DE`...) the multi-language texts in `text`
/// carry, sorted.
pub(crate) fn languages_in(text: &str) -> Vec<String> {
    let b = text.as_bytes();
    let mut found = std::collections::BTreeSet::new();
    for (i, &c) in b.iter().enumerate() {
        if c == b'@'
            && i >= 5
            && b[i - 3] == b'_'
            && b[i - 5..i - 3].iter().all(u8::is_ascii_lowercase)
            && b[i - 2..i].iter().all(u8::is_ascii_uppercase)
        {
            found.insert(String::from_utf8_lossy(&b[i - 5..i]).into_owned());
            if found.len() >= 24 {
                break;
            }
        }
    }
    found.into_iter().collect()
}

/// The export check and the survey of `path`, as the CLI prints them. Each is
/// `None` when the file does not allow it (a page macro has no exchange
/// header, for one); the import preview still works then.
fn inspect(path: &Path) -> (Option<(bool, String)>, Option<String>) {
    use novo_interop::eplan::export_script as es;
    use std::io::Read;
    let mut head = Vec::new();
    let check = std::fs::File::open(path)
        .and_then(|f| f.take(64 * 1024).read_to_end(&mut head))
        .ok()
        .and_then(|_| {
            let manifest = path
                .parent()
                .and_then(|d| std::fs::read_to_string(d.join(es::MANIFEST_FILE)).ok());
            let head = String::from_utf8_lossy(&head);
            es::check_exchange(manifest.as_deref(), &head).ok()
        })
        .map(|(v, report)| {
            (
                v.is_current(),
                format!("{}\n{}", report.text().trim_end(), v.message()),
            )
        });
    let survey = std::fs::File::open(path)
        .ok()
        .and_then(|f| survey_pxf(std::io::BufReader::new(f)).ok())
        .map(|s| s.text().trim_end().to_owned())
        .filter(|t| !t.is_empty());
    (check, survey)
}

impl NovoApp {
    /// Asks for a PXF file and previews it.
    pub(super) fn import_eplan(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Import EPLAN project (PXF)")
            .add_filter("EPLAN project or page macro", &["epj", "emp", "xml"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_eplan_from(&path);
        }
    }

    /// Reads `path` and opens the preview. Nothing changes until the person
    /// confirms it.
    pub fn import_eplan_from(&mut self, path: &Path) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(doc) = self.backend.document() else {
            self.message(Level::Info, "Open a project first");
            return false;
        };
        let bytes = std::fs::read(path).map_err(|e| e.to_string());
        let langs = bytes
            .as_ref()
            .map(|b| languages_in(&String::from_utf8_lossy(b)))
            .unwrap_or_default();
        let read = bytes.and_then(|b| read_pxf(std::io::Cursor::new(b)).map_err(|e| e.to_string()));
        match read {
            Ok(file) => {
                let opts = PxfOptions::default();
                let lang = if langs.is_empty() || langs.contains(&opts.lang) {
                    opts.lang.clone()
                } else {
                    langs[0].clone()
                };
                let plan = plan_pxf(doc.project(), &file, &PxfOptions { lang: lang.clone() });
                let (check, survey) = inspect(path);
                self.wb.eplan_import = Some(EplanImport {
                    file: name,
                    plan,
                    data: file,
                    lang,
                    langs,
                    check,
                    survey,
                });
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not import {name}: {e}"));
                false
            }
        }
    }

    /// Applies the previewed import as one undo step.
    pub fn confirm_eplan_import(&mut self) -> bool {
        let Some(import) = self.wb.eplan_import.take() else {
            return false;
        };
        let Some(doc) = self.backend.document_mut() else {
            return false;
        };
        match import.plan.apply_with_changes(doc) {
            Ok(Some((n, changes))) => {
                self.canvas.document_changed(doc, &changes);
                self.backend.record(&changes);
                self.reports_heard(&changes);
                self.fix_current_page();
                self.message(
                    Level::Info,
                    format!(
                        "Imported {n} records from {}. Undo takes them back",
                        import.file
                    ),
                );
                true
            }
            Ok(None) => {
                self.message(Level::Info, format!("Nothing new in {}", import.file));
                false
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not import {}: {e}", import.file),
                );
                false
            }
        }
    }

    pub(super) fn eplan_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(import) = &mut self.wb.eplan_import else {
            return;
        };
        let mut lang_changed = false;
        let (mut go, mut cancel) = (false, false);
        let can_import = !import.plan.is_empty();
        let report = import.plan.report();
        let modal = egui::Modal::new(egui::Id::new("eplan_import")).show(ctx, |ui| {
            ui.set_width(500.0);
            ui.heading(format!("Import {}", import.file));
            ui.add_space(4.0);
            ui.label(import.plan.summary());
            ui.horizontal(|ui| {
                ui.label("Language of titles and texts");
                egui::ComboBox::from_id_salt("eplan_import_lang")
                    .selected_text(import.lang.clone())
                    .show_ui(ui, |ui| {
                        for l in &import.langs {
                            lang_changed |= ui
                                .selectable_value(&mut import.lang, l.clone(), l)
                                .changed();
                        }
                    });
                lang_changed |= ui.text_edit_singleline(&mut import.lang).lost_focus();
                if import.langs.is_empty() {
                    ui.weak("the file names no language");
                } else {
                    ui.weak(format!("in the file: {}", import.langs.join(", ")));
                }
            });
            if let Some((current, text)) = &import.check {
                ui.add_space(6.0);
                let colour = if *current {
                    ui.visuals().weak_text_color()
                } else {
                    novo_theme::tokens::WARNING
                };
                ui.colored_label(colour, text);
            }
            ui.add_space(6.0);
            ui.weak("An import only adds. Anything that exists keeps what the project says.");
            if !report.lossy.is_empty() || !report.notes.is_empty() {
                ui.add_space(8.0);
                ui.strong("Not carried over");
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        for l in &report.lossy {
                            ui.label(format!("{} {}: {}", l.count, l.what, l.why));
                        }
                        for n in &report.notes {
                            ui.weak(n);
                        }
                    });
            }
            if let Some(survey) = &import.survey {
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Survey of the file (names and counts, no values)")
                    .id_salt("eplan_survey")
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                ui.add(egui::Label::new(
                                    egui::RichText::new(survey).monospace().small(),
                                ));
                            });
                    });
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                go = ui
                    .add_enabled(can_import, egui::Button::new("Import"))
                    .clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if lang_changed
            && let Some(doc) = self.backend.document()
            && let Some(import) = self.wb.eplan_import.as_mut()
        {
            import.plan = plan_pxf(
                doc.project(),
                &import.data,
                &PxfOptions {
                    lang: import.lang.trim().to_owned(),
                },
            );
        }
        if go {
            self.confirm_eplan_import();
        } else if cancel || modal.should_close() {
            self.wb.eplan_import = None;
        }
    }
}
