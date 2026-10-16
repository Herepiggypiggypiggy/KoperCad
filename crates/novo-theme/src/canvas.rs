//! Proposed dark palette for the drawing canvas.
//!
//! The canvas (`novo-canvas`, WS-03) owns how drawings are rendered; these are
//! the colours this theme proposes for it, as straight sRGB `[r, g, b, a]` so
//! they map one to one onto the canvas colour type without depending on egui.
//! Printing and PDF export keep black on white: this palette is for the
//! screen only.

/// Colours for drawing a schematic page on a dark screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanvasPalette {
    /// Behind the sheet.
    pub background: [u8; 4],
    /// The sheet itself, a shade lighter than the background.
    pub sheet: [u8; 4],
    /// Sheet border and title block lines.
    pub sheet_frame: [u8; 4],
    /// Minor grid points or lines.
    pub grid_minor: [u8; 4],
    /// Major grid points or lines, every fifth step.
    pub grid_major: [u8; 4],
    /// Symbol outlines.
    pub symbol: [u8; 4],
    /// Wires and connections: a mid, saturated blue, clearly darker than the
    /// near-white symbols so a wire never merges into a terminal outline.
    pub wire: [u8; 4],
    /// Device tags, terminal numbers and page text.
    pub text: [u8; 4],
    /// A cable: its sheath over the core wires and its tag.
    pub cable: [u8; 4],
    /// Cross reference text, which reads as a link.
    pub cross_reference: [u8; 4],
    /// Unconnected connection points.
    pub connection_point: [u8; 4],
    /// Selected items.
    pub selection: [u8; 4],
    /// The item under the pointer.
    pub hover: [u8; 4],
    /// Rubber band selection, opaque: the canvas applies its own fill alpha.
    pub rubber_band: [u8; 4],
    /// Design check markers.
    pub error: [u8; 4],
    pub warning: [u8; 4],
    /// Proposed terminals, cables and parts (DESIGN.md §5): steel blue at
    /// about 55%, drawn dashed. Hover is the same hue but solid and opaque,
    /// so the two differ in line and strength, not hue alone.
    pub proposal: [u8; 4],
    /// Hollow diamond on a value that differs from its block or rule.
    pub override_mark: [u8; 4],
    /// Changes since the last revision, at half strength so the drawing
    /// under them stays readable. Removed items are drawn dashed.
    pub added: [u8; 4],
    pub modified: [u8; 4],
    pub removed: [u8; 4],
    /// Text on marks, such as the proposal's accept hint.
    pub mark_text: [u8; 4],
    /// The "+" that shows the snapped cursor position. It is always on
    /// screen, so it stays a quiet grey rather than copper, which belongs
    /// to the selection.
    pub snap_marker: [u8; 4],
    /// Filled dot at a true junction (three or more conductors, DESIGN §2),
    /// the same copper dot as the logo. Darker than the selection, which
    /// also gets a halo and a thicker line, so a selected net is told apart.
    pub junction: [u8; 4],
    /// Open ring on a wire end that was terminated unconnected. The case
    /// and its Warning are defined in DESIGN §4.
    pub wire_end: [u8; 4],
    /// The small flag beside an unconnected wire end.
    pub unconnected_flag: [u8; 4],
}

/// Colours for the live potentials view (Shift+L): a wire is lit in the
/// colour of the potential it carries. Screen only, like the canvas palette.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PotentialPalette {
    pub protective_earth: [u8; 4],
    pub pen: [u8; 4],
    pub neutral: [u8; 4],
    pub dc_positive: [u8; 4],
    pub dc_negative: [u8; 4],
    /// A wire that carries no potential: a neutral grey, readable on the
    /// sheet but quieter than any lit wire, and not the selection copper.
    pub dead: [u8; 4],
    /// Line conductors and signals, by the last digit of their name, so
    /// L1, L2 and L3 stay apart.
    pub line: [[u8; 4]; 4],
}

