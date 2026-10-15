//! CPU picking against the oriented boxes of the items.
//!
//! A cabinet has hundreds of items, so a linear scan is microseconds, needs no
//! GPU read-back and can be tested headless. Translucent items (door, shell)
//! are only picked when nothing opaque is behind the pointer.

use crate::camera::Camera;
use crate::math::Ray;
use crate::scene::{ItemId, Scene};
use crate::view_state::ViewState;
use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub id: ItemId,
    /// Distance along the ray, mm.
    pub t: f32,
    pub point: Vec3,
}

/// Every item the ray crosses, nearest first, opaque items before translucent
/// ones.
pub fn pick_all(scene: &Scene, state: &ViewState, ray: &Ray) -> Vec<Hit> {
    let mut hits: Vec<(bool, Hit)> = Vec::new();
    for item in &scene.items {
        if !state.is_visible(item.id.kind) {
            continue;
        }
        let m = scene.world_matrix(item);
        let local = ray.to_local(&m);
        if let Some((near, _)) = item.local_bounds().ray_interval(&local) {
            let t = near.max(0.0);
            hits.push((
                item.translucent,
                Hit {
                    id: item.id,
                    t,
                    point: ray.at(t),
                },
            ));
        }
    }
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.t.total_cmp(&b.1.t)));
    hits.into_iter().map(|(_, h)| h).collect()
}

/// The item under the ray. `skip` steps behind the first hit, for cycling
/// through stacked items with repeated clicks.
pub fn pick(scene: &Scene, state: &ViewState, ray: &Ray, skip: usize) -> Option<Hit> {
    let hits = pick_all(scene, state, ray);
    if hits.is_empty() {
        None
    } else {
        Some(hits[skip % hits.len()])
    }
}

/// Items counted from the back: `k = 0` is the deepest opaque item under the
/// ray, `1` the one in front of it, and so on, wrapping (Alt + click, repeated
/// at the same spot). Translucent items count only when nothing opaque is hit.
pub fn pick_from_back(scene: &Scene, state: &ViewState, ray: &Ray, k: usize) -> Option<Hit> {
    let hits = pick_all(scene, state, ray);
    let opaque = |h: &Hit| scene.get(h.id).is_some_and(|i| !i.translucent);
    let n_opaque = hits.iter().filter(|h| opaque(h)).count();
    let list = if n_opaque > 0 {
        &hits[..n_opaque]
    } else {
        &hits[..]
    };
    (!list.is_empty()).then(|| list[list.len() - 1 - k % list.len()])
}

/// The deepest item under the ray (`pick_from_back` with `k = 0`).
pub fn pick_deepest(scene: &Scene, state: &ViewState, ray: &Ray) -> Option<Hit> {
    pick_from_back(scene, state, ray, 0)
}

