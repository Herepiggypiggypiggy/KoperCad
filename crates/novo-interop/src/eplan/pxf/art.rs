//! The drawing of a library symbol variant: the plain shapes (`O31` line,
//! `O89` rectangle, `O32` arc, `O34` polygon, `O30` text) inside an `O45`,
//! converted to KoperCAD symbol graphics.
//!
//! EPLAN is y up with counter-clockwise angles; a KoperCAD symbol is y down
//! with angles that run clockwise on screen, so y flips and an arc from
//! `a0` to `a1` becomes the arc from `-a1` to `-a0`. All positions are
//! millimetres from the symbol's insertion point and are kept exactly.

use std::f64::consts::TAU;

use novo_symbols::{Align, Graphic, Stroke};

use super::library::{finite, parse_point};

/// An arc as the file writes it: the centre (`A555`), a vector from the
/// centre (`A556`) whose length is the radius and whose direction is where
/// the angles start, and the start and end angle (`A553`, `A554`, radians,
/// counter-clockwise) from that direction.
pub(super) fn arc_angles(radius_vec: (f64, f64), a0: f64, a1: f64) -> Option<(f64, f64, f64)> {
    let r = radius_vec.0.hypot(radius_vec.1);
    if r <= 0.0 {
        return None;
    }
    let base = radius_vec.1.atan2(radius_vec.0);
    let (s, mut e) = (base + a0, base + a1);
    if e <= s {
        e += TAU;
    }
    // A full turn written as 0 to 2 pi stays one.
    if (a1 - a0).abs() >= TAU - 1e-6 {
        e = s + TAU;
    }
    Some((r, s, e))
}

