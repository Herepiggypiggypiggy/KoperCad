//! Name, logo and app icon.
//!
//! The logo source is `assets/branding/icon.svg`; the PNG and ICO files next
//! to it are rendered from it with `tools/brand-render` (see
//! `assets/branding/README.md`).

use crate::tokens;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Product name as shown to people.
pub const NAME: &str = "KoperCAD";

/// One-line description for the About box, installer and website.
pub const TAGLINE: &str = "Electrical design, without the weight";

/// Logo source.
pub const ICON_SVG: &str = include_str!("../../../assets/branding/icon.svg");

/// App icon, 256 x 256 PNG, for the window icon.
pub const ICON_PNG_256: &[u8] = include_bytes!("../../../assets/branding/icon-256.png");

/// App icon, 32 x 32 PNG, for small in-app use.
pub const ICON_PNG_32: &[u8] = include_bytes!("../../../assets/branding/icon-32.png");

/// Background of the logo tile.
pub const MARK_BACKGROUND: Color32 = Color32::from_rgb(0x15, 0x15, 0x17);
/// The rays and points of the star.
pub const MARK_LIGHT: Color32 = tokens::TEXT;
/// The junction at the centre.
pub const MARK_COPPER: Color32 = tokens::BRAND;

/// Paints the logo mark into `rect` with egui shapes, so the About box and the
/// start screen show it crisply at any size without an image loader.
///
/// The mark is "Nova": a new star whose four rays meet at a copper junction
/// dot, the dot that joins wires on a schematic. `tile` adds the rounded dark
/// background of the app icon.
pub fn paint_mark(painter: &Painter, rect: Rect, tile: bool) {
    // Same geometry as icon.svg, which is drawn on a 256 unit square.
    let side = rect.width().min(rect.height());
    let unit = side / 256.0;
    let origin = rect.center() - Vec2::splat(side / 2.0);
    let at = |x: f32, y: f32| Pos2::new(origin.x + x * unit, origin.y + y * unit);

    if tile {
        let tile_rect = Rect::from_min_max(at(8.0, 8.0), at(248.0, 248.0));
        let radius = (56.0 * unit).round().clamp(0.0, 255.0) as u8;
        painter.rect_filled(tile_rect, radius, MARK_BACKGROUND);
    }

    // Each ray is a kite: tip, shoulder, inner point, shoulder.
    let rays = [
        [
            (128.0, 38.0),
            (136.0, 104.0),
            (128.0, 112.0),
            (120.0, 104.0),
        ],
        [
            (128.0, 218.0),
            (120.0, 152.0),
            (128.0, 144.0),
            (136.0, 152.0),
        ],
        [
            (38.0, 128.0),
            (104.0, 120.0),
            (112.0, 128.0),
            (104.0, 136.0),
        ],
        [
            (218.0, 128.0),
            (152.0, 136.0),
            (144.0, 128.0),
            (152.0, 120.0),
        ],
    ];
    for ray in rays {
        let points = ray.iter().map(|&(x, y)| at(x, y)).collect();
        painter.add(egui::Shape::convex_polygon(
            points,
            MARK_LIGHT,
            Stroke::NONE,
        ));
    }
    for (x, y) in [(84.0, 84.0), (172.0, 84.0), (84.0, 172.0), (172.0, 172.0)] {
        painter.circle_filled(at(x, y), 5.0 * unit, MARK_LIGHT);
    }
    painter.circle_filled(at(128.0, 128.0), 11.0 * unit, MARK_COPPER);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_size(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
        let be = |i: usize| u32::from_be_bytes(png[i..i + 4].try_into().unwrap());
        (be(16), be(20))
    }

    #[test]
    fn icons_have_their_stated_sizes() {
        assert_eq!(png_size(ICON_PNG_256), (256, 256));
        assert_eq!(png_size(ICON_PNG_32), (32, 32));
    }

    #[test]
    fn svg_is_square() {
        assert!(ICON_SVG.contains(r#"viewBox="0 0 256 256""#));
    }
}
