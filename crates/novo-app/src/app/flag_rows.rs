//! Triangles in rows (docs/design/problem-triangles.md section 4, "Lists and
//! tables"): a leading triangle instead of a coloured sentence.

use eframe::egui;
use novo_flags::{Flag, Severity};
use novo_theme::{Palette, widgets::flag_triangle};

/// Side of the triangle in a row, in points (the design's 14 pt, a little
/// larger so a finger finds it).
const SIZE: f32 = 15.0;

/// A 20 pt slot with a problem triangle in it. Clicking it is the caller's
/// business (it opens the popover for flag rows).
pub(super) fn triangle_slot(
    ui: &mut egui::Ui,
    red: bool,
    dashed: bool,
    name: &str,
) -> egui::Response {
    let (slot, response) = ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::click());
    if ui.is_rect_visible(slot) {
        flag_triangle(
            ui.painter(),
            slot.center(),
            SIZE,
            red,
            dashed,
            Palette::of(ui),
        );
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    response.on_hover_text(name)
}

/// The triangle of a flag, with its accessible name on hover.
pub(super) fn flag_slot(ui: &mut egui::Ui, flag: &Flag) -> egui::Response {
    triangle_slot(
        ui,
        flag.severity == Severity::Red,
        flag.unchecked,
        &flag.accessible_name(),
    )
}

/// One row of a list: a triangle, then the text in the normal colour.
pub(super) fn problem_row(ui: &mut egui::Ui, red: bool, text: impl Into<String>) {
    let text = text.into();
    ui.horizontal_top(|ui| {
        triangle_slot(
            ui,
            red,
            false,
            &format!("{}: {text}", if red { "Error" } else { "Warning" }),
        );
        ui.add(egui::Label::new(text).wrap());
    });
}
