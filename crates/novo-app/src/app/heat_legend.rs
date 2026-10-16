//! The key for the Heat layer in the 3D view: the temperature scale with
//! units, and what each other heat layer on screen means.

use eframe::egui;
use novo_heatview::range::{self, Range};
use novo_theme::heat;

use super::heat_3d::Heat3d;

const BAR_LEN: f32 = 150.0;
const BAR_H: f32 = 10.0;
/// Room under the bar for the degree labels, and either side for the end ones.
const LABEL_H: f32 = 16.0;
const TICK_PAD: f32 = 12.0;
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

/// Draws the key inside the Heat strip above the view: a colour bar with
/// degree ticks, then one note per active layer. It sits in the strip's own
/// row (wrapping with it), so it can never cover a panel.
pub(crate) fn strip(ui: &mut egui::Ui, h: &Heat3d, range: Range, top_speed: f32) {
    if !wanted(h) {
        return;
    }
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(BAR_LEN + 2.0 * TICK_PAD, BAR_H + LABEL_H),
        egui::Sense::hover(),
    );
    response
        .on_hover_text("Air temperature in °C; the slice, shell and device colours use this scale");
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + TICK_PAD, rect.min.y),
        egui::vec2(BAR_LEN, BAR_H),
    );
    let painter = ui.painter();
    let step = BAR_LEN / STEPS as f32;
    for i in 0..STEPS {
        let [r, g, b] = heat::ramp((i as f32 + 0.5) / STEPS as f32);
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(bar.min.x + i as f32 * step, bar.min.y),
                egui::vec2(step + 0.5, BAR_H),
            ),
            0.0,
            egui::Color32::from_rgb(r, g, b),
        );
    }
    for (frac, v) in tick_marks(range) {
        let x = bar.min.x + frac * BAR_LEN;
        painter.line_segment(
            [egui::pos2(x, bar.max.y), egui::pos2(x, bar.max.y + 3.0)],
            egui::Stroke::new(1.0, crate::theme::ui_palette().text_secondary),
        );
        painter.text(
            egui::pos2(x, bar.max.y + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{v:.0}"),
            egui::FontId::proportional(10.0),
            crate::theme::ui_palette().text_secondary,
        );
    }
    ui.label(egui::RichText::new("°C").small());
    for line in notes(h, range, top_speed) {
        ui.label(egui::RichText::new(line).small());
    }
}

/// The ticks of the scale as (position 0 to 1 along the bar, degrees), only
/// those that fit on the bar.
pub(crate) fn tick_marks(range: Range) -> Vec<(f32, f64)> {
    range::ticks(range)
        .into_iter()
        .map(|v| (range.t(v), v))
        .filter(|(f, _)| (0.0..=1.0).contains(f))
        .collect()
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
    fn ticks_stay_on_the_bar_and_rise_with_temperature() {
        let t = tick_marks(Range { lo: 20.0, hi: 60.0 });
        assert!(t.len() >= 2);
        assert!(t.iter().all(|(f, _)| (0.0..=1.0).contains(f)));
        assert!(t.windows(2).all(|w| w[0].0 < w[1].0 && w[0].1 < w[1].1));
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
