//! The link to the PLC engineering tools in the app (novo-plc `link`,
//! docs/design/plc-link.md): File > Export PLC writes the I/O as the file
//! the programmer imports; File > Import PLC tags reads the programmer's
//! file back, shows what would change and applies it in one undo step.
//!
//! Exports go next to the project file (`Plant 2 PLC.xml`, or
//! `Plant 2 PLC <station>.xml` per station) under the same rule as the
//! spreadsheet: only a file this export wrote is replaced.

use eframe::egui;
use novo_plc::link::{
    ExportOptions, ImportPlan, LinkFormat, apply_import, export, plan_import, read,
};
use novo_plc::{FromKey, PlcIndex};

use super::pdf::write_replacing;
use super::sheets::{hash, ours_or_free};
use super::{Level, NovoApp, projects_dir};

impl NovoApp {
    /// Writes one of the PLC files next to the project; returns the paths.
    pub(super) fn export_plc(&mut self, format: LinkFormat) -> Vec<std::path::PathBuf> {
        let Some((dir, stem)) = self.export_base() else {
            return Vec::new();
        };
        self.export_plc_to(format, &dir, &stem)
    }

    /// Writes into `dir`, naming files after `stem`.
    pub fn export_plc_to(
        &mut self,
        format: LinkFormat,
        dir: &std::path::Path,
        stem: &str,
    ) -> Vec<std::path::PathBuf> {
        let project_name = self.project_name().unwrap_or_default();
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let functions: &dyn novo_plc::index::Functions = match self.library() {
            Some(library) => library,
            None => &FromKey,
        };
        let index = PlcIndex::build(project, functions);
        if index.cards().is_empty() {
            self.message(
                Level::Info,
                "The project has no PLC cards. Add one with: add card DI16 DQ8",
            );
            return Vec::new();
        }
        let options = ExportOptions {
            include_spares: false,
            stamp: iso_now(),
            project: project_name,
        };
        let out = export(project, &index, format, &options);
        let mut written = Vec::new();
        for file in &out.files {
            let name = if file.suffix.is_empty() {
                format!("{stem} PLC")
            } else {
                format!("{stem} PLC {}", file.suffix)
            };
            let path = ours_or_free(dir, &name, file.extension, &self.wb.sheet_exports);
            match write_replacing(&path, &file.bytes) {
                Ok(()) => {
                    self.wb
                        .sheet_exports
                        .insert(path.clone(), hash(&file.bytes));
                    written.push(path);
                }
                Err(e) => {
                    self.message(
                        Level::Error,
                        format!("Could not write {}: {e}", path.display()),
                    );
                    return written;
                }
            }
        }
        let tags: usize = out.files.iter().map(|f| f.tags).sum();
        if let Some(last) = written.last() {
            let what = if written.len() == 1 {
                last.display().to_string()
            } else {
                format!("{} files in {}", written.len(), dir.display())
            };
            let count = if tags > 0 {
                format!(" ({tags} tags)")
            } else {
                String::new()
            };
            let tool = match format {
                LinkFormat::PlcOpen => ". Not yet tried in CODESYS",
                LinkFormat::Tia => ". Not yet tried in TIA Portal",
                LinkFormat::Studio5000 => ". Not yet tried in Studio 5000",
                _ => "",
            };
            self.message_file(format!("Exported {what}{count}{tool}"), last);
            self.wb.last_export = Some(last.clone());
        }
        if let Some(first) = out.warnings.first() {
            let more = out.warnings.len() - 1;
            let tail = if more > 0 {
                format!(" (and {more} more)")
            } else {
                String::new()
            };
            self.message(Level::Warning, format!("{first}{tail}"));
        }
        written
    }

    /// Asks for a tag file and opens the import preview.
    pub(super) fn plc_import_tags(&mut self) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Import PLC tags")
            .add_filter("PLC tag file", &["csv", "xml", "xlsx"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.plc_import_tags_from(&path);
        }
    }

