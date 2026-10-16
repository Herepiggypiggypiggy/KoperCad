//! Export assembly checklist: File > Export assembly checklist (PDF) writes
//! the workshop sheet of every cabinet (`novo_reports::AssemblyChecklist`:
//! prepare, drill, mount, wire, label, check) to a PDF next to the project.
//!
//! Like the page PDF, a saved project's file goes next to it and takes its
//! name (`Plant 2.novo` gives `Plant 2 assembly.pdf`); a second export
//! replaces the first only while it is still exactly what we wrote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use novo_pdf::{ReportPdfOptions, export_report_pdf};
use novo_reports::{AssemblyChecklist, Report};

use super::pdf::{replaceable_path, write_replacing};
use super::{Level, NovoApp, projects_dir};

/// Row height in mm: room for a tick or a pencilled note.
const ROW_MM: f64 = 8.0;

impl NovoApp {
    /// Exports the checklist next to the project, or to the Documents
    /// folder for an untitled one.
    pub(super) fn export_assembly(&mut self) -> Option<PathBuf> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let dir = match &file {
            Some(file) => file.parent()?.to_owned(),
            None => match projects_dir() {
                Some(dir) => dir,
                None => {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                }
            },
        };
        let stem = match &file {
            Some(file) => file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => Path::new(&super::project_file_name(&name))
                .with_extension("")
                .to_string_lossy()
                .into_owned(),
        };
        let path = replaceable_path(&dir.join(format!("{stem} assembly.pdf")));
        self.export_assembly_to(&path)
    }

    /// Exports to `path`. Without a cabinet that has a mounting plate there
    /// is nothing to build, so it says so and writes nothing.
    pub fn export_assembly_to(&mut self, path: &Path) -> Option<PathBuf> {
        let name = self.project_name().unwrap_or_default();
        let project = self.backend.document()?.project();
        let output = AssemblyChecklist.generate(project);
        if output.sections.iter().all(|s| s.key == "none") {
            self.message(
                Level::Warning,
                "No cabinet has a mounting plate yet. Create a cabinet and set its plate on the Cabinet canvas.",
            );
            return None;
        }
        let options = ReportPdfOptions {
            title: format!("{name}  Assembly checklist"),
            row_mm: output
                .sections
                .iter()
                .map(|s| (s.key.clone(), ROW_MM))
                .collect::<BTreeMap<_, _>>(),
            ..ReportPdfOptions::default()
        };
        let bytes = export_report_pdf(&[&output], &options);
        let replaced = path.exists();
        match write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message(Level::Info, format!("{verb} {}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the assembly checklist: {e}"),
                );
                None
            }
        }
    }
}
