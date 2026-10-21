//! Import and export beyond DXF and spreadsheets (novo-interop, WS-24):
//! the cabinet as STEP, the plant as AutomationML (out and in), and DWG
//! through a free converter. File > Import and export, and the command field
//! (`export step`, `export automationml`, `import automationml`,
//! `import dwg`).
//!
//! Like the other exports, a saved project's files go next to its file and
//! take its name; only a file KoperCAD wrote is replaced, anything else keeps
//! its name and the export takes the next free one. Every import shows what it
//! would do, and every export says what the format could not carry.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui;
use novo_interop::aml::{self, AmlOptions, AmlPlan};
use novo_interop::dwg;
use novo_interop::eplan::export_script as eplan_export;
use novo_interop::report::Report;
use novo_interop::step::{self, StepOptions};

use super::pdf::write_replacing;
use super::{Level, NovoApp, projects_dir};

/// An AutomationML import waiting for the person to confirm.
pub(super) struct AmlImport {
    file: String,
    plan: AmlPlan,
    report: Report,
}

/// A DWG being converted in the background.
pub(super) struct EplanGuide {
    folder: PathBuf,
    dutch: bool,
}

pub(super) struct DwgJob {
    name: String,
    rx: Receiver<Result<PathBuf, String>>,
}

fn now_iso() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    step::iso_utc(secs)
}

/// Characters Windows does not allow in a file name, replaced.
fn file_safe(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

/// `dir/<base>.<ext>` when free or a file KoperCAD wrote (`ours` says so), else
/// the first such of `<base> 2.<ext>`...
fn replaceable(dir: &Path, base: &str, ext: &str, ours: impl Fn(&[u8]) -> bool) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{base}.{ext}")
        } else {
            format!("{base} {n}.{ext}")
        };
        let path = dir.join(name);
        if !path.exists() || std::fs::read(&path).is_ok_and(|b| ours(&b)) {
            return path;
        }
        n += 1;
    }
}

fn step_is_ours(bytes: &[u8]) -> bool {
    bytes.starts_with(b"ISO-10303-21;")
        && String::from_utf8_lossy(&bytes[..bytes.len().min(400)])
            .contains("KoperCAD cabinet export")
}

fn aml_is_ours(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(&bytes[..bytes.len().min(1500)]).contains("<WriterName>KoperCAD<")
}

impl NovoApp {
    /// The cabinet of every enclosure with a mounting plate, as STEP.
    pub(super) fn export_step(&mut self) -> Vec<PathBuf> {
        let Some((dir, stem)) = self.export_base() else {
            return Vec::new();
        };
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let options = StepOptions {
            created: now_iso(),
            author: self.settings.initials.clone(),
            ..StepOptions::default()
        };
        let files = step::export_all(doc.project(), &options);
        if files.is_empty() {
            self.message(
                Level::Info,
                "No enclosure has a mounting plate yet. Set one in the Cabinet view, then export",
            );
            return Vec::new();
        }
        let mut written = Vec::new();
        let mut report = Report::default();
        for f in &files {
            let base = if files.len() == 1 {
                format!("{stem} cabinet")
            } else {
                format!("{stem} {}", file_safe(&f.tag))
            };
            let path = replaceable(&dir, &base, "stp", step_is_ours);
            match write_replacing(&path, f.export.text.as_bytes()) {
                Ok(()) => written.push(path),
                Err(e) => {
                    self.message(
                        Level::Error,
                        format!("Could not write {}: {e}", path.display()),
                    );
                    return written;
                }
            }
            for (what, n) in &f.export.report.moved {
                report.moved(what.clone(), *n);
            }
            for l in &f.export.report.lossy {
                report.lost(l.what.clone(), l.count, l.why.clone());
            }
            for n in &f.export.report.notes {
                if !report.notes.contains(n) {
                    report.note(n.clone());
                }
            }
        }
        self.finish_export(&written, "STEP", &report);
        written
    }

    /// The plant (structure, devices, terminals, cables, nets) as AutomationML.
    pub(super) fn export_aml(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let doc = self.backend.document()?;
        let export = aml::export_project(
            doc.project(),
            &AmlOptions {
                created: now_iso(),
                ..AmlOptions::default()
            },
        );
        let path = replaceable(&dir, &stem, "aml", aml_is_ours);
        if let Err(e) = write_replacing(&path, export.text.as_bytes()) {
            self.message(
                Level::Error,
                format!("Could not write {}: {e}", path.display()),
            );
            return None;
        }
        self.finish_export(std::slice::from_ref(&path), "AutomationML", &export.report);
        Some(path)
    }

