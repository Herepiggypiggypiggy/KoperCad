//! Visibility: frustum culling and the screen-size level of detail.

use crate::camera::Camera;
use crate::math::Frustum;
use crate::scene::Scene;
use crate::view_state::ViewState;
use glam::Mat4;

/// Indices into `scene.items` of the items that are shown and inside the
/// frustum. `out` is cleared first so a caller can reuse its allocation.
pub fn visible(scene: &Scene, state: &ViewState, view_proj: &Mat4, out: &mut Vec<usize>) {
    out.clear();
    let frustum = Frustum::from_view_proj(view_proj);
    for (i, item) in scene.items.iter().enumerate() {
        if state.is_visible(item.id.kind) && frustum.intersects(&scene.world_bounds(item)) {
            out.push(i);
        }
    }
}

/// Approximate size on screen, in pixels, of a sphere of `radius` mm at
/// `distance` mm from the eye, for a viewport `height_px` tall.
pub fn screen_size_px(camera: &Camera, radius: f32, distance: f32, height_px: f32) -> f32 {
    let half = (camera.fov_y * 0.5).tan().max(1e-4);
    let d = match camera.projection {
        crate::camera::Projection::Perspective => distance.max(1e-3),
        crate::camera::Projection::Orthographic => camera.distance.max(1e-3),
    };
    2.0 * radius / (2.0 * d * half) * height_px
}

/// Which level of detail to draw: 0 is the most detailed. Below 24 px the
/// coarse levels take over; `None` below 3 px (not worth a draw).
pub fn lod_for(size_px: f32, levels: usize) -> Option<usize> {
    if levels == 0 || size_px < 3.0 {
        return None;
    }
    let want = if size_px >= 160.0 {
        0
    } else if size_px >= 24.0 {
        1
    } else {
        2
    };
    Some(want.min(levels - 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, ItemId, ItemKind, PlateFrame, Pose, Shape};
    use glam::Vec3;

    fn scene_with(positions: &[Vec3]) -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        for (i, p) in positions.iter().enumerate() {
            s.push(Item::new(
                ItemId::new(ItemKind::Device, i as u64),
                Shape::Box,
                Pose::at(*p),
                Vec3::splat(10.0),
            ));
        }
        s
    }

    #[test]
    fn items_behind_and_beside_the_camera_are_culled() {
        let scene = scene_with(&[
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(5000.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 5000.0),
        ]);
        let cam = Camera {
            target: Vec3::ZERO,
            distance: 300.0,
            yaw: 0.0,
            pitch: 0.0,
            ..Camera::default()
        };
        let vp = cam.view_proj(1.5, &scene.bounds());
        let mut out = Vec::new();
        visible(&scene, &ViewState::default(), &vp, &mut out);
        assert_eq!(out, vec![0]);
    }

    #[test]
    fn hidden_kinds_are_skipped() {
        let scene = scene_with(&[Vec3::ZERO]);
        let cam = Camera {
            distance: 300.0,
            ..Camera::default()
        };
        let vp = cam.view_proj(1.0, &scene.bounds());
        let mut state = ViewState::default();
        state.hidden.insert(ItemKind::Device);
        let mut out = Vec::new();
        visible(&scene, &state, &vp, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn detail_follows_screen_size() {
        assert_eq!(lod_for(500.0, 3), Some(0));
        assert_eq!(lod_for(50.0, 3), Some(1));
        assert_eq!(lod_for(10.0, 3), Some(2));
        assert_eq!(lod_for(10.0, 2), Some(1));
        assert_eq!(lod_for(2.0, 3), None);
        assert_eq!(lod_for(100.0, 0), None);
    }
}