    /// Reads the file at `path` and opens the preview. Nothing changes in
    /// the project until the preview is applied.
    pub fn plc_import_tags_from(&mut self, path: &std::path::Path) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        const MAX_BYTES: u64 = 32 * 1024 * 1024;
        if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
            self.message(
                Level::Error,
                format!("{name} is larger than 32 MB, which is too big for a tag file"),
            );
            return false;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message(Level::Error, format!("Could not read {name}: {e}"));
                return false;
            }
        };
        let file = match read(&bytes) {
            Ok(f) => f,
            Err(e) => {
                self.message(Level::Error, format!("{name}: {e}"));
                return false;
            }
        };
        let Some(doc) = self.backend.document() else {
            return false;
        };
        let functions: &dyn novo_plc::index::Functions = match self.library() {
            Some(library) => library,
            None => &FromKey,
        };
        let index = PlcIndex::build(doc.project(), functions);
        let plan = plan_import(doc.project(), &index, &file);
        self.wb.plc_import = Some((name, plan));
        true
    }

    /// The import preview: every channel that changes, and what was left
    /// alone. Enter applies, Esc leaves the project untouched.
    pub(super) fn plc_import_dialog(&mut self, ctx: &egui::Context) {
        let Some((name, plan)) = &self.wb.plc_import else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("plc_import")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.heading("Import PLC tags");
            ui.label(egui::RichText::new(name).weak());
            ui.add_space(4.0);
            ui.label(plan.summary());
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(320.0)
                .show(ui, |ui| {
                    for u in &plan.updates {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(format!("{}  {}", u.channel, u.address));
                            if let Some(s) = &u.new_signal {
                                ui.label(format!("signal \"{}\" → \"{s}\"", u.old_signal));
                            }
                            if let Some(t) = &u.new_tag {
                                ui.label(format!("tag {} → {t}", u.old_tag));
                            }
                        });
                    }
                    notes(
                        ui,
                        "Not found in the project",
                        plan.unmatched.iter().map(|(l, t)| format!("line {l}: {t}")),
                    );
                    notes(
                        ui,
                        "Addresses are never changed by an import",
                        plan.address_notes.iter().cloned(),
                    );
                    notes(
                        ui,
                        "Spare channels (nothing drawn to hold a signal)",
                        plan.spare.iter().map(|(c, t)| format!("{c}: {t}")),
                    );
                    notes(ui, "Check", plan.warnings.iter().cloned());
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let apply = ui
                    .add_enabled_ui(!plan.is_empty(), |ui| {
                        novo_theme::widgets::primary_button(ui, "Apply (Enter)")
                    })
                    .inner
                    .clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                (apply, cancel)
            })
            .inner
        });
        let (apply, cancel) = modal.inner;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let can_apply = self
            .wb
            .plc_import
            .as_ref()
            .is_some_and(|(_, plan)| !plan.is_empty());
        if (apply || enter) && can_apply {
            if let Some((_, plan)) = self.wb.plc_import.take() {
                self.apply_plc_import(&plan);
            }
        } else if cancel || modal.should_close() {
            self.wb.plc_import = None;
        }
    }

    /// Applies a plan from [`plc_import_tags_from`](Self::plc_import_tags_from)
    /// as one undo step.
    pub fn apply_plc_import(&mut self, plan: &ImportPlan) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Import PLC tags", |tx| apply_import(tx, plan));
        match result {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                let n = plan.updates.len();
                self.message(
                    Level::Info,
                    format!(
                        "Imported PLC tags: {n} channel{} updated",
                        if n == 1 { "" } else { "s" }
                    ),
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not import PLC tags: {e}")),
        }
    }
}

pub(super) fn notes(ui: &mut egui::Ui, title: &str, lines: impl Iterator<Item = String>) {
    let lines: Vec<String> = lines.collect();
    if lines.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new(format!("{title} ({})", lines.len())).weak());
    for line in lines.iter().take(12) {
        ui.label(line);
    }
    if lines.len() > 12 {
        ui.label(format!("... and {} more", lines.len() - 12));
    }
}

/// The current UTC time as `2026-10-15T19:29:38`, for file headers.
fn iso_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_text_has_the_iso_shape() {
        let t = iso_now();
        assert_eq!(t.len(), 19);
        assert!(t.starts_with("20") && t.as_bytes()[10] == b'T');
    }
}
