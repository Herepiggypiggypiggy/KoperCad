//! A synthetic cabinet for tests, the orbit example and benchmarks: a
//! mounting plate with wiring ducts, top-hat rails and rows of devices, and
//! optionally a hinged door. No project model involved.

use crate::mesh::{GeometryKey, MeshData, MeshLibrary, PartGeometry, unit_box};
use crate::scene::{
    Group, GroupId, Item, ItemId, ItemKind, Mark, PlateFrame, Pose, Profile, Scene, Shape, Wire,
};
use glam::Vec3;
use std::sync::Arc;

/// Width of one panel in the demo, mm.
pub const PANEL_WIDTH: f32 = 800.0;
/// Height of the plate, mm.
pub const PANEL_HEIGHT: f32 = 1000.0;
/// Rail rows per panel.
pub const ROWS: usize = 7;
/// Devices per panel in the demo (a full panel of narrow devices).
pub const DEVICES_PER_PANEL: usize = 150;

pub const HINGE: GroupId = GroupId(1);

#[derive(Debug, Clone)]
pub struct DemoOptions {
    /// Number of devices; panels are added side by side beyond 150.
    pub devices: usize,
    /// Draw about a quarter of the devices as shaped parts (stepped
    /// breakers) instead of boxes.
    pub shaped_parts: bool,
    /// Add a translucent door on a hinge at the left edge.
    pub door: bool,
    /// Up to this many wires: each runs from one device up into the duct of its
    /// row, along it and down to another device, in one of six colours; one in
    /// six is the green-yellow protective wire with its stripe.
    pub wires: usize,
}

