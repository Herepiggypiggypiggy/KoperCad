//! The canvas in the app's colours: `novo-theme`'s dark canvas palette
//! applied to `novo-canvas` and its model drawing.

use novo_canvas::cabinet::CabinetStyle;
use novo_canvas::model::ModelStyle;
use novo_canvas::widget::CanvasStyle;
use novo_canvas::{Color, Highlight, Stroke};
use novo_theme::{CanvasPalette, canvas_palette};
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the canvases draw with the dark palette. Set once per frame from
/// the resolved egui theme by [`set_dark`]; canvases built later pick it up.
static DARK: AtomicBool = AtomicBool::new(true);

pub fn is_dark() -> bool {
    DARK.load(Ordering::Relaxed)
}

/// Records the resolved theme; true when it changed, so the canvases that
/// exist must be restyled.
pub fn set_dark(dark: bool) -> bool {
    DARK.swap(dark, Ordering::Relaxed) != dark
}

/// The interface colour roles of the theme in use. Panels call this where
/// no `Ui` is at hand; it follows the same flag as the canvases.
pub fn ui_palette() -> &'static novo_theme::Palette {
    novo_theme::Palette::for_mode(is_dark())
}

fn palette() -> &'static CanvasPalette {
    canvas_palette(is_dark())
}

/// `a` over `b` at `amount` (0..=1), opaque.
fn mix(a: [u8; 4], b: [u8; 4], amount: f32) -> [u8; 4] {
    let ch = |i: usize| (f32::from(a[i]) * amount + f32::from(b[i]) * (1.0 - amount)).round() as u8;
    [ch(0), ch(1), ch(2), 255]
}

fn color(c: [u8; 4]) -> Color {
    Color(c)
}

pub fn canvas_style() -> CanvasStyle {
    canvas_style_from(palette())
}

/// The Cabinet canvas: the plate in the page-frame greys, devices in the wire
/// blue (copper stays the selection's), problems in the mark colours.
pub fn cabinet_style() -> CabinetStyle {
    let p = palette();
    CabinetStyle {
        plate_edge: color(p.text),
        usable_edge: color(p.sheet_frame),
        rail: color(p.symbol),
        device: color(p.wire),
        clearance: color(p.grid_major),
        error: color(p.error),
        warning: color(p.warning),
        ..CabinetStyle::default()
    }
}

pub fn model_style() -> ModelStyle {
    model_style_from(palette())
}

fn canvas_style_from(p: &CanvasPalette) -> CanvasStyle {
    let defaults = CanvasStyle::default();
    CanvasStyle {
        background: color(p.background),
        grid_minor: color(p.grid_minor),
        grid_major: color(p.grid_major),
        selection: Highlight {
            color: color(p.selection),
            ..defaults.selection
        },
        hover: Highlight {
            color: color(p.hover),
            ..defaults.hover
        },
        // What a selection is linked to (a coil's contacts, a net) reads
        // like a cross reference.
        related: Highlight {
            color: color(p.cross_reference),
            ..defaults.related
        },
        band_window: color(p.rubber_band),
        band_crossing: color(p.connection_point),
        snap_marker: color(p.snap_marker),
        // Marks (DESIGN.md §5), from the palette. None is copper: that hue
        // belongs to the selection.
        proposal: color(p.proposal),
        warning: color(p.warning),
        error: color(p.error),
        override_mark: color(p.override_mark),
        added: color(p.added),
        modified: color(p.modified),
        removed: color(p.removed),
        mark_text: color(p.mark_text),
    }
}

