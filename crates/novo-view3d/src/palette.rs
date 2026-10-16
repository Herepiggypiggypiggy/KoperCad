//! Materials, light rig, wire colours and room colours (3d-look.md sections 2, 3, 7).
//!
//! Room and overlay colours here are the working values until the design system
//! palette (docs/design/design-system.md) carries a 3D section; materials are
//! physical colours and stay here.

/// Straight sRGB `[r, g, b, a]`, like `canvas::CanvasPalette`.
pub type Rgba = [u8; 4];

const fn rgb(hex: u32) -> Rgba {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255]
}

/// A flat material: colour plus specular strength. No textures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Material {
    pub color: Rgba,
    /// Specular strength in percent (0 to 100); power is fixed by the renderer.
    pub specular_pct: u8,
}

const fn mat(hex: u32, specular_pct: u8) -> Material {
    Material {
        color: rgb(hex),
        specular_pct,
    }
}

pub const PLATE_GALVANISED: Material = mat(0xA9AEB2, 35);
pub const PLATE_ORANGE: Material = mat(0xC9742B, 10);
pub const DIN_RAIL: Material = mat(0xCDD0D2, 35);
pub const DUCT_RAL7030: Material = mat(0x8B8D8A, 10);
pub const ENCLOSURE_RAL7035: Material = mat(0xD7D7D7, 10);
pub const DEVICE_DARK: Material = mat(0x4A4B4F, 15);
pub const DEVICE_LIGHT: Material = mat(0xC9C8C3, 15);
pub const TERMINAL: Material = mat(0xBDB9AD, 15);
pub const DEVICE_CONTROLLER: Material = mat(0x6E8196, 15);
pub const DEVICE_DRIVE: Material = mat(0x3F5A73, 20);
pub const DEVICE_SUPPLY: Material = mat(0x8A9C6B, 15);
pub const DEVICE_CONNECTOR: Material = mat(0xA89377, 15);
pub const NAMEPLATE: Material = mat(0xECEAE5, 0);

/// Opacity of enclosure parts in the ghost view, in percent.
pub const GHOST_WALL_PCT: u8 = 12;
pub const GHOST_DOOR_PCT: u8 = 8;
/// A device with a guessed size is drawn at this opacity with dashed edges.
pub const GUESSED_PCT: u8 = 55;

/// Edge colour is the base colour times this factor (never pure black).
pub fn edge_color(base: Rgba, factor: f32) -> Rgba {
    let f = |c: u8| (c as f32 * factor).round().clamp(0.0, 255.0) as u8;
    [f(base[0]), f(base[1]), f(base[2]), base[3]]
}

/// Lights (section 2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightRig {
    pub ambient_sky: Rgba,
    pub ambient_ground: Rgba,
    pub ambient: f32,
    /// World fixed key light: degrees clockwise from front, and above the plate.
    pub key_azimuth_deg: f32,
    pub key_elevation_deg: f32,
    pub key: f32,
    /// Camera relative fill, no shadow.
    pub fill: f32,
    pub shadow_map: u32,
    /// Contact darkening of the plate next to parts: strength and reach.
    pub contact_max: f32,
    pub contact_reach_mm: f32,
}

pub const LIGHT_DARK: LightRig = LightRig {
    ambient_sky: rgb(0xD8DCE2),
    ambient_ground: rgb(0x6F6C68),
    ambient: 0.55,
    key_azimuth_deg: -35.0,
    key_elevation_deg: 50.0,
    key: 0.85,
    fill: 0.25,
    shadow_map: 1024,
    contact_max: 0.35,
    contact_reach_mm: 6.0,
};

pub const LIGHT_LIGHT: LightRig = LightRig {
    ambient: 0.65,
    ..LIGHT_DARK
};

/// Rendering quality (section 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum QualityTier {
    Low,
    Standard,
    High,
}

impl QualityTier {
    pub fn msaa(self) -> u32 {
        match self {
            Self::Low => 2,
            _ => 4,
        }
    }

    pub fn shadow_map(self) -> Option<u32> {
        match self {
            Self::Low => None,
            Self::Standard => Some(1024),
            Self::High => Some(2048),
        }
    }

    pub fn ssao(self) -> bool {
        self == Self::High
    }

    /// First tier for an adapter: integrated GPUs start on Low.
    pub fn for_adapter(integrated: bool) -> Self {
        if integrated {
            Self::Low
        } else {
            Self::Standard
        }
    }

    /// One tier down, or `None` on Low.
    pub fn step_down(self) -> Option<Self> {
        match self {
            Self::High => Some(Self::Standard),
            Self::Standard => Some(Self::Low),
            Self::Low => None,
        }
    }
}

/// Which theme the room and overlays use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View3dTheme {
    Dark,
    Light,
}

/// Room and overlay colours (section 7). Selection and hover are shared.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeColors {
    pub background_top: Rgba,
    pub background_bottom: Rgba,
    pub ground_grid: Rgba,
    pub edge_factor: f32,
    pub label_chip: Rgba,
    pub label_text: Rgba,
    pub label_border: Rgba,
    pub selection: Rgba,
    pub hover: Rgba,
    pub light: LightRig,
}

/// Copper, the same as `tokens::ACCENT`.
pub const SELECTION: Rgba = rgb(0xC88A5E);
/// Steel blue, the same as `tokens::INFO`.
pub const HOVER: Rgba = rgb(0x8FA8C8);

