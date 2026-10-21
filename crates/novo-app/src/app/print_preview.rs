//! Print preview and plot sets: File > Export print > Print preview and
//! plot sets... shows every sheet on the paper it will print on, before a
//! PDF is written.
//!
//! * The left column picks the pages (schematic pages and generated report
//!   pages alike; a report page counts as every sheet it prints), the paper,
//!   orientation, scale, margins and page numbers.
//! * The middle shows one paper at a time, the sheet drawn from the same
//!   scene the PDF is written from (`novo_pdf::draw_sheets`), the printable
//!   box, the page number strip and a warning when a sheet is cut off.
//! * A plot set is a named choice of pages with its print setup. Sets are
//!   kept per user and per project file (settings), and *Save every plot
//!   set* writes one PDF per set.
//! * The second tab shows the workshop pack: how each mounting plate is cut
//!   into drilling-template pages on A4 or A3, tile by tile.
//!
//! Placement (scale, offsets, cut-off) is `novo_pdf::print::place`, the same
//! function the PDF writer uses, so what is previewed is what is printed.

use std::collections::BTreeSet;
use std::path::PathBuf;

use eframe::egui::{self, Color32, Pos2, Rect, Stroke, vec2};
use novo_canvas::model::PlaceholderArt;
use novo_canvas::{Label, Shape, Vec2};
use novo_core::PageId;
use novo_layout::template::Paper;
use novo_pdf::print::{self, Orientation, PrintPaper, PrintScale, PrintSetup};
use novo_pdf::{DrawOp, DrawnSheet, PageSet, Placement};

use super::{Level, NovoApp};
use crate::settings::{PlotSetChoice, PrintChoice};

/// The paper of the preview and of the scale-check line, in millimetres per
/// screen point at "fit": nothing here, the preview always fits its box.
const PAPER_GAP: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tab {
    Sheets,
    Workshop,
}

/// The window while open.
pub(crate) struct PrintPreview {
    tab: Tab,
    choice: PrintChoice,
    colour: bool,
    /// The pages ticked, by id.
    pick: BTreeSet<PageId>,
    /// The project's pages at the last refresh: id, name, title.
    pages: Vec<(PageId, String, String)>,
    /// The drawn sheets for `built_for`.
    sheets: Vec<DrawnSheet>,
    /// What `sheets` was drawn for: pages, look and the project's revision.
    built_for: Option<(BTreeSet<PageId>, bool, u64)>,
    current: usize,
    /// The plot set loaded, as an index into the project's sets.
    set: Option<usize>,
    set_name: String,
    /// Said under the controls: what went wrong or what was done.
    note: Option<String>,
}

/// What the buttons of one frame asked for.
#[derive(Default)]
struct Wants {
    refresh: bool,
    save_pdf: bool,
    save_set: bool,
    delete_set: bool,
    load_set: Option<Option<usize>>,
    export_all_sets: bool,
    workshop: bool,
}

pub(crate) fn setup_of(c: &PrintChoice) -> PrintSetup {
    PrintSetup {
        paper: PrintPaper::from_key(&c.paper).unwrap_or_default(),
        orientation: Orientation::from_key(&c.orientation).unwrap_or_default(),
        scale: if c.fit {
            PrintScale::Fit
        } else {
            PrintScale::Percent(c.percent as f32)
        },
        margin_mm: c.margin_mm as f32,
        centre: c.centre,
        page_numbers: c.page_numbers,
    }
}

/// The sets saved for a project, by the tag of its file path.
fn project_key(path: Option<&std::path::Path>) -> String {
    path.map_or_else(|| "untitled".to_owned(), super::pdf::owner_tag)
}

/// File-name safe version of a plot set name.
fn file_part(name: &str) -> String {
    name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
        .trim()
        .to_owned()
}

impl NovoApp {
    /// Opens the window: all pages ticked, the last print setup.
    pub(super) fn open_print_preview(&mut self) {
        if self.backend.document().is_none() {
            return;
        }
        let mut state = PrintPreview {
            tab: Tab::Sheets,
            choice: self.settings.print_choice.clone(),
            colour: self.settings.pdf_choice.look == "colour",
            pick: BTreeSet::new(),
            pages: Vec::new(),
            sheets: Vec::new(),
            built_for: None,
            current: 0,
            set: None,
            set_name: String::new(),
            note: None,
        };
        self.refresh_pages(&mut state);
        state.pick = state.pages.iter().map(|(id, ..)| *id).collect();
        self.wb.print_preview = Some(Box::new(state));
    }

    /// Opens the window on the workshop pack tab.
    pub(super) fn open_print_preview_workshop(&mut self) {
        self.open_print_preview();
        if let Some(state) = self.wb.print_preview.as_mut() {
            state.tab = Tab::Workshop;
        }
    }