fn model_style_from(p: &CanvasPalette) -> ModelStyle {
    let defaults = ModelStyle::default();
    ModelStyle {
        symbol: Stroke {
            color: color(p.symbol),
            ..defaults.symbol
        },
        wire: Stroke {
            color: color(p.wire),
            ..defaults.wire
        },
        // Where wires meet and end (DESIGN.md §4): the junction dot is
        // copper, a wire landing on a pin uses the wire's own colour.
        joint_color: color(p.junction),
        // A cable's sheath and tag are the symbol colour at 60 percent over
        // the sheet, never copper (design/direction.md section 3).
        cable_color: color(mix(p.symbol, p.sheet, 0.6)),
        // The wire run under the pointer or being edited: the selection
        // copper from the palette, not a literal orange.
        focus_color: color(p.selection),
        pin_joint_color: color(p.wire),
        wire_end_color: color(p.wire_end),
        unconnected_color: color(p.unconnected_flag),
        tag_color: color(p.text),
        sheet_fill: Some(color(p.sheet)),
        frame_color: color(p.sheet_frame),
        sheet_text_color: color(p.sheet_text),
        note_color: color(p.cross_reference),
        note_warning_color: color(p.warning),
        ..defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_theme::{CANVAS_DARK, CANVAS_LIGHT};

    #[test]
    fn the_canvas_uses_the_dark_palette() {
        let style = canvas_style();
        assert_eq!(style.background, Color(CANVAS_DARK.background));
        assert_eq!(model_style().symbol.color, Color(CANVAS_DARK.symbol));
        assert_eq!(model_style().sheet_fill, Some(Color(CANVAS_DARK.sheet)));
    }

    #[test]
    fn the_wire_focus_is_the_palettes_selection_colour() {
        for p in [&CANVAS_DARK, &novo_theme::CANVAS_LIGHT] {
            assert_eq!(model_style_from(p).focus_color, Color(p.selection));
        }
    }

    #[test]
    fn wire_joints_and_open_ends_use_the_palette() {
        let m = model_style();
        assert_eq!(m.joint_color, Color(CANVAS_DARK.junction));
        assert_eq!(m.pin_joint_color, Color(CANVAS_DARK.wire));
        assert_eq!(m.wire_end_color, Color(CANVAS_DARK.wire_end));
        assert_eq!(m.unconnected_color, Color(CANVAS_DARK.unconnected_flag));
    }

    /// Hue in degrees of an RGB colour.
    fn hue(Color([r, g, b, _]): Color) -> f32 {
        let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        let d = max - min;
        if d == 0.0 {
            return 0.0;
        }
        let h = if max == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if max == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h * 60.0
    }

    #[test]
    fn marks_use_the_agreed_dark_colours() {
        let s = canvas_style();
        assert_eq!(s.proposal, Color::rgba(0x8F, 0xA8, 0xC8, 140));
        assert_eq!(s.override_mark, Color::rgb(0x98, 0x96, 0x91));
        assert_eq!(s.added, Color::rgba(0x7F, 0xB0, 0x86, 128));
        assert_eq!(s.modified, Color::rgba(0xD6, 0xA5, 0x5C, 128));
        assert_eq!(s.removed, Color::rgba(0xE0, 0x76, 0x6C, 128));
        assert_eq!(s.mark_text, Color::rgb(0xC4, 0xC2, 0xBD));
        assert_eq!(s.warning, Color(CANVAS_DARK.warning));
        assert_eq!(s.error, Color(CANVAS_DARK.error));
        // Every mark is set here, none left at the white-sheet defaults.
        let d = CanvasStyle::default();
        for (ours, default) in [
            (s.proposal, d.proposal),
            (s.warning, d.warning),
            (s.error, d.error),
            (s.override_mark, d.override_mark),
            (s.added, d.added),
            (s.modified, d.modified),
            (s.removed, d.removed),
            (s.mark_text, d.mark_text),
        ] {
            assert_ne!(ours, default);
        }
    }

    #[test]
    fn cables_are_neutral_not_copper() {
        let m = model_style_from(&CANVAS_DARK);
        assert_ne!(m.cable_color, m.joint_color);
        // Grey: the channels stay close together.
        let Color([r, _, b, a]) = m.cable_color;
        assert_eq!(a, 255);
        assert!(r.abs_diff(b) < 24, "{:?}", m.cable_color);
    }

    #[test]
    fn warning_marks_are_not_the_copper_selection() {
        let s = canvas_style();
        let copper = hue(s.selection.color);
        for mark in [s.warning, s.modified] {
            assert_ne!(mark, s.selection.color);
            assert!((hue(mark) - copper).abs() >= 8.0, "{mark:?}");
        }
    }

    #[test]
    fn the_light_palette_gives_a_light_canvas() {
        let dark = canvas_style_from(&CANVAS_DARK);
        let light = canvas_style_from(&CANVAS_LIGHT);
        assert_eq!(light.background, Color(CANVAS_LIGHT.background));
        assert_ne!(light.background, dark.background);
        assert_ne!(
            model_style_from(&CANVAS_LIGHT).symbol.color,
            model_style_from(&CANVAS_DARK).symbol.color
        );
    }
}

#[cfg(test)]
mod role_tests {
    /// Panels take colours from the palette of the theme in use. The raw
    /// dark tokens stay for sizes and fonts; a colour token in a screen
    /// would stay dark in the light theme.
    #[test]
    fn screens_use_palette_roles_not_dark_colour_tokens() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut hits = Vec::new();
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let s = std::fs::read_to_string(&p).unwrap();
                    // Whole file, except an inline `#[cfg(test)] mod x { .. }`
                    // (an early `#[cfg(test)]` on one item must not hide the rest).
                    let mut code = s.as_str();
                    let lines: Vec<&str> = s.lines().collect();
                    for w in 0..lines.len().saturating_sub(1) {
                        if lines[w].trim() == "#[cfg(test)]"
                            && lines[w + 1].trim_start().starts_with("mod ")
                            && lines[w + 1].trim_end().ends_with('{')
                        {
                            let at: usize = lines[..w].iter().map(|l| l.len() + 1).sum();
                            code = &s[..at.min(s.len())];
                            break;
                        }
                    }
                    for (i, l) in code.lines().enumerate() {
                        let t = l.trim_start();
                        if t.starts_with("//") {
                            continue;
                        }
                        for c in [
                            "tokens::WARNING",
                            "tokens::ERROR",
                            "tokens::SUCCESS",
                            "tokens::INFO",
                            "tokens::TEXT_MUTED",
                            "tokens::TEXT_FAINT",
                            "tokens::ACCENT",
                            "tokens::TEXT",
                            "tokens::BORDER",
                            "tokens::BG_",
                        ] {
                            if l.contains(c) {
                                hits.push(format!("{}:{}", p.display(), i + 1));
                            }
                        }
                    }
                }
            }
        }
        assert!(hits.is_empty(), "use ui_palette() instead: {hits:#?}");
    }
}
