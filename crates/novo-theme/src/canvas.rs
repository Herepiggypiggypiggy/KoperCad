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
    /// Wires and connections, tinted blue so they read apart from symbols.
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
    symbol: [0xD9, 0xD7, 0xD2, 0xFF],
    wire: [0x9F, 0xB4, 0xD0, 0xFF],
    text: [0xC4, 0xC2, 0xBD, 0xFF],
    cross_reference: [0x8F, 0xA8, 0xC8, 0xFF],
    connection_point: [0x7F, 0xB0, 0x86, 0xFF],
    selection: [0xE0, 0xA0, 0x70, 0xFF],
    hover: [0x8F, 0xA8, 0xC8, 0xFF],
    rubber_band: [0x8F, 0xA8, 0xC8, 0xFF],
    error: [0xE0, 0x76, 0x6C, 0xFF],
    warning: [0xD6, 0xA5, 0x5C, 0xFF],
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
}
