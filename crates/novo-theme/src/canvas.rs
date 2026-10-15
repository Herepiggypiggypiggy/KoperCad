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

/// Converts a palette colour for drawing with egui.
pub fn to_color32([r, g, b, a]: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The dark canvas palette.
pub const CANVAS_DARK: CanvasPalette = CanvasPalette {
    background: [0x0E, 0x0E, 0x10, 0xFF],
    sheet: [0x13, 0x13, 0x15, 0xFF],
    sheet_frame: [0x46, 0x45, 0x48, 0xFF],
    grid_minor: [0x20, 0x20, 0x23, 0xFF],
    grid_major: [0x2C, 0x2C, 0x30, 0xFF],
    symbol: [0xEC, 0xEA, 0xE5, 0xFF],
    wire: [0x6F, 0x9B, 0xDB, 0xFF],
    text: [0xC4, 0xC2, 0xBD, 0xFF],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_colours_stand_out_from_the_sheet() {
        let p = CANVAS_DARK;
        let grey = |c: [u8; 4]| (c[0] as u32 + c[1] as u32 + c[2] as u32) / 3;
        for c in [p.symbol, p.wire, p.text] {
            assert!(grey(c) > grey(p.sheet) + 120, "{c:?}");
        }
        // The grid stays quieter than anything drawn on it.
        assert!(grey(p.grid_major) < grey(p.sheet_frame));
        assert!(grey(p.grid_minor) < grey(p.grid_major));
    }

    #[test]
    fn drawing_colours_meet_wcag_aa_on_the_sheet() {
        let p = CANVAS_DARK;
        let sheet = to_color32(p.sheet);
        for c in [
            p.symbol,
            p.wire,
            p.text,
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

    #[test]
    fn wires_are_told_apart_from_symbols_and_junctions_are_not_the_selection() {
        let p = CANVAS_DARK;
        let ratio = crate::tokens::tests::contrast(to_color32(p.symbol), to_color32(p.wire));
        assert!(ratio >= 2.0, "symbol against wire: {ratio}");
        let rgb = |c: [u8; 4]| [c[0], c[1], c[2]];
        for c in [p.junction, p.wire_end] {
            assert_ne!(rgb(c), rgb(p.selection));
        }
        assert_eq!(rgb(p.unconnected_flag), rgb(p.warning));
    }

    #[test]
    fn marks_follow_the_design_rules() {
        let p = CANVAS_DARK;
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
        let ratio = crate::tokens::tests::contrast(to_color32(p.mark_text), to_color32(p.sheet));
        assert!(ratio >= 4.5);
    }
}
