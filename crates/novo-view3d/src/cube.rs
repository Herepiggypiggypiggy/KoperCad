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

/// One ninth of a visible cube face, for drawing: the region a click on it
/// would pick, with its corners in the same local space as [`view_cube_hit`]
/// (-1 to 1, x right, y up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubeCell {
    pub dir: [i8; 3],
    pub quad: [[f32; 2]; 4],
    /// How squarely the face looks at the eye, 0 to 1 (for shading).
    pub facing: f32,
}

/// The cells of the faces turned towards the camera. Visible faces of a cube
/// do not overlap on screen, so draw order does not matter.
pub fn visible_cells(camera: &Camera) -> Vec<CubeCell> {
    let (right, up, fwd) = camera.basis();
    let reach = 3f32.sqrt();
    let mut out = Vec::new();
    for axis in 0..3 {
        for sign in [-1i8, 1] {
            let mut normal = Vec3::ZERO;
            normal[axis] = sign as f32;
            let facing = normal.dot(-fwd);
            if facing <= 1e-4 {
                continue;
            }
            let (b, c) = ((axis + 1) % 3, (axis + 2) % 3);
            for i in -1i8..=1 {
                for j in -1i8..=1 {
                    let corner = |u: f32, v: f32| {
                        let mut p = Vec3::ZERO;
                        p[axis] = sign as f32;
                        p[b] = (2.0 * i as f32 + u) / 3.0;
                        p[c] = (2.0 * j as f32 + v) / 3.0;
                        [p.dot(right) / reach, p.dot(up) / reach]
                    };
                    let mut dir = [0i8; 3];
                    dir[axis] = sign;
                    dir[b] = i;
                    dir[c] = j;
                    out.push(CubeCell {
                        dir,
                        quad: [
                            corner(-1.0, -1.0),
                            corner(1.0, -1.0),
                            corner(1.0, 1.0),
                            corner(-1.0, 1.0),
                        ],
                        facing,
                    });
                }
            }
        }
    }
    out
}

/// The standard view a face click goes to; edges and corners have none.
pub fn preset_of(hit: &CubeHit) -> Option<crate::camera::ViewPreset> {
    use crate::camera::ViewPreset::*;
    match hit.dir {
        [0, 0, 1] => Some(Front),
        [0, 0, -1] => Some(Back),
        [-1, 0, 0] => Some(Left),
        [1, 0, 0] => Some(Right),
        [0, 1, 0] => Some(Top),
        [0, -1, 0] => Some(Bottom),
        _ => None,
    }
}

/// Short name written on a face.
pub fn face_name(dir: [i8; 3]) -> Option<&'static str> {
    match dir {
        [0, 0, 1] => Some("Front"),
        [0, 0, -1] => Some("Back"),
        [-1, 0, 0] => Some("Left"),
        [1, 0, 0] => Some("Right"),
        [0, 1, 0] => Some("Top"),
        [0, -1, 0] => Some("Bottom"),
        _ => None,
    }
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

    #[test]
    fn front_view_shows_nine_cells_of_one_face() {
        let cells = visible_cells(&front());
        assert_eq!(cells.len(), 9);
        assert!(cells.iter().all(|c| c.dir[2] == 1));
    }

    #[test]
    fn iso_shows_three_faces_and_cells_agree_with_hit_test() {
        let mut c = front();
        enter_standard(&mut c, ViewPreset::Iso);
        let cells = visible_cells(&c);
        assert_eq!(cells.len(), 27);
        for cell in cells {
            let mx = cell.quad.iter().map(|q| q[0]).sum::<f32>() / 4.0;
            let my = cell.quad.iter().map(|q| q[1]).sum::<f32>() / 4.0;
            let hit = view_cube_hit(&c, [mx, my]).unwrap();
            assert_eq!(hit.dir, cell.dir);
        }
    }

    #[test]
    fn faces_map_to_presets() {
        assert_eq!(
            preset_of(&CubeHit { dir: [0, 1, 0] }),
            Some(ViewPreset::Top)
        );
        assert_eq!(preset_of(&CubeHit { dir: [1, 0, 1] }), None);
        assert_eq!(face_name([0, 0, 1]), Some("Front"));
    }
}
