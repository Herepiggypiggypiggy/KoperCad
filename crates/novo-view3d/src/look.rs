//! How things look. One plain struct of colours and constants; the renderer
//! has no colour literals of its own. The visual design thread owns the values
//! (`docs/design/3d-view.md` section 9); these are the working defaults.

use crate::scene::{ItemKind, Mark};

pub type Rgb = [u8; 3];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceStyle {
    pub color: Rgb,
    pub edge: Rgb,
    /// Opacity of translucent items of this kind (1 = opaque).
    pub alpha: f32,
}

/// A mark is never shown by colour alone: it also draws a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    None,
    Cross,
    Bang,
    /// A dashed edge instead of a symbol (guessed size).
    DashedEdge,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkStyle {
    pub glyph: Glyph,
    /// Mixed into the face colour by `tint_amount`, if any.
    pub tint: Option<Rgb>,
    pub tint_amount: f32,
    pub edge: Option<Rgb>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub tint: Rgb,
    pub tint_amount: f32,
    pub edge: Rgb,
}

/// Plain data, rebuilt when the app theme changes (no function pointers, so a
/// theme can fill it from its own state).
#[derive(Debug, Clone, Copy)]
pub struct Look {
    /// Top and bottom of the background gradient.
    pub background: [Rgb; 2],
    /// Indexed by `ItemKind::index`.
    pub surfaces: [SurfaceStyle; ItemKind::COUNT],
    /// Indexed by `Mark::index`.
    pub marks: [MarkStyle; Mark::COUNT],
    pub selection: Highlight,
    pub hover: Highlight,
    pub related: Highlight,
    /// Direction the light comes from (towards the light), world space.
    pub light: [f32; 3],
    /// Share of the shading that comes from the hemisphere term, 0 to 1.
    pub ambient: f32,
    /// Darkening at the foot of things standing on the plate, 0 = off.
    pub contact_shade: f32,
}

/// Copies of the design tokens `ERROR`, `WARNING` and `TEXT_MUTED`; a test
/// keeps them equal to `novo-theme` so a theme change cannot drift silently.
pub const MARK_ERROR: Rgb = [0xE0, 0x76, 0x6C];
pub const MARK_WARNING: Rgb = [0xD6, 0xA5, 0x5C];
pub const MARK_MUTED: Rgb = [0x98, 0x96, 0x91];

fn rgb3(c: crate::palette::Rgba) -> Rgb {
    [c[0], c[1], c[2]]
}

/// Surface of a kind from the spec materials (3d-look.md section 3); the edge
/// is the base colour times the theme's edge factor, never pure black.
fn surface(kind: ItemKind, edge_factor: f32) -> SurfaceStyle {
    use crate::palette::*;
    let s = |m: Material, alpha: f32| SurfaceStyle {
        color: rgb3(m.color),
        edge: rgb3(edge_color(m.color, edge_factor)),
        alpha,
    };
    match kind {
        ItemKind::Plate => s(PLATE_GALVANISED, 1.0),
        ItemKind::Rail => s(DIN_RAIL, 1.0),
        ItemKind::Duct => s(DUCT_RAL7030, 1.0),
        ItemKind::Device => s(DEVICE_LIGHT, 1.0),
        ItemKind::Door => s(ENCLOSURE_RAL7035, GHOST_DOOR_PCT as f32 / 100.0),
        ItemKind::Enclosure => s(ENCLOSURE_RAL7035, GHOST_WALL_PCT as f32 / 100.0),
        ItemKind::Wire => SurfaceStyle {
            color: rgb3(wire_rgba(WireColor::Unknown).0),
            edge: rgb3(edge_color(wire_rgba(WireColor::Unknown).0, edge_factor)),
            alpha: 1.0,
        },
        _ => s(DEVICE_LIGHT, 1.0),
    }
}

/// Marks always draw a glyph as well as a colour (3d-look.md section 5).
fn mark_style(mark: Mark) -> MarkStyle {
    let status = |glyph: Glyph, c: Rgb, amount: f32| MarkStyle {
        glyph,
        tint: Some(c),
        tint_amount: amount,
        edge: Some(c),
    };
    const ERROR: Rgb = MARK_ERROR;
    const WARNING: Rgb = MARK_WARNING;
    match mark {
        Mark::Collision => status(Glyph::Cross, ERROR, 0.40),
        Mark::TooClose => status(Glyph::Bang, WARNING, 0.30),
        Mark::Outside => status(Glyph::Cross, ERROR, 0.25),
        Mark::GuessedSize => MarkStyle {
            glyph: Glyph::DashedEdge,
            tint: None,
            tint_amount: 0.0,
            edge: Some(MARK_MUTED),
        },
        Mark::GeometryMismatch => MarkStyle {
            glyph: Glyph::Bang,
            tint: None,
            tint_amount: 0.0,
            edge: Some(WARNING),
        },
        _ => MarkStyle {
            glyph: Glyph::None,
            tint: None,
            tint_amount: 0.0,
            edge: None,
        },
    }
}

