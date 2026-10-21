//! Handover package: one command writes the customer folder
//! `<project> handover <date>` next to the project:
//!
//! | File | What |
//! | --- | --- |
//! | `0 Contents.pdf` | what is in the folder, and what is exact or estimated |
//! | `1 Drawing set.pdf` | every page, with the design checks if any are open |
//! | `2 Revisions.pdf` | the issued revisions |
//! | `3 Parts list.pdf`, `4 Wire list.pdf`, `5 Terminal plan.pdf`, `6 Cable schedule.pdf` | the generated lists |
//! | `7 Commissioning record.pdf` | checks, punch items and sign-offs, when commissioning started |
//! | `8 Safety functions.pdf` | when safety functions are defined |
//! | `9 Lists.xlsx` | parts, devices, wires, terminals and cables for Excel |
//!
//! Every file reuses the export that already exists for it. A file with
//! nothing to say is left out and the contents page names the reason. The
//! contents page is written last, so it describes what really happened.
//! Files of an earlier package in the same folder are replaced.

use std::path::{Path, PathBuf};

use novo_checks::Summary;
use novo_commissioning::Scope;
use novo_core::Project;
use novo_pdf::ReportPdfOptions;
use novo_reports::ReportOutput;
use novo_reports::handover::{
    CommissioningFacts, FileStatus, HandoverFile, HandoverInput, handover_summary, revision_table,
};

use super::pdf::write_replacing;
use super::{Level, NovoApp};
use crate::settings::PdfChoice;

/// A list of the package: its file, what it is, and the report that fills it.
const LISTS: [(&str, &str, &str, &str); 4] = [
    (
        "3 Parts list.pdf",
        "Parts list (bill of materials)",
        "bom",
        "no parts yet",
    ),
    (
        "4 Wire list.pdf",
        "Wire list with cut lengths",
        "wires",
        "no wires yet",
    ),
    (
        "5 Terminal plan.pdf",
        "Terminal plan of every strip",
        "terminals",
        "no terminal strips",
    ),
    (
        "6 Cable schedule.pdf",
        "Cable schedule",
        "cableschedule",
        "no cables",
    ),
];

/// The reports that go into `9 Lists.xlsx`.
const WORKBOOK: [&str; 5] = ["bom", "devices", "wires", "terminals", "cableschedule"];

impl NovoApp {
    /// Writes the package into `<project> handover <date>` next to the
    /// project file (Documents for an untitled one). Returns the folder.
    pub(super) fn export_handover(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let folder = dir.join(format!("{stem} handover {}", super::revisions::today()));
        self.export_handover_to(&folder)
    }

