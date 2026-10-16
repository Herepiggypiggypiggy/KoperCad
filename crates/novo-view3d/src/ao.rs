//! Ambient occlusion of the plate (3d-look.md section 2): a cheap contact term
//! that darkens the plate close to the footprint of whatever stands on it.
//! It is baked on the CPU into a small one-channel texture laid over the
//! plate and redone only when the scene or the layers change.

use crate::math::Aabb;
use crate::scene::{ItemKind, Scene};
use crate::view_state::ViewState;

/// How far from a footprint the plate is still darkened, mm.
pub const AO_RADIUS_MM: f32 = 6.0;
/// Size of one texel on the plate, mm, unless that would pass `AO_MAX_SIDE`.
const AO_CELL_MM: f32 = 1.5;
const AO_MAX_SIDE: u32 = 512;
/// An item counts as standing on the plate when its underside is this close.
const FOOT_MM: f32 = 3.0;

/// Occlusion over the plate, row 0 at world y = 0 (the bottom edge).
#[derive(Debug, Clone, PartialEq)]
pub struct AoMap {
    pub width: u32,
    pub height: u32,
    /// 0 = open, 255 = the darkest the term goes.
    pub data: Vec<u8>,
}

impl AoMap {
    /// Occlusion at a plate point (world mm), 0 to 1 of `strength`.
    pub fn at(&self, x: f32, y: f32, plate: (f32, f32)) -> f32 {
        let u = (x / plate.0).clamp(0.0, 1.0);
        let v = (y / plate.1).clamp(0.0, 1.0);
        let i = ((u * self.width as f32) as u32).min(self.width - 1);
        let j = ((v * self.height as f32) as u32).min(self.height - 1);
        f32::from(self.data[(j * self.width + i) as usize]) / 255.0
    }
}

/// Footprints on the plate (world boxes) of the items the term is cast by:
/// opaque, visible, standing on the plate. Wires, the shell, the door and the
/// plate itself cast none.
fn footprints<'a>(scene: &'a Scene, state: &'a ViewState) -> impl Iterator<Item = Aabb> + 'a {
    scene.items.iter().filter_map(move |item| {
        let kind = item.id.kind;
        if item.translucent
            || matches!(
                kind,
                ItemKind::Plate | ItemKind::Enclosure | ItemKind::Door | ItemKind::Wire
            )
            || !state.is_visible(kind)
        {
            return None;
        }
        let b = scene.world_bounds(item);
        (b.min.z <= FOOT_MM && b.max.z >= 0.5).then_some(b)
    })
}

/// What the baked map depends on: the plate, the strength and the footprints.
/// Moving a door or a part that casts nothing leaves it alone.
pub fn key(scene: &Scene, state: &ViewState, strength: f32) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (
        scene.frame.width.to_bits(),
        scene.frame.height.to_bits(),
        strength.to_bits(),
    )
        .hash(&mut h);
    for b in footprints(scene, state) {
        for v in [b.min.x, b.min.y, b.max.x, b.max.y] {
            v.to_bits().hash(&mut h);
        }
    }
    h.finish()
}

