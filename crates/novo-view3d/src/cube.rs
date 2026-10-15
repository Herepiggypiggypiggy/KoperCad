//! View cube hit testing (3d-look.md section 6). The cube is drawn with the
//! camera's own angles; a click returns the direction to look from. Axes
//! match the scene: X right, Y up, Z towards the front of the plate.

use crate::camera::Camera;
use glam::Vec3;

/// A region of the 3 x 3 x 3 cube; each axis is -1, 0 or 1. `[0, 0, 1]` is the
/// front face, `[1, 0, 1]` the front-right edge, `[1, 1, 1]` a corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CubeHit {
    pub dir: [i8; 3],
}

impl CubeHit {
    pub fn is_face(&self) -> bool {
        self.dir.iter().filter(|c| **c != 0).count() == 1
    }

    /// Direction from the target to the eye, not normalised.
    pub fn eye_dir(&self) -> Vec3 {
        Vec3::new(self.dir[0] as f32, self.dir[1] as f32, self.dir[2] as f32)
    }
}

/// Fraction of a face edge that counts as an edge or corner band.
const BAND: f32 = 1.0 / 3.0;

/// Hit test a point of the widget. `local` runs -1 to 1 across the square
/// (x right, y up). Returns the cube region under it, if any.
pub fn view_cube_hit(camera: &Camera, local: [f32; 2]) -> Option<CubeHit> {
    let (right, up, fwd) = camera.basis();
    let reach = 3f32.sqrt();
    let o = right * (local[0] * reach) + up * (local[1] * reach) - fwd * 4.0;
    let (mut t0, mut t1) = (0.0f32, f32::INFINITY);
    for a in 0..3 {
        if fwd[a].abs() < 1e-9 {
            if o[a].abs() > 1.0 {
                return None;
            }
            continue;
        }
        let (mut a0, mut a1) = ((-1.0 - o[a]) / fwd[a], (1.0 - o[a]) / fwd[a]);
        if a0 > a1 {
            std::mem::swap(&mut a0, &mut a1);
        }
        t0 = t0.max(a0);
        t1 = t1.min(a1);
        if t0 > t1 {
            return None;
        }
    }
    let p = o + fwd * t0;
    let face = (0..3)
        .max_by(|a, b| p[*a].abs().total_cmp(&p[*b].abs()))
        .unwrap_or(0);
    let mut dir = [0i8; 3];
    for (a, d) in dir.iter_mut().enumerate() {
        if a == face || p[a].abs() > 1.0 - 2.0 * BAND {
            *d = p[a].signum() as i8;
        }
    }
    Some(CubeHit { dir })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::ViewPreset;
    use crate::feel::{enter_standard, look_from};

    fn front() -> Camera {
        let mut c = Camera::default();
        enter_standard(&mut c, ViewPreset::Front);
        c
    }

    #[test]
    fn the_middle_of_the_front_view_is_the_front_face() {
        let h = view_cube_hit(&front(), [0.0, 0.0]).unwrap();
        assert_eq!(h.dir, [0, 0, 1]);
        assert!(h.is_face());
    }

    #[test]
    fn outside_the_cube_hits_nothing() {
        assert!(view_cube_hit(&front(), [0.95, 0.0]).is_none());
    }

    #[test]
    fn corner_and_edge_bands() {
        assert_eq!(view_cube_hit(&front(), [0.5, 0.5]).unwrap().dir, [1, 1, 1]);
        assert_eq!(view_cube_hit(&front(), [0.5, 0.0]).unwrap().dir, [1, 0, 1]);
    }

    #[test]
    fn iso_shows_top_front_and_right() {
        let mut c = front();
        enter_standard(&mut c, ViewPreset::Iso);
        assert_eq!(view_cube_hit(&c, [0.0, 0.45]).unwrap().dir[1], 1);
        assert_eq!(view_cube_hit(&c, [0.35, -0.2]).unwrap().dir[0], 1);
        assert_eq!(view_cube_hit(&c, [-0.35, -0.2]).unwrap().dir[2], 1);
    }

    #[test]
    fn a_click_turns_the_camera_to_the_hit() {
        let mut c = front();
        enter_standard(&mut c, ViewPreset::Iso);
        let h = view_cube_hit(&c, [0.35, -0.2]).unwrap();
        look_from(&mut c, h.eye_dir());
        assert!(c.eye().x > c.target.x);
    }
}
