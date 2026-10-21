//! Wire production in the app (novo-reports `wirecut`): the cut list a wire
//! machine or a wireman works from, written next to the project.
//!
//! Cabinet > Routes > Export wire cut, File > Export production, or Ctrl+K "export wire
//! cut". The CSV is the generic job file (one row per wire: cut length,
//! cross-section, colour, strip lengths, ferrules, labels), semicolon
//! separated with a byte order mark as Excel in a decimal comma locale opens
//! it. The workbook also holds the wire stock, the ferrules to order and the
//! duct fill. Only a file this export wrote is replaced; anything else
//! takes the next free name.

use std::path::{Path, PathBuf};

use novo_core::StructureId;
use novo_interop::dxf::write_dxf;
use novo_reports::{CsvOptions, Report, WireCut, wire_cut_csv, wire_cut_jobs};

use super::pdf::write_replacing;
use super::sheets::{hash, ours_or_free};
use super::{Level, NovoApp};

impl NovoApp {
    /// Writes the wire-cut job file (CSV) next to the project.
    pub(super) fn export_wire_cut(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = ours_or_free(
            &dir,
            &format!("{stem} wire cut"),
            "csv",
            &self.wb.sheet_exports,
        );
        self.export_wire_cut_to(&path)
    }

    /// Writes the job file to `path`.
    pub fn export_wire_cut_to(&mut self, path: &Path) -> Option<PathBuf> {
        let project = self.machine()?;
        let jobs = wire_cut_jobs(&project);
        if jobs.is_empty() {
            self.message(Level::Info, "No wires to cut yet: connect two device pins");
            return None;
        }
        let options = CsvOptions {
            delimiter: ';',
            ..CsvOptions::default()
        };
        let text = wire_cut_csv(&jobs, &options);
        let written =
            self.write_production(path, text.as_bytes(), &format!("{} wires", jobs.len()));
        if written.is_some() {
            self.learn_signal("wire_cut_exported");
        }
        written
    }

    /// Writes the whole wire production report as a workbook.
    pub(super) fn export_wire_production(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = ours_or_free(
            &dir,
            &format!("{stem} wire production"),
            "xlsx",
            &self.wb.sheet_exports,
        );
        self.export_wire_production_to(&path)
    }

    /// Writes the workbook to `path`.
    pub fn export_wire_production_to(&mut self, path: &Path) -> Option<PathBuf> {
        let project = self.machine()?;
        let report = WireCut.generate(&project);
        if report.sections.is_empty() {
            self.message(Level::Info, "No wires to cut yet: connect two device pins");
            return None;
        }
        let report = novo_reports::i18n::translate_output(&report, &self.report_translator());
        match novo_reports::xlsx_bytes(&[&report]) {
            Ok(bytes) => self.write_production(path, &bytes, "wire production"),
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export wire production: {e}"),
                );
                None
            }
        }
    }

    /// Writes the wiring plan of a panel's plate as DXF next to the project:
    /// plate, ducts with their fill, devices, every route and the wire table.
    pub(super) fn export_wiring_dxf(&mut self, structure: StructureId) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let project = self.backend.document()?.project();
        let tag = project
            .get::<novo_core::Structure>(structure)
            .map(|s| s.tag.to_string())
            .unwrap_or_default();
        let Some(drawing) = novo_interop::wiring::wiring_dxf(project, structure) else {
            self.message(Level::Info, "Set the size of the mounting plate first");
            return None;
        };
        let path = ours_or_free(
            &dir,
            &format!("{stem} wiring {}", tag.trim_start_matches('+')),
            "dxf",
            &self.wb.sheet_exports,
        );
        let text = write_dxf(&drawing);
        self.write_production(&path, text.as_bytes(), "wiring plan")
    }

    /// The wiring plan of the panel the Cabinet shows (Ctrl+K).
    pub(super) fn export_wiring_dxf_of_panel(&mut self) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let structure = self.cabinet.location.as_ref().and_then(|l| {
            project
                .structures()
                .find(|s| s.enclosure && &s.tag == l)
                .map(|s| s.id)
        });
        match structure {
            Some(s) => {
                self.export_wiring_dxf(s);
            }
            None => self.message(Level::Info, "Open a cabinet in the Cabinet view first"),
        }
    }

    /// Adds the wire production page, so the PDF export prints the cut
    /// list, wire stock, ferrules and duct fill with the drawing.
    pub(super) fn add_wire_production_page(&mut self) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let spec = match novo_reports::generated_page_spec(doc.project(), WireCut::ID) {
            Ok(spec) => spec,
            Err(why) => {
                self.message(Level::Info, why);
                return;
            }
        };
        match doc.transact("Wire production page", |tx| {
            novo_reports::add_generated_pages(tx, std::slice::from_ref(&spec))
        }) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(
                    Level::Info,
                    format!("Added page {} {}", spec.name, spec.title),
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not add the page: {e}")),
        }
    }

    fn write_production(&mut self, path: &Path, bytes: &[u8], what: &str) -> Option<PathBuf> {
        let replaced = path.exists();
        match write_replacing(path, bytes) {
            Ok(()) => {
                self.wb.sheet_exports.insert(path.to_owned(), hash(bytes));
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(format!("{verb} {what}: {}", path.display()), path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
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
}
