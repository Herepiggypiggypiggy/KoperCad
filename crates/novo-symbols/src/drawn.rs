//! Artwork carried by a placed symbol instead of by the library.
//!
//! An importer that knows how the source program drew a symbol (an EPLAN
//! project export embeds every symbol it uses) stores that drawing on the
//! placed symbol as the [`ART_PROPERTY`] property, as the compact text
//! [`encode`] writes. The drawing then travels with the project, undoes and
//! diffs like any property, and needs no library entry on another machine.
//!
//! Coordinates are the library's: millimetres from the symbol origin, y
//! down, arc angles in degrees running clockwise on screen. Text is one line
//! of fixed artwork text. Stroke width stays the renderer's choice.

use crate::symbol::{Align, Graphic, Stroke};

/// The instance property that holds the drawing.
pub const ART_PROPERTY: &str = "art";

/// Most shapes one symbol drawing may hold; a longer one is refused.
pub const MAX_SHAPES: usize = 4000;
/// Longest drawing text that is read.
pub const MAX_LEN: usize = 400_000;

fn num(v: f64) -> String {
    let r = (v * 10_000.0).round() / 10_000.0;
    let r = if r == 0.0 { 0.0 } else { r };
    format!("{r}")
}

/// The drawing as text: one shape per `|`, fields separated by `,`.
/// `L x1 y1 x2 y2`, `D` is a dashed line, `P closed fill x y ...`,
/// `C x y r fill`, `E x y rx ry fill`, `A x y r start end`, `T x y size align text`.
/// Rectangles are written as closed polylines.
pub fn encode(graphics: &[Graphic]) -> String {
    let mut out = Vec::with_capacity(graphics.len());
    for g in graphics {
        let s = match g {
            Graphic::Line { from, to, stroke } => format!(
                "{},{},{},{},{}",
                if *stroke == Stroke::Dashed { 'D' } else { 'L' },
                num(from[0]),
                num(from[1]),
                num(to[0]),
                num(to[1])
            ),
            Graphic::Polyline {
                points,
                closed,
                fill,
                ..
            } => {
                let mut s = format!("P,{},{}", u8::from(*closed), u8::from(*fill));
                for p in points {
                    s += &format!(",{},{}", num(p[0]), num(p[1]));
                }
                s
            }
            Graphic::Rect { at, size, fill, .. } => format!(
                "P,1,{},{},{},{},{},{},{},{},{}",
                u8::from(*fill),
                num(at[0]),
                num(at[1]),
                num(at[0] + size[0]),
                num(at[1]),
                num(at[0] + size[0]),
                num(at[1] + size[1]),
                num(at[0]),
                num(at[1] + size[1]),
            ),
            Graphic::Circle {
                center, r, fill, ..
            } => format!(
                "C,{},{},{},{}",
                num(center[0]),
                num(center[1]),
                num(*r),
                u8::from(*fill)
            ),
            Graphic::Ellipse {
                center,
                rx,
                ry,
                fill,
                ..
            } => format!(
                "E,{},{},{},{},{}",
                num(center[0]),
                num(center[1]),
                num(*rx),
                num(*ry),
                u8::from(*fill)
            ),
            Graphic::Arc {
                center,
                r,
                start,
                end,
                ..
            } => format!(
                "A,{},{},{},{},{}",
                num(center[0]),
                num(center[1]),
                num(*r),
                num(*start),
                num(*end)
            ),
            Graphic::Text {
                at,
                text,
                size,
                align,
            } => format!(
                "T,{},{},{},{},{}",
                num(at[0]),
                num(at[1]),
                num(*size),
                match align {
                    Align::Left => 'l',
                    Align::Center => 'c',
                    Align::Right => 'r',
                },
                text.replace(['|', '\n', '\r'], " ")
            ),
        };
        out.push(s);
    }
    out.join("|")
}

/// Reads what [`encode`] wrote. A damaged shape is skipped; `None` when the
/// text is too long or holds too many shapes to be a symbol drawing.
pub fn decode(text: &str) -> Option<Vec<Graphic>> {
    if text.len() > MAX_LEN {
        return None;
    }
    let mut out = Vec::new();
    for shape in text.split('|').filter(|s| !s.is_empty()) {
        if out.len() >= MAX_SHAPES {
            return None;
        }
        if let Some(g) = decode_shape(shape) {
            out.push(g);
        }
    }
    Some(out)
}

