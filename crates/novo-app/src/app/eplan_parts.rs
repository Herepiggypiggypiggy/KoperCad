//! File > Import > Import parts from an EPLAN export: reads a parts export
//! the person made in their own EPLAN (parts-management XML, or the
//! delimited text export) into the part catalog, after a preview of what
//! would be added, what would change, and which rows were rejected and why.
//!
//! Knowledge only: no EPLAN part data ships with KoperCAD. The reader is
//! `novo_interop::eplan::import_auto`; the Parts window's Import button
//! uses the same reader for the EPLAN XML found inside an EDZ or ZIP.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_parts::{Catalog, CatalogPart, ImportReport, user_catalog_path};

use super::{Level, NovoApp, projects_dir};

/// Parts read from an EPLAN export, waiting for Add or Cancel.
pub(crate) struct EplanPartsImport {
    file: String,
    /// The file's text, kept so the manufacturer fallback can be changed
    /// without picking the file again.
    text: String,
    /// Manufacturer for rows that carry none.
    maker: String,
    read: Result<ImportReport, String>,
    added: usize,
    changed: usize,
    unchanged: usize,
    /// Where the catalog is saved; the user's catalog when `None`.
    catalog_path: Option<PathBuf>,
}

/// File text as a string: UTF-8 (BOM allowed) or UTF-16 with a BOM.
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, true),
        [0xfe, 0xff, rest @ ..] => utf16(rest, false),
        _ => String::from_utf8_lossy(bytes).into_owned(),
    }
}

/// Reads parts out of EPLAN export text (XML or delimited, picked by the
/// first character).
pub(crate) fn read_eplan_parts(text: &str, maker: &str) -> Result<ImportReport, String> {
    novo_interop::eplan::import_auto(text, maker.trim()).map_err(|e| e.to_string())
}

impl EplanPartsImport {
    fn new(file: String, text: String, existing: &Catalog, catalog_path: Option<PathBuf>) -> Self {
        let mut me = EplanPartsImport {
            file,
            text,
            maker: String::new(),
            read: Err(String::new()),
            added: 0,
            changed: 0,
            unchanged: 0,
            catalog_path,
        };
        me.reread(existing);
        me
    }

    fn reread(&mut self, existing: &Catalog) {
        self.read = read_eplan_parts(&self.text, &self.maker);
        if let Ok(report) = &self.read {
            let preview = existing.preview_merge(&report.parts);
            self.added = preview.added.len();
            self.changed = preview.changed.len();
            self.unchanged = preview.unchanged;
        }
    }

    /// The parts that would be added or replaced.
    pub(crate) fn parts(&self) -> &[CatalogPart] {
        self.read.as_ref().map_or(&[], |r| r.parts.as_slice())
    }

    /// `(new, replacing, already there)`.
    pub(crate) fn counts(&self) -> (usize, usize, usize) {
        (self.added, self.changed, self.unchanged)
    }

    pub(crate) fn rejected(&self) -> usize {
        self.read.as_ref().map_or(0, |r| r.skipped.len())
    }
}

fn existing_catalog(path: Option<&Path>) -> Catalog {
    let path = path.map(Path::to_path_buf).or_else(user_catalog_path);
    path.filter(|p| p.exists())
        .and_then(|p| Catalog::load(&p).ok())
        .unwrap_or_default()
}

