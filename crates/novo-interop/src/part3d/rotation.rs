//! A stored per-part rotation: the user's fix for a part that came out
//! turned wrongly (the axes of a STEP file are the maker's choice, and
//! without a catalog size they are only guessed).
//!
//! Rotations are whole quarter turns about the part's own axes, so they are
//! exact: no rounding, no change of winding, and a model of width x height
//! x depth stays an axis-aligned box.

use super::math::*;

/// Quarter turns about x, then y, then z, in part space (x width, y height,
/// z depth towards the door). Each count is 0 to 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PartRotation {
    pub quarter_turns: [u8; 3],
}

impl PartRotation {
    pub const NONE: PartRotation = PartRotation {
        quarter_turns: [0; 3],
    };

    /// Counts are taken modulo 4.
    pub fn new(x: u8, y: u8, z: u8) -> PartRotation {
        PartRotation {
            quarter_turns: [x % 4, y % 4, z % 4],
        }
    }

    /// From degrees about x, y, z; `None` unless each is a multiple of 90.
    pub fn from_degrees(deg: [i32; 3]) -> Option<PartRotation> {
        let q = |d: i32| (d % 90 == 0).then(|| d.div_euclid(90).rem_euclid(4) as u8);
        Some(PartRotation::new(q(deg[0])?, q(deg[1])?, q(deg[2])?))
    }

    pub fn degrees(&self) -> [i32; 3] {
        self.quarter_turns.map(|q| i32::from(q % 4) * 90)
    }

    pub fn is_none(&self) -> bool {
        self.quarter_turns.iter().all(|q| q % 4 == 0)
    }

    /// 0 for no rotation, else a number that names this rotation (it goes
    /// into cache keys; see `cache::cache_key_rotated`).
    pub fn code(&self) -> u8 {
        let [x, y, z] = self.quarter_turns.map(|q| q % 4);
        x + 4 * y + 16 * z
    }

    /// The point turned about the origin.
    pub fn apply(&self, p: V3) -> V3 {
        let mut p = p;
        for _ in 0..self.quarter_turns[0] % 4 {
            p = [p[0], -p[2], p[1]];
        }
        for _ in 0..self.quarter_turns[1] % 4 {
            p = [p[2], p[1], -p[0]];
        }
        for _ in 0..self.quarter_turns[2] % 4 {
            p = [-p[1], p[0], p[2]];
        }
        p
    }

    /// Turns part-space points and seats them again: x and y centred, the
    /// back at z = 0.
    pub fn apply_in_place(&self, pos: &mut [V3]) {
        if self.is_none() || pos.is_empty() {
            return;
        }
        for p in pos.iter_mut() {
            *p = self.apply(*p);
        }
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for p in pos.iter() {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
        let shift = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, lo[2]];
        for p in pos.iter_mut() {
            *p = sub(*p, shift);
        }
    }

    /// Where a point of an unrotated part lands, for a part whose box is
    /// `size` (width, height, depth; x and y centred, back at z = 0). Lets
    /// anything stored in part space (connection points) follow the mesh.
    pub fn apply_to_point(&self, size: V3, p: V3) -> V3 {
        let mut corners = Vec::with_capacity(8);
        for i in 0..8 {
            corners.push([
                if i & 1 == 0 {
                    -size[0] / 2.0
                } else {
                    size[0] / 2.0
                },
                if i & 2 == 0 {
                    -size[1] / 2.0
                } else {
                    size[1] / 2.0
                },
                if i & 4 == 0 { 0.0 } else { size[2] },
            ]);
        }
        let mut seated = corners.clone();
        self.apply_in_place(&mut seated);
        // The seating shift is the difference between a corner before and
        // after: rotate the corner, subtract where it was seated.
        let shift = sub(self.apply(corners[0]), seated[0]);
        sub(self.apply(p), shift)
    }

    /// Width, height and depth after the rotation.
    pub fn size_after(&self, size: V3) -> V3 {
        let r = self.apply(size);
        [r[0].abs(), r[1].abs(), r[2].abs()]
    }
}

/// Stable key of a part's geometry for a rotation. `PartRotation::NONE`
/// gives exactly [`super::geometry_key`], so keys of parts nobody turned
/// do not change.
pub fn geometry_key_rotated(part_id: &str, content_id: &str, rotation: PartRotation) -> u64 {
    let base = super::geometry_key(part_id, content_id);
    if rotation.is_none() {
        return base;
    }
    let mut h = base;
    for b in [0xff, rotation.code()] {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_quarter_turns_are_none_and_degrees_round_trip() {
        assert!(PartRotation::new(4, 8, 0).is_none());
        let r = PartRotation::from_degrees([90, -90, 180]).unwrap();
        assert_eq!(r.quarter_turns, [1, 3, 2]);
        assert_eq!(r.degrees(), [90, 270, 180]);
        assert!(PartRotation::from_degrees([45, 0, 0]).is_none());
    }

    #[test]
    fn a_quarter_turn_about_each_axis_is_a_proper_rotation() {
        for r in [
            PartRotation::new(1, 0, 0),
            PartRotation::new(0, 1, 0),
            PartRotation::new(0, 0, 1),
            PartRotation::new(1, 2, 3),
        ] {
            let (x, y, z) = (
                r.apply([1.0, 0.0, 0.0]),
                r.apply([0.0, 1.0, 0.0]),
                r.apply([0.0, 0.0, 1.0]),
            );
            assert!((dot(cross(x, y), z) - 1.0).abs() < 1e-12, "{r:?}");
        }
        assert_eq!(
            PartRotation::new(0, 0, 1).apply([1.0, 0.0, 0.0]),
            [0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn a_turned_box_is_seated_again_and_points_follow() {
        let size = [20.0, 90.0, 60.0];
        let r = PartRotation::new(0, 0, 1);
        assert_eq!(r.size_after(size), [90.0, 20.0, 60.0]);
        // The corner (10, 45, 0) of the box goes to (-45, 10, 0) after a
        // quarter turn about z, and the box stays centred in x and y.
        let p = r.apply_to_point(size, [10.0, 45.0, 0.0]);
        assert!((p[0] + 45.0).abs() < 1e-12 && (p[1] - 10.0).abs() < 1e-12 && p[2].abs() < 1e-12);
        // Turning about x puts the back where the front was.
        let flip = PartRotation::new(2, 0, 0);
        let q = flip.apply_to_point(size, [0.0, 0.0, 0.0]);
        assert!((q[2] - 60.0).abs() < 1e-12, "{q:?}");
    }

    #[test]
    fn keys_do_not_change_without_a_rotation() {
        let none = geometry_key_rotated("p", "c", PartRotation::NONE);
        assert_eq!(none, super::super::geometry_key("p", "c"));
        assert_ne!(
            none,
            geometry_key_rotated("p", "c", PartRotation::new(1, 0, 0))
        );
        assert_ne!(
            geometry_key_rotated("p", "c", PartRotation::new(1, 0, 0)),
            geometry_key_rotated("p", "c", PartRotation::new(0, 1, 0))
        );
    }
}
