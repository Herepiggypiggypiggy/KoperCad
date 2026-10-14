//! Name, logo and app icon.
//!
//! The logo source is `assets/branding/icon.svg`; the PNG and ICO files next
//! to it are rendered from it with `tools/brand-render` (see
//! `assets/branding/README.md`).

use crate::tokens;
use egui::{Color32, Painter, Pos2, Rect, Stroke, Vec2};

/// Product name as shown to people.
pub const NAME: &str = "NovoElectric";

/// One-line description for the About box, installer and website.
pub const TAGLINE: &str = "Electrical design, without the weight";

/// Logo source.
pub const ICON_SVG: &str = include_str!("../../../assets/branding/icon.svg");

/// App icon, 256 x 256 PNG, for the window icon.
pub const ICON_PNG_256: &[u8] = include_bytes!("../../../assets/branding/icon-256.png");

/// App icon, 32 x 32 PNG, for small in-app use.
pub const ICON_PNG_32: &[u8] = include_bytes!("../../../assets/branding/icon-32.png");

/// Background of the logo tile.
pub const MARK_BACKGROUND: Color32 = Color32::from_rgb(0x17, 0x1A, 0x1F);
/// The two contact posts of the mark.
pub const MARK_POST: Color32 = tokens::TEXT;
/// The switching blade and pivot of the mark.
pub const MARK_BLADE: Color32 = tokens::BRAND;

/// Paints the logo mark into `rect` with egui shapes, so the About box and the
/// start screen show it crisply at any size without an image loader.
///
/// The mark is an "N" drawn as an open contact: two posts and an amber blade
/// that pivots on the left post and stops short of the right one.
/// `tile` adds the rounded dark background of the app icon.
pub fn paint_mark(painter: &Painter, rect: Rect, tile: bool) {
    // Same geometry as icon.svg, which is drawn on a 256 unit square.
    let side = rect.width().min(rect.height());
    let unit = side / 256.0;
    let origin = rect.center() - Vec2::splat(side / 2.0);
    let at = |x: f32, y: f32| Pos2::new(origin.x + x * unit, origin.y + y * unit);

    if tile {
        let tile_rect = Rect::from_min_max(at(8.0, 8.0), at(248.0, 248.0));
        let radius = (52.0 * unit).round().clamp(0.0, 255.0) as u8;
        painter.rect_filled(tile_rect, radius, MARK_BACKGROUND);
    }

    let width = 22.0 * unit;
    let line = |a: Pos2, b: Pos2, color: Color32| {
        painter.line_segment([a, b], Stroke::new(width, color));
        // Round caps.
        painter.circle_filled(a, width / 2.0, color);
        painter.circle_filled(b, width / 2.0, color);
    };
    line(at(76.0, 190.0), at(76.0, 66.0), MARK_POST);
    line(at(180.0, 66.0), at(180.0, 190.0), MARK_POST);
    line(at(76.0, 66.0), at(156.0, 172.0), MARK_BLADE);
    painter.circle_filled(at(76.0, 66.0), 19.0 * unit, MARK_BLADE);
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
