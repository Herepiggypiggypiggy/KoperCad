//! The building blocks of the Cabinet toolbar (docs/design/cabinet-toolbar.md):
//! a captioned group, an icon-plus-label button, the one copper primary and
//! the count pills of the status strip. Colours are `Palette` roles; sizes
//! come from the theme's control height.

use eframe::egui;
use novo_theme::{Palette, fonts, icon_size, radius};

/// Width of the bar under which whole groups fold into More. Labels are
/// never shortened: a group either fits or folds.
pub(super) const WIDE: f32 = 1100.0;

/// Group captions are never smaller than this (design review).
const CAPTION: f32 = 12.0;

/// A named group of buttons: the caption in small caps above, the buttons
/// in a row below.
pub(super) fn group(ui: &mut egui::Ui, caption: &str, add: impl FnOnce(&mut egui::Ui)) {
    let p = Palette::of(ui);
    ui.add_space(8.0);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(
            egui::RichText::new(caption.to_uppercase())
                .size(CAPTION)
                .color(p.text_tertiary),
        );
        ui.horizontal(add);
    });
}

/// A bar button: icon, then label. `selected` is a toggle that is on.
pub(super) fn button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    let text = egui::RichText::new(format!("{icon}  {label}"));
    ui.add(egui::Button::new(text).selected(selected))
}

/// One half of the Plate | Table switch.
pub(super) fn segment(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    ui.add(egui::Button::new(label).selected(selected))
}

/// The one copper control of the bar: icon, label and the key. When it
/// cannot be used it is drawn as a plain button so the position stays and
/// the bar never shows two copper fills.
pub(super) fn primary(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> egui::Response {
    let p = Palette::of(ui);
    let text = format!("{icon}  {label}");
    if !enabled {
        let mut b = egui::Button::new(text);
        if let Some(s) = shortcut {
            b = b.shortcut_text(s);
        }
        return ui.add_enabled(false, b);
    }
    let mut b = egui::Button::new(
        egui::RichText::new(text)
            .family(fonts::strong_family())
            .size(icon_size::BUTTON.max(14.0))
            .color(p.on_accent),
    )
    .fill(p.accent)
    .stroke(egui::Stroke::NONE);
    if let Some(s) = shortcut {
        b = b.shortcut_text(egui::RichText::new(s).color(p.on_accent));
    }
    ui.add(b)
}

/// A count in the status strip. `level` colours it when it needs a look.
pub(super) fn pill(ui: &mut egui::Ui, text: &str, level: Option<novo_theme::widgets::Level>) {
    let p = Palette::of(ui);
    let (fill, color) = match level {
        Some(l) => (l.wash(p), l.color(p)),
        None => (p.control, p.text_secondary),
    };
    egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(radius::PILL))
        .inner_margin(egui::Margin::symmetric(9, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).color(color));
        });
}

/// A bar control a lesson may point at; reported to the learning layer once
/// the frame has released its borrow of the project.
pub(super) struct Anchor {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) rect: egui::Rect,
    pub(super) clicked: bool,
}

/// Remembers `response` as the anchor `id` and hands it back.
pub(super) fn note(
    anchors: &mut Vec<Anchor>,
    id: &'static str,
    label: &'static str,
    response: &egui::Response,
) -> egui::Response {
    anchors.push(Anchor {
        id,
        label,
        rect: response.rect,
        clicked: response.clicked(),
    });
    response.clone()
}
