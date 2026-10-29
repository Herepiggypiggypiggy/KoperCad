//! File > Import parts > Import parts from a CAD parts export: reads a parts
//! export the person made in their own EPLAN (parts-management XML, or the
//! delimited text export) or the parts database files of their EPLAN
//! installation (`.adb`, `.eod`) into the part catalog, after a preview of
//! what would be added, what would change, and which rows were rejected and
//! why. Maker codes are merged into one name per maker (SE and SCHNEI are
//! Schneider Electric); the person can name codes the program does not know,
//! and the names are kept for the next import.
//!
//! Knowledge only: no EPLAN part data ships with KoperCAD. The readers are
//! `novo_interop::eplan::import_auto` and `import_eod`; the Parts window's
//! Import button uses the first for the EPLAN XML found inside an EDZ or
//! ZIP. Database files are read on a worker thread.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};

use eframe::egui;
use novo_interop::eplan::{MakerSeen, import_eod, looks_like_eod};
use novo_parts::{
    ALIAS_FILE_NAME, Catalog, CatalogPart, ImportReport, MakerAliases, user_catalog_path,
};

use super::{Level, NovoApp, projects_dir};

/// Parts read from an EPLAN export, waiting for Add or Cancel.
pub(crate) struct EplanPartsImport {
    file: String,
    /// The file's content, kept so the manufacturer fallback and the maker
    /// names can be changed without picking the file again.
    source: Source,
    aliases: MakerAliases,
    /// Where the aliases are saved.
    alias_path: Option<PathBuf>,
    /// Maker codes in a database file, with the names they got.
    makers: Vec<MakerSeen>,
    /// The names being typed, by code.
    typing: BTreeMap<String, String>,
    /// Manufacturer for rows that carry none.
    maker: String,
    read: Result<ImportReport, String>,
    added: usize,
    changed: usize,
    unchanged: usize,
    /// Where the catalog is saved; the user's catalog when `None`.
    catalog_path: Option<PathBuf>,
}

/// What the chosen file holds.
enum Source {
    /// An XML or delimited text export.
    Text(String),
    /// The bytes of an `.adb` or `.eod` article file.
    Articles(Vec<u8>),
}

/// A file being read on a worker thread.
pub(crate) struct EplanPartsJob {
    rx: Receiver<Result<EplanPartsImport, String>>,
    file: String,
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

/// The alias file that goes with a catalog file (the user's by default).
fn alias_path_for(catalog_path: Option<&Path>) -> Option<PathBuf> {
    match catalog_path {
        Some(p) => Some(p.with_file_name(ALIAS_FILE_NAME)),
        None => MakerAliases::default_path(),
    }
}

/// Whether the file is a parts database rather than text: by extension, or
/// by what the bytes hold when the extension says nothing.
fn is_article_file(path: &Path, bytes: &[u8]) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "adb" | "eod" => true,
        "xml" | "csv" | "txt" | "tsv" => false,
        _ => looks_like_eod(bytes),
    }
}

