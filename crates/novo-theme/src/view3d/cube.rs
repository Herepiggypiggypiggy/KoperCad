//! View cube hit testing (3d-look.md section 6). The cube is drawn with the
//! camera's own yaw and pitch, so hit testing needs only the camera angles.
//! Faces, edges and corners are the 26 regions of a 3 x 3 x 3 cube; a click
//! returns the direction to look from.

use super::camera::Camera;

/// A cube region: each axis is -1, 0 or 1 (X right, Y back, Z up). `[0, -1, 0]`
/// is the front face, `[1, -1, 1]` the front-right-top corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CubeHit {
    pub dir: [i8; 3],
}

impl CubeHit {
    pub fn is_face(&self) -> bool {
        self.dir.iter().filter(|c| **c != 0).count() == 1
    }

    /// Direction from the target to the eye (not normalised).
    pub fn eye_dir(&self) -> [f64; 3] {
        [self.dir[0] as f64, self.dir[1] as f64, self.dir[2] as f64]
    }
}

/// Fraction of a face edge that counts as the edge or corner band.
const BAND: f64 = 1.0 / 3.0;

/// Hit test a point inside the cube widget. `local` is -1 to 1 in both axes
/// (x right, y up) across the widget square. Returns the region under it.
pub fn view_cube_hit(camera: &Camera, local: [f64; 2]) -> Option<CubeHit> {
    // The unit cube spans -1..1; the widget square covers its silhouette
    // (half diagonal at most), so scale local coordinates to that.
    let reach = 3f64.sqrt();
    let (r, u, f) = (camera.right(), camera.up(), camera.forward());
    let o = [
        r[0] * local[0] * reach + u[0] * local[1] * reach - f[0] * 4.0,
        r[1] * local[0] * reach + u[1] * local[1] * reach - f[1] * 4.0,
        r[2] * local[0] * reach + u[2] * local[1] * reach - f[2] * 4.0,
    ];
    // Slab test against [-1, 1]^3.
    let (mut t0, mut t1) = (0.0f64, f64::INFINITY);
    for a in 0..3 {
        if f[a].abs() < 1e-12 {
            if o[a].abs() > 1.0 {
                return None;
            }
            continue;
        }
        let (mut a0, mut a1) = ((-1.0 - o[a]) / f[a], (1.0 - o[a]) / f[a]);
        if a0 > a1 {
            std::mem::swap(&mut a0, &mut a1);
        }
        t0 = t0.max(a0);
        t1 = t1.min(a1);
        if t0 > t1 {
            return None;
        }
    }
    let p = [o[0] + f[0] * t0, o[1] + f[1] * t0, o[2] + f[2] * t0];
    // The entry point lies on one face: its axis is the largest coordinate.
    let face = (0..3)
        .max_by(|a, b| p[*a].abs().total_cmp(&p[*b].abs()))
        .unwrap_or(0);
    let mut dir = [0i8; 3];
    for a in 0..3 {
        if a == face || p[a].abs() > 1.0 - 2.0 * BAND {
            dir[a] = p[a].signum() as i8;
        }
    }
    Some(CubeHit { dir })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view3d::camera::StandardView;

    fn front_cam() -> Camera {
        let mut c = Camera::home([0.0; 3], 800.0);
        c.set_standard(StandardView::Front);
        c
    }

    #[test]
    fn centre_of_the_front_view_hits_the_front_face() {
        let h = view_cube_hit(&front_cam(), [0.0, 0.0]).unwrap();
        assert_eq!(h.dir, [0, -1, 0]);
        assert!(h.is_face());
    }

    #[test]
    fn outside_the_cube_hits_nothing() {
        assert!(view_cube_hit(&front_cam(), [0.95, 0.0]).is_none());
    }

    #[test]
    fn corner_region_of_the_front_face_is_a_corner() {
        // Front view: the cube spans about +-0.58 of the widget.
        let h = view_cube_hit(&front_cam(), [0.5, 0.5]).unwrap();
        assert_eq!(h.dir, [1, -1, 1]);
    }

    #[test]
    fn edge_band_is_an_edge() {
        let h = view_cube_hit(&front_cam(), [0.5, 0.0]).unwrap();
        assert_eq!(h.dir, [1, -1, 0]);
    }

    #[test]
    fn iso_camera_shows_top_front_and_right_faces() {
        let mut c = front_cam();
        c.set_standard(StandardView::Isometric);
        let top = view_cube_hit(&c, [0.0, 0.45]).unwrap();
        assert_eq!(top.dir[2], 1);
        let right = view_cube_hit(&c, [0.35, -0.2]).unwrap();
        assert_eq!(right.dir[0], 1);
        let left = view_cube_hit(&c, [-0.35, -0.2]).unwrap();
        assert_eq!(left.dir[1], -1);
    }

    #[test]
    fn clicking_a_hit_turns_the_camera_to_it() {
        let mut c = front_cam();
        c.set_standard(StandardView::Isometric);
        let h = view_cube_hit(&c, [0.35, -0.2]).unwrap();
        let mut c2 = c;
        c2.look_from(h.eye_dir());
        assert!(c2.eye()[0] > c2.target[0]);
    }
}
