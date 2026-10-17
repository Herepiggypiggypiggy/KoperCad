//! Safety function report export (Check > Safety functions > Export report):
//! the rated project functions as a PDF beside the project file.

use novo_checks::safety::{functions, rate};
use novo_core::Project;
use novo_pdf::{ReportFileFormat, ReportPdfOptions};
use novo_reports::safety::{BlockRow, FunctionRow, SafetyReportInput, safety_report};

use super::commissioning::report_path_for;
use super::pdf::write_replacing;
use super::{Level, NovoApp, projects_dir};

/// The report input for the project's stored safety functions.
pub(super) fn report_input(project: &Project, name: &str) -> SafetyReportInput {
    let functions = functions(project)
        .iter()
        .map(|f| {
            let rating = rate(project, f);
            FunctionRow {
                name: f.name.clone(),
                required: rating.required.to_string(),
                achieved: rating.total.as_ref().map(|t| t.pl.to_string()),
                pfhd: rating.total.as_ref().map(|t| t.pfhd),
                met: rating.met(),
                blocks: rating
                    .blocks
                    .iter()
                    .zip(&f.blocks)
                    .map(|(b, block)| BlockRow {
                        role: b.role.label().to_owned(),
                        name: b.name.clone(),
                        category: block.category.clone(),
                        devices: block
                            .channel1
                            .iter()
                            .chain(&block.channel2)
                            .filter_map(|p| p.device.clone())
                            .collect(),
                        mttfd_years: b.result.as_ref().map(|r| r.mttfd_years),
                        dc: b.result.as_ref().map(|r| format!("{:?}", r.dc)),
                        pfhd: b.result.as_ref().map(|r| r.pfhd),
                        pl: b.result.as_ref().map(|r| r.pl.to_string()),
                        notes: b.binding.clone(),
                    })
                    .collect(),
            }
        })
        .collect();
    SafetyReportInput {
        project_name: name.to_owned(),
        functions,
    }
}

impl NovoApp {
    /// Writes the safety report as a PDF next to the project file.
    pub(super) fn export_safety_report(&mut self) -> Option<std::path::PathBuf> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let Some(path) = report_path_for(
            "safety",
            file.as_deref(),
            projects_dir().as_deref(),
            &name,
            "pdf",
        ) else {
            self.message(Level::Error, "Could not find the Documents folder");
            return None;
        };
        self.write_safety_report(&path)
    }

    pub(super) fn write_safety_report(
        &mut self,
        path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        let name = self.project_name()?;
        let project = self.backend.document()?.project();
        let report = safety_report(&report_input(project, &name));
        let options = ReportPdfOptions {
            title: format!("{name}, safety functions"),
            language: self.report_translator(),
            ..ReportPdfOptions::default()
        };
        let result = novo_pdf::report_bytes(&report, ReportFileFormat::Pdf, &options)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_replacing(path, &bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.message_file(format!("Exported {}", path.display()), path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export the report: {e}"));
                None
            }
        }
    }
}
