//! Door hinge animation: a view-side eased angle for a hinge group.
//!
//! The angle is view state, not model state: the scene keeps the angle the
//! builder gave it and the widget draws a copy with the animated angle, so
//! opening the door never touches the project and survives scene rebuilds.

use crate::camera::ease_out;
use crate::scene::{GroupId, Scene};

/// A door swings slower than the camera moves; reduced motion jumps.
pub const DOOR_SECONDS: f64 = 0.5;

/// An angle easing from where it is to a target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoorAnim {
    pub group: GroupId,
    from: f32,
    to: f32,
    start: f64,
    duration: f64,
}

impl DoorAnim {
    /// A door at rest at `angle` (radians).
    pub fn at(group: GroupId, angle: f32) -> Self {
        Self {
            group,
            from: angle,
            to: angle,
            start: 0.0,
            duration: 0.0,
        }
    }

    /// The angle at time `now` (seconds).
    pub fn angle(&self, now: f64) -> f32 {
        if self.duration <= 0.0 {
            return self.to;
        }
        let t = ((now - self.start) / self.duration).clamp(0.0, 1.0) as f32;
        self.from + (self.to - self.from) * ease_out(t)
    }

    pub fn target(&self) -> f32 {
        self.to
    }

    pub fn animating(&self, now: f64) -> bool {
        self.duration > 0.0 && now - self.start < self.duration && self.from != self.to
    }

    /// Eases from the current angle to `to`; with `reduced` motion it jumps.
    pub fn set_target(&mut self, to: f32, now: f64, reduced: bool) {
        if to == self.to {
            return;
        }
        self.from = self.angle(now);
        self.to = to;
        self.start = now;
        self.duration = if reduced { 0.0 } else { DOOR_SECONDS };
    }
}

/// The signed angle that opens the hinge `group` by `open_deg` degrees to the
/// outside, whichever side it is hinged on (the scene builder hangs a
/// left-hinged door on a pivot at x = 0 and turns it the other way round).
pub fn open_angle(scene: &Scene, group: GroupId, open_deg: f32) -> Option<f32> {
    let g = scene.group(group)?;
    let sign = if g.pivot.x < scene.frame.width * 0.5 {
        -1.0
    } else {
        1.0
    };
    Some(sign * open_deg.to_radians())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Group, PlateFrame};
    use glam::Vec3;

    fn scene(pivot_x: f32) -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 600.0,
            height: 800.0,
        });
        s.groups.push(Group {
            id: GroupId(1),
            pivot: Vec3::new(pivot_x, 0.0, 250.0),
            axis: Vec3::Y,
            angle: 0.0,
        });
        s
    }

    #[test]
    fn the_angle_eases_to_the_target_and_stops() {
        let mut d = DoorAnim::at(GroupId(1), 0.0);
        assert!(!d.animating(0.0));
        d.set_target(1.5, 10.0, false);
        assert!(d.animating(10.1));
        assert_eq!(d.angle(10.0), 0.0);
        let mid = d.angle(10.0 + DOOR_SECONDS * 0.5);
        assert!(mid > 0.75 && mid < 1.5, "ease-out is past half way: {mid}");
        assert_eq!(d.angle(10.0 + DOOR_SECONDS), 1.5);
        assert!(!d.animating(10.0 + DOOR_SECONDS + 0.01));
    }

    #[test]
    fn retargeting_mid_swing_starts_from_where_it_is() {
        let mut d = DoorAnim::at(GroupId(1), 0.0);
        d.set_target(1.0, 0.0, false);
        let here = d.angle(0.2);
        d.set_target(0.0, 0.2, false);
        assert!((d.angle(0.2) - here).abs() < 1e-6);
        assert_eq!(d.angle(0.2 + DOOR_SECONDS), 0.0);
    }

    #[test]
    fn reduced_motion_jumps() {
        let mut d = DoorAnim::at(GroupId(1), 0.0);
        d.set_target(1.0, 5.0, true);
        assert_eq!(d.angle(5.0), 1.0);
        assert!(!d.animating(5.0));
    }

    #[test]
    fn the_open_direction_follows_the_hinge_side() {
        let left = open_angle(&scene(0.0), GroupId(1), 90.0).unwrap();
        let right = open_angle(&scene(600.0), GroupId(1), 90.0).unwrap();
        assert!(left < 0.0 && right > 0.0);
        assert!((right - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!(open_angle(&scene(0.0), GroupId(9), 90.0).is_none());
    }
}
