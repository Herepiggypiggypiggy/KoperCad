//! What the user is doing with the view, kept outside the scene.

use crate::math::Aabb;
use crate::scene::{ItemId, ItemKind};
use glam::Vec3;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    /// Draw edges over the faces.
    pub edges: bool,
    /// Draw tag labels over the image.
    pub labels: bool,
    /// Soft shadows from the key light (software renderers skip them).
    pub shadows: bool,
    /// Screen-space ambient occlusion on the High tier (the frame governor
    /// can step the tier down; turning this on again asks for it back).
    pub ssao: bool,
    /// Draw measuring lines: the enclosure's size and, for one selected part,
    /// its size and its distances to neighbours and the plate edge.
    pub dimensions: bool,
    /// Show the "?" badge (no part data) on every device. Off: only on the
    /// device under the pointer or in the selection.
    pub missing_marks: bool,
    /// A swipe reported in wheel lines (Windows, wheel and precision
    /// touchpad alike) pans instead of zooming. Off: it zooms, as a wheel.
    pub trackpad_pans: bool,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            edges: true,
            labels: true,
            shadows: true,
            ssao: true,
            dimensions: false,
            missing_marks: false,
            trackpad_pans: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ViewState {
    pub selection: BTreeSet<ItemId>,
    pub hover: Option<ItemId>,
    /// Layer switches: hide the door, the shell, wires.
    pub hidden: BTreeSet<ItemKind>,
    /// Items related to the selection (what a coil switches), drawn softer.
    pub related: BTreeSet<ItemId>,
    pub display: Display,
    /// A section plane that cuts the front of the picture away.
    pub section: Option<Section>,
}

/// The axis a section plane stands across (scene axes: X across the plate,
/// Y up, Z out of the plate towards the door).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionAxis {
    X,
    Y,
    Z,
}

impl SectionAxis {
    pub fn unit(self) -> Vec3 {
        match self {
            SectionAxis::X => Vec3::X,
            SectionAxis::Y => Vec3::Y,
            SectionAxis::Z => Vec3::Z,
        }
    }
}

/// A plane through the scene that hides everything on one side of it, so the
/// inside of the enclosure shows. Only the picture is cut: the project, the
/// bounds and the selection are not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    pub axis: SectionAxis,
    /// Where the plane stands along the axis, 0 at the scene's low end and 1
    /// at its high end.
    pub at: f32,
    /// Hide the low side instead of the high side.
    pub flip: bool,
}

impl Default for Section {
    fn default() -> Self {
        // Through the middle of the depth, hiding the door side.
        Section {
            axis: SectionAxis::Z,
            at: 0.5,
            flip: false,
        }
    }
}

impl Section {
    /// The scene's low and high ends along the axis, in mm.
    pub fn range(&self, bounds: &Aabb) -> (f32, f32) {
        let u = self.axis.unit();
        (bounds.min.dot(u), bounds.max.dot(u))
    }

    /// Where the plane stands along the axis, in mm.
    pub fn position_mm(&self, bounds: &Aabb) -> f32 {
        let (lo, hi) = self.range(bounds);
        lo + (hi - lo) * self.at.clamp(0.0, 1.0)
    }

    /// Put the plane at `mm` along the axis, kept inside the scene. A
    /// non-finite value leaves it where it is.
    pub fn set_position_mm(&mut self, bounds: &Aabb, mm: f32) {
        let (lo, hi) = self.range(bounds);
        if !mm.is_finite() || hi <= lo {
            return;
        }
        self.at = ((mm - lo) / (hi - lo)).clamp(0.0, 1.0);
    }

    /// The plane as `(n, w)`: a point `p` is cut away when `n . p + w > 0`.
    pub fn plane(&self, bounds: &Aabb) -> (Vec3, f32) {
        let u = self.axis.unit();
        let lo = bounds.min.dot(u);
        let hi = bounds.max.dot(u);
        let offset = lo + (hi - lo) * self.at.clamp(0.0, 1.0);
        if self.flip {
            (-u, offset)
        } else {
            (u, -offset)
        }
    }

