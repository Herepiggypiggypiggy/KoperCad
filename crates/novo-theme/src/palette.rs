//! Colour roles for the dark and the light theme.
//!
//! A role keeps its meaning in both themes and changes its value. The
//! numbered surfaces in [`crate::tokens`] (`BG_0`..`BG_4`) are the dark
//! values of the first five roles, so screens written against them keep
//! working while they move to the palette (docs/design/design-system.md §2.1).
//!
//! Screens get the palette of the theme they are drawn in with
//! [`Palette::of`]: `let p = Palette::of(ui);` then `p.text_primary`.

use crate::tokens;
use egui::Color32;

/// Every colour role the interface uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// `true` for the dark theme.
    pub dark: bool,
    /// Text fields, lists, tables and the area around the drawing sheet.
    pub sunken: Color32,
    /// Docked panels: top bar, outline, drawer.
    pub panel: Color32,
    /// Floating surfaces: dock, inspector, menus, tooltips, dialogs.
    pub raised: Color32,
    /// Buttons and fields at rest.
    pub control: Color32,
    /// Hovered controls and pressed rows.
    pub hover: Color32,
    /// Hairlines between regions.
    pub border_subtle: Color32,
    /// Hovered and focused outlines.
    pub border_strong: Color32,
    /// Outlines of controls at rest (checks, radios, buttons, fields): 3:1 on
    /// the panel, field and floating surfaces.
    pub border_control: Color32,
    /// Body text and values.
    pub text_primary: Color32,
    /// Text on hovered and pressed controls, one step above primary.
    pub text_strong: Color32,
    /// Labels, hints, units, section labels.
    pub text_secondary: Color32,
    /// Placeholders, coordinates, disabled text.
    pub text_tertiary: Color32,
    /// Copper: focus, active tool, selection, primary action, logo.
    pub accent: Color32,
    /// Accent text and hairlines under the pointer.
    pub accent_hover: Color32,
    /// Selected rows and selected text.
    pub accent_wash: Color32,
    /// Text on an accent fill.
    pub on_accent: Color32,
    /// Links, cross references, hover on the canvas.
    pub info: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    /// The status colours at 12% on the panel, for banners and row tints.
    pub info_wash: Color32,
    pub success_wash: Color32,
    pub warning_wash: Color32,
    pub error_wash: Color32,
    /// Alpha of the black under floating surfaces. Black shows more on a
    /// light surface, so the light theme uses less of it; its surfaces are
    /// close in value, so the shadow still does the lifting.
    pub shadow_alpha: u8,
    /// The 3D view's room and overlays (docs/design/3d-look.md).
    pub room: Room3d,
}

/// The roles of the 3D cabinet view that follow the theme: the room around
/// the cabinet and the overlays on it. Physical materials (plate, rail, duct,
/// wire colours) are the same in both themes and live in `novo-view3d`.
/// Problems are the status colours, drawn as glyphs and never colour alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Room3d {
    /// Background gradient, top.
    pub background_top: Color32,
    /// Background gradient, bottom.
    pub background_bottom: Color32,
    /// Ground grid lines.
    pub grid: Color32,
    /// Label chip fill, drawn at `label_alpha` (85%), outline and text.
    pub label_fill: Color32,
    pub label_alpha: u8,
    pub label_border: Color32,
    pub label_text: Color32,
    /// Selected item outline: the accent.
    pub selection: Color32,
    /// Hovered item outline: the info colour.
    pub hover: Color32,
    /// How much darker an edge line is than its face, in percent.
    pub edge_percent: u8,
}

/// One channel of `fg` over `bg`, `pct` percent of `fg`.
const fn channel(fg: u8, bg: u8, pct: u32) -> u8 {
    ((fg as u32 * pct + bg as u32 * (100 - pct)) / 100) as u8
}

/// `fg` over `bg`, `pct` percent of `fg`.
const fn mix(fg: Color32, bg: Color32, pct: u32) -> Color32 {
    Color32::from_rgb(
        channel(fg.r(), bg.r(), pct),
        channel(fg.g(), bg.g(), pct),
        channel(fg.b(), bg.b(), pct),
    )
}

