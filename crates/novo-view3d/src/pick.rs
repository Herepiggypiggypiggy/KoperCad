//! CPU picking against the oriented boxes of the items.
//!
//! A cabinet has hundreds of items, so a linear scan is microseconds, needs no
//! GPU read-back and can be tested headless. Translucent items (door, shell)
//! are only picked when nothing opaque is behind the pointer.

use crate::camera::Camera;
use crate::math::{Aabb, Ray};
use crate::scene::{ItemId, ItemKind, Scene};
use crate::view_state::ViewState;
use crate::wire;
use glam::{Vec2, Vec3};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub id: ItemId,
    /// Distance along the ray, mm.
    pub t: f32,
    pub point: Vec3,
    /// The item is see-through (door, shell): picked only when nothing
    /// opaque is behind the pointer.
    pub translucent: bool,
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
                    translucent: item.translucent,
                },
            ));
        }
    }
    if state.is_visible(ItemKind::Wire) {
        for wire in &scene.wires {
            let r = wire.radius.max(wire::MIN_PICK_RADIUS);
            let mut best: Option<f32> = None;
            for seg in wire::segments(wire) {
                let local = ray.to_local(&seg.frame());
                let b = Aabb::new(Vec3::new(-r, -r, 0.0), Vec3::new(r, r, seg.length()));
                if let Some((near, _)) = b.ray_interval(&local) {
                    best = Some(best.map_or(near.max(0.0), |t| t.min(near.max(0.0))));
                }
            }
            if let Some(t) = best {
                hits.push((
                    false,
                    Hit {
                        id: wire.id,
                        t,
                        point: ray.at(t),
                        translucent: false,
                    },
                ));
            }
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
    let n_opaque = hits.iter().filter(|h| !h.translucent).count();
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
/// corners in normalised device coordinates. A box that straddles the eye
/// plane is kept when the part in front of the camera overlaps (its projected
/// extent there is unbounded, so the test is generous); a box wholly behind
/// the camera is ignored.
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
    let overlaps = |world: &Aabb| -> bool {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        let mut front = 0;
        let mut behind = false;
        for c in world.corners() {
            let clip = vp * c.extend(1.0);
            if clip.w <= 0.0 {
                behind = true;
                continue;
            }
            front += 1;
            let p = Vec2::new(clip.x / clip.w, clip.y / clip.w);
            min = min.min(p);
            max = max.max(p);
        }
        if front == 0 {
            return false;
        }
        if behind {
            // Straddling the eye plane: unbounded on the screen.
            return true;
        }
        min.x <= hi.x && max.x >= lo.x && min.y <= hi.y && max.y >= lo.y
    };
    let mut out = Vec::new();
    for item in &scene.items {
        if !state.is_visible(item.id.kind) || item.translucent {
            continue;
        }
        if overlaps(&scene.world_bounds(item)) {
            out.push(item.id);
        }
    }
    if state.is_visible(ItemKind::Wire) {
        for wire in &scene.wires {
            if overlaps(&wire::world_bounds(wire)) {
                out.push(wire.id);
            }
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

    fn wire_scene() -> Scene {
        let mut s = scene();
        s.wires.push(crate::scene::Wire {
            id: ItemId::new(ItemKind::Wire, 7),
            points: vec![
                Vec3::new(100.0, 200.0, 20.0),
                Vec3::new(300.0, 200.0, 20.0),
                Vec3::new(300.0, 300.0, 20.0),
            ],
            radius: 1.0,
            tint: None,
            mark: crate::scene::Mark::None,
            label: None,
        });
        s
    }

    #[test]
    fn a_wire_is_picked_on_either_leg_and_in_front_of_the_plate() {
        let s = wire_scene();
        let state = ViewState::default();
        let on_first = pick(&s, &state, &front_ray(200.0, 200.5), 0).unwrap();
        assert_eq!(on_first.id, ItemId::new(ItemKind::Wire, 7));
        let on_second = pick(&s, &state, &front_ray(300.0, 260.0), 0).unwrap();
        assert_eq!(on_second.id.kind, ItemKind::Wire);
        // Off the wire the plate is hit instead.
        let off = pick(&s, &state, &front_ray(200.0, 230.0), 0).unwrap();
        assert_eq!(off.id.raw, 1);
        // Alt+click from the back reaches the plate under the wire.
        let back = pick_deepest(&s, &state, &front_ray(200.0, 200.0)).unwrap();
        assert_eq!(back.id.raw, 1);
        let mut hidden = ViewState::default();
        hidden.hidden.insert(ItemKind::Wire);
        assert_eq!(
            pick(&s, &hidden, &front_ray(200.0, 200.0), 0)
                .unwrap()
                .id
                .raw,
            1
        );
    }

    #[test]
    fn marquee_takes_wires_and_big_boxes_across_the_eye_plane() {
        let mut s = wire_scene();
        let state = ViewState::default();
        let mut cam = Camera::default();
        cam.set_preset(crate::camera::ViewPreset::Front);
        cam.fit(&s.bounds(), 1.0);
        let all = marquee(&s, &state, &cam, 1.0, Vec2::splat(-1.0), Vec2::splat(1.0));
        assert!(all.iter().any(|i| i.kind == ItemKind::Wire));
        // A box that holds the eye: some corners are behind the camera, yet
        // it stays selectable.
        let (_, _, fwd) = cam.basis();
        s.items.push(Item::new(
            ItemId::new(ItemKind::Other, 99),
            Shape::Box,
            Pose::at(cam.eye() + fwd * 200.0 - Vec3::new(0.0, 0.0, 500.0)),
            Vec3::new(1000.0, 1000.0, 1000.0),
        ));
        let again = marquee(&s, &state, &cam, 1.0, Vec2::splat(-0.1), Vec2::splat(0.1));
        assert!(again.iter().any(|i| i.raw == 99));
    }
}
