//! The key for the Heat layer in the 3D view: the temperature scale with
//! units, and what each other heat layer on screen means.

use eframe::egui;
use novo_heatview::range::{self, Range};
use novo_theme::{heat, tokens};

use super::heat_3d::Heat3d;

const BAR_W: f32 = 14.0;
const BAR_H: f32 = 150.0;
const STEPS: usize = 48;

/// The lines of text under the scale, one per layer that needs explaining.
pub(crate) fn notes(h: &Heat3d, range: Range, top_speed: f32) -> Vec<String> {
    let mut out = Vec::new();
    if h.iso {
        let level = range.lo + f64::from(h.iso_at) * (range.hi - range.lo);
        out.push(format!("Shell: air at {level:.0} °C"));
    }
    if h.lines {
        out.push(format!(
            "Streamlines: brighter is faster, up to {top_speed:.2} m/s"
        ));
    }
    if h.devices {
        out.push(String::from("Device bodies: air in front, same scale"));
    }
    out
}

/// Is there anything coloured by temperature on screen.
pub(crate) fn wanted(h: &Heat3d) -> bool {
    h.slice || h.iso || h.devices
}

/// Draws the key in the bottom right corner of `view`.
pub(crate) fn show(
    ctx: &egui::Context,
    view: egui::Rect,
    h: &Heat3d,
    range: Range,
    top_speed: f32,
) {
    if !wanted(h) {
        return;
    }
    egui::Area::new(egui::Id::new("heat_legend"))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::RIGHT_BOTTOM)
        .fixed_pos(view.right_bottom() + egui::vec2(-12.0, -12.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(egui::RichText::new("Air temperature, °C").small().strong());
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(120.0, BAR_H), egui::Sense::hover());
                let bar = egui::Rect::from_min_size(rect.min, egui::vec2(BAR_W, BAR_H));
                let painter = ui.painter();
                let step = BAR_H / STEPS as f32;
                for i in 0..STEPS {
                    // Hot at the top.
                    let t = 1.0 - (i as f32 + 0.5) / STEPS as f32;
                    let [r, g, b] = heat::ramp(t);
                    let y = bar.min.y + i as f32 * step;
                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(bar.min.x, y),
                            egui::vec2(BAR_W, step + 0.5),
                        ),
                        0.0,
                        egui::Color32::from_rgb(r, g, b),
                    );
                }
                for v in range::ticks(range) {
                    let y = bar.max.y - range.t(v) * BAR_H;
                    painter.line_segment(
                        [egui::pos2(bar.max.x, y), egui::pos2(bar.max.x + 4.0, y)],
                        egui::Stroke::new(1.0, tokens::TEXT_MUTED),
                    );
                    painter.text(
                        egui::pos2(bar.max.x + 7.0, y),
                        egui::Align2::LEFT_CENTER,
                        format!("{v:.0}"),
                        egui::FontId::proportional(11.0),
                        tokens::TEXT_MUTED,
                    );
                }
                for line in notes(h, range, top_speed) {
                    ui.label(egui::RichText::new(line).small());
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_layers_on_screen_get_a_note() {
        let range = Range { lo: 20.0, hi: 60.0 };
        let off = Heat3d {
            slice: true,
            iso: false,
            lines: false,
            devices: false,
            ..Heat3d::default()
        };
        assert!(notes(&off, range, 0.4).is_empty());
        let all = Heat3d {
            iso: true,
            iso_at: 0.5,
            lines: true,
            devices: true,
            ..off
        };
        let n = notes(&all, range, 0.4);
        assert_eq!(n.len(), 3);
        assert!(n[0].contains("40 °C"));
        assert!(n[1].contains("0.40 m/s"));
    }

    #[test]
    fn nothing_coloured_means_no_key() {
        let h = Heat3d {
            slice: false,
            iso: false,
            devices: false,
            lines: true,
            ..Heat3d::default()
        };
        assert!(!wanted(&h));
    }
}