impl Palette {
    /// The graphite theme: the token constants.
    pub const DARK: Palette = Palette {
        dark: true,
        sunken: tokens::BG_0,
        panel: tokens::BG_1,
        raised: tokens::BG_2,
        control: tokens::BG_3,
        hover: tokens::BG_4,
        border_subtle: tokens::BORDER,
        border_strong: tokens::BORDER_STRONG,
        border_control: Color32::from_rgb(0x6A, 0x6A, 0x72),
        text_primary: tokens::TEXT,
        text_strong: Color32::from_rgb(0xF4, 0xF2, 0xEE),
        text_secondary: tokens::TEXT_MUTED,
        text_tertiary: tokens::TEXT_FAINT,
        accent: tokens::ACCENT,
        accent_hover: tokens::ACCENT_HOVER,
        accent_wash: tokens::ACCENT_SELECTION,
        on_accent: tokens::TEXT_ON_ACCENT,
        info: tokens::INFO,
        success: tokens::SUCCESS,
        warning: tokens::WARNING,
        error: tokens::ERROR,
        info_wash: mix(tokens::INFO, tokens::BG_1, 12),
        success_wash: mix(tokens::SUCCESS, tokens::BG_1, 12),
        warning_wash: mix(tokens::WARNING, tokens::BG_1, 12),
        error_wash: mix(tokens::ERROR, tokens::BG_1, 12),
        shadow_alpha: 100,
        room: Room3d {
            background_top: tokens::BG_0,
            background_bottom: tokens::BG_1,
            grid: tokens::BORDER,
            label_fill: tokens::BG_2,
            label_alpha: 217,
            label_border: tokens::BORDER_STRONG,
            label_text: tokens::TEXT,
            selection: tokens::ACCENT,
            hover: tokens::INFO,
            edge_percent: 45,
        },
    };

    /// The warm paper theme. Panels sit a touch below the floating
    /// surfaces (lifted white with a shadow); the accent is the same copper
    /// made darker so it stays 4.5:1 on every surface.
    pub const LIGHT: Palette = {
        const PANEL: Color32 = Color32::from_rgb(0xF2, 0xF1, 0xEE);
        const INFO: Color32 = Color32::from_rgb(0x28, 0x54, 0x88);
        const SUCCESS: Color32 = Color32::from_rgb(0x27, 0x6B, 0x36);
        const WARNING: Color32 = Color32::from_rgb(0x7F, 0x54, 0x08);
        const ERROR: Color32 = Color32::from_rgb(0xA8, 0x2F, 0x25);
        Palette {
            dark: false,
            sunken: Color32::from_rgb(0xFC, 0xFB, 0xFA),
            panel: PANEL,
            raised: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            control: Color32::from_rgb(0xE9, 0xE7, 0xE3),
            hover: Color32::from_rgb(0xDF, 0xDC, 0xD7),
            border_subtle: Color32::from_rgb(0xDA, 0xD8, 0xD3),
            border_strong: Color32::from_rgb(0xB5, 0xB1, 0xAA),
            border_control: Color32::from_rgb(0x8C, 0x88, 0x80),
            text_primary: Color32::from_rgb(0x1E, 0x1D, 0x1B),
            text_strong: Color32::from_rgb(0x0C, 0x0B, 0x0A),
            text_secondary: Color32::from_rgb(0x5A, 0x58, 0x53),
            text_tertiary: Color32::from_rgb(0x66, 0x64, 0x5F),
            accent: Color32::from_rgb(0x9A, 0x54, 0x27),
            accent_hover: Color32::from_rgb(0x82, 0x44, 0x1D),
            accent_wash: Color32::from_rgb(0xF4, 0xE4, 0xD7),
            on_accent: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            info: INFO,
            success: SUCCESS,
            warning: WARNING,
            error: ERROR,
            info_wash: mix(INFO, PANEL, 12),
            success_wash: mix(SUCCESS, PANEL, 12),
            warning_wash: mix(WARNING, PANEL, 12),
            error_wash: mix(ERROR, PANEL, 12),
            shadow_alpha: 46,
            room: Room3d {
                background_top: Color32::from_rgb(0xEC, 0xEA, 0xE6),
                background_bottom: Color32::from_rgb(0xDE, 0xDC, 0xD7),
                grid: Color32::from_rgb(0xC9, 0xC6, 0xC0),
                label_fill: Color32::from_rgb(0xFF, 0xFF, 0xFF),
                label_alpha: 217,
                label_border: Color32::from_rgb(0xB5, 0xB1, 0xAA),
                label_text: Color32::from_rgb(0x1E, 0x1D, 0x1B),
                selection: Color32::from_rgb(0x9A, 0x54, 0x27),
                hover: INFO,
                edge_percent: 55,
            },
        }
    };

