//! Colours and stroke styles.

/// Straight (non-premultiplied) sRGB colour with alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color(pub [u8; 4]);

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    pub const RED: Color = Color::rgb(220, 40, 40);
    pub const GREEN: Color = Color::rgb(40, 170, 70);
    pub const BLUE: Color = Color::rgb(40, 90, 220);
    pub const TRANSPARENT: Color = Color([0, 0, 0, 0]);
    /// Default highlight for selected items.
    pub const SELECTION: Color = Color::rgb(255, 140, 0);
    /// Default highlight for the item under the cursor.
    pub const HOVER: Color = Color::rgb(0, 160, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color([r, g, b, 255])
    }

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color([r, g, b, a])
    }

    pub const fn with_alpha(self, a: u8) -> Self {
        let [r, g, b, _] = self.0;
        Color([r, g, b, a])
    }

    /// Packed as little-endian `r | g << 8 | b << 16 | a << 24`, which is the
    /// layout of `wgpu::VertexFormat::Unorm8x4`.
    pub const fn to_u32(self) -> u32 {
        u32::from_le_bytes(self.0)
    }
}

/// How a stroke width is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WidthUnit {
    /// Drawing units (mm). The line gets thicker when zooming in, like paper.
    World,
    /// Screen points. The line keeps its thickness at every zoom level.
    Screen,
}

/// Outline style of a primitive.
///
/// Strokes never render thinner than one screen pixel, so thin world-width
/// lines stay visible when zoomed out (they fade instead of vanishing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub unit: WidthUnit,
    pub color: Color,
}

impl Stroke {
    pub const fn world(width: f32, color: Color) -> Self {
        Self {
            width,
            unit: WidthUnit::World,
            color,
        }
    }

    pub const fn screen(width: f32, color: Color) -> Self {
        Self {
            width,
            unit: WidthUnit::Screen,
            color,
        }
    }

    /// Half the stroke width in world units at the given zoom
    /// (screen points per world unit).
    pub fn half_width_world(&self, zoom: f32) -> f32 {
        match self.unit {
            WidthUnit::World => self.width * 0.5,
            WidthUnit::Screen => self.width * 0.5 / zoom.max(f32::EPSILON),
        }
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke::world(0.25, Color::BLACK)
    }
}
