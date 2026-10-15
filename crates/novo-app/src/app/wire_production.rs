//! Wire production in the app (novo-reports `wirecut`): the cut list a wire
//! machine or a wireman works from, written next to the project.
//!
//! Cabinet > Routes > Export wire cut, File > Export, or Ctrl+K "export wire
//! cut". The CSV is the generic job file (one row per wire: cut length,
//! cross-section, colour, strip lengths, ferrules, labels), semicolon
//! separated with a byte order mark as Excel in a decimal comma locale opens
//! it. The workbook also holds the wire stock, the ferrules to order and the
//! duct fill. Only a file this export wrote is replaced; anything else
//! takes the next free name.

use std::path::{Path, PathBuf};

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
        let project = self.backend.document()?.project();
        let jobs = wire_cut_jobs(project);
        if jobs.is_empty() {
            self.message(Level::Info, "No wires to cut yet: connect two device pins");
            return None;
        }
        let options = CsvOptions {
            delimiter: ';',
            ..CsvOptions::default()
        };
        let text = wire_cut_csv(&jobs, &options);
        self.write_production(path, text.as_bytes(), &format!("{} wires", jobs.len()))
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
        let project = self.backend.document()?.project();
        let report = WireCut.generate(project);
        if report.sections.is_empty() {
            self.message(Level::Info, "No wires to cut yet: connect two device pins");
            return None;
        }
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

    fn write_production(&mut self, path: &Path, bytes: &[u8], what: &str) -> Option<PathBuf> {
        let replaced = path.exists();
        match write_replacing(path, bytes) {
            Ok(()) => {
                self.wb.sheet_exports.insert(path.to_owned(), hash(bytes));
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message(Level::Info, format!("{verb} {what}: {}", path.display()));
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
