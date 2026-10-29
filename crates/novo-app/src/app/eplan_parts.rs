//! File > Import parts > Import parts from an EPLAN export: reads a parts export
//! the person made in their own EPLAN (parts-management XML, or the
//! delimited text export) into the part catalog, after a preview of what
//! would be added, what would change, and which rows were rejected and why.
//!
//! Knowledge only: no EPLAN part data ships with KoperCAD. The reader is
//! `novo_interop::eplan::import_auto`; the Parts window's Import button
//! uses the same reader for the EPLAN XML found inside an EDZ or ZIP.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_parts::{
    Catalog, CatalogPart, ImportReport, PortalFile, PortalRead, looks_like_portal, read_portal,
    user_catalog_path,
};

use super::{Level, NovoApp, projects_dir};

/// Parts read from an EPLAN export, waiting for Add or Cancel.
pub(crate) struct EplanPartsImport {
    file: String,
    /// The file's text, kept so the manufacturer fallback can be changed
    /// without picking the file again.
    text: String,
    /// Every picked file by name when they are maker parts XML, which read
    /// together (a part, its connection points, its drilling, its accessories).
    portal_files: Vec<PortalFile>,
    /// What the maker parts XML reader found, with what it assumed.
    portal: Option<PortalRead>,
    /// Manufacturer for rows that carry none.
    maker: String,
    read: Result<ImportReport, String>,
    added: usize,
    changed: usize,
    unchanged: usize,
    /// Where the catalog is saved; the user's catalog when `None`.
    catalog_path: Option<PathBuf>,
}

/// One Windows-1252 byte as a character: 0x80 to 0x9F through the table,
/// everything else as Latin-1.
fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20ac}', '\u{81}', '\u{201a}', '\u{192}', '\u{201e}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{2c6}', '\u{2030}', '\u{160}', '\u{2039}', '\u{152}', '\u{8d}', '\u{17d}',
        '\u{8f}', '\u{90}', '\u{2018}', '\u{2019}', '\u{201c}', '\u{201d}', '\u{2022}', '\u{2013}',
        '\u{2014}', '\u{2dc}', '\u{2122}', '\u{161}', '\u{203a}', '\u{153}', '\u{9d}', '\u{17e}',
        '\u{178}',
    ];
    match b {
        0x80..=0x9f => HIGH[usize::from(b - 0x80)],
        _ => char::from(b),
    }
}

