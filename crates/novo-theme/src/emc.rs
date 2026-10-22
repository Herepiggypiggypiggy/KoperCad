//! Colours of the EMC view: one colour per cable class, validated for the
//! dark and the light surface (lightness band, chroma, colour-blind
//! separation of neighbours, contrast 3:1). A class is never colour alone:
//! the legend names it and the views mark it with a dash or a sheath.
//!
//! Order follows `novo_view3d::emc::Class::index`: mains, motor, control,
//! analog, bus, safety, earth. The risk ramp is the heat view's ember ramp
//! (`crate::heat::ramp`), so a hot spot reads like a hot device.

/// Class colours on the dark surface.
pub const CLASS_DARK: [[u8; 3]; 7] = [
    [0xC3, 0x75, 0x15], // mains: amber
    [0xD0, 0x44, 0x79], // motor: magenta
    [0x16, 0x9A, 0x91], // control: teal
    [0x4F, 0x7D, 0xE0], // analog: blue
    [0x6C, 0x9A, 0x25], // bus: green
    [0x91, 0x70, 0xDE], // safety: violet
    [0xD8, 0xD2, 0x3A], // earth: yellow (the green stripe is `EARTH_STRIPE_DARK`)
];

/// Class colours on the light surface.
pub const CLASS_LIGHT: [[u8; 3]; 7] = [
    [0x9A, 0x5B, 0x00],
    [0xB8, 0x24, 0x5E],
    [0x00, 0x8F, 0x84],
    [0x2C, 0x5F, 0xCC],
    [0x4F, 0x7A, 0x10],
    [0x6B, 0x45, 0xC2],
    [0x8A, 0x7D, 0x00],
];

/// The green of the green-yellow protective conductor.
pub const EARTH_STRIPE_DARK: [u8; 3] = [0x2F, 0x8F, 0x3A];
pub const EARTH_STRIPE_LIGHT: [u8; 3] = [0x1F, 0x7A, 0x2C];

/// A bond that is in order.
pub const GOOD_DARK: [u8; 3] = [0x41, 0xB0, 0x5A];
pub const GOOD_LIGHT: [u8; 3] = [0x1F, 0x7A, 0x2C];

pub fn classes(dark: bool) -> [[u8; 3]; 7] {
    if dark { CLASS_DARK } else { CLASS_LIGHT }
}

pub fn earth(dark: bool) -> ([u8; 3], [u8; 3]) {
    if dark {
        (CLASS_DARK[6], EARTH_STRIPE_DARK)
    } else {
        (CLASS_LIGHT[6], EARTH_STRIPE_LIGHT)
    }
}

pub fn good(dark: bool) -> [u8; 3] {
    if dark { GOOD_DARK } else { GOOD_LIGHT }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lin(v: u8) -> f32 {
        let s = v as f32 / 255.0;
        if s <= 0.04045 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }

    fn luma(c: [u8; 3]) -> f32 {
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    fn contrast(a: [u8; 3], b: [u8; 3]) -> f32 {
        let (x, y) = (luma(a), luma(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_class_colour_holds_3_to_1_on_its_surface() {
        // The chart surfaces of the validator: #1a1a19 dark, #fcfcfb light.
        for c in CLASS_DARK {
            assert!(contrast(c, [0x1A, 0x1A, 0x19]) >= 3.0, "{c:?}");
        }
        for c in CLASS_LIGHT {
            assert!(contrast(c, [0xFC, 0xFC, 0xFB]) >= 3.0, "{c:?}");
        }
    }

    #[test]
    fn the_classes_are_all_different() {
        for set in [CLASS_DARK, CLASS_LIGHT] {
            for i in 0..set.len() {
                for j in i + 1..set.len() {
                    assert_ne!(set[i], set[j]);
                }
            }
        }
    }
}
