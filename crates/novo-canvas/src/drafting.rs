//! Drafting primitives for layout drawings (the Cabinet canvas, WS-15):
//! hatched areas and linear dimensions. They build ordinary [`Primitive`]s
//! and [`Label`]s, so they draw, pick, print and export like everything
//! else; nothing here knows about cabinets, rails or ducts.

use crate::geom::Aabb;
use crate::shape::{Primitive, Shape};
use crate::style::{Color, Stroke};
use crate::text::{Anchor, Label};
use glam::Vec2;

/// Parallel hatch lines across `area`, `spacing` mm apart at `angle`
/// radians (clockwise on screen from the +x axis; `-FRAC_PI_4` rises to
/// the right). The lines are clipped to the area and placed on a fixed
/// world pattern, so neighbouring areas line up. A line that would lie
/// on the area's edge is left out.
pub fn hatch(area: &Aabb, spacing: f32, angle: f32, stroke: Stroke) -> Vec<Primitive> {
    if area.is_empty() || spacing <= 0.0 || !spacing.is_finite() {
        return Vec::new();
    }
    let dir = Vec2::new(angle.cos(), angle.sin());
    let normal = Vec2::new(-dir.y, dir.x);
    // Offsets along the normal that the area covers.
    let corners = [
        area.min,
        Vec2::new(area.max.x, area.min.y),
        area.max,
        Vec2::new(area.min.x, area.max.y),
    ];
    let (lo, hi) = corners
        .iter()
        .map(|c| c.dot(normal))
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), d| {
            (lo.min(d), hi.max(d))
        });
    let reach = area.size().length();
    let mut out = Vec::new();
    let mut k = (lo / spacing).ceil();
    while k * spacing <= hi {
        // A line along the area's edge would only double its outline.
        if k * spacing - lo < 1e-4 || hi - k * spacing < 1e-4 {
            k += 1.0;
            continue;
        }
        let mid = normal * (k * spacing);
        // Centre the long line on the area so clipping keeps its middle.
        let along = (area.center() - mid).dot(dir);
        let p = mid + dir * along;
        if let Some((a, b)) = clip_segment(p - dir * reach, p + dir * reach, area)
            && a.distance(b) > 1e-4
        {
            out.push(Primitive::stroked(Shape::line(a, b), stroke));
        }
        k += 1.0;
    }
    out
}

/// A rectangle as layout drawings show a part or a cut-out: an optional
/// fill, hatch lines (see [`hatch`]) and an outline on top, in that order.
pub fn hatched_rect(
    area: &Aabb,
    fill: Option<Color>,
    hatching: Option<(f32, f32, Stroke)>,
    outline: Stroke,
) -> Vec<Primitive> {
    let mut out = Vec::new();
    if area.is_empty() {
        return out;
    }
    if let Some(fill) = fill {
        out.push(Primitive::filled(Shape::rect(area.min, area.max), fill));
    }
    if let Some((spacing, angle, stroke)) = hatching {
        out.extend(hatch(area, spacing, angle, stroke));
    }
    out.push(Primitive::stroked(Shape::rect(area.min, area.max), outline));
    out
}

/// The part of segment `a`-`b` inside `rect` (Liang-Barsky).
fn clip_segment(a: Vec2, b: Vec2, rect: &Aabb) -> Option<(Vec2, Vec2)> {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q) in [
        (-d.x, a.x - rect.min.x),
        (d.x, rect.max.x - a.x),
        (-d.y, a.y - rect.min.y),
        (d.y, rect.max.y - a.y),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
        }
    }
    (t0 <= t1).then(|| (a + d * t0, a + d * t1))
}

/// How dimensions are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionStyle {
    pub stroke: Stroke,
    /// Colour of the arrowheads and the text.
    pub color: Color,
    /// Text height, mm.
    pub text_height: f32,
    /// Arrowhead length, mm; its width is a third of that.
    pub arrow: f32,
    /// Gap between the measured point and its extension line, mm.
    pub gap: f32,
    /// How far extension lines run past the dimension line, mm.
    pub overshoot: f32,
}

impl Default for DimensionStyle {
    fn default() -> Self {
        Self {
            stroke: Stroke::world(0.18, Color::BLACK),
            color: Color::BLACK,
            text_height: 2.5,
            arrow: 2.5,
            gap: 1.0,
            overshoot: 1.5,
        }
    }
}

/// A linear dimension between two points, measured along `from`-`to`.
#[derive(Clone, Debug, PartialEq)]
pub struct Dimension {
    pub from: Vec2,
    pub to: Vec2,
    /// Distance of the dimension line from the measured points, mm, to the
    /// left of `from`-`to` as seen on screen (y down); negative puts it on
    /// the other side.
    pub offset: f32,
    /// The text; `None` prints the measured length in mm.
    pub text: Option<String>,
}

