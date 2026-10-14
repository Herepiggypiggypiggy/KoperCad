//! Draw primitives: the vocabulary the model (or anything else) uses to put
//! geometry on the canvas.

use crate::geom::{self, Aabb};
use crate::style::{Color, Stroke};
use glam::{Affine2, Vec2};
use std::f32::consts::TAU;

/// Pure geometry, in world units.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Line {
        a: Vec2,
        b: Vec2,
    },
    Polyline {
        points: Vec<Vec2>,
        closed: bool,
    },
    /// Axis-aligned rectangle.
    Rect {
        min: Vec2,
        max: Vec2,
    },
    Circle {
        center: Vec2,
        radius: f32,
    },
    /// Circular arc from `start` sweeping `sweep` radians. Angles are measured
    /// from +x towards +y, which is clockwise on screen since y points down.
    Arc {
        center: Vec2,
        radius: f32,
        start: f32,
        sweep: f32,
    },
}

impl Shape {
    pub fn line(a: impl Into<Vec2>, b: impl Into<Vec2>) -> Self {
        Shape::Line {
            a: a.into(),
            b: b.into(),
        }
    }

    pub fn rect(a: impl Into<Vec2>, b: impl Into<Vec2>) -> Self {
        let r = Aabb::from_corners(a.into(), b.into());
        Shape::Rect {
            min: r.min,
            max: r.max,
        }
    }

    pub fn circle(center: impl Into<Vec2>, radius: f32) -> Self {
        Shape::Circle {
            center: center.into(),
            radius,
        }
    }

    pub fn polyline(points: impl IntoIterator<Item = Vec2>, closed: bool) -> Self {
        Shape::Polyline {
            points: points.into_iter().collect(),
            closed,
        }
    }

