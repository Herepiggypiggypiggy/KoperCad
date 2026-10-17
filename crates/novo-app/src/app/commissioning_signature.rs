//! The signature pad: signing off a Fixed punch item by drawing with a
//! finger, pen or mouse, with the typed name as the part that counts.

use eframe::egui;
use novo_commissioning::Signature;
use novo_theme::Palette;

/// Height of the pad; the width is the panel's.
const PAD_HEIGHT: f32 = 120.0;

/// A signature being taken for one punch item.
pub(super) struct SignDraft {
    pub(super) item: u64,
    pub(super) name: String,
    strokes: Vec<Vec<[u16; 2]>>,
    drawing: bool,
}

impl SignDraft {
    pub(super) fn new(item: u64, name: &str) -> Self {
        SignDraft {
            item,
            name: name.to_owned(),
            strokes: Vec::new(),
            drawing: false,
        }
    }

    pub(super) fn strokes(&self) -> Vec<Vec<[u16; 2]>> {
        self.strokes.clone()
    }

    pub(super) fn clear(&mut self) {
        self.strokes.clear();
    }

    pub(super) fn is_blank(&self) -> bool {
        self.strokes.is_empty()
    }

    /// Adds a point to the stroke being drawn (starting one when `new`),
    /// from a position in 0..=1 across the pad.
    pub(super) fn add_point(&mut self, x: f32, y: f32, new: bool) {
        let pad = f32::from(Signature::PAD);
        let point = [
            (x.clamp(0.0, 1.0) * pad).round() as u16,
            (y.clamp(0.0, 1.0) * pad).round() as u16,
        ];
        if new || self.strokes.is_empty() {
            self.strokes.push(Vec::new());
        }
        if let Some(stroke) = self.strokes.last_mut()
            && stroke.last() != Some(&point)
        {
            stroke.push(point);
        }
    }
}

/// Draws the pad and takes strokes from it.
pub(super) fn pad(ui: &mut egui::Ui, draft: &mut SignDraft) {
    let p = Palette::of(ui);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), PAD_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    if let Some(pos) = response.interact_pointer_pos()
        && response.dragged()
    {
        let x = (pos.x - rect.left()) / rect.width();
        let y = (pos.y - rect.top()) / rect.height();
        draft.add_point(x, y, !draft.drawing);
        draft.drawing = true;
    } else {
        draft.drawing = false;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, p.sunken);
    painter.rect_stroke(
        rect,
        4.0,
        egui::Stroke::new(1.0, p.border_strong),
        egui::StrokeKind::Inside,
    );
    draw_strokes(&painter, rect, &draft.strokes, p.text_primary);
    if draft.is_blank() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "Sign here",
            egui::FontId::proportional(14.0),
            p.text_tertiary,
        );
    }
}

/// A saved signature, small, under the item it vouches for.
pub(super) fn preview(ui: &mut egui::Ui, signature: &Signature) {
    if signature.strokes.is_empty() {
        return;
    }
    let p = Palette::of(ui);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(160.0, 48.0), egui::Sense::hover());
    draw_strokes(
        &ui.painter_at(rect),
        rect,
        &signature.strokes,
        p.text_secondary,
    );
}

fn draw_strokes(
    painter: &egui::Painter,
    rect: egui::Rect,
    strokes: &[Vec<[u16; 2]>],
    color: egui::Color32,
) {
    let pad = f32::from(Signature::PAD);
    for stroke in strokes {
        let points: Vec<egui::Pos2> = stroke
            .iter()
            .map(|&[x, y]| {
                egui::pos2(
                    rect.left() + f32::from(x) / pad * rect.width(),
                    rect.top() + f32::from(y) / pad * rect.height(),
                )
            })
            .collect();
        painter.add(egui::Shape::line(points, egui::Stroke::new(2.0, color)));
    }
}