    /// The palette for dark or light.
    pub const fn for_mode(dark: bool) -> &'static Palette {
        if dark { &Self::DARK } else { &Self::LIGHT }
    }

    /// The palette of the theme `ui` is drawn in.
    pub fn of(ui: &egui::Ui) -> &'static Palette {
        Self::for_mode(ui.visuals().dark_mode)
    }

    /// The palette of the theme `ctx` currently uses.
    pub fn of_ctx(ctx: &egui::Context) -> &'static Palette {
        Self::for_mode(ctx.global_style().visuals.dark_mode)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::tests::contrast;

    const BOTH: [&Palette; 2] = [&Palette::DARK, &Palette::LIGHT];

    fn surfaces(p: &Palette) -> [Color32; 5] {
        [p.sunken, p.panel, p.raised, p.control, p.hover]
    }

    #[test]
    fn the_dark_palette_is_the_token_constants() {
        let p = Palette::DARK;
        assert_eq!(p.sunken, tokens::BG_0);
        assert_eq!(p.hover, tokens::BG_4);
        assert_eq!(p.text_primary, tokens::TEXT);
        assert_eq!(p.accent, tokens::ACCENT);
    }

    #[test]
    fn primary_text_is_7_to_1_and_the_rest_4_5_to_1_on_every_surface() {
        for p in BOTH {
            for bg in surfaces(p) {
                assert!(contrast(p.text_primary, bg) >= 7.0, "{p:?} {bg:?}");
                assert!(contrast(p.text_secondary, bg) >= 4.5, "{bg:?}");
            }
            // Tertiary text sits on panels, fields and floating surfaces.
            for bg in [p.sunken, p.panel, p.raised] {
                assert!(contrast(p.text_tertiary, bg) >= 4.5, "{bg:?}");
            }
        }
    }

    #[test]
    fn accent_and_status_colours_are_readable_on_panels_fields_and_floating_surfaces() {
        for p in BOTH {
            for bg in [p.sunken, p.panel, p.raised] {
                for c in [p.accent, p.success, p.warning, p.error, p.info] {
                    assert!(contrast(c, bg) >= 4.5, "{c:?} on {bg:?}");
                }
            }
            assert!(contrast(p.on_accent, p.accent) >= 4.5);
            // The rings of checks and radios and the outline of a field.
            for bg in [p.sunken, p.panel, p.raised] {
                assert!(contrast(p.border_control, bg) >= 3.0, "{bg:?}");
            }
            // Selected rows and toggles: accent icon and hover text on the wash.
            assert!(contrast(p.accent, p.accent_wash) >= 4.5);
            assert!(contrast(p.accent_hover, p.accent_wash) >= 4.5);
        }
    }

    #[test]
    fn status_text_stays_readable_on_its_own_wash() {
        for p in BOTH {
            for (fg, wash) in [
                (p.success, p.success_wash),
                (p.warning, p.warning_wash),
                (p.error, p.error_wash),
                (p.info, p.info_wash),
            ] {
                assert!(contrast(fg, wash) >= 4.5, "{fg:?} on {wash:?}");
                assert!(contrast(p.text_primary, wash) >= 7.0, "{wash:?}");
            }
        }
    }

    #[test]
    fn controls_step_away_from_the_panel_in_both_themes() {
        for p in BOTH {
            let d = |c: Color32| contrast(c, p.panel);
            assert!(d(p.control) > 1.0, "{p:?}");
            assert!(d(p.hover) > d(p.control), "{p:?}");
            assert!(d(p.border_strong) > d(p.border_subtle), "{p:?}");
            assert!(contrast(p.accent_wash, p.panel) > 1.0);
        }
    }

    #[test]
    fn floating_surfaces_are_lifted_from_the_panel() {
        // Dark lifts by being lighter; light lifts by being whiter.
        assert!(contrast(Palette::DARK.raised, Palette::DARK.sunken) > 1.0);
        assert!(contrast(Palette::LIGHT.raised, Palette::LIGHT.panel) > 1.0);
        const { assert!(Palette::LIGHT.shadow_alpha < Palette::DARK.shadow_alpha) };
    }

    #[test]
    fn washes_are_a_faint_tint_of_the_status_colour() {
        for p in BOTH {
            assert_ne!(p.error_wash, p.panel);
            // Fully mixed is the status colour, not mixed is the surface.
            assert_eq!(mix(p.error, p.panel, 100), p.error);
            assert_eq!(mix(p.error, p.panel, 0), p.panel);
        }
    }

    #[test]
    fn the_3d_room_keeps_labels_and_outlines_readable() {
        for p in BOTH {
            let r = p.room;
            // A label chip over either end of the background gradient.
            for room in [r.background_top, r.background_bottom] {
                let a = r.label_alpha as u32;
                let blend = |f: u8, b: u8| ((f as u32 * a + b as u32 * (255 - a)) / 255) as u8;
                let chip = Color32::from_rgb(
                    blend(r.label_fill.r(), room.r()),
                    blend(r.label_fill.g(), room.g()),
                    blend(r.label_fill.b(), room.b()),
                );
                assert!(contrast(r.label_text, chip) >= 7.0, "{p:?} on {room:?}");
                assert!(contrast(r.label_border, chip) >= 1.5, "{p:?}");
            }
            for outline in [r.selection, r.hover] {
                assert!(contrast(outline, r.background_top) >= 3.0, "{p:?}");
                assert!(contrast(outline, r.background_bottom) >= 3.0, "{p:?}");
            }
            assert_ne!(r.selection, r.hover);
            assert!(r.grid != r.background_bottom);
        }
    }

    #[test]
    fn of_follows_the_themes_dark_flag() {
        let ctx = egui::Context::default();
        let mut seen = Vec::new();
        for dark in [true, false] {
            ctx.set_visuals(if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            });
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                seen.push(Palette::of(ui).dark);
            });
            out.textures_delta.clear();
        }
        assert_eq!(seen, [true, false]);
    }
}