    /// Bounds of the geometry itself, ignoring stroke width.
    pub fn bounds(&self) -> Aabb {
        match self {
            Shape::Line { a, b } => Aabb::from_corners(*a, *b),
            Shape::Polyline { points, .. } => Aabb::from_points(points.iter().copied()),
            Shape::Rect { min, max } => Aabb::new(*min, *max),
            Shape::Circle { center, radius } => Aabb::from_center(*center, Vec2::splat(*radius)),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                let mut b = Aabb::from_points([
                    geom::point_on_circle(*center, *radius, *start),
                    geom::point_on_circle(*center, *radius, start + sweep),
                ]);
                for k in 0..4 {
                    let a = k as f32 * TAU / 4.0;
                    if geom::angle_on_arc(a, *start, *sweep) {
                        b.include_point(geom::point_on_circle(*center, *radius, a));
                    }
                }
                b
            }
        }
    }

    /// Outline as a list of point runs (each run is an open polyline). Curves
    /// are flattened to within `tolerance` world units.
    pub fn outline(&self, tolerance: f32) -> Vec<Vec<Vec2>> {
        match self {
            Shape::Line { a, b } => vec![vec![*a, *b]],
            Shape::Polyline { points, closed } => {
                let mut p = points.clone();
                if *closed && p.len() > 2 {
                    p.push(p[0]);
                }
                vec![p]
            }
            Shape::Rect { min, max } => vec![vec![
                *min,
                Vec2::new(max.x, min.y),
                *max,
                Vec2::new(min.x, max.y),
                *min,
            ]],
            Shape::Circle { center, radius } => {
                vec![geom::arc_points(*center, *radius, 0.0, TAU, tolerance)]
            }
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => vec![geom::arc_points(
                *center, *radius, *start, *sweep, tolerance,
            )],
        }
    }

    /// Closed area of the shape, if it has one (used for fills and for
    /// picking inside filled shapes).
    pub fn area_polygon(&self, tolerance: f32) -> Option<Vec<Vec2>> {
        match self {
            Shape::Rect { min, max } => Some(vec![
                *min,
                Vec2::new(max.x, min.y),
                *max,
                Vec2::new(min.x, max.y),
            ]),
            Shape::Circle { center, radius } => {
                let mut p = geom::arc_points(*center, *radius, 0.0, TAU, tolerance);
                p.pop();
                Some(p)
            }
            Shape::Polyline {
                points,
                closed: true,
            } if points.len() >= 3 => Some(points.clone()),
            _ => None,
        }
    }

    /// Distance from `p` to the outline of the shape.
    pub fn distance_to_outline(&self, p: Vec2) -> f32 {
        match self {
            Shape::Line { a, b } => geom::dist_point_segment(p, *a, *b),
            Shape::Circle { center, radius } => (p.distance(*center) - radius).abs(),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => geom::dist_point_arc(p, *center, *radius, *start, *sweep),
            _ => self
                .outline(0.01)
                .iter()
                .flat_map(|run| run.windows(2))
                .map(|w| geom::dist_point_segment(p, w[0], w[1]))
                .fold(f32::INFINITY, f32::min),
        }
    }

    /// True when the outline touches `rect`.
    pub fn outline_intersects(&self, rect: &Aabb) -> bool {
        if !self.bounds().intersects(rect) {
            return false;
        }
        match self {
            Shape::Line { a, b } => geom::segment_intersects_aabb(*a, *b, rect),
            Shape::Circle { center, radius } => {
                geom::circle_outline_intersects_aabb(*center, *radius, rect)
            }
            _ => self.outline(0.01).iter().any(|run| {
                run.windows(2)
                    .any(|w| geom::segment_intersects_aabb(w[0], w[1], rect))
            }),
        }
    }

    /// Characteristic points for object snapping.
    pub fn snap_points(&self, out: &mut Vec<(Vec2, crate::snap::SnapKind)>) {
        use crate::snap::SnapKind::*;
        match self {
            Shape::Line { a, b } => {
                out.push((*a, Endpoint));
                out.push((*b, Endpoint));
                out.push(((*a + *b) * 0.5, Midpoint));
            }
            Shape::Polyline { points, closed } => {
                out.extend(points.iter().map(|p| (*p, Endpoint)));
                for w in points.windows(2) {
                    out.push(((w[0] + w[1]) * 0.5, Midpoint));
                }
                if *closed && points.len() > 2 {
                    out.push(((points[0] + points[points.len() - 1]) * 0.5, Midpoint));
                }
            }
            Shape::Rect { min, max } => {
                let c = [*min, Vec2::new(max.x, min.y), *max, Vec2::new(min.x, max.y)];
                for i in 0..4 {
                    out.push((c[i], Endpoint));
                    out.push(((c[i] + c[(i + 1) % 4]) * 0.5, Midpoint));
                }
                out.push(((*min + *max) * 0.5, Center));
            }
            Shape::Circle { center, .. } => out.push((*center, Center)),
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                out.push((geom::point_on_circle(*center, *radius, *start), Endpoint));
                out.push((
                    geom::point_on_circle(*center, *radius, start + sweep),
                    Endpoint,
                ));
                out.push((
                    geom::point_on_circle(*center, *radius, start + sweep * 0.5),
                    Midpoint,
                ));
                out.push((*center, Center));
            }
        }
    }

    /// Applies a placement. Rotation and mirroring keep circles and arcs
    /// exact; rectangles that are rotated by a non-right angle become
    /// closed polylines.
    pub fn transformed(&self, t: &Placement) -> Shape {
        let m = t.affine();
        let tp = |p: &Vec2| m.transform_point2(*p);
        match self {
            Shape::Line { a, b } => Shape::Line { a: tp(a), b: tp(b) },
            Shape::Polyline { points, closed } => Shape::Polyline {
                points: points.iter().map(tp).collect(),
                closed: *closed,
            },
            Shape::Rect { min, max } => {
                let corners = [*min, Vec2::new(max.x, min.y), *max, Vec2::new(min.x, max.y)];
                let q = (t.rotation / std::f32::consts::FRAC_PI_2).round();
                let right_angle = (t.rotation - q * std::f32::consts::FRAC_PI_2).abs() < 1e-5;
                if right_angle {
                    let b = Aabb::from_points(corners.iter().map(tp));
                    Shape::Rect {
                        min: b.min,
                        max: b.max,
                    }
                } else {
                    Shape::Polyline {
                        points: corners.iter().map(tp).collect(),
                        closed: true,
                    }
                }
            }
            Shape::Circle { center, radius } => Shape::Circle {
                center: tp(center),
                radius: radius * t.scale,
            },
            Shape::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                // Transform the start direction and flip the sweep when mirrored.
                let dir = Vec2::new(start.cos(), start.sin());
                let d = m.transform_vector2(dir);
                let sign = if t.mirror { -1.0 } else { 1.0 };
                Shape::Arc {
                    center: tp(center),
                    radius: radius * t.scale,
                    start: d.y.atan2(d.x),
                    sweep: sweep * sign,
                }
            }
        }
    }
}

/// Where a group of primitives sits on the page: mirror (about the local y
/// axis), then uniform scale, then rotation, then translation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub offset: Vec2,
    /// Radians, clockwise on screen.
    pub rotation: f32,
    pub mirror: bool,
    pub scale: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            rotation: 0.0,
            mirror: false,
            scale: 1.0,
        }
    }
}

impl Placement {
    pub fn at(offset: impl Into<Vec2>) -> Self {
        Self {
            offset: offset.into(),
            ..Default::default()
        }
    }

    pub fn affine(&self) -> Affine2 {
        let mirror = if self.mirror { -1.0 } else { 1.0 };
        Affine2::from_scale_angle_translation(
            Vec2::new(self.scale * mirror, self.scale),
            self.rotation,
            self.offset,
        )
    }
}

/// A shape with its look.
#[derive(Clone, Debug, PartialEq)]
pub struct Primitive {
    pub shape: Shape,
    pub stroke: Option<Stroke>,
    /// Fill colour for closed shapes (rectangles, circles, closed polylines).
    pub fill: Option<Color>,
}

impl Primitive {
    pub fn stroked(shape: Shape, stroke: Stroke) -> Self {
        Self {
            shape,
            stroke: Some(stroke),
            fill: None,
        }
    }

    pub fn filled(shape: Shape, fill: Color) -> Self {
        Self {
            shape,
            stroke: None,
            fill: Some(fill),
        }
    }

