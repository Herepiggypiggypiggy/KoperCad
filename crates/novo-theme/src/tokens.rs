//! Design tokens: the named colours, sizes and radii every screen uses.
//!
//! Screens refer to these names, never to raw colour values, so the look can
//! change in one place. Surfaces are numbered from the deepest (`BG_0`, behind
//! everything) to the highest (`BG_4`, a hovered control).

use egui::Color32;

// ---------------------------------------------------------------------------
// Surfaces

/// Deepest surface: text fields, lists, the area around the drawing sheet.
pub const BG_0: Color32 = Color32::from_rgb(0x10, 0x12, 0x16);
/// Panels and the menu bar.
pub const BG_1: Color32 = Color32::from_rgb(0x17, 0x1A, 0x1F);
/// Windows, dialogs, menus and tooltips.
pub const BG_2: Color32 = Color32::from_rgb(0x1E, 0x22, 0x29);
/// Resting controls: buttons, checkboxes, sliders.
pub const BG_3: Color32 = Color32::from_rgb(0x29, 0x2E, 0x37);
/// Hovered controls.
pub const BG_4: Color32 = Color32::from_rgb(0x34, 0x3A, 0x45);

/// Hairlines between panels and around windows.
pub const BORDER: Color32 = Color32::from_rgb(0x2C, 0x31, 0x3A);
/// Borders that must stand out: focused fields, hovered controls.
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x46, 0x4D, 0x5A);

// ---------------------------------------------------------------------------
// Text

/// Body text and labels.
pub const TEXT: Color32 = Color32::from_rgb(0xE4, 0xE7, 0xEC);
/// Secondary text: hints, units, column headers.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x9C, 0xA4, 0xB0);
/// Disabled text and placeholders.
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6E, 0x77, 0x84);
/// Text drawn on an `ACCENT` fill.
pub const TEXT_ON_ACCENT: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x10);

// ---------------------------------------------------------------------------
// Accent and brand

/// Interaction colour: focus rings, selected rows, checked boxes, links.
pub const ACCENT: Color32 = Color32::from_rgb(0x4C, 0x9E, 0xFF);
/// `ACCENT` under the pointer.
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0x72, 0xB3, 0xFF);
/// Translucent `ACCENT` behind selected text and selected list rows.
pub const ACCENT_SELECTION: Color32 = Color32::from_rgba_unmultiplied_const(0x4C, 0x9E, 0xFF, 0x66);

/// NovoElectric amber, used by the logo and for canvas selection. Not used for
/// ordinary controls, so it keeps its meaning.
pub const BRAND: Color32 = Color32::from_rgb(0xFF, 0xB8, 0x1C);

// ---------------------------------------------------------------------------
// Status

pub const SUCCESS: Color32 = Color32::from_rgb(0x3F, 0xB9, 0x50);
pub const WARNING: Color32 = Color32::from_rgb(0xF0, 0x9A, 0x3E);
pub const ERROR: Color32 = Color32::from_rgb(0xF8, 0x51, 0x49);
pub const INFO: Color32 = ACCENT;

// ---------------------------------------------------------------------------
// Sizes (points)

/// Body and button text.
pub const FONT_BODY: f32 = 13.5;
/// Status bar, captions, table footnotes.
pub const FONT_SMALL: f32 = 11.5;
/// Panel and dialog titles.
pub const FONT_HEADING: f32 = 17.0;
/// Device tags, part numbers, code.
pub const FONT_MONO: f32 = 12.5;

/// Height of buttons, fields and list rows.
pub const CONTROL_HEIGHT: f32 = 24.0;
/// Corner radius of buttons and fields.
pub const RADIUS_CONTROL: u8 = 4;
/// Corner radius of windows, menus and popups.
pub const RADIUS_WINDOW: u8 = 8;

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance of an opaque colour.
    fn luminance(c: Color32) -> f32 {
        let lin = |v: u8| {
            let v = v as f32 / 255.0;
            if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f32 {
        let (a, b) = (luminance(a), luminance(b));
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    #[test]
    fn body_text_meets_wcag_aa_on_every_surface() {
        for bg in [BG_0, BG_1, BG_2, BG_3, BG_4] {
            assert!(contrast(TEXT, bg) >= 7.0, "{bg:?}");
            assert!(contrast(TEXT_MUTED, bg) >= 4.5, "{bg:?}");
        }
    }

    #[test]
    fn faint_text_and_status_colours_stay_readable() {
        for bg in [BG_0, BG_1, BG_2] {
            assert!(contrast(TEXT_FAINT, bg) >= 3.0, "{bg:?}");
            for c in [ACCENT, SUCCESS, WARNING, ERROR, BRAND] {
                assert!(contrast(c, bg) >= 4.5, "{c:?} on {bg:?}");
            }
        }
        assert!(contrast(TEXT_ON_ACCENT, ACCENT) >= 4.5);
    }

    #[test]
    fn surfaces_step_up_in_lightness() {
        let steps = [BG_0, BG_1, BG_2, BG_3, BG_4].map(luminance);
        assert!(steps.windows(2).all(|w| w[0] < w[1]));
    }
}
