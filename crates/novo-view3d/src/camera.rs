//! Turntable orbit camera around a target point, +Y up.
//!
//! Depth is reversed (1 at the near plane, 0 far away) with an infinite far
//! plane for perspective, so a 2.4 m cabinet seen from 30 cm has no z-fighting
//! between a rail and the plate in a 32-bit depth buffer.

use crate::math::{Aabb, Ray};
use glam::{Mat4, Vec2, Vec3};
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// Shortest and longest camera distance, mm.
pub const MIN_DISTANCE: f32 = 20.0;
pub const MAX_DISTANCE: f32 = 100_000.0;
const MAX_PITCH: f32 = 89.0 * PI / 180.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Projection {
    #[default]
    Perspective,
    Orthographic,
}

/// The views of the view cube.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewPreset {
    Front,
    Back,
    Left,
    Right,
    Top,
    Bottom,
    Iso,
}

impl ViewPreset {
    /// Yaw and pitch of the camera for this view.
    pub fn angles(self) -> (f32, f32) {
        match self {
            ViewPreset::Front => (0.0, 0.0),
            ViewPreset::Back => (PI, 0.0),
            ViewPreset::Left => (-FRAC_PI_2, 0.0),
            ViewPreset::Right => (FRAC_PI_2, 0.0),
            ViewPreset::Top => (0.0, MAX_PITCH),
            ViewPreset::Bottom => (0.0, -MAX_PITCH),
            ViewPreset::Iso => (0.62, 0.48),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub target: Vec3,
    /// Turn about +Y; 0 looks at the front of the plate (down -Z).
    pub yaw: f32,
    /// Elevation above the horizon, clamped to +-89 degrees.
    pub pitch: f32,
    pub distance: f32,
    /// Vertical field of view, radians.
    pub fov_y: f32,
    pub projection: Projection,
}

impl Default for Camera {
    fn default() -> Self {
        let (yaw, pitch) = ViewPreset::Iso.angles();
        Self {
            target: Vec3::ZERO,
            yaw,
            pitch,
            distance: 1500.0,
            fov_y: 35f32.to_radians(),
            projection: Projection::Perspective,
        }
    }
}

impl Camera {
    /// Unit vector from the target to the eye.
    pub fn dir(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        Vec3::new(cp * sy, sp, cp * cy)
    }

    pub fn eye(&self) -> Vec3 {
        self.target + self.dir() * self.distance
    }

    /// Right, up and forward (towards the target) unit vectors.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        let fwd = -self.dir();
        let right = fwd.cross(Vec3::Y).normalize();
        let up = right.cross(fwd);
        (right, up, fwd)
    }

    pub fn view(&self) -> Mat4 {
        let (_, up, _) = self.basis();
        Mat4::look_at_rh(self.eye(), self.target, up)
    }

    fn half_height(&self) -> f32 {
        self.distance * (self.fov_y * 0.5).tan()
    }

    /// Reversed-Z projection. `bounds` fits the depth range of an orthographic
    /// view; a perspective view does not need it.
    pub fn projection_matrix(&self, aspect: f32, bounds: &Aabb) -> Mat4 {
        match self.projection {
            Projection::Perspective => {
                let near = (self.distance * 0.01).clamp(0.5, 50.0);
                Mat4::perspective_infinite_reverse_rh(self.fov_y, aspect, near)
            }
            Projection::Orthographic => {
                let (_, _, fwd) = self.basis();
                let eye = self.eye();
                let (mut lo, mut hi) = (self.distance - 1.0, self.distance + 1.0);
                if !bounds.is_empty() {
                    lo = f32::INFINITY;
                    hi = f32::NEG_INFINITY;
                    for c in bounds.corners() {
                        let t = (c - eye).dot(fwd);
                        lo = lo.min(t);
                        hi = hi.max(t);
                    }
                }
                let (near, far) = (lo - 10.0, hi + 10.0);
                let (h, w) = (self.half_height(), self.half_height() * aspect);
                Mat4::orthographic_rh(-w, w, -h, h, far, near)
            }
        }
    }

    pub fn view_proj(&self, aspect: f32, bounds: &Aabb) -> Mat4 {
        self.projection_matrix(aspect, bounds) * self.view()
    }

    /// The ray through a point of the view, in normalised device coordinates
    /// (x right, y up, both -1 to 1).
    pub fn ray(&self, ndc: Vec2, aspect: f32) -> Ray {
        let (right, up, fwd) = self.basis();
        match self.projection {
            Projection::Perspective => {
                let t = (self.fov_y * 0.5).tan();
                let dir = (fwd + right * (ndc.x * t * aspect) + up * (ndc.y * t)).normalize();
                Ray {
                    origin: self.eye(),
                    dir,
                }
            }
            Projection::Orthographic => {
                let h = self.half_height();
                Ray {
                    origin: self.eye() + right * (ndc.x * h * aspect) + up * (ndc.y * h),
                    dir: fwd,
                }
            }
        }
    }

    /// Where a world point lands: normalised device coordinates and depth
    /// (1 near, 0 far). `None` when it is behind the camera.
    pub fn project(&self, p: Vec3, aspect: f32, bounds: &Aabb) -> Option<(Vec2, f32)> {
        let c = self.view_proj(aspect, bounds) * p.extend(1.0);
        if c.w <= 0.0 || c.z < 0.0 || c.z > c.w {
            return None;
        }
        Some((Vec2::new(c.x / c.w, c.y / c.w), c.z / c.w))
    }

    /// Turns the camera about the target by a drag, radians.
    pub fn orbit(&mut self, d_yaw: f32, d_pitch: f32) {
        self.yaw = (self.yaw + d_yaw).rem_euclid(TAU);
        self.pitch = (self.pitch + d_pitch).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// Slides the target by a drag in pixels, so the model follows the pointer.
    pub fn pan(&mut self, dx_px: f32, dy_px: f32, viewport_h_px: f32) {
        let per_px = 2.0 * self.half_height() / viewport_h_px.max(1.0);
        let (right, up, _) = self.basis();
        self.target += (-right * dx_px + up * dy_px) * per_px;
    }

    /// Scales the distance by `factor` (below 1 zooms in) keeping the point
    /// under `ndc` where it is: the camera scales about `anchor` (a scene hit)
    /// or, without one, about the point of the target plane under the pointer.
    pub fn zoom_at(&mut self, factor: f32, ndc: Vec2, aspect: f32, anchor: Option<Vec3>) {
        let wanted = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        let f = wanted / self.distance;
        let p = anchor.unwrap_or_else(|| {
            let ray = self.ray(ndc, aspect);
            ray.plane_hit(self.target, self.dir())
                .map_or(self.target, |t| ray.at(t))
        });
        self.target = p + (self.target - p) * f;
        self.distance = wanted;
    }

    /// Frames `bounds` from the current direction, centred, with a margin.
    pub fn fit(&mut self, bounds: &Aabb, aspect: f32) {
        if bounds.is_empty() {
            return;
        }
        let target = bounds.center();
        let (right, up, _) = self.basis();
        let dir = self.dir();
        let tan_v = (self.fov_y * 0.5).tan();
        let tan_h = tan_v * aspect;
        let mut d = MIN_DISTANCE;
        for c in bounds.corners() {
            let rel = c - target;
            let (x, y, z) = (rel.dot(right).abs(), rel.dot(up).abs(), rel.dot(dir));
            let need = match self.projection {
                Projection::Perspective => (z + x / tan_h).max(z + y / tan_v),
                Projection::Orthographic => (x / tan_h).max(y / tan_v),
            };
            d = d.max(need);
        }
        self.target = target;
        self.distance = (d * 1.08).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Looks from a cube face, keeping target and distance.
    pub fn set_preset(&mut self, preset: ViewPreset) {
        let (yaw, pitch) = preset.angles();
        self.yaw = yaw;
        self.pitch = pitch;
    }

    /// A camera `t` of the way from `a` to `b` (0 to 1), turning the short way.
    pub fn lerp(a: &Camera, b: &Camera, t: f32) -> Camera {
        let mut dyaw = (b.yaw - a.yaw).rem_euclid(TAU);
        if dyaw > PI {
            dyaw -= TAU;
        }
        Camera {
            target: a.target.lerp(b.target, t),
            yaw: (a.yaw + dyaw * t).rem_euclid(TAU),
            pitch: a.pitch + (b.pitch - a.pitch) * t,
            distance: a.distance + (b.distance - a.distance) * t,
            fov_y: b.fov_y,
            projection: b.projection,
        }
    }
}

/// Ease-out cubic for camera transitions.
pub fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Aabb {
        Aabb::new(Vec3::new(0.0, 0.0, -3.0), Vec3::new(800.0, 1000.0, 200.0))
    }

    #[test]
    fn front_view_looks_down_minus_z_with_y_up() {
        let mut c = Camera::default();
        c.set_preset(ViewPreset::Front);
        let (right, up, fwd) = c.basis();
        assert!((right - Vec3::X).length() < 1e-5);
        assert!((up - Vec3::Y).length() < 1e-5);
        assert!((fwd - Vec3::NEG_Z).length() < 1e-5);
    }

    #[test]
    fn target_projects_to_the_centre_in_both_projections() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let c = Camera {
                target: Vec3::new(400.0, 500.0, 50.0),
                distance: 1500.0,
                projection,
                ..Camera::default()
            };
            let (ndc, depth) = c.project(c.target, 1.6, &bounds()).unwrap();
            assert!(ndc.length() < 1e-4, "{projection:?} {ndc}");
            assert!(depth > 0.0 && depth <= 1.0);
        }
    }

