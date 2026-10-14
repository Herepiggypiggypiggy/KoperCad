//! Small building blocks that carry the house style, so screens don't each
//! reinvent a panel header or a primary button.

use crate::{fonts, tokens};
use egui::{Button, Response, RichText, Stroke, Ui, Vec2};

/// A quiet uppercase label that opens a panel section, in place of a heading.
pub fn section_label(ui: &mut Ui, text: &str) -> Response {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(tokens::FONT_SMALL - 0.5)
            .family(fonts::strong_family())
            .extra_letter_spacing(1.2)
            .color(tokens::TEXT_MUTED),
    )
}

/// The one button in a dialog that does the main thing: copper text on the
/// normal control fill with a copper hairline, rather than a loud solid fill.
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        Button::new(
            RichText::new(text)
                .family(fonts::strong_family())
                .color(tokens::ACCENT_HOVER),
        )
        .stroke(Stroke::new(1.0, tokens::ACCENT)),
    )
}

/// A square, frameless toolbar button showing only an icon. `active` marks
/// the current tool with a copper icon and a short copper bar beneath it.
pub fn tool_button(ui: &mut Ui, icon: &str, active: bool, tooltip: &str) -> Response {
    let color = if active {
        tokens::ACCENT_HOVER
    } else {
        tokens::TEXT_MUTED
    };
    let side = tokens::CONTROL_HEIGHT + 4.0;
    let response = ui
        .add(
            Button::new(RichText::new(icon).size(16.0).color(color))
                .frame(false)
                .min_size(Vec2::splat(side)),
        )
        .on_hover_text(tooltip);
    if active {
        let r = response.rect;
        let bar = egui::Rect::from_center_size(
            egui::pos2(r.center().x, r.bottom() - 1.0),
            Vec2::new(12.0, 2.0),
        );
        ui.painter().rect_filled(bar, 1, tokens::ACCENT);
    }
    response
}