    fn plot_sets(&self) -> Vec<PlotSetChoice> {
        let key = project_key(self.backend.file().as_deref());
        self.settings
            .plot_sets
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    fn refresh_pages(&self, state: &mut PrintPreview) {
        state.pages = self
            .backend
            .document()
            .map(|d| {
                d.project()
                    .pages_in_order()
                    .iter()
                    .map(|p| (p.id, p.name.to_string(), p.title.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        let known: BTreeSet<PageId> = state.pages.iter().map(|(id, ..)| *id).collect();
        state.pick.retain(|id| known.contains(id));
    }

    /// The options for the pages ticked, in the project's order.
    fn print_options(
        &mut self,
        state: &PrintPreview,
        pages: &BTreeSet<PageId>,
    ) -> novo_pdf::PdfOptions {
        let mut choice = self.settings.pdf_choice.clone();
        choice.look = if state.colour { "colour" } else { "black" }.into();
        // The appendix (design checks) is part of the PDF, not of the
        // sheets ticked here.
        choice.checks_appendix = false;
        self.pdf_options_for(
            PageSet::Pages(pages.iter().copied().collect()),
            &choice,
            Some(setup_of(&state.choice)),
        )
    }

    /// Draws the sheets for the ticked pages when they, the look or the
    /// project changed since the last time.
    fn rebuild_sheets(&mut self, state: &mut PrintPreview, force: bool) {
        let Some(revision) = self.backend.document().map(|d| d.project().revision()) else {
            return;
        };
        let key = (state.pick.clone(), state.colour, revision);
        if !force && state.built_for.as_ref() == Some(&key) {
            return;
        }
        if state.built_for.as_ref().is_none_or(|k| k.2 != revision) {
            self.refresh_pages(state);
        }
        let options = self.print_options(state, &state.pick.clone());
        let sheets = {
            let Some(doc) = self.backend.document() else {
                return;
            };
            let mut reports = novo_reports::LiveReports::standard();
            let art = self
                .library()
                .map(|lib| novo_editor::art::LibraryArt::new(lib, options.style.symbol));
            match &art {
                Some(art) => novo_pdf::draw_sheets(doc.project(), &options, art, &mut reports),
                None => {
                    novo_pdf::draw_sheets(doc.project(), &options, &PlaceholderArt, &mut reports)
                }
            }
        };
        match sheets {
            Ok(sheets) => state.sheets = sheets,
            Err(e) => {
                state.sheets.clear();
                state.note = Some(format!("Could not draw the pages: {e}"));
            }
        }
        state.current = state.current.min(state.sheets.len().saturating_sub(1));
        state.built_for = Some(key);
    }

    pub(super) fn print_preview_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.wb.print_preview.take() else {
            return;
        };
        if self.backend.document().is_none() {
            return;
        }
        self.rebuild_sheets(&mut state, false);
        let sets = self.plot_sets();
        let workshop = self.workshop_overview(state.tab == Tab::Workshop);
        let mut open = true;
        let mut wants = Wants::default();
        let palette = crate::theme::ui_palette();
        super::panel::dialog(ctx, "Print preview and plot sets", [1080.0, 700.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.tab, Tab::Sheets, "Sheets and reports");
                    ui.selectable_value(&mut state.tab, Tab::Workshop, "Workshop pack");
                });
                ui.separator();
                match state.tab {
                    Tab::Sheets => sheets_tab(ui, &mut state, &sets, &mut wants, palette),
                    Tab::Workshop => {
                        let mut paper = self.wb.workshop_paper;
                        workshop_tab(ui, &workshop, &mut paper, &mut wants);
                        self.wb.workshop_paper = paper;
                    }
                }
            });
        self.print_actions(&mut state, wants, &sets);
        if open {
            self.wb.print_preview = Some(state);
        }
    }

    fn print_actions(&mut self, state: &mut PrintPreview, wants: Wants, sets: &[PlotSetChoice]) {
        self.settings.print_choice = state.choice.clone();
        if wants.refresh {
            self.refresh_pages(state);
            self.rebuild_sheets(state, true);
        }
        if let Some(index) = wants.load_set {
            state.set = index;
            if let Some(set) = index.and_then(|i| sets.get(i)) {
                let by_name: BTreeSet<&str> = set.pages.iter().map(String::as_str).collect();
                state.pick = state
                    .pages
                    .iter()
                    .filter(|(_, name, _)| by_name.contains(name.as_str()))
                    .map(|(id, ..)| *id)
                    .collect();
                state.choice = set.print.clone();
                state.colour = set.look == "colour";
                state.set_name = set.name.clone();
                let missing = set.pages.len() - state.pick.len().min(set.pages.len());
                state.note = (missing > 0).then(|| {
                    format!(
                        "{missing} page{} of this set no longer exist{}.",
                        if missing == 1 { "" } else { "s" },
                        if missing == 1 { "s" } else { "" }
                    )
                });
            }
        }
        if wants.save_set {
            self.save_plot_set(state);
        }
        if wants.delete_set
            && let Some(i) = state.set
        {
            let key = project_key(self.backend.file().as_deref());
            if let Some(list) = self.settings.plot_sets.get_mut(&key)
                && i < list.len()
            {
                let gone = list.remove(i);
                state.note = Some(format!("Deleted the plot set {}.", gone.name));
            }
            state.set = None;
        }
        if wants.save_pdf {
            self.save_preview_pdf(state);
        }
        if wants.export_all_sets {
            self.export_every_plot_set(state);
        }
        if wants.workshop {
            self.export_workshop();
        }
    }

    fn save_plot_set(&mut self, state: &mut PrintPreview) {
        let name = state.set_name.trim().to_owned();
        if name.is_empty() {
            state.note = Some("Type a name for the plot set first.".into());
            return;
        }
        if state.pick.is_empty() {
            state.note = Some("Tick at least one page for the plot set.".into());
            return;
        }
        let set = PlotSetChoice {
            name: name.clone(),
            pages: state
                .pages
                .iter()
                .filter(|(id, ..)| state.pick.contains(id))
                .map(|(_, n, _)| n.clone())
                .collect(),
            print: state.choice.clone(),
            look: if state.colour { "colour" } else { "black" }.into(),
        };
        let key = project_key(self.backend.file().as_deref());
        let list = self.settings.plot_sets.entry(key).or_default();
        let replaced = if let Some(i) = list.iter().position(|s| s.name == name) {
            list[i] = set;
            state.set = Some(i);
            true
        } else {
            list.push(set);
            state.set = Some(list.len() - 1);
            false
        };
        state.note = Some(format!(
            "{} the plot set {name}.",
            if replaced { "Updated" } else { "Saved" }
        ));
    }

    /// Writes the ticked pages with the setup on screen.
    fn save_preview_pdf(&mut self, state: &mut PrintPreview) -> Option<PathBuf> {
        if state.pick.is_empty() {
            state.note = Some("Tick at least one page to print.".into());
            return None;
        }
        let (dir, stem) = self.pdf_base()?;
        let tail = match state
            .set
            .and_then(|i| self.plot_sets().get(i).map(|s| s.name.clone()))
        {
            Some(name) => format!("plot {}", file_part(&name)),
            None => "print".to_owned(),
        };
        let path = self.own_pdf_path(&dir.join(format!("{} {tail}.pdf", file_part(&stem))));
        let options = self.print_options(state, &state.pick.clone());
        self.settings.pdf_choice.look = if state.colour { "colour" } else { "black" }.into();
        self.export_options_to(&path, &options)
    }

    /// One PDF per saved plot set of this project.
    fn export_every_plot_set(&mut self, state: &mut PrintPreview) -> Vec<PathBuf> {
        let sets = self.plot_sets();
        if sets.is_empty() {
            state.note = Some("Save a plot set first.".into());
            return Vec::new();
        }
        let Some((dir, stem)) = self.pdf_base() else {
            return Vec::new();
        };
        let pages: Vec<(PageId, String)> = state
            .pages
            .iter()
            .map(|(id, name, _)| (*id, name.clone()))
            .collect();
        let mut written: Vec<PathBuf> = Vec::new();
        for set in &sets {
            let ids: BTreeSet<PageId> = pages
                .iter()
                .filter(|(_, n)| set.pages.contains(n))
                .map(|(id, _)| *id)
                .collect();
            if ids.is_empty() {
                self.message(
                    Level::Warning,
                    format!("Plot set {} has no pages left, skipped", set.name),
                );
                continue;
            }
            let as_state = PrintPreview {
                tab: Tab::Sheets,
                choice: set.print.clone(),
                colour: set.look == "colour",
                pick: ids.clone(),
                pages: Vec::new(),
                sheets: Vec::new(),
                built_for: None,
                current: 0,
                set: None,
                set_name: String::new(),
                note: None,
            };
            let options = self.print_options(&as_state, &ids);
            let path = self.own_pdf_path(&dir.join(format!(
                "{} plot {}.pdf",
                file_part(&stem),
                file_part(&set.name)
            )));
            if let Some(p) = self.export_options_to(&path, &options) {
                written.push(p);
            }
        }
        state.note = Some(format!(
            "Wrote {} plot set PDF{}.",
            written.len(),
            if written.len() == 1 { "" } else { "s" }
        ));
        written
    }

    /// The plates of the workshop pack and how each is cut into pages on the
    /// paper chosen for it.
    fn workshop_overview(&self, wanted: bool) -> Vec<WorkshopPlate> {
        if !wanted {
            return Vec::new();
        }
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let mut out = Vec::new();
        for s in project
            .structures()
            .filter(|s| s.enclosure && s.plate.is_some())
        {
            let Some((sheets, _)) = novo_layout::model::template_documents(project, s.id) else {
                continue;
            };
            out.push(WorkshopPlate {
                tag: s.tag.to_string(),
                sheets: sheets
                    .iter()
                    .map(|sh| WorkshopSurface {
                        title: sh.title.clone(),
                        size: (sh.width, sh.height),
                        sheet: sh.clone(),
                    })
                    .collect(),
            });
        }
        out
    }
}

struct WorkshopPlate {
    tag: String,
    sheets: Vec<WorkshopSurface>,
}

struct WorkshopSurface {
    title: String,
    size: (f64, f64),
    sheet: novo_layout::template::TemplateSheet,
}

// ---- Sheets tab ----------------------------------------------------------

fn sheets_tab(
    ui: &mut egui::Ui,
    state: &mut PrintPreview,
    sets: &[PlotSetChoice],
    wants: &mut Wants,
    palette: &novo_theme::Palette,
) {
    let height = ui.available_height().max(420.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            vec2(270.0, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("print_controls")
                    .max_height(height)
                    .show(ui, |ui| controls(ui, state, sets, wants, palette));
            },
        );
        ui.separator();
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| preview_area(ui, state, height, palette),
        );
    });
}

