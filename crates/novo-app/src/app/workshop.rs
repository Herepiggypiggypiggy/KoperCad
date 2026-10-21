//! Workshop pack: one command writes the hand-workshop bundle into a folder
//! next to the project, `<project> workshop`:
//!
//! 1. the 1:1 drilling template of every mounting plate (and any door or
//!    wall that has holes), tiled on A4 or A3 with cross hairs, hole sizes
//!    and a scale-check ruler (`novo_layout::template`, `novo_pdf::template`),
//! 2. the bench cut list, 3. the ferrule counts, 4. the labels, and
//! 5. the assembly print pack with its tick boxes.
//!
//! The last four are the Bench output exports; they are written under the
//! settings of the Bench output window. A step that has nothing to print
//! (no wires, no plate) is skipped and the summary says so.

use std::path::{Path, PathBuf};

use novo_core::Project;
use novo_layout::model::template_documents;
use novo_layout::template::Paper;

use super::{Level, NovoApp};

/// Most pages one template PDF may take. A plate this large is a data entry
/// slip (metres typed as millimetres); printing it would not be.
const MAX_TEMPLATE_PAGES: usize = 300;

/// One cabinet's template files.
pub(super) struct TemplatePdfs {
    pub tag: String,
    /// The tiled 1:1 template (overview, then A4 or A3 tiles).
    pub tiled: Vec<u8>,
    /// The mounting plate alone at true size on one page the size of the
    /// plate, for a print shop; `None` when the plate is too small or too
    /// large for one page.
    pub plate: Option<Vec<u8>>,
}

/// The enclosures that have a mounting plate and the PDFs of each one's
/// template, and the tags left out for needing more than
/// `MAX_TEMPLATE_PAGES` pages.
pub(super) fn template_pdfs(
    project: &Project,
    name: &str,
    paper: Paper,
) -> (Vec<TemplatePdfs>, Vec<String>) {
    let mut pdfs = Vec::new();
    let mut too_big = Vec::new();
    for s in project
        .structures()
        .filter(|s| s.enclosure && s.plate.is_some())
    {
        let Some((sheets, _)) = template_documents(project, s.id) else {
            continue;
        };
        let pages = sheets.iter().fold(0usize, |n, sh| {
            n.saturating_add(novo_pdf::template::page_count(sh, paper))
        });
        if pages > MAX_TEMPLATE_PAGES {
            too_big.push(format!("{} ({pages} pages)", s.tag));
            continue;
        }
        pdfs.push(TemplatePdfs {
            tag: s.tag.to_string(),
            tiled: novo_pdf::template::export_template_pdf(&sheets, name, paper),
            plate: sheets
                .first()
                .and_then(|plate| novo_pdf::template::export_plate_pdf(plate, name)),
        });
    }
    (pdfs, too_big)
}

/// A file name part without characters Windows does not allow.
pub(super) fn clean(s: &str) -> String {
    s.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
}

impl NovoApp {
    /// Writes the pack into `<project> workshop` next to the project file
    /// (Documents for an untitled one). Returns the folder.
    pub(super) fn export_workshop(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let folder = dir.join(format!("{stem} workshop"));
        self.export_workshop_to(&folder)
    }

    /// Writes the pack into `folder`, replacing the files of an earlier pack.
    pub fn export_workshop_to(&mut self, folder: &Path) -> Option<PathBuf> {
        let name = self.machine_title().unwrap_or_default();
        let paper = self.wb.workshop_paper;
        let (templates, too_big) = template_pdfs(&*self.machine()?, &name, paper);
        let mut done: Vec<String> = Vec::new();
        let mut skipped: Vec<String> = Vec::new();
        let mut no_single_sheet: Vec<String> = Vec::new();

        if templates.is_empty() && too_big.is_empty() {
            skipped.push("drilling template (no cabinet has a mounting plate)".to_owned());
        }
        for tag in &too_big {
            skipped.push(format!(
                "drilling template {tag}, over {MAX_TEMPLATE_PAGES} pages: check the plate size"
            ));
        }
        for t in &templates {
            let tag = &t.tag;
            let file = folder.join(format!(
                "1 Drilling template {} {}.pdf",
                clean(tag),
                paper.name()
            ));
            if self.workshop_write(&file, &t.tiled) {
                done.push(format!("drilling template {tag} ({})", paper.name()));
            }
            match &t.plate {
                Some(bytes) => {
                    let file = folder.join(format!("1 Plate full size {}.pdf", clean(tag)));
                    if self.workshop_write(&file, bytes) {
                        done.push(format!("plate {tag} on one sheet for a print shop"));
                    }
                }
                None => no_single_sheet.push(tag.clone()),
            }
        }
        let steps: [(&str, &str); 4] = [
            ("2 Cut list.pdf", "cut list"),
            ("3 Ferrules.pdf", "ferrules"),
            ("4 Labels.pdf", "labels"),
            ("5 Assembly checklist.pdf", "assembly checklist"),
        ];
        for (file, what) in steps {
            let path = folder.join(file);
            let written = match what {
                "cut list" => self.export_bench_cut_to(&path),
                "ferrules" => self.export_ferrules_to(&path),
                "labels" => self.export_labels_to(&path),
                _ => self.export_assembly_to(&path),
            };
            match written {
                Some(_) => done.push(what.to_owned()),
                None => skipped.push(what.to_owned()),
            }
        }
        if done.is_empty() {
            self.message(
                Level::Warning,
                "Nothing to put in a workshop pack yet: set a mounting plate in the Cabinet and connect some wires",
            );
            return None;
        }
        let mut text = format!("Workshop pack: {} in {}", done.join(", "), folder.display());
        if !skipped.is_empty() {
            text.push_str(&format!(
                ". Left out, nothing to print yet: {}",
                skipped.join(", ")
            ));
        }
        if !no_single_sheet.is_empty() {
            text.push_str(&format!(
                ". No one-sheet plate for {}: the plate is under 150 x 40 mm or over 5080 mm, use the tiled template",
                no_single_sheet.join(", ")
            ));
        }
        self.message_file(text, folder);
        self.wb.last_export = Some(folder.to_owned());
        Some(folder.to_owned())
    }

    fn workshop_write(&mut self, path: &Path, bytes: &[u8]) -> bool {
        match super::pdf::write_replacing(path, bytes) {
            Ok(()) => true,
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not write {}: {e}", path.display()),
                );
                false
            }
        }
    }
}
