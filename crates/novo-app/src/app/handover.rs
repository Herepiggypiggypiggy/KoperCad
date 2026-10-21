//! Handover package: one command writes the customer folder
//! `<project> handover <date>` next to the project:
//!
//! | File | What |
//! | --- | --- |
//! | `0 Contents.pdf` | what is in the folder, and what is exact or estimated |
//! | `1 Drawing set.pdf` | every page, through the last print setup, with the design checks if any are open |
//! | `1 Plot set <name>.pdf` | one per saved plot set, with its own paper and scale |
//! | `2 Revisions.pdf` | the issued revisions |
//! | `3 Parts list.pdf`, `4 Wire list.pdf`, `5 Terminal plan.pdf`, `6 Cable schedule.pdf` | the generated lists |
//! | `7 Commissioning record.pdf` | checks, punch items and sign-offs, when commissioning started |
//! | `8 Safety functions.pdf` | when safety functions are defined |
//! | `9 Lists.xlsx` | parts, devices, wires, terminals and cables for Excel |
//! | `Workshop/` | the workshop pack, when there is a mounting plate or a wire |
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

use eframe::egui;

use super::pdf::write_replacing;
use super::{Level, NovoApp};
use crate::settings::{PdfChoice, PrintChoice};

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

/// `<stem> handover <date>` without characters Windows does not allow in a
/// folder name, and without a trailing dot or space, which it also refuses.
fn folder_name(stem: &str, date: &str) -> String {
    let name = format!("{} handover {date}", super::workshop::clean(stem));
    name.trim_end_matches(['.', ' ']).to_owned()
}

/// The fixed names of the package's own files. A rerun removes the ones it
/// no longer writes, so an old file never contradicts the new contents page.
const OWN_FILES: [&str; 10] = [
    "0 Contents.pdf",
    "1 Drawing set.pdf",
    "2 Revisions.pdf",
    "3 Parts list.pdf",
    "4 Wire list.pdf",
    "5 Terminal plan.pdf",
    "6 Cable schedule.pdf",
    "7 Commissioning record.pdf",
    "8 Safety functions.pdf",
    "9 Lists.xlsx",
];

fn is_own_file(name: &str) -> bool {
    OWN_FILES.contains(&name) || (name.starts_with("1 Plot set ") && name.ends_with(".pdf"))
}

fn existing_own_files(folder: &Path) -> std::collections::BTreeSet<String> {
    std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| is_own_file(n))
        .collect()
}

/// What the question before writing shows: the open problems.
pub(crate) struct HandoverAsk {
    summary: Summary,
    /// The first few problems in words.
    lines: Vec<String>,
}

impl NovoApp {
    /// The `export.handover` command: writes the package at once when the
    /// design checks have no error or warning, else asks first (see
    /// [`NovoApp::handover_ask_dialog`]).
    pub(super) fn ask_handover(&mut self) {
        let problems = self.problems();
        let summary = Summary::of(problems);
        if summary.errors + summary.warnings == 0 {
            self.export_handover();
            return;
        }
        let lines = problems
            .iter()
            .filter(|p| p.severity != novo_checks::Severity::Info)
            .take(4)
            .map(|p| p.message.clone())
            .collect();
        self.wb.handover_ask = Some(HandoverAsk { summary, lines });
    }