fn controls(
    ui: &mut egui::Ui,
    state: &mut PrintPreview,
    sets: &[PlotSetChoice],
    wants: &mut Wants,
    palette: &novo_theme::Palette,
) {
    novo_theme::widgets::section_label(ui, "Plot set");
    let current = state
        .set
        .and_then(|i| sets.get(i))
        .map_or("Not saved".to_owned(), |s| s.name.clone());
    egui::ComboBox::from_id_salt("plot_set")
        .selected_text(current)
        .width(240.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(state.set.is_none(), "Not saved")
                .clicked()
            {
                wants.load_set = Some(None);
            }
            for (i, s) in sets.iter().enumerate() {
                if ui
                    .selectable_label(
                        state.set == Some(i),
                        format!("{} ({} pages)", s.name, s.pages.len()),
                    )
                    .clicked()
                {
                    wants.load_set = Some(Some(i));
                }
            }
        });
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.set_name)
                .hint_text("Name, like Issue for tender")
                .desired_width(150.0),
        );
        if ui
            .button("Save")
            .on_hover_text("Save the pages and setup below under this name")
            .clicked()
        {
            wants.save_set = true;
        }
        if ui
            .add_enabled(state.set.is_some(), egui::Button::new("Delete"))
            .clicked()
        {
            wants.delete_set = true;
        }
    });
    ui.add_space(6.0);

    novo_theme::widgets::section_label(ui, "Pages");
    ui.horizontal(|ui| {
        if ui.small_button("All").clicked() {
            state.pick = state.pages.iter().map(|(id, ..)| *id).collect();
        }
        if ui.small_button("None").clicked() {
            state.pick.clear();
        }
        if ui
            .small_button("Refresh")
            .on_hover_text("Draw the pages again")
            .clicked()
        {
            wants.refresh = true;
        }
    });
    egui::ScrollArea::vertical()
        .id_salt("print_pages")
        .max_height(170.0)
        .show(ui, |ui| {
            for (id, name, title) in state.pages.clone() {
                let mut on = state.pick.contains(&id);
                let label = if title.is_empty() {
                    name
                } else {
                    format!("{name}  {title}")
                };
                if ui.checkbox(&mut on, label).changed() {
                    if on {
                        state.pick.insert(id);
                    } else {
                        state.pick.remove(&id);
                    }
                }
            }
        });
    ui.add_space(6.0);

    novo_theme::widgets::section_label(ui, "Paper");
    let c = &mut state.choice;
    let paper = PrintPaper::from_key(&c.paper).unwrap_or_default();
    egui::ComboBox::from_id_salt("print_paper")
        .selected_text(paper.name())
        .width(240.0)
        .show_ui(ui, |ui| {
            for p in PrintPaper::ALL {
                if ui.selectable_label(p == paper, p.name()).clicked() {
                    c.paper = p.key().into();
                }
            }
        });
    ui.horizontal(|ui| {
        let current = Orientation::from_key(&c.orientation).unwrap_or_default();
        for o in Orientation::ALL {
            if ui.selectable_label(o == current, o.name()).clicked() {
                c.orientation = o.key().into();
            }
        }
    });
    ui.add_space(4.0);
    novo_theme::widgets::section_label(ui, "Scale");
    ui.horizontal(|ui| {
        ui.radio_value(&mut c.fit, true, "Fit to page");
        ui.radio_value(&mut c.fit, false, "Fixed");
    });
    ui.add_enabled_ui(!c.fit, |ui| {
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut c.percent)
                    .range(print::MIN_PERCENT as u32..=print::MAX_PERCENT as u32)
                    .suffix(" %"),
            );
            if ui
                .small_button("100 %")
                .on_hover_text("True size")
                .clicked()
            {
                c.percent = 100;
            }
        });
    });
    ui.add_space(4.0);
    novo_theme::widgets::section_label(ui, "Margins and numbers");
    ui.horizontal(|ui| {
        ui.label("Margin");
        ui.add(
            egui::DragValue::new(&mut c.margin_mm)
                .range(0..=40)
                .suffix(" mm"),
        );
    });
    ui.checkbox(&mut c.centre, "Centre the sheet on the paper");
    ui.checkbox(
        &mut c.page_numbers,
        "Page numbers and sheet name below the sheet",
    );
    ui.horizontal(|ui| {
        ui.label("Look");
        ui.selectable_value(&mut state.colour, false, "Black on white");
        ui.selectable_value(&mut state.colour, true, "In colour");
    });
    ui.add_space(8.0);

    let can = !state.pick.is_empty();
    if ui
        .add_enabled(can, egui::Button::new("Save PDF"))
        .on_hover_text("Writes the pages ticked, as previewed, to your Documents\\KoperCAD folder")
        .clicked()
    {
        wants.save_pdf = true;
    }
    if ui
        .add_enabled(!sets.is_empty(), egui::Button::new("Save every plot set"))
        .on_hover_text("One PDF per saved plot set of this project")
        .clicked()
    {
        wants.export_all_sets = true;
    }
    if let Some(note) = &state.note {
        ui.add_space(4.0);
        ui.colored_label(palette.warning, note);
    }
}