/// File text as a string: UTF-16 with a BOM, UTF-8, else Windows-1252 (German EPLAN exports).
pub(crate) fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = (0..rest.len() / 2)
            .map(|i| {
                let pair = [rest[2 * i], rest[2 * i + 1]];
                if le {
                    u16::from_le_bytes(pair)
                } else {
                    u16::from_be_bytes(pair)
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    match bytes {
        [0xff, 0xfe, rest @ ..] => utf16(rest, true),
        [0xfe, 0xff, rest @ ..] => utf16(rest, false),
        _ => match std::str::from_utf8(bytes) {
            Ok(text) => text.to_owned(),
            Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
        },
    }
}

/// Reads parts out of EPLAN export text (XML or delimited, picked by the
/// first character).
pub(crate) fn read_eplan_parts(text: &str, maker: &str) -> Result<ImportReport, String> {
    novo_interop::eplan::import_auto(text, maker.trim()).map_err(|e| e.to_string())
}

impl EplanPartsImport {
    fn new(
        file: String,
        text: String,
        portal_files: Vec<PortalFile>,
        existing: &Catalog,
        catalog_path: Option<PathBuf>,
    ) -> Self {
        let mut me = EplanPartsImport {
            file,
            text,
            portal_files,
            portal: None,
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
        if self.portal_files.is_empty() {
            self.portal = None;
            self.read = read_eplan_parts(&self.text, &self.maker);
        } else {
            self.read = match read_portal(
                &self.portal_files,
                self.maker.trim(),
                &super::revisions::today(),
            ) {
                Ok(read) => {
                    let skipped = read
                        .skipped
                        .iter()
                        .chain(&read.unattached)
                        .map(|s| (0, s.clone()))
                        .collect();
                    let report = ImportReport {
                        parts: read.parts.clone(),
                        skipped,
                        data_columns: Vec::new(),
                    };
                    self.portal = Some(read);
                    Ok(report)
                }
                Err(e) => {
                    self.portal = None;
                    Err(e.to_string())
                }
            };
        }
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

    /// What the maker parts XML reader assumed, empty for other files.
    #[cfg(test)]
    pub(crate) fn portal_assumptions(&self) -> &[String] {
        self.portal.as_ref().map_or(&[], |p| p.assumed.as_slice())
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
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Import parts from a CAD parts export or maker parts XML files")
            .add_filter(
                "Parts export or maker parts XML (XML or text)",
                &["xml", "csv", "txt", "tsv"],
            );
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(paths) = dialog.pick_files() {
            self.import_eplan_parts_from_files(&paths, None);
        }
    }

    /// Reads `path` and opens the preview; nothing is saved until the person
    /// adds the parts. `catalog_path` is the catalog file to merge into
    /// (default: the user's catalog).
    #[cfg(test)]
    pub(crate) fn import_eplan_parts_from(
        &mut self,
        path: &Path,
        catalog_path: Option<PathBuf>,
    ) -> bool {
        self.import_eplan_parts_from_files(&[path.to_path_buf()], catalog_path)
    }

    /// Reads one or more files and opens the preview. Several files, or a
    /// maker parts XML (`partsmanagement`), are read together as one set.
    pub(crate) fn import_eplan_parts_from_files(
        &mut self,
        paths: &[PathBuf],
        catalog_path: Option<PathBuf>,
    ) -> bool {
        const MAX_FILES: usize = 5000;
        if paths.len() > MAX_FILES {
            self.message(
                Level::Error,
                format!("{} files picked; import at most {MAX_FILES} at once (or use the parts inbox for a whole folder)", paths.len()),
            );
            return false;
        }
        let mut files: Vec<(String, String)> = Vec::new();
        for path in paths {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match std::fs::read(path) {
                Ok(bytes) => files.push((name, decode_text(&bytes))),
                Err(e) => {
                    self.message(Level::Error, format!("Could not read {name}: {e}"));
                    return false;
                }
            }
        }
        let Some((first_name, first_text)) = files.first().cloned() else {
            return false;
        };
        let portal = files.iter().any(|(_, t)| looks_like_portal(t));
        if !portal && files.len() > 1 {
            self.message(
                Level::Error,
                "Pick one EPLAN parts export, or several maker parts XML files (root element partsmanagement)",
            );
            return false;
        }
        let label = if files.len() > 1 {
            format!("{first_name} and {} more", files.len() - 1)
        } else {
            first_name.clone()
        };
        let portal_files = if portal {
            files
                .into_iter()
                .map(|(name, text)| PortalFile { name, text })
                .collect()
        } else {
            Vec::new()
        };
        let existing = existing_catalog(catalog_path.as_deref());
        let import = EplanPartsImport::new(
            label.clone(),
            first_text,
            portal_files,
            &existing,
            catalog_path,
        );
        if let Err(e) = &import.read {
            self.message(Level::Error, format!("No parts in {label}: {e}"));
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
            // Maker parts XML adds to a part that is already there: its price
            // and classification from a catalog file stay.
            let merged = if import.portal.is_some() {
                catalog.get(&part.key()).map(|old| old.with_maker_xml(part))
            } else {
                None
            };
            catalog.upsert(merged.unwrap_or_else(|| part.clone()));
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
        let modal = super::panel::modal(
            ctx,
            "eplan_parts_import",
            format!("Parts from {}", import.file),
            [560.0, 540.0],
            |ui| {
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
                if let Some(portal) = &import.portal {
                    portal_summary(ui, portal);
                }
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
                                let mut line = format!(
                                    "{}  {}  {}",
                                    p.manufacturer, p.order_number, p.description
                                );
                                if import.portal.is_some() {
                                    line.push_str(&format!(
                                        "  ({} connection points, {} holes, {} accessories)",
                                        p.terminals.len(),
                                        p.drilling.len(),
                                        p.accessories.len()
                                    ));
                                }
                                ui.label(line);
                            }
                            if report.parts.len() > 200 {
                                ui.weak(format!("... and {} more", report.parts.len() - 200));
                            }
                        });
                    if !report.skipped.is_empty() {
                        ui.add_space(6.0);
                        ui.colored_label(
                            crate::theme::ui_palette().warning,
                            format!("{} row(s) left out or not joined", report.skipped.len()),
                        );
                        egui::ScrollArea::vertical()
                            .id_salt("eplan_parts_rejects")
                            .max_height(120.0)
                            .show(ui, |ui| {
                                for (line, why) in &report.skipped {
                                    if *line == 0 {
                                        ui.label(why);
                                    } else {
                                        ui.label(format!("line {line}: {why}"));
                                    }
                                }
                            });
                    }
                }
                ui.add_space(6.0);
                ui.weak("Only your own files are read. Nothing is added until you choose Add.");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    go = ui
                        .add_enabled(can_add, egui::Button::new("Add to catalog"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            },
        );
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

/// What the maker parts XML reader read, what it assumed and what it kept
/// without understanding, so the person can judge the import.
fn portal_summary(ui: &mut egui::Ui, portal: &PortalRead) {
    let c = &portal.counts;
    ui.add_space(4.0);
    ui.label(format!(
        "Read from {} file(s): {} connection points on {} parts ({} with a wire range, {} with torque), {} holes on {} parts, {} accessories on {} parts.",
        c.files,
        c.terminals,
        c.parts_with_terminals,
        c.with_wire_range,
        c.with_torque,
        c.holes,
        c.parts_with_drilling,
        c.accessories,
        c.parts_with_accessories,
    ));
    let palette = crate::theme::ui_palette();
    egui::CollapsingHeader::new("What was assumed, and what was not understood")
        .id_salt("portal_assumed")
        .default_open(true)
        .show(ui, |ui| {
            for a in &portal.assumed {
                ui.colored_label(palette.warning, format!("Assumed: {a}"));
            }
            if portal.unrecognised.is_empty() {
                ui.weak("Every attribute was placed.");
            } else {
                ui.weak("Kept as written, not understood (name and how often):");
                for (name, n) in portal.unrecognised.iter().take(12) {
                    ui.weak(format!("  {name} x{n}"));
                }
                if portal.unrecognised.len() > 12 {
                    ui.weak(format!("  and {} more", portal.unrecognised.len() - 12));
                }
            }
            if ui
                .button("Copy format report")
                .on_hover_text("Names only, no values: send it when a maker's file reads badly")
                .clicked()
            {
                ui.ctx().copy_text(portal.format_report());
            }
        });
}

#[cfg(test)]
mod tests {
    use super::decode_text;

    #[test]
    fn windows_1252_and_utf8_both_decode() {
        assert_eq!(decode_text(b"Weidm\xFCller"), "Weidm\u{fc}ller");
        assert_eq!(decode_text(b"\x80 \x96"), "\u{20ac} \u{2013}");
        assert_eq!(decode_text("Weidm\u{fc}ller".as_bytes()), "Weidm\u{fc}ller");
    }
}