fn decode_shape(s: &str) -> Option<Graphic> {
    let (tag, rest) = s.split_once(',')?;
    let f = |v: &str| v.trim().parse::<f64>().ok().filter(|x| x.is_finite());
    if tag == "T" {
        let mut p = rest.splitn(5, ',');
        let (x, y, size) = (f(p.next()?)?, f(p.next()?)?, f(p.next()?)?);
        let align = match p.next()? {
            "l" => Align::Left,
            "r" => Align::Right,
            _ => Align::Center,
        };
        return Some(Graphic::Text {
            at: [x, y],
            text: p.next()?.to_string(),
            size,
            align,
        });
    }
    let n: Vec<f64> = rest.split(',').map(f).collect::<Option<_>>()?;
    match tag {
        "L" | "D" if n.len() == 4 => Some(Graphic::Line {
            from: [n[0], n[1]],
            to: [n[2], n[3]],
            stroke: if tag == "D" {
                Stroke::Dashed
            } else {
                Stroke::Solid
            },
        }),
        "P" if n.len() >= 6 && n.len().is_multiple_of(2) => Some(Graphic::Polyline {
            closed: n[0] != 0.0,
            fill: n[1] != 0.0,
            points: n[2..].chunks(2).map(|c| [c[0], c[1]]).collect(),
            stroke: Stroke::Solid,
        }),
        "C" if n.len() == 4 => Some(Graphic::Circle {
            center: [n[0], n[1]],
            r: n[2],
            fill: n[3] != 0.0,
            stroke: Stroke::Solid,
        }),
        "E" if n.len() == 5 => Some(Graphic::Ellipse {
            center: [n[0], n[1]],
            rx: n[2],
            ry: n[3],
            fill: n[4] != 0.0,
            stroke: Stroke::Solid,
        }),
        "A" if n.len() == 5 => Some(Graphic::Arc {
            center: [n[0], n[1]],
            r: n[2],
            start: n[3],
            end: n[4],
            stroke: Stroke::Solid,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drawing_round_trips() {
        let g = vec![
            Graphic::Line {
                from: [0.0, 0.0],
                to: [4.0, -2.5],
                stroke: Stroke::Solid,
            },
            Graphic::Line {
                from: [1.0, 1.0],
                to: [2.0, 1.0],
                stroke: Stroke::Dashed,
            },
            Graphic::Polyline {
                points: vec![[0.0, 0.0], [8.0, 0.0], [8.0, 2.0]],
                closed: true,
                fill: true,
                stroke: Stroke::Solid,
            },
            Graphic::Circle {
                center: [1.0, 2.0],
                r: 3.0,
                fill: false,
                stroke: Stroke::Solid,
            },
            Graphic::Arc {
                center: [0.0, 0.0],
                r: 2.0,
                start: 30.0,
                end: 210.0,
                stroke: Stroke::Solid,
            },
            Graphic::Text {
                at: [1.0, 1.0],
                text: "3~, M".into(),
                size: 2.5,
                align: Align::Center,
            },
        ];
        assert_eq!(decode(&encode(&g)).unwrap(), g);
    }

    #[test]
    fn a_rectangle_comes_back_as_a_closed_polyline() {
        let r = Graphic::Rect {
            at: [0.0, 0.0],
            size: [4.0, 2.0],
            fill: false,
            stroke: Stroke::Solid,
        };
        let back = decode(&encode(&[r])).unwrap();
        assert_eq!(
            back,
            vec![Graphic::Polyline {
                points: vec![[0.0, 0.0], [4.0, 0.0], [4.0, 2.0], [0.0, 2.0]],
                closed: true,
                fill: false,
                stroke: Stroke::Solid,
            }]
        );
    }

    #[test]
    fn damaged_text_is_skipped_and_huge_text_refused() {
        assert_eq!(decode("L,1,2|X,1|L,nan,0,0,0|L,0,0,1,1").unwrap().len(), 1);
        assert!(decode(&"L,0,0,1,1|".repeat(MAX_SHAPES + 1)).is_none());
        assert!(decode(&"x".repeat(MAX_LEN + 1)).is_none());
    }
}