    /// Writes the package into `folder`.
    pub fn export_handover_to(&mut self, folder: &Path) -> Option<PathBuf> {
        let date = super::revisions::today();
        let mut files: Vec<HandoverFile> = Vec::new();
        let note =
            |files: &mut Vec<HandoverFile>, name: &str, what: &str, r: Result<(), String>| {
                files.push(HandoverFile {
                    name: name.to_owned(),
                    what: what.to_owned(),
                    status: match r {
                        Ok(()) => FileStatus::Written,
                        Err(why) => FileStatus::Left(why),
                    },
                });
            };

        // 1. The drawing set. The saved options' look and title apply; the
        // page choice does not: a handover always holds every page.
        let choice = PdfChoice {
            scope: "all".into(),
            ranges: String::new(),
            checks_appendix: true,
            ..self.settings.pdf_choice.clone()
        };
        let drawing = folder.join("1 Drawing set.pdf");
        let r = if self.backend.document()?.project().pages().next().is_none() {
            Err("the project has no pages".to_owned())
        } else {
            self.export_pdf_choice_to(&drawing, novo_pdf::PageSet::All, &choice)
                .map(|_| ())
                .ok_or_else(|| "the PDF could not be written".to_owned())
        };
        note(
            &mut files,
            "1 Drawing set.pdf",
            "Every page of the drawing",
            r,
        );

        // 2. Revisions.
        let revisions = revision_table(self.backend.document()?.project());
        let r = self.handover_pdf(&folder.join("2 Revisions.pdf"), &[&revisions], &date);
        note(&mut files, "2 Revisions.pdf", "Issued revisions", r);

        // 3 to 6. The generated lists.
        for (file, what, id, empty) in LISTS {
            let r = match self.handover_report(id) {
                Some(output) if output.row_count() > 0 => {
                    self.handover_pdf(&folder.join(file), &[&output], &date)
                }
                Some(_) => Err(empty.to_owned()),
                None => Err("report not available".to_owned()),
            };
            note(&mut files, file, what, r);
        }

        // 7. Commissioning record.
        self.load_commissioning();
        let facts = self.commissioning_facts();
        let r = match facts {
            Some(f) if f.total > 0 => self
                .write_commissioning_report(
                    novo_pdf::ReportFileFormat::Pdf,
                    &folder.join("7 Commissioning record.pdf"),
                )
                .map(|_| ())
                .ok_or_else(|| "the report could not be written".to_owned()),
            _ => Err("commissioning has not started".to_owned()),
        };
        note(
            &mut files,
            "7 Commissioning record.pdf",
            "Checks, punch items and sign-offs",
            r,
        );

        // 8. Safety functions.
        let has_functions =
            !novo_checks::safety::functions(self.backend.document()?.project()).is_empty();
        let r = if has_functions {
            self.write_safety_report(&folder.join("8 Safety functions.pdf"))
                .map(|_| ())
                .ok_or_else(|| "the report could not be written".to_owned())
        } else {
            Err("no safety functions defined".to_owned())
        };
        note(
            &mut files,
            "8 Safety functions.pdf",
            "Safety functions with performance level (calculation aid)",
            r,
        );

        // 9. The workbook.
        let r = self.handover_workbook(&folder.join("9 Lists.xlsx"));
        note(
            &mut files,
            "9 Lists.xlsx",
            "Parts, devices, wires, terminals and cables for Excel",
            r,
        );

        // 0. The contents page, from what happened above.
        let written = files
            .iter()
            .filter(|f| f.status == FileStatus::Written)
            .count();
        if written == 0 {
            self.message(
                Level::Warning,
                "Nothing to put in a handover package yet: draw something first",
            );
            return None;
        }
        let changed_pages = self.pages_changed_since_latest().len();
        let problems = Some(Summary::of(self.problems()));
        let input = HandoverInput {
            date: date.clone(),
            files: files.clone(),
            problems,
            commissioning: facts,
            changed_pages,
        };
        let contents = handover_summary(self.backend.document()?.project(), &input);
        if let Err(e) = self.handover_pdf(&folder.join("0 Contents.pdf"), &[&contents], &date) {
            self.message(
                Level::Error,
                format!("Could not write the contents page: {e}"),
            );
            return None;
        }

        let left: Vec<String> = files
            .iter()
            .filter_map(|f| match &f.status {
                FileStatus::Left(why) => Some(format!("{} ({why})", f.name)),
                FileStatus::Written => None,
            })
            .collect();
        let mut text = format!(
            "Handover package: {} files in {}. 0 Contents.pdf says what is exact and what is an estimate",
            written + 1,
            folder.display()
        );
        if !left.is_empty() {
            text.push_str(&format!(". Left out: {}", left.join(", ")));
        }
        self.message_file(text, folder);
        self.wb.last_export = Some(folder.to_owned());
        Some(folder.to_owned())
    }

    fn handover_report(&mut self, id: &str) -> Option<ReportOutput> {
        let doc = self.backend.document()?;
        let project: &Project = doc.project();
        self.wb.reports.live(project).get(project, id).cloned()
    }

    /// Writes `outputs` as one report PDF.
    fn handover_pdf(
        &mut self,
        path: &Path,
        outputs: &[&ReportOutput],
        date: &str,
    ) -> Result<(), String> {
        let name = self.project_name().unwrap_or_default();
        let options = ReportPdfOptions {
            title: format!("{name}: {}", outputs.first().map_or("", |o| &o.title)),
            footer_right: date.to_owned(),
            language: self.report_translator(),
            ..ReportPdfOptions::default()
        };
        let bytes = novo_pdf::export_report_pdf(outputs, &options);
        write_replacing(path, &bytes).map_err(|e| format!("could not write: {e}"))
    }

    fn handover_workbook(&mut self, path: &Path) -> Result<(), String> {
        let language = self.report_translator();
        let outputs: Vec<ReportOutput> = WORKBOOK
            .iter()
            .filter_map(|id| self.handover_report(id))
            .filter(|o| o.row_count() > 0)
            .map(|o| novo_reports::i18n::translate_output(&o, &language))
            .collect();
        if outputs.is_empty() {
            return Err("nothing to list yet".to_owned());
        }
        let refs: Vec<&ReportOutput> = outputs.iter().collect();
        let bytes = novo_reports::xlsx_bytes(&refs).map_err(|e| e.to_string())?;
        write_replacing(path, &bytes).map_err(|e| format!("could not write: {e}"))
    }

    /// How far commissioning is, or `None` when it never started.
    fn commissioning_facts(&mut self) -> Option<CommissioningFacts> {
        let mode = self.wb.commissioning.as_ref()?;
        if mode.store.is_empty() {
            return None;
        }
        let project = self.backend.document()?.project();
        let p = novo_commissioning::progress(project, &mode.store, Scope::Project);
        let open: Vec<_> = mode
            .store
            .punch_items()
            .filter(|i| !i.state.is_closed())
            .collect();
        Some(CommissioningFacts {
            total: p.total,
            done: p.done(),
            failed: p.failed,
            open_punch: open.len(),
            blocking_punch: open
                .iter()
                .filter(|i| i.severity == novo_commissioning::Severity::Blocking)
                .count(),
        })
    }
}
