//! Workshop pack: one command writes the hand-workshop bundle into a folder
//! next to the project, `<project> workshop`:
//!
//! 1. the 1:1 drilling template of every mounting plate (and any door or
//!    wall that has holes), tiled on A4 or A3 with cross hairs, hole sizes
//!    and a scale-check ruler (`novo_layout::template`, `novo_pdf::template`),
//!    plus a print-scale check page and, for doors and walls with cut-outs,
//!    one A4 card per cut-out shape (`novo_pdf::template_cards`),
//! 2. the labels, and
//! 3. the bench packet, `0 Bench packet.pdf`: one PDF with a cover, the
//!    cabinet build order, the cut list, the ferrules, a tick sheet per
//!    schematic page and the terminal strip plans (`bench_packet`).
//!
//! The labels and the packet are written under the settings of the Bench
//! output window. A step that has nothing to print (no wires, no plate) is
//! skipped and the summary says so.

use std::path::{Path, PathBuf};

use novo_core::Project;
use novo_layout::model::template_documents;
use novo_layout::template::Paper;

use novo_reports::packet::CoverLine;

use super::bench_packet::PACKET_FILE;
use super::{Level, NovoApp};

/// Most pages one template PDF may take. A plate this large is a data entry
/// slip (metres typed as millimetres); printing it would not be.
const MAX_TEMPLATE_PAGES: usize = 300;

/// The print-scale check page in the pack.
const SCALE_CHECK_FILE: &str = "0 Print scale check.pdf";

/// One cabinet's template files.
pub(super) struct TemplatePdfs {
    pub tag: String,
    /// The tiled 1:1 template (overview, then A4 or A3 tiles).
    pub tiled: Vec<u8>,
    /// The mounting plate alone at true size on one page the size of the
    /// plate, for a print shop; `None` when the plate is too small or too
    /// large for one page.
    pub plate: Option<Vec<u8>>,
    /// One A4 card per cut-out shape (doors, walls), when there are cut-outs.
    pub cards: Option<novo_pdf::template_cards::CutoutCards>,
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
            cards: novo_pdf::template_cards::export_cutout_cards_pdf(&sheets, name),
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
        if !templates.is_empty() {
            let file = folder.join(SCALE_CHECK_FILE);
            let bytes = novo_pdf::template_cards::export_scale_check_pdf(&name);
            if self.workshop_write(&file, &bytes) {
                done.push("print scale check".to_owned());
            }
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
            if let Some(cards) = &t.cards {
                let file = folder.join(format!("1 Cut-out cards {}.pdf", clean(tag)));
                if self.workshop_write(&file, &cards.pdf) {
                    done.push(format!(
                        "cut-out cards {tag} ({} cut-outs, {} pages)",
                        cards.cutouts, cards.pages
                    ));
                }
                for big in &cards.too_large {
                    skipped.push(format!(
                        "cut-out card for {big} (too large for A4, it is on the tiled template)"
                    ));
                }
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
        // Files of the first version of this pack, now inside the packet.
        let mut replaced: Vec<&str> = Vec::new();
        for old in [
            "2 Cut list.pdf",
            "3 Ferrules.pdf",
            "4 Labels.pdf",
            "5 Assembly checklist.pdf",
        ] {
            if std::fs::remove_file(folder.join(old)).is_ok() {
                replaced.push(old);
            }
        }
        let mut separate: Vec<CoverLine> = Vec::new();
        if !templates.is_empty() {
            separate.push(CoverLine::new(
                "Print scale check",
                SCALE_CHECK_FILE,
                "Print first, at 100 % (actual size); the square must measure 100 mm",
                "A check of your printer, not of the drawing; printed accuracy depends on the printer",
            ));
        }
        for t in &templates {
            separate.push(CoverLine::new(
                &format!("Drilling template {}", t.tag),
                &format!("1 Drilling template {} {}.pdf", clean(&t.tag), paper.name()),
                "Print at 100 % (actual size), then check the 100 mm ruler",
                "Hole positions from the cabinet layout; about 0.5 mm on paper (not measured)",
            ));
            if t.cards.is_some() {
                separate.push(CoverLine::new(
                    &format!("Cut-out cards {}", t.tag),
                    &format!("1 Cut-out cards {}.pdf", clean(&t.tag)),
                    "Print at 100 % (actual size), then check the 50 mm square",
                    "Sizes and positions from the part data and layout; about 0.5 mm on paper (not measured)",
                ));
            }
        }
        let labels = folder.join("2 Labels.pdf");
        match self.export_labels_to(&labels) {
            Some(_) => {
                done.push("labels".to_owned());
                separate.push(CoverLine::new(
                    "Labels",
                    "2 Labels.pdf",
                    "Print at 100 % on the label stock",
                    "Texts from the drawing; sheet pitch from published data, never printed here",
                ));
            }
            None => skipped.push("labels".to_owned()),
        }
        match self.export_bench_packet_to(&folder.join(PACKET_FILE), &separate) {
            Some(_) => done.push("bench packet".to_owned()),
            None => skipped.push("bench packet".to_owned()),
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
        if !replaced.is_empty() {
            text.push_str(&format!(
                ". Replaced by the packet and removed: {}",
                replaced.join(", ")
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

    /// Reports > Bench output > Print scale check: the one-page check next to
    /// the project, replacing an earlier one.
    pub(super) fn export_scale_check(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = dir.join(format!("{} print scale check.pdf", clean(&stem)));
        let name = self.machine_title().unwrap_or_default();
        let bytes = novo_pdf::template_cards::export_scale_check_pdf(&name);
        let replaced = path.exists();
        if self.workshop_write(&path, &bytes) {
            let verb = if replaced { "Replaced" } else { "Exported" };
            self.message_file(
                format!(
                    "{verb} {}. Print it at 100 % and measure the square: it must be 100 mm",
                    path.display()
                ),
                &path,
            );
            self.wb.last_export = Some(path.clone());
            Some(path)
        } else {
            None
        }
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
