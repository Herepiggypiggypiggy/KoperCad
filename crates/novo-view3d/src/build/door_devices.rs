//! Devices mounted on the door: drawn standing out from the inside of the
//! door into the enclosure and swinging with it, and the devices a swing hit
//! involves (the door device and the plate device it touches) marked as
//! collisions. Devices on the other surfaces are overlays in novo-app
//! (view3d_surface_devices.rs).

use std::collections::BTreeSet;

use glam::{Quat, Vec3};
use novo_layout::state::CabinetLayout;
use novo_layout::swing::Obstacle;

use super::{DOOR_HINGE, Inputs};
use crate::scene::{Item, ItemId, ItemKind, Mark, Pose, Scene, Shape};

/// The devices a door swing hit involves, both sides of each hit.
pub(super) fn swing_marked(layout: &CabinetLayout) -> BTreeSet<u64> {
    let mut out = BTreeSet::new();
    if let Some(report) = layout.swing_report() {
        for hit in &report.hits {
            out.insert(hit.door_device);
            if let Obstacle::Device(id) = hit.obstacle {
                out.insert(id);
            }
        }
    }
    out
}

/// Adds the door devices of `layout` to `scene`, in the door's hinge group,
/// their backs on the inside of the door at `inner` mm off the plate.
pub(super) fn push(
    scene: &mut Scene,
    layout: &CabinetLayout,
    inputs: &Inputs,
    inner: f64,
    marked: &BTreeSet<u64>,
) {
    if scene.get(ItemId::new(ItemKind::Door, 0)).is_none() {
        return;
    }
    let frame = scene.frame;
    let mut devices: Vec<_> = layout.door_devices.iter().collect();
    devices.sort_by_key(|d| d.id);
    for d in devices {
        let (depth, guessed) = if d.depth > 0.0 {
            (d.depth, false)
        } else {
            (super::DEFAULT_DEPTH_MM, true)
        };
        let r = d.rect;
        let mut item = Item::new(
            ItemId::new(ItemKind::Device, d.id),
            Shape::Box,
            Pose {
                translation: frame.world(r.x + r.w / 2.0, r.y + r.h / 2.0, inner),
                // The box grows along its local +z: turned about Y it grows
                // from the door back towards the plate.
                rotation: Quat::from_rotation_y(std::f32::consts::PI),
            },
            Vec3::new(r.w as f32, r.h as f32, depth as f32),
        );
        item.group = Some(DOOR_HINGE);
        if marked.contains(&d.id) {
            item.mark = Mark::Collision;
        } else if guessed {
            item.mark = Mark::GuessedSize;
        }
        item.label = inputs.labels.get(&d.id).cloned();
        item.tint = inputs.classes.get(&d.id).map(|c| c.tint());
        scene.push(item);
    }
}