    pub fn with_fill(mut self, fill: Color) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn transformed(&self, t: &Placement) -> Primitive {
        let mut stroke = self.stroke;
        if let Some(s) = stroke.as_mut()
            && s.unit == crate::style::WidthUnit::World
        {
            s.width *= t.scale;
        }
        Primitive {
            shape: self.shape.transformed(t),
            stroke,
            fill: self.fill,
        }
    }

    /// Bounds including the stroke width at the given zoom.
    pub fn bounds(&self, zoom: f32) -> Aabb {
        let hw = self.stroke.map_or(0.0, |s| s.half_width_world(zoom));
        self.shape.bounds().expand(hw)
    }

    /// Distance from `p` to the visible ink of the primitive at the given zoom
    /// (zero when `p` is on the stroke or inside a fill).
    pub fn distance(&self, p: Vec2, zoom: f32) -> f32 {
        if self.fill.is_some()
            && let Some(poly) = self.shape.area_polygon(0.01)
            && geom::point_in_polygon(p, &poly)
        {
            return 0.0;
        }
        let hw = self.stroke.map_or(0.0, |s| s.half_width_world(zoom));
        (self.shape.distance_to_outline(p) - hw).max(0.0)
    }

    /// True when any visible ink touches `rect` (crossing selection).
    pub fn intersects(&self, rect: &Aabb) -> bool {
        if self.shape.outline_intersects(rect) {
            return true;
        }
        // A filled area that encloses the whole box also touches it.
        self.fill.is_some()
            && self
                .shape
                .area_polygon(0.01)
                .is_some_and(|poly| geom::point_in_polygon(rect.center(), &poly))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn arc_bounds_include_extremes() {
        let s = Shape::Arc {
            center: Vec2::ZERO,
            radius: 10.0,
            start: -FRAC_PI_2 * 0.5,
            sweep: FRAC_PI_2,
        };
        let b = s.bounds();
        assert!((b.max.x - 10.0).abs() < 1e-5);
        assert!(b.min.x > 7.0);
    }

    #[test]
    fn rotate_rect_quarter_turn_stays_rect() {
        let r = Shape::rect(Vec2::ZERO, Vec2::new(10.0, 2.0));
        let t = Placement {
            rotation: FRAC_PI_2,
            ..Default::default()
        };
        match r.transformed(&t) {
            Shape::Rect { min, max } => {
                assert!((max - min - Vec2::new(2.0, 10.0)).length() < 1e-4);
            }
            other => panic!("expected rect, got {other:?}"),
        }
        let t = Placement {
            rotation: 0.3,
            ..Default::default()
        };
        assert!(matches!(
            r.transformed(&t),
            Shape::Polyline { closed: true, .. }
        ));
    }

    #[test]
    fn mirror_flips_arc() {
        let a = Shape::Arc {
            center: Vec2::ZERO,
            radius: 1.0,
            start: 0.0,
            sweep: FRAC_PI_2,
        };
        let t = Placement {
            mirror: true,
            ..Default::default()
        };
        let Shape::Arc { start, sweep, .. } = a.transformed(&t) else {
            panic!()
        };
        assert!((start.abs() - PI).abs() < 1e-5);
        assert!((sweep + FRAC_PI_2).abs() < 1e-6);
        // The original end point (0, 1) stays at (0, 1) under a mirror about y.
        let end = geom::point_on_circle(Vec2::ZERO, 1.0, start + sweep);
        assert!(end.distance(Vec2::new(0.0, 1.0)) < 1e-5);
    }

    #[test]
    fn primitive_distance_respects_fill_and_stroke() {
        let p = Primitive::stroked(
            Shape::rect(Vec2::ZERO, Vec2::splat(10.0)),
            Stroke::world(1.0, Color::BLACK),
        );
        assert_eq!(p.distance(Vec2::new(10.4, 5.0), 1.0), 0.0);
        assert!(p.distance(Vec2::new(5.0, 5.0), 1.0) > 4.0);
        let p = p.with_fill(Color::WHITE);
        assert_eq!(p.distance(Vec2::new(5.0, 5.0), 1.0), 0.0);
        // Screen-width strokes get wider in world units when zoomed out.
        let s = Primitive::stroked(
            Shape::line(Vec2::ZERO, Vec2::X * 10.0),
            Stroke::screen(4.0, Color::BLACK),
        );
        assert_eq!(s.distance(Vec2::new(5.0, 1.5), 1.0), 0.0);
        assert!(s.distance(Vec2::new(5.0, 1.5), 10.0) > 1.0);
    }

    #[test]
    fn crossing_tests() {
        let c = Primitive::stroked(Shape::circle(Vec2::ZERO, 5.0), Stroke::default());
        assert!(c.intersects(&Aabb::new(Vec2::new(4.0, -1.0), Vec2::new(6.0, 1.0))));
        assert!(!c.intersects(&Aabb::new(Vec2::splat(-1.0), Vec2::splat(1.0))));
        let c = c.with_fill(Color::WHITE);
        assert!(c.intersects(&Aabb::new(Vec2::splat(-1.0), Vec2::splat(1.0))));
    }
}