/// Bakes the term: `strength` (0 to 1) at the foot of a device, falling to
/// nothing `AO_RADIUS_MM` away.
pub fn bake(scene: &Scene, state: &ViewState, strength: f32) -> AoMap {
    let (pw, ph) = (scene.frame.width.max(1.0), scene.frame.height.max(1.0));
    let cell = AO_CELL_MM.max(pw.max(ph) / AO_MAX_SIDE as f32);
    let width = ((pw / cell).ceil() as u32).max(1);
    let height = ((ph / cell).ceil() as u32).max(1);
    let mut data = vec![0u8; (width * height) as usize];
    let top = (strength.clamp(0.0, 1.0) * 255.0).round();
    for b in footprints(scene, state) {
        let reach = |lo: f32, hi: f32, n: u32| {
            let a = ((lo - AO_RADIUS_MM) / cell).floor().max(0.0) as u32;
            let z = (((hi + AO_RADIUS_MM) / cell).ceil().max(0.0) as u32).min(n);
            (a.min(n), z)
        };
        let (i0, i1) = reach(b.min.x, b.max.x, width);
        let (j0, j1) = reach(b.min.y, b.max.y, height);
        for j in j0..j1 {
            let cy = (j as f32 + 0.5) * cell;
            let dy = (b.min.y - cy).max(cy - b.max.y).max(0.0);
            for i in i0..i1 {
                let cx = (i as f32 + 0.5) * cell;
                let dx = (b.min.x - cx).max(cx - b.max.x).max(0.0);
                let t = ((dx * dx + dy * dy).sqrt() / AO_RADIUS_MM).clamp(0.0, 1.0);
                let falloff = 1.0 - t * t * (3.0 - 2.0 * t);
                let v = (falloff * top) as u8;
                let px = &mut data[(j * width + i) as usize];
                *px = (*px).max(v);
            }
        }
    }
    AoMap {
        width,
        height,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, ItemId, PlateFrame, Pose, Shape};
    use glam::Vec3;

    fn scene_with_box(kind: ItemKind, z: f32) -> Scene {
        let mut scene = Scene::new(PlateFrame {
            width: 200.0,
            height: 100.0,
        });
        scene.push(Item::new(
            ItemId::new(kind, 1),
            Shape::Box,
            Pose::at(Vec3::new(100.0, 50.0, z)),
            Vec3::new(40.0, 20.0, 30.0),
        ));
        scene
    }

    #[test]
    fn the_plate_darkens_next_to_a_device_and_fades_out_within_the_radius() {
        let scene = scene_with_box(ItemKind::Device, 0.0);
        let map = bake(&scene, &ViewState::default(), 0.35);
        let plate = (200.0, 100.0);
        // The footprint spans x 80..120, y 40..60.
        let edge = map.at(79.0, 50.0, plate);
        let mid = map.at(77.0, 50.0, plate);
        let far = map.at(60.0, 50.0, plate);
        assert!(edge > 0.25 && edge <= 0.35, "close to the foot: {edge}");
        assert!(mid < edge && mid > 0.0, "fading: {mid}");
        assert_eq!(far, 0.0, "20 mm away is untouched");
        assert_eq!(map.at(10.0, 10.0, plate), 0.0);
    }

    #[test]
    fn things_not_standing_on_the_plate_leave_it_alone() {
        let lifted = scene_with_box(ItemKind::Device, 20.0);
        let map = bake(&lifted, &ViewState::default(), 0.35);
        assert!(map.data.iter().all(|&v| v == 0), "a part 20 mm up");
        let plate = scene_with_box(ItemKind::Plate, 0.0);
        let map = bake(&plate, &ViewState::default(), 0.35);
        assert!(map.data.iter().all(|&v| v == 0), "the plate itself");
    }

    #[test]
    fn a_hidden_layer_casts_no_occlusion_and_zero_strength_is_off() {
        let scene = scene_with_box(ItemKind::Device, 0.0);
        let mut state = ViewState::default();
        state.hidden.insert(ItemKind::Device);
        assert!(bake(&scene, &state, 0.35).data.iter().all(|&v| v == 0));
        assert!(
            bake(&scene, &ViewState::default(), 0.0)
                .data
                .iter()
                .all(|&v| v == 0)
        );
    }

    #[test]
    fn the_key_follows_footprints_not_the_door_or_unrelated_changes() {
        let mut scene = scene_with_box(ItemKind::Device, 0.0);
        let state = ViewState::default();
        let base = key(&scene, &state, 0.35);
        assert_eq!(base, key(&scene, &state, 0.35), "stable");
        assert_ne!(base, key(&scene, &state, 0.2), "strength");
        let mut hidden = ViewState::default();
        hidden.hidden.insert(ItemKind::Device);
        assert_ne!(base, key(&scene, &hidden, 0.35), "layers");
        // A door swinging (or anything translucent) does not touch the map.
        let mut door = Item::new(
            ItemId::new(ItemKind::Door, 0),
            Shape::Box,
            Pose::at(Vec3::new(10.0, 10.0, 0.0)),
            Vec3::new(5.0, 5.0, 5.0),
        );
        door.translucent = true;
        scene.push(door);
        assert_eq!(base, key(&scene, &state, 0.35), "door");
    }
}
