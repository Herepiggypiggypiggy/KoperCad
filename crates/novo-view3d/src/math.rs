//! Small geometry helpers: boxes, rays and view frustums.

use glam::{Mat4, Vec3, Vec4};

/// Axis-aligned box in world millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    /// The empty box: extending it with a point gives that point.
    pub const EMPTY: Aabb = Aabb {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn is_empty(&self) -> bool {
        self.min.x > self.max.x || self.min.y > self.max.y || self.min.z > self.max.z
    }

    pub fn extend(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    pub fn union(&self, o: &Aabb) -> Aabb {
        Aabb {
            min: self.min.min(o.min),
            max: self.max.max(o.max),
        }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Half the diagonal: the radius of the bounding sphere.
    pub fn radius(&self) -> f32 {
        self.size().length() * 0.5
    }

    pub fn corners(&self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        [
            Vec3::new(a.x, a.y, a.z),
            Vec3::new(b.x, a.y, a.z),
            Vec3::new(a.x, b.y, a.z),
            Vec3::new(b.x, b.y, a.z),
            Vec3::new(a.x, a.y, b.z),
            Vec3::new(b.x, a.y, b.z),
            Vec3::new(a.x, b.y, b.z),
            Vec3::new(b.x, b.y, b.z),
        ]
    }

    /// The box that encloses this box after `m` (any affine matrix).
    pub fn transformed(&self, m: &Mat4) -> Aabb {
        let mut out = Aabb::EMPTY;
        for c in self.corners() {
            out.extend(m.transform_point3(c));
        }
        out
    }

    /// Nearest and farthest ray parameters inside the box (slab test), or
    /// `None` when the ray misses. `t_near` may be negative when the origin
    /// is inside.
    pub fn ray_interval(&self, ray: &Ray) -> Option<(f32, f32)> {
        let inv = ray.dir.recip();
        let t1 = (self.min - ray.origin) * inv;
        let t2 = (self.max - ray.origin) * inv;
        let near = t1.min(t2);
        let far = t1.max(t2);
        let t_near = near.x.max(near.y).max(near.z);
        let t_far = far.x.min(far.y).min(far.z);
        (t_far >= t_near.max(0.0)).then_some((t_near, t_far))
    }
}

/// A ray with a unit direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub dir: Vec3,
}

impl Ray {
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.dir * t
    }

    /// The ray in the space `m` maps from (`m` is local-to-world). The
    /// direction is not renormalised, so `t` stays comparable between spaces.
    pub fn to_local(&self, local_to_world: &Mat4) -> Ray {
        let inv = local_to_world.inverse();
        Ray {
            origin: inv.transform_point3(self.origin),
            dir: inv.transform_vector3(self.dir),
        }
    }

    /// Parameter where the ray crosses the plane through `point` with
    /// `normal`, if it does in front of the origin.
    pub fn plane_hit(&self, point: Vec3, normal: Vec3) -> Option<f32> {
        let denom = normal.dot(self.dir);
        if denom.abs() < 1e-8 {
            return None;
        }
        let t = normal.dot(point - self.origin) / denom;
        (t >= 0.0).then_some(t)
    }
}

/// Six clip planes of a view-projection matrix (wgpu clip space, depth 0..1;
/// works for the reversed-Z matrices used here).
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [Vec4; 6],
}

impl Frustum {
    pub fn from_view_proj(m: &Mat4) -> Self {
        let r0 = m.row(0);
        let r1 = m.row(1);
        let r2 = m.row(2);
        let r3 = m.row(3);
        let norm = |p: Vec4| p / p.truncate().length().max(f32::MIN_POSITIVE);
        Self {
            planes: [
                norm(r3 + r0),
                norm(r3 - r0),
                norm(r3 + r1),
                norm(r3 - r1),
                // Depth range 0..1: z >= 0 and z <= w. With an infinite far
                // plane the first one is the plane at infinity and never
                // rejects anything.
                r2,
                norm(r3 - r2),
            ],
        }
    }

    /// False only when the box is completely outside one plane.
    pub fn intersects(&self, b: &Aabb) -> bool {
        for p in &self.planes {
            let n = p.truncate();
            if n == Vec3::ZERO {
                continue;
            }
            let v = Vec3::new(
                if n.x >= 0.0 { b.max.x } else { b.min.x },
                if n.y >= 0.0 { b.max.y } else { b.min.y },
                if n.z >= 0.0 { b.max.z } else { b.min.z },
            );
            if n.dot(v) + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_and_misses_a_box() {
        let b = Aabb::new(Vec3::splat(-1.0), Vec3::splat(1.0));
        let hit = Ray {
            origin: Vec3::new(0.0, 0.0, 5.0),
            dir: Vec3::NEG_Z,
        };
        let (near, far) = b.ray_interval(&hit).unwrap();
        assert!((near - 4.0).abs() < 1e-5 && (far - 6.0).abs() < 1e-5);
        let miss = Ray {
            origin: Vec3::new(3.0, 0.0, 5.0),
            dir: Vec3::NEG_Z,
        };
        assert!(b.ray_interval(&miss).is_none());
        let behind = Ray {
            origin: Vec3::new(0.0, 0.0, -5.0),
            dir: Vec3::NEG_Z,
        };
        assert!(b.ray_interval(&behind).is_none());
    }

    #[test]
    fn plane_hit_needs_a_forward_crossing() {
        let ray = Ray {
            origin: Vec3::new(1.0, 2.0, 10.0),
            dir: Vec3::NEG_Z,
        };
        assert_eq!(ray.plane_hit(Vec3::ZERO, Vec3::Z), Some(10.0));
        assert_eq!(ray.plane_hit(Vec3::new(0.0, 0.0, 20.0), Vec3::Z), None);
    }
}
