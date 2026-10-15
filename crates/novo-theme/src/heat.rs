//! Colours of the heat view: the "ember" temperature ramp, a diverging ramp
//! for differences and the status mapping of device badges.
//!
//! Plain `[u8; 3]` sRGB so the pure `novo-heatview` engine and the renderers
//! can share it without a UI dependency. Spec: `docs/design/thermal-visuals.md`.

/// Ten stops of the temperature ramp, cold to hot. Lightness rises
/// monotonically, so the picture survives greyscale and common colour-vision
/// deficiencies; the first stop is lifted off the dark surface, the last is
/// kept off pure white.
pub const RAMP: [[u8; 3]; 10] = [
    [0x1F, 0x0C, 0x48],
    [0x3B, 0x0F, 0x70],
    [0x5F, 0x18, 0x7F],
    [0x8C, 0x29, 0x81],
    [0xB6, 0x36, 0x79],
    [0xDE, 0x49, 0x68],
    [0xF7, 0x70, 0x5C],
    [0xFE, 0x9F, 0x6D],
    [0xFE, 0xCF, 0x92],
    [0xFC, 0xFD, 0xBF],
];

/// Diverging ramp for "B minus A": cool steel, a neutral middle, ember.
pub const DIVERGING: [[u8; 3]; 5] = [
    [0x3E, 0x6F, 0xA8],
    [0x8F, 0xA8, 0xC8],
    [0x5A, 0x58, 0x55],
    [0xF7, 0x70, 0x5C],
    [0xFC, 0xFD, 0xBF],
];

/// Entries in a lookup table.
pub const LUT_SIZE: usize = 256;

fn lerp_stops(stops: &[[u8; 3]], t: f32) -> [u8; 3] {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let x = t * (stops.len() - 1) as f32;
    let i = (x.floor() as usize).min(stops.len() - 2);
    let f = x - i as f32;
    let mut out = [0u8; 3];
    for (k, o) in out.iter_mut().enumerate() {
        let (a, b) = (stops[i][k] as f32, stops[i + 1][k] as f32);
        *o = (a + (b - a) * f).round() as u8;
    }
    out
}

/// Temperature colour at `t` in 0 (coldest) to 1 (hottest); NaN is cold.
pub fn ramp(t: f32) -> [u8; 3] {
    lerp_stops(&RAMP, t)
}

/// Difference colour at `t` in 0 (most negative) to 1 (most positive); 0.5
/// is no change.
pub fn diverging(t: f32) -> [u8; 3] {
    lerp_stops(&DIVERGING, t)
}

/// The temperature ramp as a [`LUT_SIZE`] entry lookup table.
pub fn ramp_lut() -> Vec<[u8; 3]> {
    (0..LUT_SIZE)
        .map(|i| ramp(i as f32 / (LUT_SIZE - 1) as f32))
        .collect()
}

/// Verdict of a device against its limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// No limit known: neutral dot, no judgement.
    Unknown,
    Ok,
    Near,
    Over,
}

/// Verdict from the share of the allowed temperature rise used: below 80%
/// is fine, up to 100% is near, above is over. `ambient`, `value` and
/// `limit` in degrees C.
pub fn verdict(ambient: f64, value: f64, limit: Option<f64>) -> Verdict {
    let Some(limit) = limit else {
        return Verdict::Unknown;
    };
    let allowed = limit - ambient;
    if allowed <= 0.0 {
        return if value > limit {
            Verdict::Over
        } else {
            Verdict::Near
        };
    }
    let share = (value - ambient) / allowed;
    if share > 1.0 {
        Verdict::Over
    } else if share >= 0.8 {
        Verdict::Near
    } else {
        Verdict::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luma(c: [u8; 3]) -> f32 {
        let lin = |v: u8| {
            let s = v as f32 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
    }

    #[test]
    fn ramp_lightness_rises_monotonically() {
        let lut = ramp_lut();
        for w in lut.windows(2) {
            assert!(
                luma(w[1]) >= luma(w[0]) - 1e-4,
                "{:?} then {:?}",
                w[0],
                w[1]
            );
        }
        assert!(luma(lut[255]) > 4.0 * luma(lut[0]));
    }

    #[test]
    fn ramp_ends_and_clamps() {
        assert_eq!(ramp(0.0), RAMP[0]);
        assert_eq!(ramp(1.0), RAMP[9]);
        assert_eq!(ramp(-3.0), RAMP[0]);
        assert_eq!(ramp(9.0), RAMP[9]);
        assert_eq!(ramp(f32::NAN), RAMP[0]);
    }

    #[test]
    fn ramp_is_not_the_accent() {
        let accent = [0xC8, 0x8A, 0x5E];
        let near = |a: [u8; 3], b: [u8; 3]| {
            a.iter()
                .zip(b)
                .map(|(x, y)| (*x as i32 - y as i32).abs())
                .sum::<i32>()
                < 20
        };
        assert!(ramp_lut().iter().all(|c| !near(*c, accent)));
    }

    #[test]
    fn diverging_middle_is_neutral() {
        let m = diverging(0.5);
        assert!(m.iter().max().unwrap() - m.iter().min().unwrap() < 12);
        assert_eq!(diverging(0.0), DIVERGING[0]);
    }

    #[test]
    fn verdicts_follow_the_share_of_allowed_rise() {
        assert_eq!(verdict(35.0, 40.0, None), Verdict::Unknown);
        assert_eq!(verdict(35.0, 55.0, Some(70.0)), Verdict::Ok); // 57%
        assert_eq!(verdict(35.0, 66.0, Some(70.0)), Verdict::Near); // 89%
        assert_eq!(verdict(35.0, 70.0, Some(70.0)), Verdict::Near); // exactly 100%
        assert_eq!(verdict(35.0, 71.0, Some(70.0)), Verdict::Over);
        assert_eq!(verdict(75.0, 71.0, Some(70.0)), Verdict::Over); // limit below ambient
    }
}
