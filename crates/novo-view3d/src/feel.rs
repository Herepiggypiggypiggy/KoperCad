//! How the camera moves (3d-look.md section 6): the feel constants, eased
//! transitions, drag inertia, reduced motion, pivot choice and saved views.
//! It works on [`Camera`] and leaves its maths alone.

use crate::camera::{Camera, Projection, ease_out};
use glam::Vec3;
use std::f32::consts::PI;

/// Orbit speed: 0.25 degrees per pixel, in radians.
pub const ORBIT_RAD_PER_PX: f32 = 0.25 * PI / 180.0;
/// One wheel notch zooms by 12%.
pub const ZOOM_PER_NOTCH: f32 = 0.12;
pub const TRANSITION_SECS: f32 = 0.22;
pub const INERTIA_HALF_LIFE_SECS: f32 = 0.09;
/// Right drag under this many points is a click (context menu), as in 2D.
pub const DRAG_THRESHOLD_PX: f32 = 4.0;
const STOP_PX_PER_SEC: f32 = 8.0;

/// Distance factor for a wheel step; positive notches zoom in.
pub fn zoom_factor(notches: f32) -> f32 {
    (1.0 - ZOOM_PER_NOTCH).powf(notches)
}

/// Yaw and pitch change for a drag in pixels. Dragging right moves the scene
/// right, so the eye goes left.
pub fn orbit_delta(dx_px: f32, dy_px: f32) -> (f32, f32) {
    (-dx_px * ORBIT_RAD_PER_PX, dy_px * ORBIT_RAD_PER_PX)
}

/// What the orbit turns about: the selection centre if there is one, else the
/// scene point under the pointer at drag start, else the old target.
pub fn orbit_pivot(selection_center: Option<Vec3>, hit: Option<Vec3>, target: Vec3) -> Vec3 {
    selection_center.or(hit).unwrap_or(target)
}

/// Move the orbit pivot without moving the picture: the camera keeps its
/// eye and direction, only the target slides along the view line.
pub fn rebase_pivot(camera: &mut Camera, pivot: Vec3) {
    let eye = camera.eye();
    camera.target = pivot;
    camera.distance = (eye - pivot).length().max(crate::camera::MIN_DISTANCE);
    let d = (eye - pivot) / camera.distance;
    camera.yaw = d.x.atan2(d.z).rem_euclid(std::f32::consts::TAU);
    camera.pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-1.553, 1.553);
}

/// An eased move between two cameras. Reduced motion completes at once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    from: Camera,
    to: Camera,
    t: f32,
    secs: f32,
}

impl Transition {
    pub fn new(from: Camera, to: Camera, reduced_motion: bool) -> Self {
        Self {
            from,
            to,
            t: 0.0,
            secs: if reduced_motion { 0.0 } else { TRANSITION_SECS },
        }
    }

    pub fn done(&self) -> bool {
        self.secs <= 0.0 || self.t >= 1.0
    }

    /// Advance by `dt` seconds and give the camera to show. The projection
    /// switches only at the end so nothing jumps mid flight.
    pub fn step(&mut self, dt: f32) -> Camera {
        if self.secs <= 0.0 {
            self.t = 1.0;
            return self.to;
        }
        self.t = (self.t + dt / self.secs).min(1.0);
        let mut c = Camera::lerp(&self.from, &self.to, ease_out(self.t));
        if self.t < 1.0 {
            c.projection = self.from.projection;
        } else {
            c = self.to;
        }
        c
    }
}

/// Orbit momentum after a drag is released: a 90 ms half life, stopped by a
/// click, absent with reduced motion.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Inertia {
    /// Pixels per second.
    pub vx: f32,
    pub vy: f32,
}

impl Inertia {
    pub fn release(vx: f32, vy: f32, reduced_motion: bool) -> Self {
        if reduced_motion {
            Self::default()
        } else {
            Self { vx, vy }
        }
    }

    pub fn active(&self) -> bool {
        self.vx.hypot(self.vy) > STOP_PX_PER_SEC
    }

    pub fn stop(&mut self) {
        *self = Self::default();
    }

    pub fn step(&mut self, camera: &mut Camera, dt: f32) {
        if !self.active() {
            self.stop();
            return;
        }
        let (dy, dp) = orbit_delta(self.vx * dt, self.vy * dt);
        camera.orbit(dy, dp);
        let k = 0.5f32.powf(dt / INERTIA_HALF_LIFE_SECS);
        self.vx *= k;
        self.vy *= k;
    }
}

/// Look from a direction (target to eye), keeping distance. Straight up or
/// down keeps the current yaw.
pub fn look_from(camera: &mut Camera, dir: Vec3) {
    let n = dir.length();
    if n < 1e-6 {
        return;
    }
    let d = dir / n;
    camera.pitch = d.y.clamp(-1.0, 1.0).asin().clamp(-1.553, 1.553);
    if d.x.hypot(d.z) > 1e-4 {
        camera.yaw = d.x.atan2(d.z).rem_euclid(std::f32::consts::TAU);
    }
}

