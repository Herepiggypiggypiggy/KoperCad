//! Wires as chains of straight tubes. A tube is drawn and picked as a thin
//! box along the segment, so wires cost no extra mesh and instance like
//! everything else.

use crate::math::Aabb;
use crate::scene::Wire;
use glam::{Mat4, Quat, Vec3};

/// Thinnest tube that is picked, mm: a 1 mm wire stays clickable.
pub const MIN_PICK_RADIUS: f32 = 1.5;

/// One straight piece of a wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub a: Vec3,
    pub b: Vec3,
}

impl Segment {
    pub fn length(&self) -> f32 {
        (self.b - self.a).length()
    }

    /// Places the unit box (x, y in ±0.5, z in 0..1) at `a` with its z axis
    /// along the segment. Scale it by (2r, 2r, length).
    pub fn frame(&self) -> Mat4 {
        let dir = (self.b - self.a).normalize_or_zero();
        let rot = Quat::from_rotation_arc(Vec3::Z, dir);
        Mat4::from_rotation_translation(rot, self.a)
    }

    pub fn centre(&self) -> Vec3 {
        (self.a + self.b) * 0.5
    }
}

/// The non-degenerate segments of a wire.
pub fn segments(wire: &Wire) -> impl Iterator<Item = Segment> + '_ {
    wire.points
        .windows(2)
        .map(|w| Segment { a: w[0], b: w[1] })
        .filter(|s| s.length() > 1e-3)
}

/// Axis-aligned bounds of the whole wire, grown by its radius.
pub fn world_bounds(wire: &Wire) -> Aabb {
    let mut b = Aabb::EMPTY;
    for p in &wire.points {
        b.extend(*p);
    }
    if !b.is_empty() {
        b.min -= Vec3::splat(wire.radius);
        b.max += Vec3::splat(wire.radius);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{ItemId, ItemKind, Mark};

    fn wire(points: &[Vec3]) -> Wire {
        Wire {
            id: ItemId::new(ItemKind::Wire, 1),
            points: points.to_vec(),
            radius: 1.0,
            tint: None,
            mark: Mark::None,
            label: None,
        }
    }

    #[test]
    fn degenerate_segments_are_skipped() {
        let w = wire(&[Vec3::ZERO, Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)]);
        assert_eq!(segments(&w).count(), 1);
    }

    #[test]
    fn the_frame_points_the_unit_box_along_the_segment() {
        let s = Segment {
            a: Vec3::new(5.0, 5.0, 5.0),
            b: Vec3::new(5.0, 25.0, 5.0),
        };
        let m = s.frame();
        let tip = m.transform_point3(Vec3::new(0.0, 0.0, s.length()));
        assert!((tip - s.b).length() < 1e-4);
    }

    #[test]
    fn bounds_include_the_radius() {
        let w = wire(&[Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)]);
        let b = world_bounds(&w);
        assert_eq!(b.min, Vec3::splat(-1.0));
        assert_eq!(b.max, Vec3::new(11.0, 1.0, 1.0));
    }
}
