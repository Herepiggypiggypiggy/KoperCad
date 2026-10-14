//! The egui style built from the tokens.

use crate::tokens::*;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Vec2,
    Visuals, style::Selection, style::WidgetVisuals, style::Widgets,
};

/// The NovoElectric dark visuals.
pub fn visuals() -> Visuals {
    let control = CornerRadius::same(RADIUS_CONTROL);
    let window = CornerRadius::same(RADIUS_WINDOW);
    let widget = |bg: Color32, border: Color32, fg: Color32| WidgetVisuals {
        bg_fill: bg,
        weak_bg_fill: bg,
        bg_stroke: Stroke::new(1.0, border),
        corner_radius: control,
        fg_stroke: Stroke::new(1.0, fg),
        expansion: 0.0,
    };

    Visuals {
        dark_mode: true,
        override_text_color: None,
        weak_text_color: Some(TEXT_MUTED),
        widgets: Widgets {
            // Labels, separators, indent lines.
            noninteractive: widget(BG_1, BORDER, TEXT),
            // Buttons and fields at rest: a fill and no outline.
            inactive: widget(BG_3, Color32::TRANSPARENT, TEXT),
            hovered: widget(BG_4, BORDER_STRONG, Color32::WHITE),
            // Pressed or dragged.
            active: widget(BG_4, ACCENT, Color32::WHITE),
            // An open menu button or combo box.
            open: widget(BG_3, BORDER_STRONG, TEXT),
        },
        selection: Selection {
            bg_fill: ACCENT_SELECTION,
            stroke: Stroke::new(1.0, ACCENT_HOVER),
        },
        hyperlink_color: ACCENT,
        faint_bg_color: Color32::from_rgb(0x1B, 0x1E, 0x24),
        extreme_bg_color: BG_0,
        text_edit_bg_color: Some(BG_0),
        code_bg_color: BG_3,
        warn_fg_color: WARNING,
        error_fg_color: ERROR,
        window_corner_radius: window,
        window_shadow: Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(120),
        },
        window_fill: BG_2,
        window_stroke: Stroke::new(1.0, BORDER),
        menu_corner_radius: window,
        panel_fill: BG_1,
        popup_shadow: Shadow {
            offset: [0, 4],
            blur: 12,
            spread: 0,
            color: Color32::from_black_alpha(110),
        },
        striped: true,
        ..Visuals::dark()
    }
}

/// The full NovoElectric style: [`visuals`] plus text sizes and spacing.
pub fn style() -> Style {
    let mut style = Style {
        visuals: visuals(),
        ..Style::default()
    };
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(FONT_SMALL, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(FONT_BODY, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(FONT_BODY, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(FONT_HEADING, crate::fonts::strong_family()),
        ),
        (
            TextStyle::Monospace,
            FontId::new(FONT_MONO, FontFamily::Monospace),
        ),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(8.0, 6.0);
    s.button_padding = Vec2::new(10.0, 4.0);
    s.interact_size = Vec2::new(40.0, CONTROL_HEIGHT);
    s.window_margin = Margin::same(12);
    s.menu_margin = Margin::same(6);
    s.menu_spacing = 2.0;
    s.indent = 16.0;
    s.icon_width = 14.0;
    s.icon_width_inner = 8.0;
    s.icon_spacing = 6.0;
    s.combo_height = 240.0;
    s.tooltip_width = 360.0;

    style.animation_time = 0.08;
    style
}

/// Installs the NovoElectric look on `ctx`.
///
/// Sets the fonts, icons and dark style. NovoElectric is dark only for now,
/// so this also pins the theme to dark: the operating system's light or dark
/// setting no longer switches it.
pub fn apply(ctx: &egui::Context) {
    ctx.set_fonts(crate::fonts::definitions());
    ctx.set_theme(egui::Theme::Dark);
    ctx.set_style_of(egui::Theme::Dark, style());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_pins_dark_and_lays_out_text_and_icons() {
        let ctx = egui::Context::default();
        apply(&ctx);
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label(format!("{} Save", crate::icons::action::SAVE));
            ui.heading("Properties");
        });
        output.textures_delta.clear();
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(ctx.global_style().visuals.panel_fill, BG_1);
        assert_eq!(
            ctx.global_style().text_styles[&TextStyle::Body].size,
            FONT_BODY
        );
    }
}