    /// The project as an EPLAN PXF file (`.epj`) for EPLAN to import. The
    /// report says what was left out and that no real EPLAN has opened a
    /// KoperCAD file yet.
    pub(super) fn export_eplan(
        &mut self,
        options: novo_interop::eplan::pxf::PxfWriteOptions,
    ) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let doc = self.backend.document()?;
        let export = novo_interop::eplan::pxf::write_pxf(doc.project(), &options);
        let path = replaceable(
            &dir,
            &stem,
            "epj",
            novo_interop::eplan::pxf::is_kopercad_pxf,
        );
        if let Err(e) = write_replacing(&path, export.text.as_bytes()) {
            self.message(
                Level::Error,
                format!("Could not write {}: {e}", path.display()),
            );
            return None;
        }
        self.finish_export(std::slice::from_ref(&path), "EPLAN project", &export.report);
        Some(path)
    }

    /// Says where the file went and, when the format left something out,
    /// what (a warning stays on screen until dismissed).
    fn finish_export(&mut self, written: &[PathBuf], what: &str, report: &Report) {
        let Some(last) = written.last() else {
            return;
        };
        let place = match written {
            [one] => one.display().to_string(),
            many => format!(
                "{} files to {}",
                many.len(),
                last.parent().unwrap_or(Path::new(".")).display()
            ),
        };
        self.wb.last_export = Some(last.clone());
        // The toast carries an "Open folder" button.
        if report.is_lossless() {
            self.raise(
                Level::Info,
                format!("Exported {what} to {place}. {}", report.summary()),
                Some(last.clone()),
            );
        } else {
            self.raise(
                Level::Warning,
                format!("Exported {what} to {place}.\n{}", report.text()),
                Some(last.clone()),
            );
        }
    }

    /// Asks for an AutomationML file and previews it.
    pub(super) fn import_aml(&mut self) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Import AutomationML")
            .add_filter("AutomationML", &["aml", "xml"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_aml_from(&path);
        }
    }

    /// Reads `path` and opens the preview. Nothing changes until the person
    /// confirms it.
    pub fn import_aml_from(&mut self, path: &Path) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Some(doc) = self.backend.document() else {
            self.message(Level::Info, "Open a project first");
            return false;
        };
        let read = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                aml::read_aml(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())
            });
        match read {
            Ok(plant) => {
                let plan = aml::plan_import(doc.project(), &plant);
                let report = plan.report();
                self.wb.aml_import = Some(AmlImport {
                    file: name,
                    plan,
                    report,
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
    pub fn confirm_aml_import(&mut self) -> bool {
        let Some(import) = self.wb.aml_import.take() else {
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

    pub(super) fn aml_import_dialog(&mut self, ctx: &egui::Context) {
        let Some(import) = &self.wb.aml_import else {
            return;
        };
        let (mut go, mut cancel) = (false, false);
        let can_import = !import.plan.is_empty();
        let modal = egui::Modal::new(egui::Id::new("aml_import")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading(format!("Import {}", import.file));
            ui.add_space(4.0);
            ui.label(import.plan.summary());
            ui.add_space(6.0);
            ui.weak("An import only adds. Anything that exists keeps what the project says.");
            if !import.report.lossy.is_empty()
                || !import.report.guessed.is_empty()
                || !import.report.notes.is_empty()
            {
                ui.add_space(8.0);
                if !import.report.lossy.is_empty() {
                    ui.strong("Not carried over");
                }
                for l in &import.report.lossy {
                    ui.label(format!("{} {}: {}", l.count, l.what, l.why));
                }
                if !import.report.guessed.is_empty() {
                    ui.strong("Guessed, not exact");
                }
                for l in &import.report.guessed {
                    ui.label(format!("{} {}: {}", l.count, l.what, l.why));
                }
                for n in &import.report.notes {
                    ui.weak(n);
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                go = ui
                    .add_enabled(can_import, egui::Button::new("Import"))
                    .clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if go {
            self.confirm_aml_import();
        } else if cancel || modal.should_close() {
            self.wb.aml_import = None;
        }
    }

    /// Asks for a folder and writes the EPLAN export script, the instructions
    /// (English and Dutch) and a self-test file there, then shows the steps.
    pub(super) fn eplan_export_script(&mut self) {
        let mut dialog =
            crate::dialogs::FileDialog::new().set_title("Folder for the EPLAN export script");
        if let Some(dir) = novo_storage::profile::document_dir().filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.write_eplan_export_script(&folder);
        }
    }

    /// Writes the bundle into `folder` (created when missing) and opens the
    /// guide. A file of ours is replaced; the person's other files are not
    /// touched because the bundle names are fixed and ours alone.
    pub fn write_eplan_export_script(&mut self, folder: &Path) -> bool {
        let written = std::fs::create_dir_all(folder).and_then(|()| {
            for file in eplan_export::bundle() {
                std::fs::write(folder.join(file.name), file.text)?;
            }
            Ok(())
        });
        match written {
            Ok(()) => {
                self.wb.eplan_guide = Some(EplanGuide {
                    folder: folder.to_path_buf(),
                    dutch: false,
                });
                true
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!(
                        "Could not write the EPLAN script to {}: {e}",
                        folder.display()
                    ),
                );
                false
            }
        }
    }

    pub(super) fn eplan_guide_dialog(&mut self, ctx: &egui::Context) {
        let Some(guide) = &mut self.wb.eplan_guide else {
            return;
        };
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("eplan_guide")).show(ctx, |ui| {
            ui.set_width(560.0);
            ui.heading("Export from EPLAN");
            ui.weak(format!("Files written to {}", guide.folder.display()));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut guide.dutch, false, "English");
                ui.selectable_value(&mut guide.dutch, true, "Nederlands");
            });
            let lang = if guide.dutch {
                eplan_export::Lang::Dutch
            } else {
                eplan_export::Lang::English
            };
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(eplan_export::instructions(lang)).monospace());
                });
            ui.add_space(8.0);
            close = ui.button("Close").clicked();
        });
        if close || modal.should_close() {
            self.wb.eplan_guide = None;
        }
    }

    /// Asks for a DWG, converts it with the free converter and shows it as
    /// the current page's background.
    pub(super) fn import_dwg(&mut self) {
        if self.current_page.is_none() {
            self.message(Level::Info, "Open a page first");
            return;
        }
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Import DWG as background")
            .add_filter("DWG drawing", &["dwg"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_dwg_from(&path);
        }
    }

    /// Starts converting `path`; the result arrives in [`Self::poll_dwg`].
    pub fn import_dwg_from(&mut self, path: &Path) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if self.wb.dwg_job.is_some() {
            self.message(Level::Info, "A DWG is still being converted");
            return false;
        }
        let Some(converter) = dwg::find_installed(self.settings.dwg_converter.as_deref()) else {
            self.message(
                Level::Warning,
                format!("No DWG converter found. {}", dwg::guidance()),
            );
            return false;
        };
        let out = novo_storage::profile::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("KoperCAD")
            .join("dwg");
        let (tx, rx) = std::sync::mpsc::channel();
        let input = path.to_path_buf();
        let label = converter.kind.label();
        std::thread::spawn(move || {
            let result = converter
                .convert(&input, &out, Duration::from_secs(180))
                .map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        self.message(Level::Info, format!("Converting {name} with {label}..."));
        self.wb.dwg_job = Some(DwgJob { name, rx });
        true
    }

    /// Picks up a finished conversion and imports its DXF as background.
    pub(super) fn poll_dwg(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.wb.dwg_job else {
            return;
        };
        match job.rx.try_recv() {
            Ok(Ok(dxf)) => {
                let name = job.name.clone();
                self.wb.dwg_job = None;
                if self.import_dxf_background_from(&dxf) {
                    self.message(Level::Info, format!("{name} converted to DXF and imported"));
                }
            }
            Ok(Err(e)) => {
                let name = job.name.clone();
                self.wb.dwg_job = None;
                self.message(Level::Error, format!("Could not import {name}: {e}"));
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(250));
            }
            Err(TryRecvError::Disconnected) => {
                self.wb.dwg_job = None;
            }
        }
    }

    /// Lets the person point KoperCAD at a converter program of their own.
    pub(super) fn choose_dwg_converter(&mut self) {
        let mut dialog =
            crate::dialogs::FileDialog::new().set_title("Choose dwg2dxf or ODAFileConverter");
        if let Some(dir) = novo_storage::profile::data_local_dir().filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(program) = dialog.pick_file() {
            self.choose_dwg_converter_path(&program);
        }
    }

    pub fn choose_dwg_converter_path(&mut self, program: &Path) -> bool {
        match dwg::ConverterKind::of_program(program) {
            Some(kind) => {
                self.settings.dwg_converter = Some(program.to_path_buf());
                self.message(
                    Level::Info,
                    format!("DWG files will be converted with {}", kind.label()),
                );
                true
            }
            None => {
                self.message(
                    Level::Warning,
                    "That is not dwg2dxf or the ODA File Converter. Choose one of those programs",
                );
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_files_we_wrote_are_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let free = replaceable(dir.path(), "Plant", "aml", aml_is_ours);
        assert_eq!(free, dir.path().join("Plant.aml"));
        std::fs::write(&free, "<CAEXFile>someone else's</CAEXFile>").unwrap();
        assert_eq!(
            replaceable(dir.path(), "Plant", "aml", aml_is_ours),
            dir.path().join("Plant 2.aml")
        );
        std::fs::write(
            dir.path().join("Plant 2.aml"),
            "<CAEXFile><WriterName>KoperCAD</WriterName></CAEXFile>",
        )
        .unwrap();
        assert_eq!(
            replaceable(dir.path(), "Plant", "aml", aml_is_ours),
            dir.path().join("Plant 2.aml")
        );
        assert!(step_is_ours(
            b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('KoperCAD cabinet export'),'2;1');"
        ));
        assert!(!step_is_ours(
            b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('x'),'2;1');"
        ));
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_safe("+CAB1/2"), "+CAB1_2");
    }
}
