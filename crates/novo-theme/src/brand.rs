//! Name, logo and app icon.
//!
//! The logo source is `assets/branding/icon.svg`; the PNG and ICO files next
//! to it are rendered from it with `tools/brand-render` (see
//! `assets/branding/README.md`).

use crate::{Palette, fonts, tokens};
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Painter, Pos2, Rect, Sense, Stroke, Ui, Vec2};

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

/// The maker's name, shown under the wordmark.
pub const MAKER: &str = "KoperVonk";

/// The mark, the wordmark ("Koper" in the text colour, "CAD" in copper) and
/// "BY KOPERVONK" under it, centred in the width of `ui`: the start screen's
/// and the About card's header. `mark` is the mark's side in points.
pub fn lockup(ui: &mut Ui, mark: f32) {
    let p = Palette::of(ui);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(mark), Sense::hover());
        paint_mark(ui.painter(), rect, true);
        ui.add_space(crate::space::M);
        let size = (mark * 0.45).max(16.0);
        let font = FontId::new(size, fonts::strong_family());
        let mut job = LayoutJob::default();
        job.append(
            "Koper",
            0.0,
            TextFormat::simple(font.clone(), p.text_primary),
        );
        job.append("CAD", 0.0, TextFormat::simple(font, p.accent));
        ui.label(job);
        ui.label(
            egui::RichText::new(format!("BY {}", MAKER.to_uppercase()))
                .size(
                    ui.style().text_styles[&egui::TextStyle::Small]
                        .size
                        .max(12.0),
                )
                .family(fonts::strong_family())
                .extra_letter_spacing(1.2)
                .color(p.text_secondary),
        );
    });
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

    #[test]
    fn the_lockup_lays_out_in_both_themes() {
        for theme in [crate::ThemeChoice::Dark, crate::ThemeChoice::Light] {
            let ctx = egui::Context::default();
            crate::apply_with(&ctx, theme, crate::Density::Standard);
            let mut height = 0.0;
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
                let before = ui.cursor().top();
                lockup(ui, 64.0);
                height = ui.cursor().top() - before;
            });
            out.textures_delta.clear();
            assert!(height > 64.0, "mark, name and maker take room: {height}");
        }
    }
}