impl EplanPartsImport {
    fn new(
        file: String,
        source: Source,
        existing: &Catalog,
        catalog_path: Option<PathBuf>,
    ) -> Self {
        let alias_path = alias_path_for(catalog_path.as_deref());
        let aliases = alias_path
            .as_deref()
            .map(MakerAliases::load)
            .unwrap_or_default();
        let mut me = EplanPartsImport {
            file,
            source,
            aliases,
            alias_path,
            makers: Vec::new(),
            typing: BTreeMap::new(),
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
        self.read = match &self.source {
            Source::Text(text) => read_eplan_parts(text, &self.maker),
            Source::Articles(bytes) => import_eod(bytes, &self.aliases, self.maker.trim())
                .map(|read| {
                    self.makers = read.makers;
                    read.report
                })
                .map_err(|e| e.to_string()),
        };
        if let Ok(report) = &self.read {
            let preview = existing.preview_merge(&report.parts);
            self.added = preview.added.len();
            self.changed = preview.changed.len();
            self.unchanged = preview.unchanged;
        }
    }

    #[cfg(test)]
    pub(crate) fn name_maker(&mut self, code: &str, name: &str) {
        self.aliases.set(code, name);
    }

    #[cfg(test)]
    pub(crate) fn reread_for_test(&mut self, existing: &Catalog) {
        self.reread(existing);
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

/// Largest file read into memory.
const MAX_FILE_BYTES: u64 = 1_000_000_000;

/// Holds the read for `KOPERCAD_QA_SLOW_READ_MS` milliseconds (at most a
/// minute) so a QA scenario can photograph the window while it reads.
fn qa_hold() {
    let ms = std::env::var("KOPERCAD_QA_SLOW_READ_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(0, |ms| ms.min(60_000));
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// Reads `path` and builds the preview; runs on the worker thread.
fn read_job(
    path: &Path,
    name: &str,
    catalog_path: Option<PathBuf>,
) -> Result<EplanPartsImport, String> {
    qa_hold();
    let size = std::fs::metadata(path).map_or(0, |m| m.len());
    if size > MAX_FILE_BYTES {
        return Err(format!(
            "{name} is {} MB; a parts database larger than {} MB is not read. Export the parts you need from EPLAN instead.",
            size / 1_000_000,
            MAX_FILE_BYTES / 1_000_000
        ));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("Could not read {name}: {e}"))?;
    let existing = existing_catalog(catalog_path.as_deref());
    let source = if is_article_file(path, &bytes) {
        Source::Articles(bytes)
    } else {
        Source::Text(decode_text(&bytes))
    };
    let import = EplanPartsImport::new(name.to_owned(), source, &existing, catalog_path);
    match &import.read {
        Err(e) => Err(format!("No parts in {name}: {e}")),
        Ok(_) => Ok(import),
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
            .set_title("Import parts from an EPLAN export")
            .add_filter(
                "EPLAN parts export (XML or text)",
                &["xml", "csv", "txt", "tsv"],
            );
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.start_eplan_parts_job(&path, None);
        }
    }

    /// Like [`Self::import_eplan_parts_from`], but the file is read and
    /// decoded on a worker thread (a parts database is several megabytes);
    /// the preview opens when it is done.
    pub(crate) fn start_eplan_parts_job(&mut self, path: &Path, catalog_path: Option<PathBuf>) {
        if self.wb.eplan_parts_job.is_some() {
            return;
        }
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel();
        let (path, name) = (path.to_path_buf(), file.clone());
        let spawned = std::thread::Builder::new()
            .name("eplan-parts".into())
            .spawn(move || {
                let done = std::panic::catch_unwind(|| read_job(&path, &name, catalog_path))
                    .unwrap_or_else(|_| Err(format!("Could not read {name}")));
                let _ = tx.send(done);
            });
        match spawned {
            Ok(_) => self.wb.eplan_parts_job = Some(EplanPartsJob { rx, file }),
            Err(e) => self.message(
                Level::Error,
                format!("Could not start reading {file}: {e}. Close other programs and try again."),
            ),
        }
    }

    /// Opens the preview of a finished read, once per frame.
    fn poll_eplan_parts_job(&mut self) {
        let Some(job) = &self.wb.eplan_parts_job else {
            return;
        };
        let answer = match job.rx.try_recv() {
            Ok(done) => done,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(format!("Reading {} stopped", job.file)),
        };
        self.wb.eplan_parts_job = None;
        match answer {
            Ok(import) => self.wb.eplan_parts = Some(import),
            Err(e) => self.message(Level::Error, e),
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
        let source = if is_article_file(path, &bytes) {
            Source::Articles(bytes)
        } else {
            Source::Text(decode_text(&bytes))
        };
        let import = EplanPartsImport::new(name.clone(), source, &existing, catalog_path);
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
        if import.aliases != MakerAliases::default()
            && let Some(p) = &import.alias_path
            && let Err(e) = import.aliases.save(p)
        {
            self.message(
                Level::Warning,
                format!("Could not save the maker names: {e}"),
            );
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
        self.poll_eplan_parts_job();
        if let Some(job) = &self.wb.eplan_parts_job {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            let modal = super::panel::modal(
                ctx,
                "eplan_parts_reading",
                format!("Reading {}", job.file),
                [380.0, 90.0],
                |ui| {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading the parts. You can keep this window open.");
                    });
                },
            );
            if modal.should_close() {
                self.wb.eplan_parts_job = None;
            }
        }
        let Some(import) = &mut self.wb.eplan_parts else {
            return;
        };
        let (mut go, mut cancel, mut maker_changed) = (false, false, false);
        let mut aliases_changed = false;
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
                if !import.makers.is_empty() {
                    makers_ui(ui, import, &mut aliases_changed);
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
                            crate::theme::ui_palette().warning,
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
            },
        );
        if maker_changed || aliases_changed {
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

/// The maker codes of a database file and the name each one gets. A name
/// can be typed over; it is kept for the next import.
fn makers_ui(ui: &mut egui::Ui, import: &mut EplanPartsImport, changed: &mut bool) {
    let palette = crate::theme::ui_palette();
    let unknown = import.makers.iter().filter(|m| !m.known).count();
    ui.add_space(4.0);
    ui.strong("Makers in this file");
    ui.weak("One name per maker. Type a name next to a code to merge it with a maker.");
    let rows: Vec<MakerSeen> = import.makers.clone();
    egui::ScrollArea::vertical()
        .id_salt("eplan_parts_makers")
        .max_height(120.0)
        .show(ui, |ui| {
            egui::Grid::new("eplan_parts_maker_grid")
                .num_columns(3)
                .show(ui, |ui| {
                    for m in &rows {
                        ui.label(format!("{} ({})", m.code, m.parts));
                        let text = import
                            .typing
                            .entry(m.code.clone())
                            .or_insert_with(|| m.name.clone());
                        let r = ui.add(egui::TextEdit::singleline(text).desired_width(200.0));
                        if r.lost_focus() && *text != m.name {
                            import.aliases.set(&m.code, text);
                            import.typing.remove(&m.code);
                            *changed = true;
                        }
                        if m.known {
                            ui.weak("known");
                        } else {
                            ui.colored_label(palette.warning, "not known");
                        }
                        ui.end_row();
                    }
                });
        });
    if unknown > 0 {
        ui.weak(format!(
            "{unknown} code(s) are not known and are added under the code itself."
        ));
    }
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
