//! Small 2D geometry toolkit: bounding boxes, distances and intersection tests.
//!
//! World coordinates are `f32` drawing units (millimetres for schematic pages).
//! The y axis points down, like a sheet of paper and like the screen.

use glam::Vec2;
use std::f32::consts::TAU;

/// Axis-aligned bounding box. An empty box has `min > max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec2,
    pub max: Vec2,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: Vec2::splat(f32::INFINITY),
        max: Vec2::splat(f32::NEG_INFINITY),
    };

    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// The box spanned by two arbitrary corners.
    pub fn from_corners(a: Vec2, b: Vec2) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }

    pub fn from_points(points: impl IntoIterator<Item = Vec2>) -> Self {
        let mut b = Self::EMPTY;
        for p in points {
            b.include_point(p);
        }
        b
    }

    pub fn from_center(center: Vec2, half: Vec2) -> Self {
        Self {
            min: center - half,
            max: center + half,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y
    }

    pub fn size(&self) -> Vec2 {
        if self.is_empty() {
            Vec2::ZERO
        } else {
            self.max - self.min
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn include_point(&mut self, p: Vec2) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn union(&self, other: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    pub fn expand(&self, by: f32) -> Aabb {
        if self.is_empty() {
            return *self;
        }
        Aabb {
            min: self.min - Vec2::splat(by),
            max: self.max + Vec2::splat(by),
        }
    }

    pub fn contains_point(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x <= self.max.x && p.y >= self.min.y && p.y <= self.max.y
    }

    /// True when `other` lies completely inside `self`.
    pub fn contains(&self, other: &Aabb) -> bool {
        !other.is_empty()
            && other.min.x >= self.min.x
            && other.min.y >= self.min.y
            && other.max.x <= self.max.x
            && other.max.y <= self.max.y
    }

    pub fn intersects(&self, other: &Aabb) -> bool {
        !(self.is_empty()
            || other.is_empty()
            || other.min.x > self.max.x
            || other.max.x < self.min.x
            || other.min.y > self.max.y
            || other.max.y < self.min.y)
    }

    /// Distance from `p` to the box (zero inside).
    pub fn distance_to_point(&self, p: Vec2) -> f32 {
        let d = (self.min - p).max(p - self.max).max(Vec2::ZERO);
        d.length()
    }
}

/// Distance from `p` to the segment `a`-`b`.
pub fn dist_point_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Normalises an angle to `[0, TAU)`.
pub fn normalize_angle(a: f32) -> f32 {
    let r = a.rem_euclid(TAU);
    if r >= TAU { 0.0 } else { r }
}

/// True when `angle` lies on the arc starting at `start` and sweeping `sweep`
/// radians (positive sweeps go clockwise on screen because y points down).
pub fn angle_on_arc(angle: f32, start: f32, sweep: f32) -> bool {
    if sweep.abs() >= TAU {
        return true;
    }
    let (s, w) = if sweep >= 0.0 {
        (start, sweep)
    } else {
        (start + sweep, -sweep)
    };
    normalize_angle(angle - s) <= w + 1e-6
}

pub fn point_on_circle(center: Vec2, radius: f32, angle: f32) -> Vec2 {
    center + Vec2::new(angle.cos(), angle.sin()) * radius
}

/// Distance from `p` to a circular arc.
pub fn dist_point_arc(p: Vec2, center: Vec2, radius: f32, start: f32, sweep: f32) -> f32 {
    let d = p - center;
    let angle = d.y.atan2(d.x);
    if angle_on_arc(angle, start, sweep) {
        (d.length() - radius).abs()
    } else {
        let a = point_on_circle(center, radius, start);
        let b = point_on_circle(center, radius, start + sweep);
        p.distance(a).min(p.distance(b))
    }
}

/// Segment-segment intersection point, if the segments cross.
pub fn segment_intersection(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Option<Vec2> {
    let r = b - a;
    let s = d - c;
    let denom = r.perp_dot(s);
    if denom.abs() <= f32::EPSILON {
        return None;
    }
    let t = (c - a).perp_dot(s) / denom;
    let u = (c - a).perp_dot(r) / denom;
    if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
        Some(a + r * t)
    } else {
        None
    }
}

/// True when the segment `a`-`b` touches the box.
pub fn segment_intersects_aabb(a: Vec2, b: Vec2, rect: &Aabb) -> bool {
    if rect.contains_point(a) || rect.contains_point(b) {
        return true;
    }
    if !Aabb::from_corners(a, b).intersects(rect) {
        return false;
    }
    let corners = [
        rect.min,
        Vec2::new(rect.max.x, rect.min.y),
        rect.max,
        Vec2::new(rect.min.x, rect.max.y),
    ];
    (0..4).any(|i| segment_intersection(a, b, corners[i], corners[(i + 1) % 4]).is_some())
}

/// True when the circle outline (not its interior) touches the box.
pub fn circle_outline_intersects_aabb(center: Vec2, radius: f32, rect: &Aabb) -> bool {
    let nearest = rect.distance_to_point(center);
    let corners = [
        rect.min,
        Vec2::new(rect.max.x, rect.min.y),
        rect.max,
        Vec2::new(rect.min.x, rect.max.y),
    ];
    let farthest = corners
        .iter()
        .map(|c| c.distance(center))
        .fold(0.0_f32, f32::max);
    nearest <= radius && farthest >= radius
}

/// Even-odd point in polygon test.
pub fn point_in_polygon(p: Vec2, poly: &[Vec2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (pi, pj) = (poly[i], poly[j]);
        if (pi.y > p.y) != (pj.y > p.y) && p.x < (pj.x - pi.x) * (p.y - pi.y) / (pj.y - pi.y) + pi.x
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Number of segments needed to approximate a circle of `radius` so the chord
/// error stays below `tolerance`.
pub fn circle_segments(radius: f32, sweep: f32, tolerance: f32) -> usize {
    let r = radius.abs().max(1e-6);
    let tol = tolerance.clamp(1e-6, r);
    let step = 2.0 * (1.0 - tol / r).clamp(-1.0, 1.0).acos();
    let full = if step > 0.0 { TAU / step } else { 64.0 };
    let n = (full * (sweep.abs() / TAU)).ceil() as usize;
    let min = ((sweep.abs() / TAU) * 12.0).ceil() as usize;
    n.clamp(min.max(1), 512)
}

/// Points along an arc, both ends included.
pub fn arc_points(center: Vec2, radius: f32, start: f32, sweep: f32, tolerance: f32) -> Vec<Vec2> {
    let n = circle_segments(radius, sweep, tolerance);
    (0..=n)
        .map(|i| point_on_circle(center, radius, start + sweep * (i as f32 / n as f32)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, PI};

    #[test]
    fn aabb_basics() {
        let b = Aabb::from_corners(Vec2::new(4.0, 1.0), Vec2::new(0.0, 3.0));
        assert_eq!(b.min, Vec2::new(0.0, 1.0));
        assert_eq!(b.size(), Vec2::new(4.0, 2.0));
        assert!(b.contains_point(Vec2::new(2.0, 2.0)));
        assert!(!b.contains_point(Vec2::new(5.0, 2.0)));
        assert!(Aabb::EMPTY.is_empty());
        assert!(!Aabb::EMPTY.intersects(&b));
        assert_eq!(b.distance_to_point(Vec2::new(7.0, 7.0)), 5.0);
    }

    #[test]
    fn segment_distance() {
        let d = dist_point_segment(Vec2::new(5.0, 3.0), Vec2::ZERO, Vec2::new(10.0, 0.0));
        assert!((d - 3.0).abs() < 1e-6);
        let d = dist_point_segment(Vec2::new(-3.0, 4.0), Vec2::ZERO, Vec2::new(10.0, 0.0));
        assert!((d - 5.0).abs() < 1e-6);
    }

    #[test]
    fn arcs() {
        // Quarter arc from 0 to +90 degrees (pointing down on screen).
        assert!(angle_on_arc(0.5, 0.0, FRAC_PI_2));
        assert!(!angle_on_arc(-0.5, 0.0, FRAC_PI_2));
        assert!(angle_on_arc(-0.5, 0.0, -FRAC_PI_2));
        let d = dist_point_arc(Vec2::new(0.0, 12.0), Vec2::ZERO, 10.0, 0.0, FRAC_PI_2);
        assert!((d - 2.0).abs() < 1e-5);
        // Point on the missing side of the circle measures to the nearest end.
        let d = dist_point_arc(Vec2::new(-10.0, 0.0), Vec2::ZERO, 10.0, 0.0, FRAC_PI_2);
        assert!((d - (200.0f32).sqrt()).abs() < 1e-4);
        assert!(angle_on_arc(PI, 0.0, TAU));
    }

    #[test]
    fn segment_box() {
        let r = Aabb::new(Vec2::ZERO, Vec2::splat(10.0));
        assert!(segment_intersects_aabb(
            Vec2::new(-5.0, 5.0),
            Vec2::new(15.0, 5.0),
            &r
        ));
        assert!(!segment_intersects_aabb(
            Vec2::new(-5.0, -5.0),
            Vec2::new(-1.0, 20.0),
            &r
        ));
        assert!(segment_intersects_aabb(
            Vec2::new(2.0, 2.0),
            Vec2::new(3.0, 3.0),
            &r
        ));
    }

    #[test]
    fn circle_box() {
        let r = Aabb::new(Vec2::ZERO, Vec2::splat(10.0));
        assert!(circle_outline_intersects_aabb(Vec2::new(5.0, 5.0), 6.0, &r));
        // Box completely inside the circle does not touch the outline.
        assert!(!circle_outline_intersects_aabb(
            Vec2::new(5.0, 5.0),
            100.0,
            &r
        ));
        // Circle completely inside the box.
        assert!(circle_outline_intersects_aabb(Vec2::new(5.0, 5.0), 1.0, &r));
    }

    #[test]
    fn polygon() {
        let sq = [
            Vec2::ZERO,
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        ];
        assert!(point_in_polygon(Vec2::new(5.0, 5.0), &sq));
        assert!(!point_in_polygon(Vec2::new(15.0, 5.0), &sq));
    }

    #[test]
    fn segment_counts() {
        let n = circle_segments(10.0, TAU, 0.01);
        assert!((12..=512).contains(&n));
        assert!(circle_segments(0.001, TAU, 0.01) >= 12);
        let pts = arc_points(Vec2::ZERO, 1.0, 0.0, FRAC_PI_2, 0.001);
        assert!(pts[0].distance(Vec2::new(1.0, 0.0)) < 1e-6);
        assert!(pts.last().unwrap().distance(Vec2::new(0.0, 1.0)) < 1e-5);
    }
}
