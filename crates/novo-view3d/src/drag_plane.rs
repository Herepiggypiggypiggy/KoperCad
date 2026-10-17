//! The plane an item is dragged in. Items on the mounting plate are dragged
//! across the plate plane; an item mounted on a wall, the roof, the floor or
//! an extra plate carries the plane of its own surface, so a drag reads in
//! that surface's coordinates (x along `u`, y along `v`, mm).

use glam::Vec3;

use crate::math::Ray;
use crate::scene::PlateFrame;

/// A mounting surface in world millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DragPlane {
    /// The surface's corner, where its x and y are 0.
    pub origin: Vec3,
    /// Unit direction of the surface's x.
    pub u: Vec3,
    /// Unit direction of the surface's y.
    pub v: Vec3,
}

impl DragPlane {
    /// The surface coordinates under `ray`, if it crosses the plane in front
    /// of its origin. `None` for a plane seen edge on.
    pub fn point(&self, ray: &Ray) -> Option<(f64, f64)> {
        let normal = self.u.cross(self.v);
        let t = ray.plane_hit(self.origin, normal)?;
        let d = ray.at(t) - self.origin;
        Some((d.dot(self.u) as f64, d.dot(self.v) as f64))
    }
}

/// The point a drag follows under `ray`: on `plane` when the item has one,
/// otherwise on the plate plane in plate coordinates (x right, y down).
pub fn drag_point(ray: &Ray, plane: Option<&DragPlane>, frame: &PlateFrame) -> Option<(f64, f64)> {
    match plane {
        Some(p) => p.point(ray),
        None => ray
            .plane_hit(Vec3::ZERO, Vec3::Z)
            .map(|t| frame.plate(ray.at(t))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> PlateFrame {
        PlateFrame {
            width: 600.0,
            height: 800.0,
        }
    }

    #[test]
    fn without_a_plane_the_plate_is_used_with_y_down() {
        let ray = Ray {
            origin: Vec3::new(100.0, 700.0, 500.0),
            dir: Vec3::NEG_Z,
        };
        // World y 700 is plate y 100 (the plate is 800 high).
        assert_eq!(drag_point(&ray, None, &frame()), Some((100.0, 100.0)));
    }

    #[test]
    fn a_roof_plane_reads_in_its_own_coordinates() {
        // The roof: corner at the top front-left, x to the right, y going
        // back into the cabinet (-z), seen from above.
        let plane = DragPlane {
            origin: Vec3::new(0.0, 800.0, 300.0),
            u: Vec3::X,
            v: Vec3::NEG_Z,
        };
        let ray = Ray {
            origin: Vec3::new(250.0, 1500.0, 100.0),
            dir: Vec3::NEG_Y,
        };
        assert_eq!(plane.point(&ray), Some((250.0, 200.0)));
    }

    #[test]
    fn a_plane_seen_edge_on_gives_nothing() {
        let plane = DragPlane {
            origin: Vec3::ZERO,
            u: Vec3::X,
            v: Vec3::Y,
        };
        let ray = Ray {
            origin: Vec3::new(0.0, 0.0, 10.0),
            dir: Vec3::X,
        };
        assert_eq!(plane.point(&ray), None);
    }
}
