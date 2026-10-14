//! Design tokens: the named colours, sizes and radii every screen uses.
//!
//! Screens refer to these names, never to raw colour values, so the look can
//! change in one place. Surfaces are numbered from the deepest (`BG_0`, behind
//! everything) to the highest (`BG_4`, a hovered control).
//!
//! The palette is graphite: near-neutral greys with a faint warm cast, and a
//! single copper accent used sparingly. Hierarchy comes from value and
//! spacing, not colour.

use egui::Color32;

// ---------------------------------------------------------------------------
// Surfaces

/// Deepest surface: text fields, lists, the area around the drawing sheet.
pub const BG_0: Color32 = Color32::from_rgb(0x0E, 0x0E, 0x10);
/// Panels: the top bar, the outline and the drawer.
pub const BG_1: Color32 = Color32::from_rgb(0x15, 0x15, 0x17);
/// Floating surfaces: the inspector, the dock, the command field and
/// tooltips.
pub const BG_2: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x1E);
/// Resting controls: buttons, checkboxes, sliders.
pub const BG_3: Color32 = Color32::from_rgb(0x23, 0x23, 0x27);
/// Hovered controls and selected list rows.
pub const BG_4: Color32 = Color32::from_rgb(0x2D, 0x2D, 0x32);

/// Hairlines between panels and around windows.
pub const BORDER: Color32 = Color32::from_rgb(0x25, 0x25, 0x29);
/// Borders that must stand out: hovered and focused controls.
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x3B, 0x3B, 0x41);

// ---------------------------------------------------------------------------
// Text

/// Body text and labels.
pub const TEXT: Color32 = Color32::from_rgb(0xE3, 0xE1, 0xDC);
/// Secondary text: hints, units, column headers.
pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x98, 0x96, 0x91);
/// Placeholders, coordinates, margin labels and disabled text. Still 4.5:1
/// on the panel and canvas surfaces, because people read it.
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x85, 0x83, 0x80);
/// Text drawn on an `ACCENT` fill.
pub const TEXT_ON_ACCENT: Color32 = Color32::from_rgb(0x14, 0x10, 0x0D);

// ---------------------------------------------------------------------------
// Accent

/// Copper, the one accent: focus, the active tool, the primary button, links
/// and the logo. Rare by design, so it always means "here".
pub const ACCENT: Color32 = Color32::from_rgb(0xC8, 0x8A, 0x5E);
/// `ACCENT` under the pointer.
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(0xDB, 0xA2, 0x7C);
/// Faint copper wash behind selected text and selected list rows.
pub const ACCENT_SELECTION: Color32 = Color32::from_rgb(0x3A, 0x2E, 0x26);

/// The brand colour is the accent.
pub const BRAND: Color32 = ACCENT;

// ---------------------------------------------------------------------------
// Status (muted, so they inform without shouting)

pub const SUCCESS: Color32 = Color32::from_rgb(0x7F, 0xB0, 0x86);
pub const WARNING: Color32 = Color32::from_rgb(0xD6, 0xA5, 0x5C);
pub const ERROR: Color32 = Color32::from_rgb(0xE0, 0x76, 0x6C);
pub const INFO: Color32 = Color32::from_rgb(0x8F, 0xA8, 0xC8);

// ---------------------------------------------------------------------------
// Sizes (points)

/// Body and button text.
pub const FONT_BODY: f32 = 13.0;
/// Hints, captions, section labels, the HUD.
pub const FONT_SMALL: f32 = 11.0;
/// The inspector's title and document headings. Panels use small section
/// labels instead of headings.
pub const FONT_HEADING: f32 = 15.0;
/// Device tags, part numbers, code.
pub const FONT_MONO: f32 = 12.0;

/// Height of buttons, fields and list rows.
pub const CONTROL_HEIGHT: f32 = 22.0;
/// Corner radius of buttons and fields.
pub const RADIUS_CONTROL: u8 = 3;
/// Corner radius of windows, menus and popups.
pub const RADIUS_WINDOW: u8 = 6;

#[cfg(test)]
pub(crate) mod tests {
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

    pub(crate) fn contrast(a: Color32, b: Color32) -> f32 {
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
            assert!(contrast(TEXT_FAINT, bg) >= 4.5, "{bg:?}");
            for c in [ACCENT, SUCCESS, WARNING, ERROR, INFO] {
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