/// Items whose projected box overlaps the screen rectangle given by two
/// corners in normalised device coordinates. Items behind the camera are
/// ignored.
pub fn marquee(
    scene: &Scene,
    state: &ViewState,
    camera: &Camera,
    aspect: f32,
    a: Vec2,
    b: Vec2,
) -> Vec<ItemId> {
    let (lo, hi) = (a.min(b), a.max(b));
    let bounds = scene.bounds();
    let vp = camera.view_proj(aspect, &bounds);
    let mut out = Vec::new();
    for item in &scene.items {
        if !state.is_visible(item.id.kind) || item.translucent {
            continue;
        }
        let world = scene.world_bounds(item);
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        let mut all_front = true;
        for c in world.corners() {
            let clip = vp * c.extend(1.0);
            if clip.w <= 0.0 {
                all_front = false;
                break;
            }
            let p = Vec2::new(clip.x / clip.w, clip.y / clip.w);
            min = min.min(p);
            max = max.max(p);
        }
        if all_front && min.x <= hi.x && max.x >= lo.x && min.y <= hi.y && max.y >= lo.y {
            out.push(item.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, ItemKind, PlateFrame, Pose, Shape};
    use glam::Quat;

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 600.0,
            height: 800.0,
        });
        // A plate behind, a device in front of it, a rotated device beside.
        s.push(Item::new(
            ItemId::new(ItemKind::Plate, 1),
            Shape::Box,
            Pose::at(Vec3::new(300.0, 400.0, -3.0)),
            Vec3::new(600.0, 800.0, 3.0),
        ));
        s.push(Item::new(
            ItemId::new(ItemKind::Device, 2),
            Shape::Box,
            Pose::at(Vec3::new(100.0, 100.0, 7.5)),
            Vec3::new(45.0, 90.0, 60.0),
        ));
        s.push(Item::new(
            ItemId::new(ItemKind::Device, 3),
            Shape::Box,
            Pose {
                translation: Vec3::new(300.0, 100.0, 7.5),
                rotation: Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2),
            },
            Vec3::new(45.0, 90.0, 60.0),
        ));
        s
    }

    fn front_ray(x: f32, y: f32) -> Ray {
        Ray {
            origin: Vec3::new(x, y, 1000.0),
            dir: Vec3::NEG_Z,
        }
    }

    #[test]
    fn the_nearest_item_wins_and_the_plate_is_behind() {
        let s = scene();
        let hits = pick_all(&s, &ViewState::default(), &front_ray(100.0, 100.0));
        let ids: Vec<u64> = hits.iter().map(|h| h.id.raw).collect();
        assert_eq!(ids, vec![2, 1]);
        assert!((hits[0].t - (1000.0 - 67.5)).abs() < 1e-3);
        assert_eq!(
            pick(&s, &ViewState::default(), &front_ray(100.0, 100.0), 1)
                .unwrap()
                .id
                .raw,
            1
        );
    }

    #[test]
    fn alt_click_takes_the_deepest_opaque_item() {
        let mut s = scene();
        let mut door = Item::new(
            ItemId::new(ItemKind::Door, 9),
            Shape::Box,
            Pose::at(Vec3::new(300.0, 400.0, 150.0)),
            Vec3::new(600.0, 800.0, 3.0),
        );
        door.translucent = true;
        s.push(door);
        let state = ViewState::default();
        let deep = pick_deepest(&s, &state, &front_ray(100.0, 100.0)).unwrap();
        assert_eq!(deep.id.raw, 1, "the plate under the device, not the door");
        // Only a door in the way: it is the deepest there is.
        let only_door = pick_deepest(&s, &state, &front_ray(580.0, 780.0)).unwrap();
        assert_eq!(only_door.id.raw, 1);
        let again = pick_from_back(&s, &state, &front_ray(100.0, 100.0), 1).unwrap();
        assert_eq!(again.id.raw, 2, "the next click steps up to the device");
        let wrapped = pick_from_back(&s, &state, &front_ray(100.0, 100.0), 2).unwrap();
        assert_eq!(wrapped.id.raw, 1);
        s.items.remove(0);
        let door_only = pick_deepest(&s, &state, &front_ray(580.0, 780.0)).unwrap();
        assert_eq!(door_only.id.raw, 9);
    }

    #[test]
    fn a_rotated_box_is_hit_where_it_is_turned() {
        let s = scene();
        let state = ViewState::default();
        // Turned -90 degrees: 90 wide (x) and 45 tall (y) around (300, 100).
        let inside = pick(&s, &state, &front_ray(340.0, 100.0), 0).unwrap();
        assert_eq!(inside.id.raw, 3);
        let above = pick(&s, &state, &front_ray(300.0, 130.0), 0).unwrap();
        assert_eq!(
            above.id.raw, 1,
            "outside the turned box falls through to the plate"
        );
    }

    #[test]
    fn misses_and_hidden_kinds_give_nothing() {
        let s = scene();
        let mut state = ViewState::default();
        assert!(pick(&s, &state, &front_ray(-50.0, -50.0), 0).is_none());
        state.hidden.insert(ItemKind::Device);
        state.hidden.insert(ItemKind::Plate);
        assert!(pick(&s, &state, &front_ray(100.0, 100.0), 0).is_none());
    }

    #[test]
    fn translucent_items_rank_behind_opaque_ones() {
        let mut s = scene();
        let mut door = Item::new(
            ItemId::new(ItemKind::Door, 9),
            Shape::Box,
            Pose::at(Vec3::new(300.0, 400.0, 150.0)),
            Vec3::new(600.0, 800.0, 3.0),
        );
        door.translucent = true;
        s.push(door);
        let ids: Vec<u64> = pick_all(&s, &ViewState::default(), &front_ray(100.0, 100.0))
            .iter()
            .map(|h| h.id.raw)
            .collect();
        assert_eq!(ids, vec![2, 1, 9]);
    }

    #[test]
    fn marquee_selects_what_it_covers_in_the_picture() {
        let s = scene();
        let state = ViewState::default();
        let mut cam = Camera::default();
        cam.set_preset(crate::camera::ViewPreset::Front);
        cam.fit(&s.bounds(), 1.0);
        let all = marquee(&s, &state, &cam, 1.0, Vec2::splat(-1.0), Vec2::splat(1.0));
        assert_eq!(all.len(), 3);
        // A small rectangle around device 2 (upper left of the plate).
        let (p, _) = cam
            .project(Vec3::new(100.0, 100.0, 37.0), 1.0, &s.bounds())
            .unwrap();
        let some = marquee(
            &s,
            &state,
            &cam,
            1.0,
            p - Vec2::splat(0.02),
            p + Vec2::splat(0.02),
        );
        assert!(some.iter().any(|i| i.raw == 2));
        assert!(!some.iter().any(|i| i.raw == 3));
    }
}
