//! The mounting view of a circuit macro: the reference plate with its DIN
//! rails and the cabinet devices on it, then the devices in the door and the
//! ones on the machine as short lists. The data is
//! [`novo_parts::MountingView`]; this file only fits and paints it. Sizes
//! are typical values for a generic device, so the view says so.

use eframe::egui::{self, Align2, FontId, Rect, Stroke, StrokeKind};
use novo_core::Point;
use novo_parts::{MountItem, MountingView};

/// Shown under every mounting view: the sizes are a guide, not a datasheet.
const SIZES_NOTE: &str =
    "Typical sizes of generic devices; the real size comes from the part you assign.";

/// Paints `view` at most `max_width` wide, with its lists below.
pub(super) fn show(ui: &mut egui::Ui, view: &MountingView, values: &[(String, String)]) {
    let pal = crate::theme::ui_palette();
    let (pw, ph) = (view.plate.width, view.plate.height);
    let width = ui.available_width().min(380.0);
    let height = (width as f64 * ph / pw.max(1.0)).clamp(90.0, 240.0) as f32;
    ui.weak(format!(
        "Plate {pw:.0} x {ph:.0} mm, rails {}. Layout suggestion, not a measured cabinet.",
        view.rails.len()
    ));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, pal.sunken);
    if let Some((scale, offset)) =
        super::macro_thumb::fit((Point::new(0.0, 0.0), Point::new(pw, ph)), rect.size(), 6.0)
    {
        let at = |x: f64, y: f64| {
            rect.min + egui::vec2(x as f32 * scale + offset.x, y as f32 * scale + offset.y)
        };
        let plate = Rect::from_min_max(at(0.0, 0.0), at(pw, ph));
        painter.rect_filled(plate, 2.0, pal.panel);
        painter.rect_stroke(
            plate,
            2.0,
            Stroke::new(1.0, pal.border_strong),
            StrokeKind::Inside,
        );
        for rail in &view.rails {
            let strip = Rect::from_min_max(
                at(rail.x, rail.y - 17.5),
                at(rail.x + rail.length, rail.y + 17.5),
            );
            painter.rect_filled(strip, 0.0, pal.control);
            painter.line_segment(
                [at(rail.x, rail.y), at(rail.x + rail.length, rail.y)],
                Stroke::new(1.0, pal.border_control),
            );
        }
        for item in view.panel() {
            let Some([l, t, r, b]) = item.rect() else {
                continue;
            };
            let device = Rect::from_min_max(at(l, t), at(r, b));
            painter.rect_filled(device, 2.0, pal.accent_wash);
            painter.rect_stroke(
                device,
                2.0,
                Stroke::new(1.0, pal.accent),
                StrokeKind::Inside,
            );
            if device.width() > 18.0 && device.height() > 10.0 {
                painter.text(
                    device.center(),
                    Align2::CENTER_CENTER,
                    &item.tag,
                    FontId::proportional(10.0),
                    pal.text_primary,
                );
            }
        }
    }
    ui.weak(format!(
        "Plate {pw:.0} x {ph:.0} mm, rails {}",
        view.rails.len()
    ));
    let list = |ui: &mut egui::Ui, title: &str, items: Vec<&MountItem>, place: bool| {
        if items.is_empty() {
            return;
        }
        novo_theme::widgets::section_label(ui, title);
        for i in items {
            let mut text = format!("{}  {}", i.tag, i.description_with(values));
            if place {
                if let Some(p) = &i.place {
                    text.push_str(&format!(" \u{b7} {p}"));
                }
            } else if let Some([w, h, _]) = i.size {
                text.push_str(&format!(" \u{b7} {w:.0} x {h:.0} mm"));
            }
            if let Some(n) = &i.note {
                text.push_str(&format!(" \u{b7} {n}"));
            }
            ui.add(egui::Label::new(egui::RichText::new(text).weak()).wrap());
        }
    };
    list(
        ui,
        "In the door or operator panel",
        view.door().collect(),
        false,
    );
    list(ui, "On the machine", view.field().collect(), true);
    ui.add(egui::Label::new(egui::RichText::new(SIZES_NOTE).weak().small()).wrap());
}