    /// Whether the plane cuts the point away.
    pub fn cuts(&self, bounds: &Aabb, p: Vec3) -> bool {
        let (n, w) = self.plane(bounds);
        n.dot(p) + w > 0.0
    }

    /// The part of the parameter interval `[t0, t1]` of a ray that is not
    /// cut away; `None` when all of it is.
    pub fn clip_interval(
        &self,
        bounds: &Aabb,
        origin: Vec3,
        dir: Vec3,
        t0: f32,
        t1: f32,
    ) -> Option<(f32, f32)> {
        let (n, w) = self.plane(bounds);
        // f(t) = n . (origin + t dir) + w is cut away where it is positive.
        let f0 = n.dot(origin) + w;
        let slope = n.dot(dir);
        let (mut lo, mut hi) = (t0, t1);
        if slope.abs() < 1e-9 {
            return (f0 <= 0.0).then_some((lo, hi));
        }
        let t_zero = -f0 / slope;
        if slope > 0.0 {
            hi = hi.min(t_zero);
        } else {
            lo = lo.max(t_zero);
        }
        (lo <= hi).then_some((lo, hi))
    }
}

impl ViewState {
    pub fn is_visible(&self, kind: ItemKind) -> bool {
        !self.hidden.contains(&kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Aabb {
        Aabb::new(Vec3::ZERO, Vec3::new(100.0, 200.0, 50.0))
    }

    #[test]
    fn the_default_plane_hides_the_door_side_of_the_depth() {
        let s = Section::default();
        assert!(s.cuts(&bounds(), Vec3::new(10.0, 10.0, 40.0)));
        assert!(!s.cuts(&bounds(), Vec3::new(10.0, 10.0, 10.0)));
    }

    #[test]
    fn the_plane_position_reads_and_sets_in_mm() {
        let mut s = Section::default();
        // Default: middle of the 50 mm depth.
        assert!((s.position_mm(&bounds()) - 25.0).abs() < 1e-4);
        s.set_position_mm(&bounds(), 40.0);
        assert!((s.at - 0.8).abs() < 1e-4);
        s.set_position_mm(&bounds(), 900.0);
        assert_eq!(s.at, 1.0);
        s.set_position_mm(&bounds(), -5.0);
        assert_eq!(s.at, 0.0);
        s.set_position_mm(&bounds(), f32::NAN);
        assert_eq!(s.at, 0.0);
    }

    #[test]
    fn flipping_hides_the_other_side() {
        let s = Section {
            flip: true,
            ..Section::default()
        };
        assert!(!s.cuts(&bounds(), Vec3::new(10.0, 10.0, 40.0)));
        assert!(s.cuts(&bounds(), Vec3::new(10.0, 10.0, 10.0)));
    }

    #[test]
    fn the_plane_follows_its_axis_and_position() {
        let s = Section {
            axis: SectionAxis::X,
            at: 0.25,
            flip: false,
        };
        assert!(s.cuts(&bounds(), Vec3::new(30.0, 0.0, 0.0)));
        assert!(!s.cuts(&bounds(), Vec3::new(20.0, 0.0, 0.0)));
    }

    #[test]
    fn a_ray_keeps_only_the_part_behind_the_plane() {
        let s = Section::default();
        // From in front of the door straight into the plate: the plane is at
        // z = 25, so the kept part starts there.
        let kept = s
            .clip_interval(&bounds(), Vec3::new(5.0, 5.0, 100.0), -Vec3::Z, 0.0, 200.0)
            .unwrap();
        assert!((kept.0 - 75.0).abs() < 1e-4 && kept.1 == 200.0, "{kept:?}");
        // A ray wholly in front of the plane is cut away.
        assert!(
            s.clip_interval(&bounds(), Vec3::new(5.0, 5.0, 100.0), -Vec3::Z, 0.0, 50.0)
                .is_none()
        );
    }
}
