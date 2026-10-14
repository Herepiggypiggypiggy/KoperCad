//! The hook between the shell and the drawing canvas.
//!
//! The shell gives the central area to a [`CanvasView`]. `novo-canvas`
//! (WS-03) provides the real one; [`PlaceholderCanvas`] stands in until then.

use crate::backend::PageId;
use eframe::egui;

pub trait CanvasView {
    /// Draws `page` (or an empty state when `None`) into the whole of `ui`.
    fn show(&mut self, ui: &mut egui::Ui, page: Option<PageId>);

    /// Fits the open page into the view.
    fn zoom_to_fit(&mut self) {}

    /// Short text for the status bar, such as cursor coordinates.
    fn status_text(&self) -> Option<String> {
        None
    }
}

/// Draws a sheet outline on a dot grid and reports the cursor position, so the
/// shell layout can be checked before the real canvas lands.
#[derive(Default)]
pub struct PlaceholderCanvas {
    cursor_mm: Option<egui::Pos2>,
}

/// Grid pitch of the placeholder, in screen points.
const GRID: f32 = 20.0;
/// Millimetres per grid step, so the status bar reads like a drawing.
const MM_PER_GRID: f32 = 5.0;

impl CanvasView for PlaceholderCanvas {
    fn show(&mut self, ui: &mut egui::Ui, page: Option<PageId>) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        let rect = response.rect;
        let visuals = ui.visuals();
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);

        if page.is_none() {
            self.cursor_mm = None;
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No project open\nCtrl+N new project · Ctrl+O open · Ctrl+K commands",
                egui::FontId::proportional(15.0),
                visuals.weak_text_color(),
            );
            return;
        }

        let dot = visuals.weak_text_color().gamma_multiply(0.5);
        let mut y = rect.top() + GRID;
        while y < rect.bottom() {
            let mut x = rect.left() + GRID;
            while x < rect.right() {
                painter.circle_filled(egui::pos2(x, y), 0.8, dot);
                x += GRID;
            }
            y += GRID;
        }

        let sheet = rect.shrink(GRID * 2.0);
        painter.rect_stroke(
            sheet,
            0.0,
            egui::Stroke::new(1.0, visuals.text_color()),
            egui::StrokeKind::Inside,
        );
        painter.text(
            sheet.center(),
            egui::Align2::CENTER_CENTER,
            "Canvas engine not connected yet",
            egui::FontId::proportional(14.0),
            visuals.weak_text_color(),
        );

        self.cursor_mm = response.hover_pos().map(|p| {
            let local = p - rect.left_top();
            egui::pos2(local.x / GRID * MM_PER_GRID, local.y / GRID * MM_PER_GRID)
        });
    }

    fn status_text(&self) -> Option<String> {
        self.cursor_mm
            .map(|p| format!("X {:.1} mm  Y {:.1} mm", p.x, p.y))
    }
}