pub const COLORS_DARK: ThemeColors = ThemeColors {
    background_top: rgb(0x0E0E10),
    background_bottom: rgb(0x151517),
    ground_grid: rgb(0x232327),
    edge_factor: 0.45,
    // BG_2 at 85%.
    label_chip: [0x1B, 0x1B, 0x1E, 217],
    label_text: rgb(0xE3E1DC),
    label_border: rgb(0x3B3B41),
    selection: SELECTION,
    hover: HOVER,
    light: LIGHT_DARK,
};

pub const COLORS_LIGHT: ThemeColors = ThemeColors {
    background_top: rgb(0xECEAE6),
    background_bottom: rgb(0xDEDCD7),
    ground_grid: rgb(0xCFCDC8),
    edge_factor: 0.55,
    label_chip: [0xFF, 0xFF, 0xFF, 230],
    label_text: rgb(0x1B1B1E),
    label_border: rgb(0xB9B7B1),
    selection: SELECTION,
    hover: HOVER,
    light: LIGHT_LIGHT,
};

impl View3dTheme {
    pub fn colors(self) -> &'static ThemeColors {
        match self {
            Self::Dark => &COLORS_DARK,
            Self::Light => &COLORS_LIGHT,
        }
    }
}

/// Wire colour by role (IEC 60757 style, section 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireColor {
    ControlAc,
    Power,
    Neutral,
    Dc24Plus,
    Protective,
    Unknown,
}

/// Wire colour; the second colour is a stripe (PE only).
pub fn wire_rgba(role: WireColor) -> (Rgba, Option<Rgba>) {
    match role {
        WireColor::ControlAc => (rgb(0xB33A3A), None),
        WireColor::Power => (rgb(0x101012), None),
        WireColor::Neutral => (rgb(0x2F6FD0), None),
        WireColor::Dc24Plus => (rgb(0xC64A4A), None),
        WireColor::Protective => (rgb(0x7FB020), Some(rgb(0xE3D34A))),
        WireColor::Unknown => (rgb(0x8C8F94), None),
    }
}

/// Wire tube radius in mm for a cross section in mm2 (section 3).
pub fn wire_radius_mm(cross_section_mm2: f32) -> f32 {
    match cross_section_mm2 {
        s if s <= 1.5 => 1.0,
        s if s <= 2.5 => 1.3,
        s if s <= 6.0 => 2.0,
        s => 2.0 + (s - 6.0).sqrt() * 0.4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(c: u8) -> f32 {
        let c = c as f32 / 255.0;
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luminance(c: Rgba) -> f32 {
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    fn contrast(a: Rgba, b: Rgba) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    /// Blend a translucent chip over the room's darkest or lightest background.
    fn over(chip: Rgba, bg: Rgba) -> Rgba {
        let a = chip[3] as f32 / 255.0;
        let m = |i: usize| (chip[i] as f32 * a + bg[i] as f32 * (1.0 - a)).round() as u8;
        [m(0), m(1), m(2), 255]
    }

    #[test]
    fn label_text_meets_seven_to_one_on_the_chip_in_both_themes() {
        for t in [View3dTheme::Dark, View3dTheme::Light] {
            let c = t.colors();
            for bg in [
                c.background_top,
                c.background_bottom,
                rgb(0xA9AEB2),
                rgb(0xD7D7D7),
            ] {
                assert!(
                    contrast(c.label_text, over(c.label_chip, bg)) >= 7.0,
                    "{t:?}"
                );
            }
        }
    }

    #[test]
    fn selection_and_hover_are_not_material_colours() {
        let all = [
            PLATE_GALVANISED,
            PLATE_ORANGE,
            DIN_RAIL,
            DUCT_RAL7030,
            ENCLOSURE_RAL7035,
            DEVICE_DARK,
            DEVICE_LIGHT,
            TERMINAL,
            NAMEPLATE,
        ];
        let dist = |a: Rgba, b: Rgba| {
            (0..3)
                .map(|i| (a[i] as i32 - b[i] as i32).pow(2))
                .sum::<i32>()
        };
        for m in all {
            assert!(dist(m.color, SELECTION) > 40 * 40, "{:?}", m.color);
        }
        // Hover outlines sit on devices, not on the bluish grey plate.
        for m in [DEVICE_DARK, DEVICE_LIGHT, TERMINAL] {
            assert!(dist(m.color, HOVER) > 40 * 40, "{:?}", m.color);
        }
    }

    #[test]
    fn edges_are_darker_but_never_black() {
        let e = edge_color(DEVICE_DARK.color, COLORS_DARK.edge_factor);
        assert!(e[0] < DEVICE_DARK.color[0] && e[0] > 0);
    }

    #[test]
    fn protective_wire_has_a_stripe_and_others_do_not() {
        assert!(wire_rgba(WireColor::Protective).1.is_some());
        assert!(wire_rgba(WireColor::Neutral).1.is_none());
    }

    #[test]
    fn wire_radius_grows_with_section() {
        assert!(wire_radius_mm(1.5) < wire_radius_mm(2.5));
        assert!(wire_radius_mm(2.5) < wire_radius_mm(6.0));
        assert!(wire_radius_mm(6.0) < wire_radius_mm(16.0));
    }

    #[test]
    fn integrated_gpus_start_low_and_only_step_down() {
        assert_eq!(QualityTier::for_adapter(true), QualityTier::Low);
        assert_eq!(QualityTier::for_adapter(false), QualityTier::Standard);
        assert_eq!(QualityTier::High.step_down(), Some(QualityTier::Standard));
        assert_eq!(QualityTier::Low.step_down(), None);
        assert_eq!(QualityTier::Low.msaa(), 2);
        assert!(QualityTier::Low.shadow_map().is_none());
    }
}
