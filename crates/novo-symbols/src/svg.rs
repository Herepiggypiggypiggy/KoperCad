//! Renders a symbol to a standalone SVG, for previews, documentation and
//! visual regression tests. The GPU canvas draws from the same data.

use std::fmt::Write as _;

use crate::symbol::{Align, Graphic, Point, Stroke, Symbol};

/// Options for [`to_svg`].
#[derive(Debug, Clone)]
pub struct SvgOptions {
    /// Pixels per millimetre of the output's width and height.
    pub scale: f64,
    /// Stroke width in millimetres.
    pub stroke_width: f64,
    /// Margin around the artwork in millimetres.
    pub margin: f64,
    /// Draw connection points and their designations.
    pub show_connections: bool,
    /// Draw anchors with sample texts, such as `-K1` for the tag.
    pub show_anchors: bool,
}

impl Default for SvgOptions {
    fn default() -> Self {
        SvgOptions {
            scale: 6.0,
            stroke_width: 0.35,
            margin: 3.0,
            show_connections: true,
            show_anchors: true,
        }
    }
}

fn n(v: f64) -> String {
    let r = (v * 1000.0).round() / 1000.0;
    if r == 0.0 { "0".into() } else { format!("{r}") }
}

fn pt(p: Point) -> String {
    format!("{},{}", n(p[0]), n(p[1]))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn anchor_attr(align: Align) -> &'static str {
    match align {
        Align::Left => "start",
        Align::Center => "middle",
        Align::Right => "end",
    }
}

fn paint(fill: bool, stroke: Stroke) -> String {
    let mut s = String::new();
    s.push_str(if fill {
        " fill=\"currentColor\""
    } else {
        " fill=\"none\""
    });
    if stroke == Stroke::Dashed {
        s.push_str(" stroke-dasharray=\"1.2 0.8\"");
    }
    s
}

/// Renders `symbol` as an SVG document. Colours use `currentColor` so the
/// embedding page decides them.
pub fn to_svg(symbol: &Symbol, options: &SvgOptions) -> String {
    let (mut min, mut max) = symbol.bounds();
    if options.show_anchors {
        for a in &symbol.anchors {
            min = [min[0].min(a.at[0] - 8.0), min[1].min(a.at[1] - 2.0)];
            max = [max[0].max(a.at[0] + 8.0), max[1].max(a.at[1] + 2.0)];
        }
    }
    let m = options.margin;
    let (x, y) = (min[0] - m, min[1] - m);
    let (w, h) = (max[0] - min[0] + 2.0 * m, max[1] - min[1] + 2.0 * m);

    let mut s = String::new();
    let _ = writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\">",
        n(x),
        n(y),
        n(w),
        n(h),
        n(w * options.scale),
        n(h * options.scale)
    );
    let _ = writeln!(s, "<title>{}</title>", escape(&symbol.name));
    let _ = writeln!(
        s,
        "<g stroke=\"currentColor\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
        n(options.stroke_width)
    );
    for g in &symbol.graphics {
        let _ = match g {
            Graphic::Line { from, to, stroke } => writeln!(
                s,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\"{}/>",
                n(from[0]),
                n(from[1]),
                n(to[0]),
                n(to[1]),
                paint(false, *stroke)
            ),
            Graphic::Polyline {
                points,
                closed,
                fill,
                stroke,
            } => {
                let tag = if *closed { "polygon" } else { "polyline" };
                let pts: Vec<_> = points.iter().map(|p| pt(*p)).collect();
                writeln!(
                    s,
                    "<{tag} points=\"{}\"{}/>",
                    pts.join(" "),
                    paint(*fill, *stroke)
                )
            }
            Graphic::Rect {
                at,
                size,
                fill,
                stroke,
            } => writeln!(
                s,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{}/>",
                n(at[0]),
                n(at[1]),
                n(size[0]),
                n(size[1]),
                paint(*fill, *stroke)
            ),
            Graphic::Circle {
                center,
                r,
                fill,
                stroke,
            } => writeln!(
                s,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}/>",
                n(center[0]),
                n(center[1]),
                n(*r),
                paint(*fill, *stroke)
            ),
            Graphic::Ellipse {
                center,
                rx,
                ry,
                fill,
                stroke,
            } => writeln!(
                s,
                "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{}/>",
                n(center[0]),
                n(center[1]),
                n(*rx),
                n(*ry),
                paint(*fill, *stroke)
            ),
            Graphic::Arc {
                center,
                r,
                start,
                end,
                stroke,
            } => {
                let at = |deg: f64| {
                    let a = deg.to_radians();
                    [center[0] + r * a.cos(), center[1] + r * a.sin()]
                };
                let sweep = end - start;
                if sweep >= 360.0 {
                    writeln!(
                        s,
                        "<circle cx=\"{}\" cy=\"{}\" r=\"{}\"{}/>",
                        n(center[0]),
                        n(center[1]),
                        n(*r),
                        paint(false, *stroke)
                    )
                } else {
                    let (p0, p1) = (at(*start), at(*end));
                    let large = if sweep > 180.0 { 1 } else { 0 };
                    writeln!(
                        s,
                        "<path d=\"M {} A {} {} 0 {large} 1 {}\"{}/>",
                        pt(p0),
                        n(*r),
                        n(*r),
                        pt(p1),
                        paint(false, *stroke)
                    )
                }
            }
            Graphic::Text {
                at,
                text,
                size,
                align,
            } => writeln!(
                s,
                "<text x=\"{}\" y=\"{}\" font-size=\"{}\" text-anchor=\"{}\" dominant-baseline=\"central\" fill=\"currentColor\" stroke=\"none\" font-family=\"sans-serif\">{}</text>",
                n(at[0]),
                n(at[1]),
                n(*size),
                anchor_attr(*align),
                escape(text)
            ),
        };
    }
    s.push_str("</g>\n");

    if options.show_connections {
        s.push_str("<g fill=\"#d9480f\" stroke=\"none\" font-family=\"sans-serif\">\n");
        for c in &symbol.connections {
            let v = c.dir.vector();
            let label = [c.at[0] + v[0] * 1.6 + 1.2, c.at[1] + v[1] * 1.6];
            let _ = writeln!(
                s,
                "<circle cx=\"{}\" cy=\"{}\" r=\"0.6\"/><text x=\"{}\" y=\"{}\" font-size=\"1.8\" dominant-baseline=\"central\">{}</text>",
                n(c.at[0]),
                n(c.at[1]),
                n(label[0]),
                n(label[1]),
                escape(&c.id)
            );
        }
        s.push_str("</g>\n");
    }
    if options.show_anchors {
        s.push_str("<g fill=\"#1971c2\" stroke=\"none\" font-family=\"sans-serif\">\n");
        for a in &symbol.anchors {
            let sample = match a.role.as_str() {
                "tag" => "-K1",
                "address" => "%I0.0",
                "function_text" => "Text",
                _ => "…",
            };
            let _ = writeln!(
                s,
                "<text x=\"{}\" y=\"{}\" font-size=\"2.5\" text-anchor=\"{}\" dominant-baseline=\"central\">{}</text>",
                n(a.at[0]),
                n(a.at[1]),
                anchor_attr(a.align),
                sample
            );
        }
        s.push_str("</g>\n");
    }
    s.push_str("</svg>\n");
    s
}