/// A short line about the sheet on screen.
fn sheet_summary(sheet: &DrawnSheet, placement: &Placement) -> String {
    let (w, h) = sheet.size_mm;
    let (pw, ph) = placement.paper;
    format!(
        "Sheet {}: {:.0} x {:.0} mm on {:.0} x {:.0} mm paper, printed at {}",
        sheet.name,
        w,
        h,
        pw,
        ph,
        placement.percent_text()
    )
}

fn preview_area(
    ui: &mut egui::Ui,
    state: &mut PrintPreview,
    height: f32,
    palette: &novo_theme::Palette,
) {
    let setup = setup_of(&state.choice);
    let total = state.sheets.len();
    if total == 0 {
        ui.add_space(40.0);
        ui.vertical_centered(|ui| {
            ui.weak(if state.pick.is_empty() {
                "Tick the pages to print on the left."
            } else {
                "Nothing to show for these pages."
            });
        });
        return;
    }
    let places: Vec<Placement> = state
        .sheets
        .iter()
        .map(|s| print::place(s.size_mm, &setup))
        .collect();
    let cut: Vec<usize> = places
        .iter()
        .enumerate()
        .filter(|(_, p)| p.cut_off)
        .map(|(i, _)| i)
        .collect();

    // Keys: left and right (or Page Up and Down) turn the sheets.
    let typing = ui.ctx().memory(|m| m.focused().is_some());
    let step = ui.input(|i| {
        if typing {
            return 0;
        }
        let next = i.key_pressed(egui::Key::ArrowRight) || i.key_pressed(egui::Key::PageDown);
        let prev = i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::PageUp);
        i32::from(next) - i32::from(prev)
    });
    if step != 0 {
        state.current = (state.current as i32 + step).clamp(0, total as i32 - 1) as usize;
    }
    state.current = state.current.min(total - 1);
    let index = state.current;

    ui.horizontal(|ui| {
        if ui.add_enabled(index > 0, egui::Button::new("<")).clicked() {
            state.current -= 1;
        }
        ui.label(format!("Paper {} of {total}", index + 1));
        if ui
            .add_enabled(index + 1 < total, egui::Button::new(">"))
            .clicked()
        {
            state.current += 1;
        }
        ui.separator();
        ui.weak(sheet_summary(&state.sheets[index], &places[index]));
    });
    if !cut.is_empty() {
        let names: Vec<&str> = cut
            .iter()
            .take(6)
            .map(|i| state.sheets[*i].name.as_str())
            .collect();
        let more = if cut.len() > 6 { ", ..." } else { "" };
        ui.colored_label(
            palette.warning,
            format!(
                "{} sheet{} cut off at the margins at this scale ({}{more}). Use Fit to page, a larger paper or a smaller scale.",
                cut.len(),
                if cut.len() == 1 { " is" } else { "s are" },
                names.join(", ")
            ),
        );
    }

    let strip_h = 78.0;
    let area = vec2(ui.available_width(), (height - 60.0 - strip_h).max(240.0));
    let (rect, _) = ui.allocate_exact_size(area, egui::Sense::hover());
    paint_paper(
        ui,
        rect,
        &state.sheets[index],
        &places[index],
        &setup,
        index,
        total,
        state.colour,
    );

    // The strip of all papers: a click shows one.
    egui::ScrollArea::horizontal()
        .id_salt("print_strip")
        .max_height(strip_h)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                #[allow(clippy::needless_range_loop)]
                for i in 0..total {
                    let (r, resp) = ui.allocate_exact_size(vec2(60.0, 66.0), egui::Sense::click());
                    let p = &places[i];
                    let scale =
                        ((r.width() - 8.0) / p.paper.0).min((r.height() - 16.0) / p.paper.1);
                    let size = vec2(p.paper.0 * scale, p.paper.1 * scale);
                    let paper = Rect::from_min_size(
                        Pos2::new(r.center().x - size.x / 2.0, r.min.y + 2.0),
                        size,
                    );
                    let painter = ui.painter_at(r);
                    painter.rect_filled(paper, 1.0, Color32::WHITE);
                    let sheet_rect = Rect::from_min_size(
                        paper.min + vec2(p.offset.0, p.offset.1) * scale,
                        vec2(state.sheets[i].size_mm.0, state.sheets[i].size_mm.1)
                            * p.scale
                            * scale,
                    )
                    .intersect(paper);
                    painter.rect_filled(
                        sheet_rect,
                        0.0,
                        if p.cut_off {
                            palette.warning.gamma_multiply(0.45)
                        } else {
                            Color32::from_gray(205)
                        },
                    );
                    let selected = i == index;
                    painter.rect_stroke(
                        paper,
                        1.0,
                        Stroke::new(
                            if selected { 2.0 } else { 1.0 },
                            if selected {
                                palette.accent
                            } else {
                                Color32::GRAY
                            },
                        ),
                        egui::StrokeKind::Outside,
                    );
                    painter.text(
                        Pos2::new(r.center().x, r.max.y - 2.0),
                        egui::Align2::CENTER_BOTTOM,
                        &state.sheets[i].name,
                        egui::FontId::proportional(10.0),
                        ui.visuals().text_color(),
                    );
                    if resp.clicked() {
                        state.current = i;
                    }
                    resp.on_hover_text(sheet_summary(&state.sheets[i], p));
                }
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn paint_paper(
    ui: &egui::Ui,
    area: Rect,
    sheet: &DrawnSheet,
    placement: &Placement,
    setup: &PrintSetup,
    index: usize,
    total: usize,
    colour: bool,
) {
    let (pw, ph) = placement.paper;
    let ppm = ((area.width() - 2.0 * PAPER_GAP) / pw).min((area.height() - 2.0 * PAPER_GAP) / ph);
    if ppm <= 0.0 {
        return;
    }
    let size = vec2(pw, ph) * ppm;
    let paper = Rect::from_center_size(area.center(), size);
    let painter = ui.painter_at(area);
    painter.rect_filled(
        paper.translate(vec2(3.0, 3.0)),
        2.0,
        Color32::from_black_alpha(70),
    );
    painter.rect_filled(paper, 0.0, Color32::WHITE);
    // The printable box.
    let (bx, by, bw, bh) = placement.printable;
    let printable = Rect::from_min_size(paper.min + vec2(bx, by) * ppm, vec2(bw, bh) * ppm);
    painter.rect_stroke(
        printable,
        0.0,
        Stroke::new(1.0, Color32::from_rgb(120, 170, 230).gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );

    let sheet_painter = if placement.cut_off {
        painter.with_clip_rect(printable)
    } else {
        painter.clone()
    };
    let scale = placement.scale;
    let map = |p: Vec2| -> Pos2 {
        paper.min
            + vec2(
                placement.offset.0 + p.x * scale,
                placement.offset.1 + p.y * scale,
            ) * ppm
    };
    let ink = |c: novo_canvas::Color| -> Color32 {
        let [r, g, b, a] = c.0;
        Color32::from_rgba_unmultiplied(r, g, b, a)
    };
    let _ = colour;
    for op in &sheet.ops {
        match op {
            DrawOp::Primitive(p) => paint_primitive(&sheet_painter, p, &map, scale * ppm, &ink),
            DrawOp::Label(l) => paint_label(&sheet_painter, l, &map, scale * ppm, &ink),
        }
    }
    // The strip under the sheet, in paper millimetres.
    let paper_map = |p: Vec2| -> Pos2 { paper.min + vec2(p.x, p.y) * ppm };
    for l in novo_pdf::footer_labels(setup, &sheet.name, placement, index + 1, total) {
        paint_label(&painter, &l, &paper_map, ppm, &ink);
    }
    if placement.cut_off {
        painter.rect_stroke(
            printable,
            0.0,
            Stroke::new(2.0, Color32::from_rgb(220, 120, 30)),
            egui::StrokeKind::Inside,
        );
    }
}

fn stroke_of(
    s: &novo_canvas::Stroke,
    px_per_sheet_mm: f32,
    ink: &dyn Fn(novo_canvas::Color) -> Color32,
) -> Stroke {
    let mm = match s.unit {
        novo_canvas::WidthUnit::World => s.width,
        novo_canvas::WidthUnit::Screen => s.width * 25.4 / 72.0,
    };
    // As on paper: no pen is thinner than 0.15 mm, and a hairline still
    // shows on screen.
    Stroke::new(
        (mm * px_per_sheet_mm).max(0.15 * px_per_sheet_mm).max(0.6),
        ink(s.color),
    )
}

fn paint_primitive(
    painter: &egui::Painter,
    p: &novo_canvas::Primitive,
    map: &dyn Fn(Vec2) -> Pos2,
    px: f32,
    ink: &dyn Fn(novo_canvas::Color) -> Color32,
) {
    let stroke = p
        .stroke
        .as_ref()
        .filter(|s| s.color.0[3] > 0 && s.width > 0.0);
    let stroke_e = stroke.map_or(Stroke::NONE, |s| stroke_of(s, px, ink));
    let fill = p
        .fill
        .filter(|c| c.0[3] > 0)
        .map_or(Color32::TRANSPARENT, ink);
    match &p.shape {
        Shape::Line { a, b } => {
            painter.line_segment([map(*a), map(*b)], stroke_e);
        }
        Shape::Polyline { points, closed } => {
            let pts: Vec<Pos2> = points.iter().map(|q| map(*q)).collect();
            if pts.len() == 1 {
                painter.circle_filled(pts[0], stroke_e.width / 2.0, stroke_e.color);
            } else if pts.len() > 1 {
                painter.add(egui::Shape::Path(egui::epaint::PathShape {
                    points: pts,
                    closed: *closed,
                    fill: if *closed { fill } else { Color32::TRANSPARENT },
                    stroke: stroke_e.into(),
                }));
            }
        }
        Shape::Rect { min, max } => {
            let r = Rect::from_two_pos(map(*min), map(*max));
            if fill.a() > 0 {
                painter.rect_filled(r, 0.0, fill);
            }
            if stroke.is_some() {
                painter.rect_stroke(r, 0.0, stroke_e, egui::StrokeKind::Middle);
            }
        }
        Shape::Circle { center, radius } => {
            let c = map(*center);
            let r = radius * px;
            painter.add(egui::epaint::CircleShape {
                center: c,
                radius: r,
                fill,
                stroke: stroke_e,
            });
        }
        Shape::Arc {
            center,
            radius,
            start,
            sweep,
        } => {
            let steps = ((sweep.abs() / 0.15).ceil() as usize).clamp(4, 96);
            let pts: Vec<Pos2> = (0..=steps)
                .map(|i| {
                    let a = start + sweep * i as f32 / steps as f32;
                    map(*center + Vec2::new(a.cos(), a.sin()) * *radius)
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke_e));
        }
    }
}

fn paint_label(
    painter: &egui::Painter,
    l: &Label,
    map: &dyn Fn(Vec2) -> Pos2,
    px: f32,
    ink: &dyn Fn(novo_canvas::Color) -> Color32,
) {
    let size = l.height * px;
    if size < 2.5 || l.color.0[3] == 0 || l.text.trim().is_empty() {
        return;
    }
    let at = map(l.pos);
    let galley = painter.layout_no_wrap(
        l.text.clone(),
        egui::FontId::proportional(size),
        ink(l.color),
    );
    let f = l.anchor.fractions();
    let g = galley.size();
    let local = vec2(-g.x * f.x, -g.y * f.y);
    let (sin, cos) = l.rotation.sin_cos();
    let turned = vec2(local.x * cos - local.y * sin, local.x * sin + local.y * cos);
    let mut shape = egui::epaint::TextShape::new(at + turned, galley, ink(l.color));
    shape.angle = l.rotation;
    painter.add(shape);
}

// ---- Workshop tab -------------------------------------------------------

fn workshop_tab(ui: &mut egui::Ui, plates: &[WorkshopPlate], paper: &mut Paper, wants: &mut Wants) {
    ui.horizontal(|ui| {
        ui.label("Drilling templates print at true size (1:1) on");
        ui.selectable_value(paper, Paper::A4, "A4");
        ui.selectable_value(paper, Paper::A3, "A3");
        ui.weak("Tiles overlap by 10 mm and carry cross marks to match them.");
    });
    ui.horizontal(|ui| {
        if ui.button("Write the workshop pack").clicked() {
            wants.workshop = true;
        }
        ui.weak("Also writes the cut list, ferrules, labels and assembly checklist (Bench output settings).");
    });
    ui.separator();
    if plates.is_empty() {
        ui.weak("No cabinet has a mounting plate yet. Set one in the Cabinet workspace and the drilling template appears here.");
        return;
    }
    egui::ScrollArea::vertical().show(ui, |ui| {
        for plate in plates {
            let pages: usize = plate
                .sheets
                .iter()
                .map(|s| novo_pdf::template::page_count(&s.sheet, *paper))
                .sum();
            let tiles: usize = plate
                .sheets
                .iter()
                .map(|s| novo_pdf::template::tile_plan(&s.sheet, *paper).count())
                .sum();
            ui.strong(pages_line(&plate.tag, pages, tiles, plate.sheets.len()));
            ui.horizontal_wrapped(|ui| {
                for s in &plate.sheets {
                    let plan = novo_pdf::template::tile_plan(&s.sheet, *paper);
                    ui.vertical(|ui| {
                        ui.label(format!(
                            "{}: {:.0} x {:.0} mm on {} {} page{} ({} x {})",
                            s.title,
                            s.size.0,
                            s.size.1,
                            paper.name(),
                            if plan.landscape {
                                "landscape"
                            } else {
                                "portrait"
                            },
                            if plan.count() == 1 { "" } else { "s" },
                            plan.cols,
                            plan.rows
                        ));
                        paint_tiles(ui, &plan);
                    });
                }
            });
            ui.add_space(8.0);
        }
    });
}

/// The surface with its tiles named: what one page of the template covers.
fn paint_tiles(ui: &mut egui::Ui, plan: &novo_layout::template::TilePlan) {
    let width = 300.0;
    let scale = (width / plan.width as f32).min(170.0 / plan.height as f32);
    let size = vec2(plan.width as f32 * scale, plan.height as f32 * scale);
    let (r, _) = ui.allocate_exact_size(size + vec2(2.0, 2.0), egui::Sense::hover());
    let painter = ui.painter_at(r);
    let base = r.min + vec2(1.0, 1.0);
    painter.rect_filled(
        Rect::from_min_size(base, size),
        0.0,
        Color32::from_gray(250),
    );
    let (cw, ch) = plan.content_mm();
    for row in 0..plan.rows {
        for col in 0..plan.cols {
            let (ox, oy) = plan.origin(col, row);
            let tile = Rect::from_min_size(
                base + vec2(ox as f32, oy as f32) * scale,
                vec2(cw as f32, ch as f32) * scale,
            )
            .intersect(Rect::from_min_size(base, size));
            painter.rect_filled(
                tile,
                0.0,
                Color32::from_rgb(120, 170, 230).gamma_multiply(0.18),
            );
            painter.rect_stroke(
                tile,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(70, 120, 200)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                tile.center(),
                egui::Align2::CENTER_CENTER,
                novo_layout::template::TilePlan::name(col, row),
                egui::FontId::proportional(13.0),
                Color32::from_rgb(40, 80, 150),
            );
        }
    }
    painter.rect_stroke(
        Rect::from_min_size(base, size),
        0.0,
        Stroke::new(1.5, Color32::DARK_GRAY),
        egui::StrokeKind::Outside,
    );
}

#[cfg(test)]
impl NovoApp {
    fn with_preview<R>(&mut self, f: impl FnOnce(&mut Self, &mut PrintPreview) -> R) -> Option<R> {
        let mut state = self.wb.print_preview.take()?;
        let out = f(self, &mut state);
        self.wb.print_preview = Some(state);
        Some(out)
    }

    /// Pages ticked and sheets drawn.
    pub(super) fn print_preview_counts(&mut self) -> Option<(usize, usize)> {
        self.with_preview(|app, s| {
            app.rebuild_sheets(s, false);
            (s.pick.len(), s.sheets.len())
        })
    }

    /// Ticks only the first `n` pages.
    pub(super) fn print_preview_pick(&mut self, n: usize) {
        self.with_preview(|_, s| {
            s.pick = s.pages.iter().take(n).map(|(id, ..)| *id).collect();
        });
    }

    pub(super) fn print_preview_set_paper(&mut self, key: &str) {
        self.with_preview(|_, s| s.choice.paper = key.into());
    }

    pub(super) fn print_preview_name_set(&mut self, name: &str) {
        self.with_preview(|_, s| s.set_name = name.into());
    }

    pub(super) fn print_preview_save_set(&mut self) {
        self.with_preview(|app, s| app.save_plot_set(s));
    }

    pub(super) fn print_preview_save_pdf(&mut self) -> Option<PathBuf> {
        self.with_preview(|app, s| app.save_preview_pdf(s))
            .flatten()
    }

    pub(super) fn print_preview_export_all_sets(&mut self) -> Vec<PathBuf> {
        self.with_preview(|app, s| app.export_every_plot_set(s))
            .unwrap_or_default()
    }

    pub(super) fn print_preview_show_workshop(&mut self) {
        self.with_preview(|_, s| s.tab = Tab::Workshop);
    }

    pub(super) fn plot_sets_for_test(&self) -> Vec<PlotSetChoice> {
        self.plot_sets()
    }
}

/// "+CAB: 10 pages (8 tiles and 2 overviews)": a sheet prints its tiles and one overview.
fn pages_line(tag: &str, pages: usize, tiles: usize, overviews: usize) -> String {
    let n = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    format!(
        "{tag}: {} ({} and {})",
        n(pages, "page", "pages"),
        n(tiles, "tile", "tiles"),
        n(overviews, "overview", "overviews")
    )
}

#[cfg(test)]
mod pages_line_tests {
    use super::pages_line;

    #[test]
    fn the_parts_add_up_for_one_and_for_two_sheets() {
        assert_eq!(
            pages_line("+CAB", 9, 8, 1),
            "+CAB: 9 pages (8 tiles and 1 overview)"
        );
        assert_eq!(
            pages_line("+CAB", 10, 8, 2),
            "+CAB: 10 pages (8 tiles and 2 overviews)"
        );
        assert_eq!(
            pages_line("+K1", 2, 1, 1),
            "+K1: 2 pages (1 tile and 1 overview)"
        );
    }
}