impl Default for DemoOptions {
    fn default() -> Self {
        Self {
            devices: DEVICES_PER_PANEL,
            shaped_parts: true,
            door: true,
            wires: 0,
        }
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// A circuit breaker in the unit box: a base, a narrower front and a toggle.
/// Scaled per axis to the item size by the renderer.
fn breaker() -> PartGeometry {
    let lod0 = MeshData::from_boxes(&[
        (Vec3::new(-0.5, -0.5, 0.0), Vec3::new(0.5, 0.5, 0.55)),
        (Vec3::new(-0.42, -0.4, 0.55), Vec3::new(0.42, 0.4, 0.9)),
        (Vec3::new(-0.16, -0.1, 0.9), Vec3::new(0.16, 0.22, 1.0)),
    ]);
    PartGeometry {
        lods: vec![lod0, unit_box()],
    }
}

pub const BREAKER: GeometryKey = GeometryKey(1);

/// The scene and the library for the options.
pub fn cabinet(opts: &DemoOptions) -> (Scene, Arc<MeshLibrary>) {
    let panels = opts.devices.div_ceil(DEVICES_PER_PANEL).max(1);
    let (w, h) = (PANEL_WIDTH * panels as f32, PANEL_HEIGHT);
    let frame = PlateFrame {
        width: w,
        height: h,
    };
    let mut scene = Scene::new(frame);
    scene.revision = 1;
    let mut lib = MeshLibrary::new();
    lib.insert(BREAKER, breaker());

    scene.push(Item::new(
        ItemId::new(ItemKind::Plate, 1),
        Shape::Box,
        Pose::at(Vec3::new(w * 0.5, h * 0.5, -3.0)),
        Vec3::new(w, h, 3.0),
    ));

    let mut rng = Lcg(0x5eed);
    let rows = ROWS;
    let per_row = opts.devices.div_ceil(rows).max(1);
    let mut placed = 0usize;
    // Where each device stands: its row, centre x and the y of the duct above.
    let mut slots: Vec<(usize, f32, f32)> = Vec::new();
    let margin = 30.0;
    for row in 0..rows {
        let top = 50.0 + row as f32 * 135.0;
        // Duct above the row, rail through the middle of the device zone.
        let duct_y = top + 15.0;
        let rail_y = top + 80.0;
        scene.push(Item::new(
            ItemId::new(ItemKind::Duct, 100 + row as u64),
            Shape::Profile(Profile::Channel),
            Pose::at(frame.world(w as f64 * 0.5, duct_y as f64, 0.0)),
            Vec3::new(w - 2.0 * margin, 30.0, 40.0),
        ));
        scene.push(Item::new(
            ItemId::new(ItemKind::Rail, 200 + row as u64),
            Shape::Profile(Profile::TopHat35x7_5),
            Pose::at(frame.world(w as f64 * 0.5, rail_y as f64, 0.0)),
            Vec3::new(w - 2.0 * margin, 35.0, 7.5),
        ));
        // Devices fill the rail from the left; widths in modules of 9 mm.
        let mut x = margin + 5.0;
        for _ in 0..per_row {
            if placed >= opts.devices {
                break;
            }
            let modules = [2, 2, 2, 3, 3, 4, 4, 6][(rng.next() % 8) as usize];
            let dw = modules as f32 * 9.0;
            if x + dw > w - margin {
                break;
            }
            let shaped = opts.shaped_parts && placed.is_multiple_of(4);
            let mut item = Item::new(
                ItemId::new(ItemKind::Device, 10_000 + placed as u64),
                if shaped {
                    Shape::Part(BREAKER)
                } else {
                    Shape::Box
                },
                Pose::at(frame.world((x + dw * 0.5) as f64, rail_y as f64, 7.5)),
                Vec3::new(dw - 0.5, 90.0, 55.0 + (rng.next() % 3) as f32 * 5.0),
            );
            item.label = Some(format!("-F{}", placed + 1).into());
            if placed % 37 == 5 {
                item.mark = Mark::Collision;
            } else if placed % 53 == 7 {
                item.mark = Mark::GuessedSize;
            }
            scene.push(item);
            slots.push((row, x + dw * 0.5, duct_y));
            placed += 1;
            x += dw + 1.0;
        }
    }

    // Wires: from a device up to the duct of its row, along it, down again.
    let palette: [([u8; 3], Option<[u8; 3]>); 6] = [
        ([0x1B, 0x1B, 0x1E], None),
        ([0x2F, 0x6F, 0xD0], None),
        ([0xB3, 0x3A, 0x3A], None),
        ([0x6B, 0x42, 0x26], None),
        ([0x8C, 0x8F, 0x94], None),
        ([0x7F, 0xB0, 0x20], Some([0xE3, 0xD3, 0x4A])),
    ];
    for k in 0..opts.wires.min(slots.len().saturating_sub(1)) {
        let (row, x0, duct_y) = slots[k];
        // A partner in the same row, a few devices along.
        let Some(&(_, x1, _)) = slots[k + 1..].iter().find(|s| s.0 == row) else {
            continue;
        };
        let lane = duct_y - 4.0 + (k % 5) as f32 * 2.0;
        let (body, stripe) = palette[k % palette.len()];
        let z = 20.0;
        let at = |x: f32, y: f32| frame.world(x as f64, y as f64, z);
        scene.wires.push(Wire {
            id: ItemId::new(ItemKind::Wire, 50_000 + k as u64),
            points: vec![
                at(x0, lane + 40.0),
                at(x0, lane),
                at(x1, lane),
                at(x1, lane + 40.0),
            ],
            radius: 1.5,
            tint: Some(body),
            stripe,
            mark: Mark::None,
            label: None,
        });
    }

    if opts.door {
        let mut door = Item::new(
            ItemId::new(ItemKind::Door, 2),
            Shape::Box,
            Pose::at(Vec3::new(w * 0.5, h * 0.5, 80.0)),
            Vec3::new(w, h, 3.0),
        );
        door.translucent = true;
        door.group = Some(HINGE);
        scene.push(door);
        scene.groups.push(Group {
            id: HINGE,
            pivot: Vec3::new(0.0, 0.0, 80.0),
            axis: Vec3::Y,
            angle: 0.0,
        });
    }
    (scene, Arc::new(lib))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_has_the_requested_devices() {
        for n in [10usize, 100, 400, 1000] {
            let (s, _) = cabinet(&DemoOptions {
                devices: n,
                ..DemoOptions::default()
            });
            let devices = s
                .items
                .iter()
                .filter(|i| i.id.kind == ItemKind::Device)
                .count();
            assert_eq!(devices, n);
        }
    }

    #[test]
    fn the_demo_has_the_requested_wires_with_unique_ids() {
        let (s, _) = cabinet(&DemoOptions {
            devices: 300,
            wires: 100,
            ..DemoOptions::default()
        });
        // The last device of a row has no partner after it, so a few fewer.
        let n = s.wires.len();
        assert!((90..=100).contains(&n), "{n}");
        let mut ids: Vec<_> = s.wires.iter().map(|w| w.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
        assert!(
            s.wires
                .iter()
                .all(|w| w.tint.is_some() && w.points.len() == 4)
        );
    }

    #[test]
    fn ids_are_unique() {
        let (s, _) = cabinet(&DemoOptions {
            devices: 300,
            ..DemoOptions::default()
        });
        let mut ids: Vec<_> = s.items.iter().map(|i| i.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
