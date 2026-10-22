//! The single-line diagram in the Reports workspace.
//!
//! Where the user finds it: Reports > Single-line diagram (Ctrl+K "single
//! line"). The report has two tabs: the drawn diagram of the power
//! distribution from the supply to its loads, and the schedule (the same
//! boxes as a table, which Export writes to Excel and CSV). The drawing is the
//! one the generated page prints ([`novo_reports::single_line`]); this file
//! only paints it. Failing boxes and runs are outlined in the error colour.

use eframe::egui;
use novo_reports::{Align, PageLayout, PageSetup};

use super::NovoApp;

/// Pixels per millimetre of the drawing when it is shown at 100 %.
const PX_PER_MM: f32 = 3.4;

/// What the view remembers.
#[derive(Default)]
pub(crate) struct SingleLineView {
    /// The schedule tab instead of the drawing.
    pub(super) schedule: bool,
    /// The sheet shown, 0-based.
    pub(super) sheet: usize,
    /// Zoom factor on top of 100 %; 0 fits the drawing to the width.
    zoom: f32,
    /// The sheets at a project revision.
    cache: Option<(u64, Vec<PageLayout>)>,
}

/// The part of a layout that has anything on it, mm: `(x0, y0, x1, y1)`.
fn content_box(layout: &PageLayout) -> Option<(f64, f64, f64, f64)> {
    let mut b: Option<(f64, f64, f64, f64)> = None;
    let mut grow = |x: f64, y: f64| {
        b = Some(match b {
            None => (x, y, x, y),
            Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
        });
    };
    for l in &layout.lines {
        grow(l.from.0, l.from.1);
        grow(l.to.0, l.to.1);
    }
    for t in &layout.texts {
        // Text is as wide as its characters, not its box.
        let est = t.text.chars().count() as f64 * t.text_mm * 0.55;
        grow(t.x, t.y);
        grow(t.x + est.min(t.width), t.y + t.height);
    }
    b
}

impl NovoApp {
    /// The sheets of the diagram of the project as it is now.
    pub(super) fn single_line_sheets(&mut self) -> Vec<PageLayout> {
        let generation = self.machine_generation();
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let revision = doc.project().revision() ^ (generation << 40);
        if let Some((at, sheets)) = &self.single_line_view.cache
            && *at == revision
        {
            return sheets.clone();
        }
        let sheets = match self.machine() {
            Some(project) => {
                let sl = novo_reports::single_line::build(&project);
                novo_reports::single_line::layouts(&sl, &PageSetup::default())
            }
            None => Vec::new(),
        };
        self.single_line_view.cache = Some((revision, sheets.clone()));
        sheets
    }

    /// The PDF of the single-line report on the plain layout: the drawn
    /// sheets, then the schedule as tables.
    pub(super) fn single_line_pdf(
        &mut self,
        output: &novo_reports::ReportOutput,
    ) -> Option<Vec<u8>> {
        let name = self.project_name().unwrap_or_default();
        let setup = PageSetup::default();
        let mut layouts = {
            let project = self.machine()?;
            let sl = novo_reports::single_line::build(&project);
            novo_reports::single_line::layouts(&sl, &setup)
        };
        let mut bookmarks = Vec::new();
        if !layouts.is_empty() {
            bookmarks.push((0, "Single-line diagram".to_owned()));
        }
        bookmarks.push((layouts.len(), "Schedule".to_owned()));
        for page in novo_reports::paginate(output, &setup) {
            layouts.push(novo_reports::layout_page(output, &page, &setup));
        }
        Some(novo_pdf::export_layouts_pdf(
            &layouts,
            &format!("{name}  {}", output.title),
            &bookmarks,
        ))
    }