/// The dark potential palette.
pub const POTENTIALS_DARK: PotentialPalette = PotentialPalette {
    protective_earth: [0x7F, 0xD0, 0x8A, 0xFF],
    pen: [0xB8, 0xD0, 0x6A, 0xFF],
    neutral: [0x5C, 0x8D, 0xE0, 0xFF],
    dc_positive: [0xE0, 0x76, 0x6C, 0xFF],
    dc_negative: [0x6C, 0x7C, 0xE0, 0xFF],
    dead: [0x78, 0x76, 0x72, 0xFF],
    line: [
        [0xE0, 0xA2, 0x4A, 0xFF],
        [0xC0, 0x70, 0xD0, 0xFF],
        [0x4F, 0xC1, 0xD0, 0xFF],
        [0xD0, 0xD0, 0x5C, 0xFF],
    ],
};

/// Converts a palette colour for drawing with egui.
pub fn to_color32([r, g, b, a]: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The dark canvas palette.
pub const CANVAS_DARK: CanvasPalette = CanvasPalette {
    background: [0x0E, 0x0E, 0x10, 0xFF],
    sheet: [0x13, 0x13, 0x15, 0xFF],
    sheet_frame: [0x46, 0x45, 0x48, 0xFF],
    grid_minor: [0x2E, 0x2E, 0x33, 0xFF],
    grid_major: [0x40, 0x40, 0x45, 0xFF],
    symbol: [0xEC, 0xEA, 0xE5, 0xFF],
    wire: [0x6F, 0x9B, 0xDB, 0xFF],
    text: [0xC4, 0xC2, 0xBD, 0xFF],
    cable: [0xA9, 0xA6, 0xC8, 0xFF],
    cross_reference: [0x8F, 0xA8, 0xC8, 0xFF],
    connection_point: [0x7F, 0xB0, 0x86, 0xFF],
    selection: [0xE0, 0xA0, 0x70, 0xFF],
    hover: [0x8F, 0xA8, 0xC8, 0xFF],
    rubber_band: [0x8F, 0xA8, 0xC8, 0xFF],
    error: [0xE0, 0x76, 0x6C, 0xFF],
    warning: [0xD6, 0xA5, 0x5C, 0xFF],
    proposal: [0x8F, 0xA8, 0xC8, 0x8C],
    override_mark: [0x98, 0x96, 0x91, 0xFF],
    added: [0x7F, 0xB0, 0x86, 0x80],
    modified: [0xD6, 0xA5, 0x5C, 0x80],
    removed: [0xE0, 0x76, 0x6C, 0x80],
    mark_text: [0xC4, 0xC2, 0xBD, 0xFF],
    snap_marker: [0x85, 0x83, 0x80, 0xFF],
    junction: [0xC8, 0x8A, 0x5E, 0xFF],
    wire_end: [0xC8, 0x8A, 0x5E, 0xFF],
    unconnected_flag: [0xD6, 0xA5, 0x5C, 0xFF],
};

/// The light canvas palette: black ink on a white sheet that matches print,
/// on a warm grey desk.
pub const CANVAS_LIGHT: CanvasPalette = CanvasPalette {
    background: [0xD9, 0xD7, 0xD2, 0xFF],
    sheet: [0xFF, 0xFF, 0xFF, 0xFF],
    sheet_frame: [0x6F, 0x6C, 0x66, 0xFF],
    grid_minor: [0xD2, 0xCF, 0xC9, 0xFF],
    grid_major: [0xB4, 0xB0, 0xA8, 0xFF],
    symbol: [0x1E, 0x1D, 0x1B, 0xFF],
    wire: [0x25, 0x63, 0xC4, 0xFF],
    text: [0x3A, 0x38, 0x35, 0xFF],
    cable: [0x4F, 0x4A, 0x86, 0xFF],
    cross_reference: [0x28, 0x54, 0x88, 0xFF],
    connection_point: [0x27, 0x6B, 0x36, 0xFF],
    selection: [0xB3, 0x5A, 0x18, 0xFF],
    hover: [0x28, 0x54, 0x88, 0xFF],
    rubber_band: [0x28, 0x54, 0x88, 0xFF],
    error: [0xA8, 0x2F, 0x25, 0xFF],
    warning: [0x7F, 0x54, 0x08, 0xFF],
    proposal: [0x28, 0x54, 0x88, 0x8C],
    override_mark: [0x66, 0x64, 0x5F, 0xFF],
    added: [0x27, 0x6B, 0x36, 0x80],
    modified: [0x7F, 0x54, 0x08, 0x80],
    removed: [0xA8, 0x2F, 0x25, 0x80],
    mark_text: [0x3A, 0x38, 0x35, 0xFF],
    snap_marker: [0x8A, 0x87, 0x82, 0xFF],
    junction: [0x9A, 0x54, 0x27, 0xFF],
    wire_end: [0x9A, 0x54, 0x27, 0xFF],
    unconnected_flag: [0x7F, 0x54, 0x08, 0xFF],
};

/// The light potential palette.
pub const POTENTIALS_LIGHT: PotentialPalette = PotentialPalette {
    protective_earth: [0x2F, 0x7A, 0x3F, 0xFF],
    pen: [0x5F, 0x74, 0x10, 0xFF],
    neutral: [0x2E, 0x5F, 0xB5, 0xFF],
    dc_positive: [0xB3, 0x36, 0x2B, 0xFF],
    dc_negative: [0x4A, 0x4F, 0xB0, 0xFF],
    dead: [0x85, 0x83, 0x7E, 0xFF],
    line: [
        [0xA8, 0x5F, 0x00, 0xFF],
        [0x8E, 0x3F, 0xA0, 0xFF],
        [0x0F, 0x7B, 0x8A, 0xFF],
        [0x6B, 0x6B, 0x10, 0xFF],
    ],
};

/// The canvas palette for dark or light.
pub const fn canvas_palette(dark: bool) -> &'static CanvasPalette {
    if dark { &CANVAS_DARK } else { &CANVAS_LIGHT }
}