impl NovoApp {
    /// The command `parts.import_eplan`: asks for the person's own EPLAN
    /// parts export and previews it.
    pub(super) fn import_eplan_parts(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Import parts from an EPLAN export")
            .add_filter(
                "EPLAN parts export (XML or text)",
                &["xml", "csv", "txt", "tsv"],
            );
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_eplan_parts_from(&path, None);
        }
    }

    /// Reads `path` and opens the preview; nothing is saved until the person
    /// adds the parts. `catalog_path` is the catalog file to merge into
    /// (default: the user's catalog).
    pub(crate) fn import_eplan_parts_from(
        &mut self,
        path: &Path,
        catalog_path: Option<PathBuf>,
    ) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message(Level::Error, format!("Could not read {name}: {e}"));
                return false;
            }
        };
        let existing = existing_catalog(catalog_path.as_deref());
        let import =
            EplanPartsImport::new(name.clone(), decode_text(&bytes), &existing, catalog_path);
        if let Err(e) = &import.read {
            self.message(Level::Error, format!("No parts in {name}: {e}"));
            return false;
        }
        self.wb.eplan_parts = Some(import);
        true
    }

    /// Merges the previewed parts into the catalog and saves it.
    pub(crate) fn confirm_eplan_parts(&mut self) -> bool {
        let Some(import) = self.wb.eplan_parts.take() else {
            return false;
        };
        let Some(path) = import.catalog_path.clone().or_else(user_catalog_path) else {
            self.message(Level::Error, "No profile folder to save the catalog in");
            return false;
        };
        let mut catalog = existing_catalog(Some(&path));
        for part in import.parts() {
            catalog.upsert(part.clone());
        }
        if let Err(e) = catalog.save(&path) {
            self.message(Level::Error, format!("Could not save the catalog: {e}"));
            return false;
        }
        let (added, changed, _) = import.counts();
        self.message(
            Level::Info,
            format!(
                "Parts from {}: {added} added, {changed} updated, {} rejected",
                import.file,
                import.rejected()
            ),
        );
        if import.catalog_path.is_none()
            && let Some(win) = self.wb.parts.as_mut()
            && let Ok(c) = Catalog::load_default()
        {
            win.set_catalog(c);
        }
        true
    }

    pub(super) fn eplan_parts_dialog(&mut self, ctx: &egui::Context) {
        let Some(import) = &mut self.wb.eplan_parts else {
            return;
        };
        let (mut go, mut cancel, mut maker_changed) = (false, false, false);
        let can_add = !import.parts().is_empty();
        let modal = egui::Modal::new(egui::Id::new("eplan_parts_import")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.heading(format!("Parts from {}", import.file));
            ui.add_space(4.0);
            let (added, changed, unchanged) = import.counts();
            ui.label(format!(
                "{} part(s) read: {added} new, {changed} would replace a catalog entry, {unchanged} already there as they are.",
                import.parts().len()
            ));
            ui.horizontal(|ui| {
                ui.label("Manufacturer for rows without one");
                maker_changed = ui.text_edit_singleline(&mut import.maker).changed();
            });
            if let Ok(report) = &import.read {
                if !report.data_columns.is_empty() {
                    ui.weak(format!(
                        "Kept as technical data: {}",
                        report.data_columns.join(", ")
                    ));
                }
                egui::ScrollArea::vertical()
                    .max_height(220.0)
                    .show(ui, |ui| {
                        for p in report.parts.iter().take(200) {
                            ui.label(format!(
                                "{}  {}  {}",
                                p.manufacturer, p.order_number, p.description
                            ));
                        }
                        if report.parts.len() > 200 {
                            ui.weak(format!("... and {} more", report.parts.len() - 200));
                        }
                    });
                if !report.skipped.is_empty() {
                    ui.add_space(6.0);
                    ui.colored_label(
                        novo_theme::tokens::WARNING,
                        format!("{} row(s) rejected", report.skipped.len()),
                    );
                    egui::ScrollArea::vertical()
                        .id_salt("eplan_parts_rejects")
                        .max_height(120.0)
                        .show(ui, |ui| {
                            for (line, why) in &report.skipped {
                                ui.label(format!("line {line}: {why}"));
                            }
                        });
                }
            }
            ui.add_space(6.0);
            ui.weak("Only your own export is read. Nothing is added until you choose Add.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                go = ui
                    .add_enabled(can_add, egui::Button::new("Add to catalog"))
                    .clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if maker_changed {
            let existing = existing_catalog(import.catalog_path.as_deref());
            import.reread(&existing);
        }
        if go {
            self.confirm_eplan_parts();
        } else if cancel || modal.should_close() {
            self.wb.eplan_parts = None;
        }
    }
}