    pub(super) fn handover_ask_dialog(&mut self, ctx: &egui::Context) {
        let Some(ask) = self.wb.handover_ask.take() else {
            return;
        };
        let (mut fix, mut anyway) = (false, false);
        let modal = egui::Modal::new(egui::Id::new("handover_ask")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("The design still has open problems");
            ui.add_space(4.0);
            ui.label(format!(
                "{} remain. The package lists them on the contents page and in the drawing set, so the customer would see them.",
                ask.summary.pill_text()
            ));
            ui.add_space(4.0);
            for line in &ask.lines {
                ui.weak(format!("• {line}"));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Fix these first").clicked() {
                    fix = true;
                }
                if ui.button("Hand over anyway").clicked() {
                    anyway = true;
                }
                ui.weak("Esc cancels");
            });
        });
        if fix {
            self.show_issues(false);
        } else if anyway {
            self.export_handover();
        } else if !modal.should_close() {
            self.wb.handover_ask = Some(ask);
        }
    }

    /// Writes the package into `<project> handover <date>` next to the
    /// project file (Documents for an untitled one). Returns the folder.
    pub(super) fn export_handover(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let folder = dir.join(folder_name(&stem, &super::revisions::today()));
        self.export_handover_to(&folder)
    }

    /// Writes the package into `folder`.
    pub fn export_handover_to(&mut self, folder: &Path) -> Option<PathBuf> {
        let date = super::revisions::today();
        let before = existing_own_files(folder);
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

        // 1. The drawing set: every page, through the print setup last used
        // in the print preview (plain when it was never changed), plus one
        // PDF per saved plot set with that set's own paper and scale.
        let choice = PdfChoice {
            scope: "all".into(),
            ranges: String::new(),
            checks_appendix: true,
            ..self.settings.pdf_choice.clone()
        };
        let print = (self.settings.print_choice != PrintChoice::default())
            .then(|| super::print_preview::setup_of(&self.settings.print_choice));
        let drawing = folder.join("1 Drawing set.pdf");
        let r = if self.backend.document()?.project().pages().next().is_none() {
            Err("the project has no pages".to_owned())
        } else {
            let options = self.pdf_options_for(novo_pdf::PageSet::All, &choice, print);
            self.export_options_to(&drawing, &options)
                .map(|_| ())
                .ok_or_else(|| "the PDF could not be written".to_owned())
        };
        note(
            &mut files,
            "1 Drawing set.pdf",
            "Every page of the drawing",
            r,
        );
        for (set, ids) in self.plot_sets_with_pages() {
            let file = format!(
                "1 Plot set {}.pdf",
                super::print_preview::file_part(&set.name)
            );
            let r = self
                .write_plot_set(&set, &ids, &folder.join(&file))
                .map(|_| ())
                .ok_or_else(|| "the PDF could not be written".to_owned());
            note(
                &mut files,
                &file,
                &format!("Plot set {}: {} pages", set.name, ids.len()),
                r,
            );
        }

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

        // The workshop pack, in a subfolder, when there is something to print.
        let project = self.backend.document()?.project();
        let workshop_wanted = project
            .structures()
            .any(|s| s.enclosure && s.plate.is_some())
            || !novo_reports::wire_cut_jobs(project).is_empty();
        let r = if workshop_wanted {
            self.export_workshop_to(&folder.join("Workshop"))
                .map(|_| ())
                .ok_or_else(|| "the pack could not be written".to_owned())
        } else {
            Err("no mounting plate and no wires".to_owned())
        };
        note(
            &mut files,
            "Workshop (folder)",
            "Drilling templates, cut list, ferrules, labels and assembly checklist",
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
        let mut notes = Vec::new();
        let workshop_left = files
            .iter()
            .any(|f| f.name == "Workshop (folder)" && matches!(f.status, FileStatus::Left(_)));
        if workshop_left && folder.join("Workshop").is_dir() {
            notes.push(
                "A Workshop folder from an earlier package is still here and is not part of this one."
                    .to_owned(),
            );
        }
        let input = HandoverInput {
            date: date.clone(),
            files: files.clone(),
            problems,
            commissioning: facts,
            changed_pages,
            notes,
        };
        let contents = handover_summary(self.backend.document()?.project(), &input);
        if let Err(e) = self.handover_pdf(&folder.join("0 Contents.pdf"), &[&contents], &date) {
            self.message(
                Level::Error,
                format!("Could not write the contents page: {e}"),
            );
            return None;
        }

        // Files of an earlier run that this run did not write are stale.
        let kept: std::collections::BTreeSet<String> = files
            .iter()
            .filter(|f| f.status == FileStatus::Written)
            .map(|f| f.name.clone())
            .chain(["0 Contents.pdf".to_owned()])
            .collect();
        let stale: Vec<&String> = before.iter().filter(|n| !kept.contains(*n)).collect();
        let mut removed = 0;
        let mut stuck: Vec<String> = Vec::new();
        for name in &stale {
            match std::fs::remove_file(folder.join(name)) {
                Ok(()) => removed += 1,
                Err(e) => stuck.push(format!("{name}: {e}")),
            }
        }
        let replaced = before.iter().filter(|n| kept.contains(*n)).count();
        let workshop_old = files
            .iter()
            .any(|f| f.name == "Workshop (folder)" && matches!(f.status, FileStatus::Left(_)))
            && folder.join("Workshop").is_dir();

        let left: Vec<String> = files
            .iter()
            .filter_map(|f| match &f.status {
                FileStatus::Left(why) => Some(format!("{} ({why})", f.name)),
                FileStatus::Written => None,
            })
            .collect();
        let mut text = format!(
            "Handover package: {} files in {}",
            written + 1,
            folder.display()
        );
        if replaced > 0 {
            text.push_str(&format!(", {replaced} replaced"));
        }
        if removed > 0 {
            text.push_str(&format!(
                ", {removed} removed that an earlier package had and this one has not"
            ));
        }
        text.push_str(". 0 Contents.pdf says what is exact and what is an estimate");
        if !left.is_empty() {
            text.push_str(&format!(". Left out: {}", left.join(", ")));
        }
        if workshop_old {
            text.push_str(". The Workshop folder from an earlier package was kept");
        }
        if !stuck.is_empty() {
            text.push_str(&format!(". Could not remove {}", stuck.join(", ")));
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
            title: {
                // Some report titles already start with the project name.
                let title = outputs.first().map_or("", |o| o.title.as_str());
                if title.starts_with(&name) {
                    title.to_owned()
                } else {
                    format!("{name}: {title}")
                }
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_name_has_no_characters_windows_refuses() {
        assert_eq!(
            folder_name("Pump: v1/2*?", "2026-10-21"),
            "Pump- v1-2-- handover 2026-10-21"
        );
        assert_eq!(
            folder_name("Plant.", "2026-10-21"),
            "Plant. handover 2026-10-21"
        );
        assert!(!folder_name("a<b>c|d\"e", "x").contains(['<', '>', '|', '"', '\\']));
    }

    #[test]
    fn only_the_packages_own_names_are_cleaned_up() {
        assert!(is_own_file("8 Safety functions.pdf"));
        assert!(is_own_file("1 Plot set Issue A3.pdf"));
        assert!(!is_own_file("Notes from the customer.pdf"));
        assert!(!is_own_file("Workshop"));
    }
}
