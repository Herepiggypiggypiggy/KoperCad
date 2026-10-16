//! The egui style built from the palette and the density.

use crate::density::{Density, ThemeChoice};
use crate::palette::Palette;
use crate::tokens::*;
use egui::{
    Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, Style, TextStyle, Theme,
    Vec2, Visuals, style::Selection, style::WidgetVisuals, style::Widgets,
};

/// The visuals for `p`'s theme.
pub fn visuals_for(p: &Palette) -> Visuals {
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
    let base = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let shadow = |offset: i8, blur: u8, alpha: u8| Shadow {
        offset: [0, offset],
        blur,
        spread: 0,
        color: Color32::from_black_alpha(alpha),
    };

    Visuals {
        dark_mode: p.dark,
        override_text_color: None,
        weak_text_color: Some(p.text_secondary),
        widgets: Widgets {
            // Labels, separators, indent lines.
            noninteractive: widget(p.panel, p.border_subtle, p.text_primary),
            // Buttons and fields at rest: a quiet fill and a hairline.
            inactive: widget(p.control, p.border_subtle, p.text_primary),
            hovered: widget(p.hover, p.border_strong, p.text_strong),
            // Pressed or dragged: the only place a control turns copper.
            active: widget(p.hover, p.accent, p.text_strong),
            // An open menu button or combo box.
            open: widget(p.hover, p.border_strong, p.text_primary),
        },
        // Selected text and rows are the copper wash; the stroke is the
        // keyboard focus ring.
        selection: Selection {
            bg_fill: p.accent_wash,
            stroke: Stroke::new(1.5, p.accent),
        },
        hyperlink_color: p.info,
        faint_bg_color: if p.dark {
            Color32::from_rgb(0x18, 0x18, 0x1A)
        } else {
            Color32::from_rgb(0xF7, 0xF6, 0xF3)
        },
        extreme_bg_color: p.sunken,
        text_edit_bg_color: Some(p.sunken),
        code_bg_color: p.control,
        warn_fg_color: p.warning,
        error_fg_color: p.error,
        window_corner_radius: window,
        window_shadow: shadow(6, 20, p.shadow_alpha),
        window_fill: p.raised,
        window_stroke: Stroke::new(1.0, p.border_subtle),
        menu_corner_radius: window,
        panel_fill: p.panel,
        popup_shadow: shadow(3, 10, p.shadow_alpha.saturating_sub(10)),
        striped: true,
        ..base
    }
}

/// The dark visuals (the look before the light theme existed).
pub fn visuals() -> Visuals {
    visuals_for(&Palette::DARK)
}

/// The full style for `p`'s theme at `density`: [`visuals_for`] plus text
/// sizes and spacing.
pub fn style_for(p: &Palette, density: Density) -> Style {
    let m = density.metrics();
    let mut style = Style {
        visuals: visuals_for(p),
        ..Style::default()
    };
    style.text_styles = [
        (
            TextStyle::Small,
            FontId::new(m.font_small, FontFamily::Proportional),
        ),
        (
            TextStyle::Body,
            FontId::new(m.font_body, FontFamily::Proportional),
        ),
        (
            TextStyle::Button,
            FontId::new(m.font_body, FontFamily::Proportional),
        ),
        (
            TextStyle::Heading,
            FontId::new(m.font_title, crate::fonts::strong_family()),
        ),
        (
            TextStyle::Monospace,
            FontId::new(m.font_mono, FontFamily::Monospace),
        ),
    ]
    .into();

    let s = &mut style.spacing;
    s.item_spacing = Vec2::from(m.item_spacing);
    s.button_padding = Vec2::from(m.button_padding);
    s.interact_size = Vec2::new(40.0, m.control_height);
    s.window_margin = Margin::same(12);
    s.menu_margin = Margin::same(6);
    s.menu_spacing = 2.0;
    s.indent = 16.0;
    s.icon_width = 14.0;
    s.icon_width_inner = 8.0;
    s.icon_spacing = 6.0;
    s.combo_height = 240.0;
    s.tooltip_width = 360.0;
    style.interaction.interact_radius = m.interact_radius;

    style.animation_time = 0.08;
    style
}

/// The dark style at standard density.
pub fn style() -> Style {
    style_for(&Palette::DARK, Density::Standard)
}

/// Installs the KoperCAD look on `ctx`: fonts, icons, and both themes at
/// `density`. Dark is the default theme; use [`set_theme`] for the person's
/// choice.
pub fn apply(ctx: &egui::Context) {
    apply_with(ctx, ThemeChoice::Dark, Density::Standard);
}

/// [`apply`] with the person's choice of theme and density.
pub fn apply_with(ctx: &egui::Context, theme: ThemeChoice, density: Density) {
    ctx.set_fonts(crate::fonts::definitions());
    set_density(ctx, density);
    set_theme(ctx, theme);
}

/// Switches between dark, light and the system theme at once.
pub fn set_theme(ctx: &egui::Context, theme: ThemeChoice) {
    ctx.set_theme(theme.preference());
}

/// Re-applies both themes at `density`.
pub fn set_density(ctx: &egui::Context, density: Density) {
    ctx.set_style_of(Theme::Dark, style_for(&Palette::DARK, density));
    ctx.set_style_of(Theme::Light, style_for(&Palette::LIGHT, density));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lay_out(ctx: &egui::Context) {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.label(format!("{} Save", crate::icons::action::SAVE));
            ui.heading("Properties");
        });
        output.textures_delta.clear();
    }

    #[test]
    fn apply_pins_dark_and_lays_out_text_and_icons() {
        let ctx = egui::Context::default();
        apply(&ctx);
        lay_out(&ctx);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        assert_eq!(ctx.global_style().visuals.panel_fill, BG_1);
        assert_eq!(
            ctx.global_style().text_styles[&TextStyle::Body].size,
            FONT_BODY
        );
    }

    #[test]
    fn the_light_theme_has_light_surfaces_and_the_dark_one_keeps_its_own() {
        let ctx = egui::Context::default();
        apply_with(&ctx, ThemeChoice::Light, Density::Standard);
        lay_out(&ctx);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        let v = ctx.global_style().visuals.clone();
        assert!(!v.dark_mode);
        assert_eq!(v.panel_fill, Palette::LIGHT.panel);
        assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, BG_1);
    }

    #[test]
    fn switching_the_theme_does_not_reset_the_density() {
        let ctx = egui::Context::default();
        apply_with(&ctx, ThemeChoice::Dark, Density::Touch);
        set_theme(&ctx, ThemeChoice::Light);
        lay_out(&ctx);
        let s = ctx.global_style();
        assert_eq!(
            s.spacing.interact_size.y,
            Density::Touch.metrics().control_height
        );
        assert_eq!(s.text_styles[&TextStyle::Body].size, 15.0);
    }

    #[test]
    fn set_density_changes_metrics_in_both_themes() {
        let ctx = egui::Context::default();
        apply(&ctx);
        set_density(&ctx, Density::Compact);
        for theme in [egui::Theme::Dark, egui::Theme::Light] {
            assert_eq!(ctx.style_of(theme).spacing.interact_size.y, 20.0);
        }
    }
}