/// Entering an axis aligned view switches to orthographic; the first orbit
/// afterwards returns to perspective.
pub fn enter_standard(camera: &mut Camera, preset: crate::camera::ViewPreset) {
    camera.set_preset(preset);
    camera.projection = if preset == crate::camera::ViewPreset::Iso {
        Projection::Perspective
    } else {
        Projection::Orthographic
    };
}

pub fn leave_auto_ortho(camera: &mut Camera) {
    camera.projection = Projection::Perspective;
}

/// A named camera stored with the project, with the cutaway flag.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedView {
    pub name: String,
    pub camera: Camera,
    pub cutaway: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::ViewPreset;

    fn cam() -> Camera {
        Camera {
            target: Vec3::new(300.0, 400.0, 0.0),
            ..Camera::default()
        }
    }

    #[test]
    fn a_wheel_notch_is_twelve_percent() {
        assert!((zoom_factor(1.0) - 0.88).abs() < 1e-6);
        assert!((zoom_factor(-1.0) - 1.0 / 0.88).abs() < 0.2);
    }

    #[test]
    fn orbit_is_a_quarter_degree_per_pixel_and_dragging_right_turns_the_eye_left() {
        let (dyaw, dpitch) = orbit_delta(100.0, 0.0);
        assert!((dyaw + 25.0f32.to_radians()).abs() < 1e-6);
        assert_eq!(dpitch, 0.0);
    }

    #[test]
    fn pivot_prefers_selection_then_hit_then_target() {
        let t = Vec3::ZERO;
        let (a, b) = (Vec3::X, Vec3::Y);
        assert_eq!(orbit_pivot(Some(a), Some(b), t), a);
        assert_eq!(orbit_pivot(None, Some(b), t), b);
        assert_eq!(orbit_pivot(None, None, t), t);
    }

    #[test]
    fn rebasing_the_pivot_keeps_the_eye() {
        let mut c = cam();
        let eye = c.eye();
        rebase_pivot(&mut c, Vec3::new(100.0, 200.0, 50.0));
        assert!((c.eye() - eye).length() < 0.01, "{:?} {:?}", c.eye(), eye);
    }

    #[test]
    fn transitions_take_the_short_way_and_end_exactly() {
        let mut a = cam();
        a.yaw = 170f32.to_radians();
        let mut b = a;
        b.yaw = 190f32.to_radians();
        let mut t = Transition::new(a, b, false);
        let mid = t.step(TRANSITION_SECS / 4.0);
        assert!(mid.yaw > 170f32.to_radians() && mid.yaw < 190f32.to_radians());
        assert_eq!(t.step(1.0), b);
        assert!(t.done());
    }

    #[test]
    fn projection_switches_at_the_end() {
        let a = cam();
        let mut b = a;
        b.projection = Projection::Orthographic;
        let mut t = Transition::new(a, b, false);
        assert_eq!(t.step(0.05).projection, Projection::Perspective);
        assert_eq!(t.step(1.0).projection, Projection::Orthographic);
    }

    #[test]
    fn reduced_motion_jumps_and_has_no_inertia() {
        let a = cam();
        let mut b = a;
        b.yaw = 1.0;
        let mut t = Transition::new(a, b, true);
        assert!(t.done());
        assert_eq!(t.step(0.0), b);
        assert!(!Inertia::release(900.0, 0.0, true).active());
    }

    #[test]
    fn inertia_halves_every_90_ms_then_stops() {
        let mut c = cam();
        let mut i = Inertia::release(800.0, 0.0, false);
        i.step(&mut c, INERTIA_HALF_LIFE_SECS);
        assert!((i.vx - 400.0).abs() < 1.0);
        for _ in 0..300 {
            i.step(&mut c, 0.016);
        }
        assert!(!i.active());
    }

    #[test]
    fn look_from_sets_angles_and_keeps_yaw_when_straight_down() {
        let mut c = cam();
        look_from(&mut c, Vec3::X);
        assert!((c.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
        assert!(c.pitch.abs() < 1e-5);
        look_from(&mut c, Vec3::Y);
        assert!(c.pitch > 1.5);
        assert!((c.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    }

    #[test]
    fn standard_views_are_orthographic_and_iso_is_not() {
        let mut c = cam();
        enter_standard(&mut c, ViewPreset::Top);
        assert_eq!(c.projection, Projection::Orthographic);
        enter_standard(&mut c, ViewPreset::Iso);
        assert_eq!(c.projection, Projection::Perspective);
        enter_standard(&mut c, ViewPreset::Front);
        leave_auto_ortho(&mut c);
        assert_eq!(c.projection, Projection::Perspective);
    }
}