const KINDS: [ItemKind; ItemKind::COUNT] = [
    ItemKind::Enclosure,
    ItemKind::Plate,
    ItemKind::Rail,
    ItemKind::Duct,
    ItemKind::Device,
    ItemKind::Door,
    ItemKind::Wire,
    ItemKind::Other,
];

const MARKS: [Mark; Mark::COUNT] = [
    Mark::None,
    Mark::Collision,
    Mark::TooClose,
    Mark::Outside,
    Mark::GuessedSize,
    Mark::GeometryMismatch,
];

impl Look {
    pub fn surface(&self, kind: ItemKind) -> SurfaceStyle {
        self.surfaces[kind.index()]
    }

    pub fn mark(&self, mark: Mark) -> MarkStyle {
        self.marks[mark.index()]
    }

    /// The look for a theme (3d-look.md section 7). Materials are the same in
    /// both; the room, edges and light level change.
    pub fn for_theme(theme: crate::palette::View3dTheme) -> Self {
        let c = theme.colors();
        let key = c.light;
        let (az, el) = (
            key.key_azimuth_deg.to_radians(),
            key.key_elevation_deg.to_radians(),
        );
        Self {
            background: [rgb3(c.background_top), rgb3(c.background_bottom)],
            surfaces: KINDS.map(|k| surface(k, c.edge_factor)),
            marks: MARKS.map(mark_style),
            selection: Highlight {
                tint: rgb3(c.selection),
                tint_amount: 0.12,
                edge: rgb3(c.selection),
            },
            hover: Highlight {
                tint: rgb3(c.hover),
                tint_amount: 0.0,
                edge: rgb3(c.hover),
            },
            related: Highlight {
                tint: rgb3(c.hover),
                tint_amount: 0.15,
                edge: rgb3(c.hover),
            },
            // Towards the light: front left above, +Y up, +Z out of the plate.
            light: [az.sin() * el.cos(), el.sin(), az.cos() * el.cos()],
            ambient: key.ambient,
            contact_shade: key.contact_max,
        }
    }

    /// Calm technical illustration on the dark app theme.
    pub fn dark() -> Self {
        Self::for_theme(crate::palette::View3dTheme::Dark)
    }

    pub fn light() -> Self {
        Self::for_theme(crate::palette::View3dTheme::Light)
    }
}

impl Default for Look {
    fn default() -> Self {
        Self::dark()
    }
}

/// Linear mix of two colours, `t` clamped to 0..1.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2])]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_problem_mark_draws_a_glyph() {
        let look = Look::dark();
        for m in [
            Mark::Collision,
            Mark::TooClose,
            Mark::Outside,
            Mark::GuessedSize,
            Mark::GeometryMismatch,
        ] {
            assert_ne!(look.mark(m).glyph, Glyph::None, "{m:?}");
        }
    }

    #[test]
    fn the_light_vector_is_a_unit_vector_from_front_left_above() {
        let l = Look::dark().light;
        let n = (l[0] * l[0] + l[1] * l[1] + l[2] * l[2]).sqrt();
        assert!((n - 1.0).abs() < 1e-5);
        assert!(l[0] < 0.0 && l[1] > 0.0 && l[2] > 0.0);
    }

    #[test]
    fn materials_match_in_both_themes_but_edges_and_room_differ() {
        let (d, l) = (Look::dark(), Look::light());
        assert_eq!(
            d.surface(ItemKind::Plate).color,
            l.surface(ItemKind::Plate).color
        );
        assert_ne!(
            d.surface(ItemKind::Plate).edge,
            l.surface(ItemKind::Plate).edge
        );
        assert_ne!(d.background, l.background);
    }

    #[test]
    fn selection_is_copper_and_hover_is_not_tinted() {
        let look = Look::dark();
        assert_eq!(look.selection.edge, [0xC8, 0x8A, 0x5E]);
        assert_eq!(look.hover.tint_amount, 0.0);
        assert_ne!(look.hover.edge, look.selection.edge);
    }

    #[test]
    fn the_shell_is_ghosted() {
        let look = Look::dark();
        assert!(look.surface(ItemKind::Enclosure).alpha < 0.2);
        assert!(look.surface(ItemKind::Door).alpha < look.surface(ItemKind::Enclosure).alpha);
    }

    #[test]
    fn literals_equal_the_design_tokens() {
        use novo_theme::tokens as t;
        let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
        assert_eq!(MARK_ERROR, rgb(t::ERROR));
        assert_eq!(MARK_WARNING, rgb(t::WARNING));
        assert_eq!(MARK_MUTED, rgb(t::TEXT_MUTED));
        let look = Look::dark();
        assert_eq!(look.selection.edge, rgb(t::ACCENT));
        assert_eq!(look.hover.edge, rgb(t::INFO));
        assert_eq!(look.background[0], rgb(t::BG_0));
    }
}
