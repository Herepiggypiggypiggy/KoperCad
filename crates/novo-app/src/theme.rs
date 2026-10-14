//! The canvas in the app's colours: `novo-theme`'s dark canvas palette
//! applied to `novo-canvas` and its model drawing.

use novo_canvas::model::ModelStyle;
use novo_canvas::widget::CanvasStyle;
use novo_canvas::{Color, Highlight, Stroke};
use novo_theme::{CANVAS_DARK, CanvasPalette};

fn color(c: [u8; 4]) -> Color {
    Color(c)
}

pub fn canvas_style() -> CanvasStyle {
    canvas_style_from(&CANVAS_DARK)
}

pub fn model_style() -> ModelStyle {
    model_style_from(&CANVAS_DARK)
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
        snap_marker: color(p.selection),
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
        tag_color: color(p.text),
        sheet_fill: Some(color(p.sheet)),
        frame_color: color(p.sheet_frame),
        ..defaults
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_canvas_uses_the_dark_palette() {
        let style = canvas_style();
        assert_eq!(style.background, Color(CANVAS_DARK.background));
        assert_eq!(model_style().symbol.color, Color(CANVAS_DARK.symbol));
        assert_eq!(model_style().sheet_fill, Some(Color(CANVAS_DARK.sheet)));
    }
}
