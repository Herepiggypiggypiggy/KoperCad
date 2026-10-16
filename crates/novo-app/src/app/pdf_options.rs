//! The PDF options dialog: File > Export drawings > Export PDF... asks which
//! pages, bookmarks, links and extras go into the PDF, remembers the answer
//! per user (the settings file under the user's profile) and exports.
//! Export PDF and Export this page as PDF stay one-click with the defaults.

use eframe::egui;
use novo_core::PageId;
use novo_pdf::PageSet;
use std::path::PathBuf;

use super::{Level, NovoApp};
use crate::settings::PdfChoice;

/// The dialog while open: the choice being edited and what is wrong with it.
pub(crate) struct PdfOptionsState {
    pub(crate) choice: PdfChoice,
}

/// The look of the drawing: print style (black on white) or the colours of
/// the canvas without its editing marks.
pub(crate) fn style_of(choice: &PdfChoice) -> novo_canvas::model::ModelStyle {
    match choice.look.as_str() {
        "colour" => novo_canvas::model::ModelStyle {
            corner_half: 0.0,
            ..Default::default()
        },
        _ => novo_pdf::print_style(),
    }
}

/// Page numbers (1 is the first page) from text such as `1-3, 5, 7-`: a
/// number, a range `a-b`, or `a-` for "to the last page". Separators are
/// commas or semicolons. Returns the numbers sorted, each once, or what is
/// wrong with the text.
pub(crate) fn parse_ranges(text: &str, count: usize) -> Result<Vec<usize>, String> {
    let mut out = std::collections::BTreeSet::new();
    let mut any = false;
    for part in text.split([',', ';']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        any = true;
        let number = |s: &str| {
            s.trim()
                .parse::<usize>()
                .map_err(|_| format!("`{part}` is not a page number or range"))
        };
        let (from, to) = match part.split_once('-') {
            None => {
                let n = number(part)?;
                (n, n)
            }
            Some((a, b)) if b.trim().is_empty() => (number(a)?, count),
            Some((a, b)) => (number(a)?, number(b)?),
        };
        if from == 0 || to == 0 || from > count || to > count {
            return Err(format!(
                "`{part}` is outside the {count} page{}",
                if count == 1 { "" } else { "s" }
            ));
        }
        if from > to {
            return Err(format!("`{part}` runs backwards"));
        }
        out.extend(from..=to);
    }
    if !any {
        return Err("Type the pages to export, like 1-3, 5".into());
    }
    Ok(out.into_iter().collect())
}

/// The pages `choice` selects, from the project's pages in order and the
/// page on screen.
pub(crate) fn page_set(
    choice: &PdfChoice,
    pages: &[PageId],
    current: Option<PageId>,
) -> Result<PageSet, String> {
    match choice.scope.as_str() {
        "current" => current
            .map(|p| PageSet::Pages(vec![p]))
            .ok_or_else(|| "No page is open".to_owned()),
        "ranges" => Ok(PageSet::Pages(
            parse_ranges(&choice.ranges, pages.len())?
                .into_iter()
                .map(|n| pages[n - 1])
                .collect(),
        )),
        _ => Ok(PageSet::All),
    }
}

impl NovoApp {
    /// Opens the dialog with the last choice.
    pub(super) fn open_pdf_options(&mut self) {
        if self.backend.document().is_none() {
            return;
        }
        self.wb.pdf_options = Some(PdfOptionsState {
            choice: self.settings.pdf_choice.clone(),
        });
    }