/// A shape's element (`O31`...) as KoperCAD graphics. `get` reads an
/// attribute of the object, `text_size` is the character height a text row
/// states, when it does.
pub(super) fn graphic(
    kind: u32,
    get: &dyn Fn(&str) -> Option<String>,
    text_size: Option<f64>,
) -> Option<Graphic> {
    let at = |n: &str| get(n).and_then(|v| parse_point(&v));
    let flip = |p: (f64, f64)| [p.0, -p.1];
    match kind {
        31 => Some(Graphic::Line {
            from: flip(at("A531")?),
            to: flip(at("A532")?),
            stroke: Stroke::Solid,
        }),
        89 => {
            let (a, b) = (flip(at("A1651")?), flip(at("A1652")?));
            Some(Graphic::Rect {
                at: [a[0].min(b[0]), a[1].min(b[1])],
                size: [(a[0] - b[0]).abs(), (a[1] - b[1]).abs()],
                fill: false,
                stroke: Stroke::Solid,
            })
        }
        32 => {
            let c = flip(at("A555")?);
            let v = at("A556")?;
            let a0 = get("A553").and_then(|s| finite(&s))?;
            let a1 = get("A554").and_then(|s| finite(&s))?;
            let ratio = get("A557").and_then(|s| finite(&s)).unwrap_or(1.0);
            let (r, s, e) = arc_angles(v, a0, a1)?;
            let full = (e - s) >= TAU - 1e-6;
            if (ratio - 1.0).abs() > 1e-6 && ratio > 0.0 {
                // An ellipse arc: sampled, the one case without a primitive.
                let n = 48;
                let base = v.1.atan2(v.0);
                let (cb, sb) = (base.cos(), base.sin());
                let pts = (0..=n)
                    .map(|i| {
                        let u = a0 + (e - s) * f64::from(i) / f64::from(n);
                        let (x, y) = (r * u.cos(), r * ratio * u.sin());
                        [c[0] + x * cb - y * sb, c[1] - (x * sb + y * cb)]
                    })
                    .collect();
                return Some(Graphic::Polyline {
                    points: pts,
                    closed: full,
                    fill: false,
                    stroke: Stroke::Solid,
                });
            }
            if full {
                Some(Graphic::Circle {
                    center: c,
                    r,
                    fill: false,
                    stroke: Stroke::Solid,
                })
            } else {
                Some(Graphic::Arc {
                    center: c,
                    r,
                    start: (-e).to_degrees(),
                    end: (-s).to_degrees(),
                    stroke: Stroke::Solid,
                })
            }
        }
        34 => {
            let pts: Vec<[f64; 2]> = get("A621")?
                .split(';')
                .filter_map(parse_point)
                .map(flip)
                .collect();
            let filled = get("A623").is_some_and(|v| v.trim() != "0");
            // A filled shape that closes on itself is a closed polygon.
            let closed = filled || pts.first() == pts.last();
            (pts.len() >= 2).then_some(Graphic::Polyline {
                points: pts,
                closed,
                fill: filled,
                stroke: Stroke::Solid,
            })
        }
        30 => {
            let raw = get("A511")?;
            // Placeholders for what the placement fills in (`??_??@3;`) are
            // not artwork.
            if raw.contains("??") || raw.trim().is_empty() {
                return None;
            }
            let text = super::text::MlText::parse(&raw).get("en_US").to_string();
            if text.trim().is_empty() {
                return None;
            }
            let size = text_size.filter(|s| *s > 0.0).unwrap_or(2.5);
            let p = at("A501")?;
            Some(Graphic::Text {
                at: [p.0, -p.1 - size / 2.0],
                text,
                size,
                align: Align::Left,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attrs<'a>(list: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |n| {
            list.iter()
                .find(|(k, _)| *k == n)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    fn a_line_flips_y() {
        let g = graphic(31, &attrs(&[("A531", "0/1"), ("A532", "4/-2")]), None).unwrap();
        assert_eq!(
            g,
            Graphic::Line {
                from: [0.0, -1.0],
                to: [4.0, 2.0],
                stroke: Stroke::Solid
            }
        );
    }

    #[test]
    fn a_full_turn_is_a_circle_whatever_the_radius_vector_points_to() {
        let g = graphic(
            32,
            &attrs(&[
                ("A553", "0"),
                ("A554", "6.28318530717959"),
                ("A555", "5/2"),
                ("A556", "0/0.5"),
                ("A557", "1"),
            ]),
            None,
        )
        .unwrap();
        assert_eq!(
            g,
            Graphic::Circle {
                center: [5.0, -2.0],
                r: 0.5,
                fill: false,
                stroke: Stroke::Solid
            }
        );
    }

    #[test]
    fn an_upper_half_arc_becomes_a_screen_upper_half_arc() {
        // EPLAN 0..pi is the upper half (y up). On screen (y down) the upper
        // half runs from 180 to 360 degrees.
        let g = graphic(
            32,
            &attrs(&[
                ("A553", "0"),
                ("A554", "3.14159265358979"),
                ("A555", "0/0"),
                ("A556", "2/0"),
                ("A557", "1"),
            ]),
            None,
        )
        .unwrap();
        let Graphic::Arc { r, start, end, .. } = g else {
            panic!("an arc")
        };
        assert!((r - 2.0).abs() < 1e-9);
        assert!(
            (start - -180.0).abs() < 1e-6 && end.abs() < 1e-6,
            "{start} {end}"
        );
    }

    #[test]
    fn an_arc_that_wraps_past_zero_keeps_its_direction() {
        // 3.665 rad (210 deg) to 0: the lower half, counter-clockwise.
        let (r, s, e) = arc_angles((1.0, 0.0), 3.66519142918809, 0.0).unwrap();
        assert_eq!(r, 1.0);
        assert!((e - s - 150f64.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn placeholder_text_is_not_artwork_but_fixed_text_is() {
        assert!(graphic(30, &attrs(&[("A501", "0/0"), ("A511", "??_??@3;")]), None).is_none());
        let t = graphic(30, &attrs(&[("A501", "1/2"), ("A511", "M")]), Some(2.0));
        assert!(matches!(t, Some(Graphic::Text { text, .. }) if text == "M"));
    }
}
