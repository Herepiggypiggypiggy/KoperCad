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

fn dark_surface(kind: ItemKind) -> SurfaceStyle {
    let s = |color: Rgb, edge: Rgb, alpha: f32| SurfaceStyle { color, edge, alpha };
    match kind {
        ItemKind::Plate => s([0x8f, 0x8a, 0x82], [0x5f, 0x5b, 0x55], 1.0),
        ItemKind::Rail => s([0xb8, 0xbd, 0xc5], [0x6d, 0x72, 0x7a], 1.0),
        ItemKind::Duct => s([0xcf, 0xd2, 0xd6], [0x7b, 0x7f, 0x85], 1.0),
        ItemKind::Device => s([0xe1, 0xe4, 0xe9], [0x4a, 0x4f, 0x57], 1.0),
        ItemKind::Door => s([0x7f, 0x95, 0xad], [0xa9, 0xbb, 0xcf], 0.22),
        ItemKind::Enclosure => s([0x6b, 0x79, 0x8a], [0x93, 0xa3, 0xb6], 0.16),
        ItemKind::Wire => s([0xc1, 0x7a, 0x3a], [0x8c, 0x56, 0x26], 1.0),
        _ => s([0xc4, 0xc7, 0xcc], [0x6d, 0x72, 0x7a], 1.0),
    }
}

fn dark_mark(mark: Mark) -> MarkStyle {
    let m = |glyph: Glyph, tint: Rgb, amount: f32, edge: Rgb| MarkStyle {
        glyph,
        tint: Some(tint),
        tint_amount: amount,
        edge: Some(edge),
    };
    match mark {
        Mark::Collision => m(Glyph::Cross, [0xe5, 0x48, 0x4d], 0.55, [0xff, 0x6b, 0x6b]),
        Mark::TooClose => m(Glyph::Bang, [0xf0, 0xa2, 0x2e], 0.40, [0xff, 0xb7, 0x4a]),
        Mark::Outside => m(Glyph::Cross, [0xe5, 0x48, 0x4d], 0.30, [0xff, 0x6b, 0x6b]),
        Mark::GuessedSize => MarkStyle {
            glyph: Glyph::DashedEdge,
            tint: None,
            tint_amount: 0.0,
            edge: Some([0x8c, 0xb4, 0xff]),
        },
        Mark::GeometryMismatch => MarkStyle {
            glyph: Glyph::Bang,
            tint: None,
            tint_amount: 0.0,
            edge: Some([0xf0, 0xa2, 0x2e]),
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

    /// Calm technical illustration on the dark app theme.
    pub fn dark() -> Self {
        Self {
            background: [[0x2d, 0x30, 0x35], [0x1b, 0x1d, 0x20]],
            surfaces: KINDS.map(dark_surface),
            marks: MARKS.map(dark_mark),
            selection: Highlight {
                tint: [0xd9, 0x8b, 0x4b],
                tint_amount: 0.35,
                edge: [0xff, 0xa8, 0x5c],
            },
            hover: Highlight {
                tint: [0xff, 0xff, 0xff],
                tint_amount: 0.18,
                edge: [0xff, 0xd5, 0xa8],
            },
            related: Highlight {
                tint: [0x8c, 0xb4, 0xff],
                tint_amount: 0.25,
                edge: [0x8c, 0xb4, 0xff],
            },
            light: [-0.45, 0.75, 0.5],
            ambient: 0.55,
            contact_shade: 0.22,
        }
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