impl Dimension {
    pub fn new(from: impl Into<Vec2>, to: impl Into<Vec2>, offset: f32) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            offset,
            text: None,
        }
    }

    /// Distance between the measured points, mm.
    pub fn length(&self) -> f32 {
        self.from.distance(self.to)
    }

    /// The printed text: the given one, or the length rounded to 0.1 mm
    /// without a trailing `.0`.
    pub fn text(&self) -> String {
        if let Some(t) = &self.text {
            return t.clone();
        }
        let tenths = (self.length() * 10.0).round() / 10.0;
        if tenths.fract().abs() < 1e-3 {
            format!("{}", tenths as i64)
        } else {
            format!("{tenths:.1}")
        }
    }

    /// Extension lines, the dimension line with two arrowheads, and the
    /// text centred above the line, rotated with it and always readable
    /// (left to right, or bottom to top). Nothing for coincident points.
    pub fn build(&self, style: &DimensionStyle) -> (Vec<Primitive>, Option<Label>) {
        let len = self.length();
        if len < 1e-4 {
            return (Vec::new(), None);
        }
        let dir = (self.to - self.from) / len;
        // Left of the direction on a y-down screen.
        let left = Vec2::new(dir.y, -dir.x);
        let off = left * self.offset;
        let side = if self.offset >= 0.0 { left } else { -left };
        let (a, b) = (self.from + off, self.to + off);
        let mut prims = Vec::new();
        if self.offset.abs() > style.gap {
            for p in [self.from, self.to] {
                prims.push(Primitive::stroked(
                    Shape::line(p + side * style.gap, p + off + side * style.overshoot),
                    style.stroke,
                ));
            }
        }
        prims.push(Primitive::stroked(Shape::line(a, b), style.stroke));
        // Arrowheads point outwards to the extension lines; on a dimension
        // too short for both, they sit outside instead.
        let inside = len >= 2.5 * style.arrow;
        let half = style.arrow / 6.0;
        for (tip, out) in [(a, -dir), (b, dir)] {
            let back = if inside { -out } else { out };
            let base = tip + back * style.arrow;
            let w = Vec2::new(-out.y, out.x) * half;
            prims.push(Primitive::filled(
                Shape::polyline([tip, base + w, base - w], true),
                style.color,
            ));
        }
        // Text above the line (away from the measured points), reading
        // left to right or bottom to top.
        let mut angle = dir.y.atan2(dir.x);
        let mut up = side;
        if dir.x < -1e-4 || (dir.x.abs() <= 1e-4 && dir.y > 0.0) {
            angle += std::f32::consts::PI;
        }
        if angle > std::f32::consts::PI {
            angle -= std::f32::consts::TAU;
        }
        if up.length_squared() < 1e-12 {
            up = Vec2::new(0.0, -1.0);
        }
        let mid = (a + b) * 0.5 + up * (style.text_height * 0.3);
        // Bottom centre on the line when `up` is the text's own up,
        // top centre otherwise.
        let text_up = Vec2::new(angle.sin(), -angle.cos());
        let anchor = if text_up.dot(up) >= 0.0 {
            Anchor::BottomCenter
        } else {
            Anchor::TopCenter
        };
        let label = Label::new(mid, self.text(), style.text_height)
            .anchored(anchor)
            .rotated(angle)
            .colored(style.color);
        (prims, Some(label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke() -> Stroke {
        Stroke::world(0.1, Color::BLACK)
    }

    fn ends(p: &Primitive) -> (Vec2, Vec2) {
        match &p.shape {
            Shape::Line { a, b } => (*a, *b),
            other => panic!("not a line: {other:?}"),
        }
    }

    #[test]
    fn hatch_lines_stay_inside_and_follow_a_world_pattern() {
        let area = Aabb::new(Vec2::new(0.0, 0.0), Vec2::new(40.0, 20.0));
        let lines = hatch(&area, 5.0, -std::f32::consts::FRAC_PI_4, stroke());
        assert!(lines.len() > 5);
        let grown = area.expand(1e-3);
        for l in &lines {
            let (a, b) = ends(l);
            assert!(grown.contains_point(a) && grown.contains_point(b));
            // 45 degrees, rising to the right.
            let d = (b - a).normalize();
            assert!((d.x.abs() - d.y.abs()).abs() < 1e-3 && d.x * d.y < 0.0);
        }
        // A neighbouring area continues the same lines.
        let next = Aabb::new(Vec2::new(40.0, 0.0), Vec2::new(80.0, 20.0));
        let normal = Vec2::new(1.0, 1.0).normalize();
        let offsets = |ls: &[Primitive]| -> Vec<f32> {
            ls.iter().map(|l| ends(l).0.dot(normal) / 5.0).collect()
        };
        for o in offsets(&lines).into_iter().chain(offsets(&hatch(
            &next,
            5.0,
            -std::f32::consts::FRAC_PI_4,
            stroke(),
        ))) {
            assert!((o - o.round()).abs() < 1e-3, "{o}");
        }
        assert!(hatch(&Aabb::EMPTY, 5.0, 0.0, stroke()).is_empty());
        assert!(hatch(&area, 0.0, 0.0, stroke()).is_empty());
    }

    #[test]
    fn horizontal_hatch_spaces_evenly() {
        let area = Aabb::new(Vec2::new(0.0, 0.5), Vec2::new(10.0, 10.5));
        let lines = hatch(&area, 2.0, 0.0, stroke());
        let ys: Vec<f32> = lines.iter().map(|l| ends(l).0.y).collect();
        assert_eq!(ys.len(), 5);
        for (i, y) in ys.iter().enumerate() {
            assert!((y - (2.0 + 2.0 * i as f32)).abs() < 1e-4, "{ys:?}");
        }
    }

    #[test]
    fn dimension_text_is_the_rounded_length() {
        assert_eq!(Dimension::new((0.0, 0.0), (125.0, 0.0), 10.0).text(), "125");
        assert_eq!(
            Dimension::new((0.0, 0.0), (12.25, 0.0), 10.0).text(),
            "12.3"
        );
        let mut d = Dimension::new((0.0, 0.0), (3.0, 4.0), 10.0);
        assert_eq!(d.text(), "5");
        d.text = Some("2x 35".into());
        assert_eq!(d.text(), "2x 35");
    }

    #[test]
    fn horizontal_dimension_above_its_points() {
        let style = DimensionStyle::default();
        // Left to right with a positive offset: the line sits above (y up
        // on paper is y down here, so a smaller y).
        let d = Dimension::new((0.0, 100.0), (60.0, 100.0), 10.0);
        let (prims, label) = d.build(&style);
        // Two extension lines, the dimension line, two arrowheads.
        assert_eq!(prims.len(), 5);
        let (a, b) = ends(&prims[2]);
        assert_eq!((a, b), (Vec2::new(0.0, 90.0), Vec2::new(60.0, 90.0)));
        let (e0, e1) = ends(&prims[0]);
        assert!((e0.y - (100.0 - style.gap)).abs() < 1e-4);
        assert!((e1.y - (90.0 - style.overshoot)).abs() < 1e-4);
        let label = label.unwrap();
        assert_eq!(label.text, "60");
        assert_eq!(label.anchor, Anchor::BottomCenter);
        assert!(label.rotation.abs() < 1e-5);
        assert!(label.pos.y < 90.0 && (label.pos.x - 30.0).abs() < 1e-4);
        // Arrowheads point outwards: their tips are the line's ends.
        for (p, tip) in prims[3..].iter().zip([a, b]) {
            let Shape::Polyline { points, closed } = &p.shape else {
                panic!("arrowhead")
            };
            assert!(*closed && points[0] == tip);
            assert!(p.fill.is_some());
        }
    }

    #[test]
    fn dimension_text_always_reads_upright() {
        let style = DimensionStyle::default();
        // Measured right to left: same text angle as left to right.
        let (_, l) = Dimension::new((60.0, 0.0), (0.0, 0.0), -10.0).build(&style);
        assert!(l.unwrap().rotation.abs() < 1e-5);
        // Vertical, either way: reads bottom to top.
        for (from, to) in [((0.0, 0.0), (0.0, 50.0)), ((0.0, 50.0), (0.0, 0.0))] {
            let (_, l) = Dimension::new(from, to, 10.0).build(&style);
            let r = l.unwrap().rotation;
            assert!(
                (r + std::f32::consts::FRAC_PI_2).abs() < 1e-4,
                "{from:?}->{to:?}: {r}"
            );
        }
    }

    #[test]
    fn short_dimension_puts_arrows_outside_and_empty_draws_nothing() {
        let style = DimensionStyle::default();
        let (prims, _) = Dimension::new((0.0, 0.0), (4.0, 0.0), 8.0).build(&style);
        let Shape::Polyline { points, .. } = &prims[3].shape else {
            panic!("arrowhead")
        };
        // The left arrow's base is left of its tip: it sits outside and
        // points inwards.
        assert!(points[1].x < points[0].x);
        let (p, l) = Dimension::new((1.0, 1.0), (1.0, 1.0), 8.0).build(&style);
        assert!(p.is_empty() && l.is_none());
    }

    #[test]
    fn hatched_rect_stacks_fill_hatch_and_outline() {
        let area = Aabb::new(Vec2::ZERO, Vec2::new(20.0, 10.0));
        let grey = Color::rgb(200, 200, 200);
        let p = hatched_rect(&area, Some(grey), Some((4.0, 0.0, stroke())), stroke());
        assert_eq!(p.first().unwrap().fill, Some(grey));
        assert!(matches!(p.last().unwrap().shape, Shape::Rect { .. }));
        assert!(p.last().unwrap().fill.is_none());
        assert_eq!(p.len(), 1 + 2 + 1);
        assert_eq!(hatched_rect(&area, None, None, stroke()).len(), 1);
        assert!(hatched_rect(&Aabb::EMPTY, Some(grey), None, stroke()).is_empty());
    }
}