    /// The tabs and the drawing. Returns `true` when the drawing is what the
    /// workspace shows, so the table is left out.
    pub(super) fn single_line_view(&mut self, ui: &mut egui::Ui) -> bool {
        let sheets = self.single_line_sheets();
        let mut open_inputs = false;
        let view = &mut self.single_line_view;
        view.sheet = view.sheet.min(sheets.len().saturating_sub(1));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut view.schedule, false, "Diagram");
            ui.selectable_value(&mut view.schedule, true, "Schedule");
            if !view.schedule && sheets.len() > 1 {
                ui.separator();
                if ui
                    .add_enabled(view.sheet > 0, egui::Button::new("<"))
                    .on_hover_text("Previous sheet")
                    .clicked()
                {
                    view.sheet -= 1;
                }
                ui.label(format!("Sheet {} of {}", view.sheet + 1, sheets.len()));
                if ui
                    .add_enabled(view.sheet + 1 < sheets.len(), egui::Button::new(">"))
                    .on_hover_text("Next sheet")
                    .clicked()
                {
                    view.sheet += 1;
                }
            }
            if !view.schedule && !sheets.is_empty() {
                ui.separator();
                if ui.small_button("-").on_hover_text("Zoom out").clicked() {
                    view.zoom = (if view.zoom == 0.0 { 1.0 } else { view.zoom } / 1.25).max(0.3);
                }
                if ui
                    .small_button("Fit")
                    .on_hover_text("Fit the whole sheet")
                    .clicked()
                {
                    view.zoom = 0.0;
                }
                if ui.small_button("+").on_hover_text("Zoom in").clicked() {
                    view.zoom = (if view.zoom == 0.0 { 1.0 } else { view.zoom } * 1.25).min(4.0);
                }
            }
            ui.separator();
            if ui
                .small_button("Enter supply data")
                .on_hover_text(
                    "Supply voltage, short-circuit current or transformer, diversity: the \
                     numbers the short-circuit current and the voltage drop start from",
                )
                .clicked()
            {
                open_inputs = true;
            }
        });
        if open_inputs {
            self.toggle_electrical_open();
        }
        let view = &self.single_line_view;
        if view.schedule {
            return false;
        }
        let Some(layout) = sheets.get(view.sheet) else {
            ui.add_space(8.0);
            ui.label("There is no power distribution to draw yet.");
            ui.weak(
                "The diagram follows the supply (a potential feed) through fuses, breakers and \
                 contactors to motors, heaters, drives and power supplies. Draw a feeder with a \
                 supply and a load, or open the Schedule tab for what was found.",
            );
            return true;
        };
        let zoom = view.zoom;
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            paint(ui, layout, zoom);
        });
        true
    }
}

/// Paints one sheet. The page margins are cropped; `zoom` 0 fits the whole sheet.
fn paint(ui: &mut egui::Ui, layout: &PageLayout, zoom: f32) {
    let palette = crate::theme::ui_palette();
    let Some((x0, y0, x1, y1)) = content_box(layout) else {
        return;
    };
    let pad = 4.0;
    let (x0, y0, x1, y1) = (x0 - pad, y0 - pad, x1 + pad, y1 + pad);
    let fit = (ui.available_width() / (x1 - x0) as f32)
        .min(ui.available_height() / (y1 - y0) as f32)
        .clamp(1.2, 6.0);
    let scale = if zoom == 0.0 { fit } else { PX_PER_MM * zoom };
    let size = egui::vec2((x1 - x0) as f32 * scale, (y1 - y0) as f32 * scale);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, palette.sunken);
    let at = |x: f64, y: f64| {
        egui::pos2(
            rect.min.x + (x - x0) as f32 * scale,
            rect.min.y + (y - y0) as f32 * scale,
        )
    };
    for line in &layout.lines {
        let (colour, width) = if line.heavy {
            (palette.error, 2.0)
        } else {
            (palette.text_secondary, 1.0)
        };
        painter.line_segment(
            [at(line.from.0, line.from.1), at(line.to.0, line.to.1)],
            egui::Stroke::new(width, colour),
        );
    }
    for t in &layout.texts {
        if t.text.is_empty() {
            continue;
        }
        let top_left = at(t.x, t.y);
        let area = egui::Rect::from_min_size(
            top_left,
            egui::vec2(t.width as f32 * scale, t.height as f32 * scale),
        );
        let (anchor, x) = match t.align {
            Align::Left => (egui::Align2::LEFT_CENTER, area.left()),
            Align::Center => (egui::Align2::CENTER_CENTER, area.center().x),
            Align::Right => (egui::Align2::RIGHT_CENTER, area.right()),
        };
        let font = egui::FontId::proportional((t.text_mm as f32 * scale).max(6.0));
        let colour = if t.text.starts_with("FAIL") {
            palette.error
        } else if t.bold {
            palette.text_strong
        } else {
            palette.text_primary
        };
        painter
            .with_clip_rect(area.expand2(egui::vec2(0.0, 2.0)).intersect(rect))
            .text(
                egui::pos2(x, area.center().y),
                anchor,
                &t.text,
                font,
                colour,
            );
    }
}