    /// Exports with `choice` and remembers it. Says why when the pages
    /// cannot be worked out. The file name follows the pages: the project's
    /// PDF for all of them, `<project> page <name>.pdf` for the one on
    /// screen, `<project> pages <numbers>.pdf` for a selection.
    pub(super) fn export_pdf_with_choice(&mut self, choice: &PdfChoice) -> Option<PathBuf> {
        let (pages, current) = {
            let doc = self.backend.document()?;
            let pages: Vec<PageId> = doc.project().pages().map(|p| p.id).collect();
            (pages, self.current_page)
        };
        let set = match page_set(choice, &pages, current) {
            Ok(set) => set,
            Err(e) => {
                self.message(Level::Warning, format!("Export PDF: {e}"));
                return None;
            }
        };
        self.settings.pdf_choice = choice.clone();
        let path = match (&set, choice.scope.as_str()) {
            (PageSet::All, _) => self.pdf_path()?,
            (_, scope) => {
                let tail = if scope == "current" {
                    let name = current
                        .and_then(|p| self.backend.document()?.project().page(p))
                        .map(|p| p.name.to_string())?;
                    format!("page {name}")
                } else {
                    format!("pages {}", choice.ranges.trim())
                };
                let (dir, stem) = self.export_base()?;
                let file = format!("{stem} {tail}.pdf").replace(['/', '\\', ':'], "-");
                super::pdf::replaceable_path(&dir.join(file))
            }
        };
        self.export_pdf_choice_to(&path, set, choice)
    }

    pub(super) fn pdf_options_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.wb.pdf_options.take() else {
            return;
        };
        let pages: Vec<(PageId, String)> = self
            .backend
            .document()
            .map(|d| {
                d.project()
                    .pages()
                    .map(|p| (p.id, p.name.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let ids: Vec<PageId> = pages.iter().map(|(id, _)| *id).collect();
        let problem = page_set(&state.choice, &ids, self.current_page).err();
        let mut open = true;
        let mut export = false;
        let mut defaults = false;
        super::panel::dialog(ctx, "Export PDF", [440.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let c = &mut state.choice;
                ui.strong("Pages");
                ui.radio_value(&mut c.scope, "all".into(), "All pages");
                ui.radio_value(&mut c.scope, "current".into(), "The page on screen");
                ui.horizontal(|ui| {
                    ui.radio_value(&mut c.scope, "ranges".into(), "Pages");
                    let edit = ui.add_enabled(
                        c.scope == "ranges",
                        egui::TextEdit::singleline(&mut c.ranges)
                            .hint_text("1-3, 5, 7-")
                            .desired_width(160.0),
                    );
                    edit.on_hover_text(format!(
                        "Page numbers in the project's order (1 to {}), like 1-3, 5, 7-",
                        pages.len()
                    ));
                });
                ui.add_space(6.0);
                ui.strong("In the PDF");
                ui.checkbox(&mut c.bookmarks, "Bookmarks, one per page");
                ui.checkbox(
                    &mut c.links,
                    "Links between a device's parts and between pages",
                );
                ui.checkbox(&mut c.xref_notes, "Cross references next to symbols");
                ui.checkbox(&mut c.title_revisions, "Revision row in the title block");
                ui.checkbox(
                    &mut c.changed_marks,
                    "Mark pages changed since the last revision",
                );
                ui.checkbox(&mut c.checks_appendix, "Design checks after the drawing");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Look");
                    ui.selectable_value(&mut c.look, "black".into(), "Black on white");
                    ui.selectable_value(&mut c.look, "colour".into(), "In colour");
                });
                ui.horizontal(|ui| {
                    ui.label("Title");
                    ui.add(
                        egui::TextEdit::singleline(&mut c.title)
                            .hint_text("The project name")
                            .desired_width(240.0),
                    );
                });
                ui.add_space(6.0);
                if let Some(p) = &problem {
                    ui.colored_label(ui.visuals().warn_fg_color, p);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(problem.is_none(), egui::Button::new("Export"))
                        .clicked()
                    {
                        export = true;
                    }
                    if ui.button("Use the defaults").clicked() {
                        defaults = true;
                    }
                });
                ui.weak("Your choice is remembered for next time.");
            });
        if defaults {
            state.choice = PdfChoice::default();
        }
        if export {
            let choice = state.choice.clone();
            self.export_pdf_with_choice(&choice);
            return;
        }
        if open {
            self.wb.pdf_options = Some(state);
        }
    }
}