/// The potential palette for dark or light.
pub const fn potential_palette(dark: bool) -> &'static PotentialPalette {
    if dark {
        &POTENTIALS_DARK
    } else {
        &POTENTIALS_LIGHT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_colours_stand_out_from_the_sheet() {
        for p in [CANVAS_DARK, CANVAS_LIGHT] {
            let grey = |c: [u8; 4]| (c[0] as u32 + c[1] as u32 + c[2] as u32) / 3;
            // How far a colour sits from the sheet, light or dark.
            let away = |c: [u8; 4]| grey(c).abs_diff(grey(p.sheet));
            for c in [p.symbol, p.wire, p.text] {
                assert!(away(c) > 120, "{c:?}");
            }
            // The grid stays quieter than anything drawn on it.
            assert!(away(p.grid_major) < away(p.sheet_frame));
            assert!(away(p.grid_minor) < away(p.grid_major));
        }
    }

    #[test]
    fn drawing_colours_meet_wcag_aa_on_the_sheet() {
        for p in [CANVAS_DARK, CANVAS_LIGHT] {
            let sheet = to_color32(p.sheet);
            for c in [
                p.symbol,
                p.wire,
                p.text,
                p.cable,
                p.cross_reference,
                p.connection_point,
                p.selection,
                p.error,
                p.warning,
                p.junction,
                p.wire_end,
                p.unconnected_flag,
            ] {
                let ratio = crate::tokens::tests::contrast(to_color32(c), sheet);
                assert!(ratio >= 4.5, "{c:?}: {ratio}");
            }
        }
    }

    #[test]
    fn grid_dots_show_on_the_sheet_without_competing_with_the_drawing() {
        for p in [CANVAS_DARK, CANVAS_LIGHT] {
            let sheet = to_color32(p.sheet);
            let minor = crate::tokens::tests::contrast(to_color32(p.grid_minor), sheet);
            let major = crate::tokens::tests::contrast(to_color32(p.grid_major), sheet);
            assert!(minor >= 1.3, "minor grid dots: {minor}");
            assert!(major >= minor + 0.3, "major grid dots: {major}");
            let frame = crate::tokens::tests::contrast(to_color32(p.sheet_frame), sheet);
            assert!(major < frame, "grid stays quieter than the page frame");
        }
    }

    #[test]
    fn wires_are_told_apart_from_symbols_and_junctions_are_not_the_selection() {
        for p in [CANVAS_DARK, CANVAS_LIGHT] {
            let ratio = crate::tokens::tests::contrast(to_color32(p.symbol), to_color32(p.wire));
            assert!(ratio >= 2.0, "symbol against wire: {ratio}");
            let rgb = |c: [u8; 4]| [c[0], c[1], c[2]];
            for c in [p.junction, p.wire_end] {
                assert_ne!(rgb(c), rgb(p.selection));
            }
            assert_eq!(rgb(p.unconnected_flag), rgb(p.warning));
        }
    }

    #[test]
    fn marks_follow_the_design_rules() {
        for p in [CANVAS_DARK, CANVAS_LIGHT] {
            let marks = [
                p.proposal,
                p.override_mark,
                p.added,
                p.modified,
                p.removed,
                p.warning,
                p.error,
            ];
            let rgb = |c: [u8; 4]| [c[0], c[1], c[2]];
            // Copper belongs to the selection; no mark, and not the always
            // visible snap marker, uses it.
            for m in marks.into_iter().chain([p.snap_marker]) {
                assert_ne!(rgb(m), rgb(p.selection), "{m:?}");
            }
            // A proposal is a fainter form of the hover hue, never as strong.
            assert_eq!(rgb(p.proposal), rgb(p.hover));
            assert!(p.proposal[3] < p.hover[3]);
            // Change marks stay translucent so the drawing shows through.
            for c in [p.added, p.modified, p.removed] {
                assert!(c[3] < 0xFF, "{c:?}");
            }
            let ratio =
                crate::tokens::tests::contrast(to_color32(p.mark_text), to_color32(p.sheet));
            assert!(ratio >= 4.5);
        }
    }

    #[test]
    fn potential_colours_are_readable_on_the_sheet_and_tell_the_conductors_apart() {
        for (p, canvas) in [
            (POTENTIALS_DARK, CANVAS_DARK),
            (POTENTIALS_LIGHT, CANVAS_LIGHT),
        ] {
            let sheet = to_color32(canvas.sheet);
            let all: Vec<[u8; 4]> = [
                p.protective_earth,
                p.pen,
                p.neutral,
                p.dc_positive,
                p.dc_negative,
            ]
            .into_iter()
            .chain(p.line)
            .collect();
            for c in &all {
                let ratio = crate::tokens::tests::contrast(to_color32(*c), sheet);
                assert!(ratio >= 4.5, "{c:?}: {ratio}");
            }
            for (i, a) in all.iter().enumerate() {
                for b in &all[i + 1..] {
                    assert_ne!(a, b);
                }
            }
            // The three line conductors differ clearly in hue, not just a shade.
            let spread = |a: [u8; 4], b: [u8; 4]| {
                (0..3)
                    .map(|i| (i32::from(a[i]) - i32::from(b[i])).unsigned_abs())
                    .sum::<u32>()
            };
            for (i, a) in p.line[..3].iter().enumerate() {
                for b in &p.line[i + 1..3] {
                    assert!(spread(*a, *b) >= 120, "{a:?} {b:?}");
                }
            }
        }
    }

    #[test]
    fn a_wire_without_potential_stays_visible_and_is_told_apart() {
        for (p, canvas) in [
            (POTENTIALS_DARK, CANVAS_DARK),
            (POTENTIALS_LIGHT, CANVAS_LIGHT),
        ] {
            let contrast = |a: [u8; 4], b: [u8; 4]| {
                crate::tokens::tests::contrast(to_color32(a), to_color32(b))
            };
            // At least 3:1 against the sheet it is drawn on.
            let ratio = contrast(p.dead, canvas.sheet);
            assert!(ratio >= 3.0, "dead wire on the sheet: {ratio}");
            // Not the selection, the symbols, or any energised potential.
            let rgb = |c: [u8; 4]| [c[0], c[1], c[2]];
            for other in [
                p.protective_earth,
                p.pen,
                p.neutral,
                p.dc_positive,
                p.dc_negative,
            ]
            .into_iter()
            .chain(p.line)
            .chain([canvas.selection, canvas.symbol, canvas.wire])
            {
                assert_ne!(rgb(p.dead), rgb(other), "{other:?}");
            }
            // Quieter than the symbols, so the drawing still reads first.
            assert!(contrast(p.dead, canvas.sheet) < contrast(canvas.symbol, canvas.sheet));
        }
    }
}