    #[test]
    fn nearer_points_have_larger_depth() {
        let mut c = Camera::default();
        c.set_preset(ViewPreset::Front);
        c.target = Vec3::new(400.0, 500.0, 0.0);
        let near = c
            .project(Vec3::new(400.0, 500.0, 100.0), 1.0, &bounds())
            .unwrap()
            .1;
        let far = c
            .project(Vec3::new(400.0, 500.0, -100.0), 1.0, &bounds())
            .unwrap()
            .1;
        assert!(near > far);
    }

    #[test]
    fn ray_through_a_projected_point_passes_through_it() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let c = Camera {
                target: Vec3::new(400.0, 500.0, 0.0),
                projection,
                ..Camera::default()
            };
            let p = Vec3::new(300.0, 700.0, 40.0);
            let (ndc, _) = c.project(p, 1.5, &bounds()).unwrap();
            let ray = c.ray(ndc, 1.5);
            let t = (p - ray.origin).dot(ray.dir);
            assert!((ray.at(t) - p).length() < 0.05, "{projection:?}");
        }
    }

    #[test]
    fn zoom_keeps_the_point_under_the_pointer_fixed() {
        for projection in [Projection::Perspective, Projection::Orthographic] {
            let mut c = Camera {
                target: Vec3::new(400.0, 500.0, 0.0),
                projection,
                ..Camera::default()
            };
            let ndc = Vec2::new(0.4, -0.3);
            // The point of the target plane under the pointer.
            let ray = c.ray(ndc, 1.5);
            let p = ray.at(ray.plane_hit(c.target, c.dir()).unwrap());
            c.zoom_at(0.5, ndc, 1.5, None);
            let (after, _) = c.project(p, 1.5, &bounds()).unwrap();
            assert!((after - ndc).length() < 1e-3, "{projection:?} {after}");
            assert!((c.distance - 750.0).abs() < 1e-3);
        }
    }

    #[test]
    fn zoom_is_clamped() {
        let mut c = Camera::default();
        c.zoom_at(1e-6, Vec2::ZERO, 1.0, None);
        assert_eq!(c.distance, MIN_DISTANCE);
        c.zoom_at(1e9, Vec2::ZERO, 1.0, None);
        assert_eq!(c.distance, MAX_DISTANCE);
    }

    #[test]
    fn orbit_clamps_pitch_and_wraps_yaw() {
        let mut c = Camera::default();
        c.orbit(10.0, 10.0);
        assert!(c.pitch <= MAX_PITCH + 1e-6);
        assert!((0.0..TAU).contains(&c.yaw));
        c.orbit(0.0, -100.0);
        assert!(c.pitch >= -MAX_PITCH - 1e-6);
    }

    #[test]
    fn pan_moves_the_model_with_the_pointer() {
        let mut c = Camera::default();
        c.set_preset(ViewPreset::Front);
        let before = c.target;
        c.pan(100.0, 0.0, 800.0);
        // Dragging right: the camera target moves left (-X).
        assert!(c.target.x < before.x);
        c.pan(0.0, 100.0, 800.0);
        // Dragging down: the target moves up.
        assert!(c.target.y > before.y);
    }

    #[test]
    fn fit_shows_every_corner_of_the_bounds() {
        let b = bounds();
        for projection in [Projection::Perspective, Projection::Orthographic] {
            for aspect in [0.8f32, 1.0, 1.78] {
                let mut c = Camera {
                    projection,
                    ..Camera::default()
                };
                c.fit(&b, aspect);
                for corner in b.corners() {
                    let (ndc, _) = c.project(corner, aspect, &b).expect("in front");
                    assert!(
                        ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0,
                        "{projection:?} aspect {aspect}: {corner} at {ndc}"
                    );
                }
            }
        }
    }

    #[test]
    fn lerp_turns_the_short_way() {
        let a = Camera {
            yaw: 0.1,
            ..Camera::default()
        };
        let b = Camera {
            yaw: TAU - 0.1,
            ..Camera::default()
        };
        let mid = Camera::lerp(&a, &b, 0.5);
        assert!(mid.yaw < 0.01 || mid.yaw > TAU - 0.01);
    }
}
