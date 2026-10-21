//! Typical shapes for devices that have no maker model: a breaker has a
//! stepped front and a toggle, a contactor a body with terminal covers, a
//! terminal block its stepped ribs. They are drawn from the device family
//! (see `appearance::DeviceClass`) and the part's real width, height and
//! depth, so the silhouette and the room it takes are the part's, while the
//! details are typical, not the maker's. An imported maker model replaces
//! them (`mesh::MeshLibrary`).
//!
//! The geometry key carries the family and the size in half millimetres, so
//! the app can build what a scene needs from the scene alone
//! ([`ensure`]) and two devices of one size share one mesh.

use glam::Vec3;

use crate::appearance::DeviceClass;
use crate::mesh::{GeometryKey, MeshData, MeshLibrary, PartGeometry, unit_box};
use crate::scene::{Scene, Shape};

/// Top byte of a generated key. A maker key is a hash of a part id and a
/// file hash, so meeting this tag by accident is about one in 250.
const TAG: u64 = 0xD5;
const DIM_BITS: u32 = 14;
const DIM_MASK: u64 = (1 << DIM_BITS) - 1;
/// Largest side a generated shape handles, mm (half millimetres in 14 bits).
const MAX_MM: f32 = 8000.0;
/// Below this a side is too thin for details: the plain box does.
const MIN_MM: f32 = 3.0;
/// Most ribs a wide terminal block gets.
const MAX_RIBS: usize = 40;

const FAMILIES: [DeviceClass; 7] = [
    DeviceClass::Protection,
    DeviceClass::Switching,
    DeviceClass::Terminal,
    DeviceClass::Controller,
    DeviceClass::Drive,
    DeviceClass::Supply,
    DeviceClass::Connector,
];

/// The key of the typical shape for a family at a size (mm), or `None` when
/// the family has no typical shape or a side is out of range.
pub fn key(class: DeviceClass, size: Vec3) -> Option<GeometryKey> {
    let family = FAMILIES.iter().position(|c| *c == class)? as u64;
    let q = |v: f32| -> Option<u64> {
        (MIN_MM..=MAX_MM)
            .contains(&v)
            .then(|| (v * 2.0).round() as u64)
    };
    let (w, h, d) = (q(size.x)?, q(size.y)?, q(size.z)?);
    Some(GeometryKey(
        TAG << 56 | family << 48 | w << (2 * DIM_BITS) | h << DIM_BITS | d,
    ))
}

/// The family and size a generated key stands for.
pub fn spec(key: GeometryKey) -> Option<(DeviceClass, Vec3)> {
    if key.0 >> 56 != TAG {
        return None;
    }
    let class = *FAMILIES.get(((key.0 >> 48) & 0xFF) as usize)?;
    let mm = |shift: u32| ((key.0 >> shift) & DIM_MASK) as f32 / 2.0;
    Some((class, Vec3::new(mm(2 * DIM_BITS), mm(DIM_BITS), mm(0))))
}

/// Turns the typical shapes back into plain boxes (the layer is off).
pub fn strip(scene: &mut Scene) {
    let mut changed = false;
    for item in &mut scene.items {
        if matches!(item.shape, Shape::Part(k) if spec(k).is_some()) {
            item.shape = Shape::Box;
            changed = true;
        }
    }
    if changed {
        scene.revision += 1;
    }
}

/// Builds into `library` the typical shapes `scene` asks for and it lacks.
/// Returns whether it added any.
pub fn ensure(library: &mut MeshLibrary, scene: &Scene) -> bool {
    let mut added = false;
    for item in &scene.items {
        if let Shape::Part(k) = item.shape
            && library.get(k).is_none()
            && let Some((class, size)) = spec(k)
        {
            library.insert(k, geometry(class, size));
            added = true;
        }
    }
    added
}

/// The detailed mesh and the plain box, as the renderer's two detail levels.
pub fn geometry(class: DeviceClass, size: Vec3) -> PartGeometry {
    let mut plain = unit_box();
    for p in &mut plain.positions {
        p[0] *= size.x;
        p[1] *= size.y;
        p[2] *= size.z;
    }
    plain.bounds = crate::math::Aabb::EMPTY;
    for p in &plain.positions {
        plain.bounds.extend(Vec3::from_array(*p));
    }
    PartGeometry {
        lods: vec![mesh(class, size), plain],
    }
}

/// One block: centre (x, y), width, height, from z0 to z1.
struct Block {
    cx: f32,
    cy: f32,
    sx: f32,
    sy: f32,
    z0: f32,
    z1: f32,
}

fn boxes(blocks: &[Block]) -> MeshData {
    let list: Vec<(Vec3, Vec3)> = blocks
        .iter()
        .map(|b| {
            (
                Vec3::new(b.cx - b.sx / 2.0, b.cy - b.sy / 2.0, b.z0),
                Vec3::new(b.cx + b.sx / 2.0, b.cy + b.sy / 2.0, b.z1),
            )
        })
        .collect();
    MeshData::from_boxes(&list)
}

/// The typical mesh of a family: x width and y height centred, z from the
/// mounting face to the front, filling exactly `size`.
pub fn mesh(class: DeviceClass, size: Vec3) -> MeshData {
    let (w, h, d) = (size.x, size.y, size.z);
    let b = |cx: f32, cy: f32, sx: f32, sy: f32, z0: f32, z1: f32| Block {
        cx,
        cy,
        sx,
        sy,
        z0: z0 * d,
        z1: z1 * d,
    };
    let blocks = match class {
        // Modular breaker: the body, a narrower stepped front, terminal
        // cages top and bottom and the toggle.
        DeviceClass::Protection => vec![
            b(0.0, 0.0, w, h, 0.0, 0.62),
            b(0.0, 0.0, w, h * 0.62, 0.62, 0.9),
            b(0.0, h * 0.42, w * 0.7, h * 0.14, 0.62, 0.8),
            b(0.0, -h * 0.42, w * 0.7, h * 0.14, 0.62, 0.8),
            b(0.0, 0.0, w * 0.5, h * 0.1, 0.9, 1.0),
        ],
        // Contactor: base, main body, terminal covers, the aux block and
        // the indicator on the front.
        DeviceClass::Switching => vec![
            b(0.0, 0.0, w, h, 0.0, 0.55),
            b(0.0, 0.0, w * 0.92, h * 0.74, 0.55, 0.86),
            b(0.0, h * 0.43, w * 0.8, h * 0.14, 0.55, 0.72),
            b(0.0, -h * 0.43, w * 0.8, h * 0.14, 0.55, 0.72),
            b(0.0, h * 0.18, w * 0.4, h * 0.14, 0.86, 0.95),
            b(0.0, -h * 0.12, w * 0.4, h * 0.2, 0.86, 1.0),
        ],
        DeviceClass::Terminal => terminal_blocks(w, h, d),
        // PLC: housing, the front door, terminal covers, a status strip.
        DeviceClass::Controller => vec![
            b(0.0, 0.0, w, h, 0.0, 0.8),
            b(0.0, 0.0, w * 0.96, h * 0.9, 0.8, 0.92),
            b(0.0, h * 0.42, w * 0.9, h * 0.12, 0.92, 1.0),
            b(0.0, -h * 0.42, w * 0.9, h * 0.12, 0.92, 1.0),
            b(0.0, h * 0.22, w * 0.3, h * 0.05, 0.92, 0.97),
        ],
        // Drive: housing, front panel, display, lower terminal cover.
        DeviceClass::Drive => vec![
            b(0.0, 0.0, w, h, 0.0, 0.75),
            b(0.0, h * 0.1, w * 0.86, h * 0.6, 0.75, 0.9),
            b(0.0, h * 0.2, w * 0.5, h * 0.16, 0.9, 1.0),
            b(0.0, -h * 0.4, w * 0.92, h * 0.18, 0.75, 0.88),
        ],
        // Power supply: housing, terminal bands, adjustment knob.
        DeviceClass::Supply => vec![
            b(0.0, 0.0, w, h, 0.0, 0.85),
            b(0.0, h * 0.43, w * 0.9, h * 0.14, 0.85, 1.0),
            b(0.0, -h * 0.43, w * 0.9, h * 0.14, 0.85, 1.0),
            b(w * 0.2, h * 0.1, w * 0.2, h * 0.1, 0.85, 0.95),
        ],
        // Connector: the base and its shroud.
        DeviceClass::Connector | DeviceClass::Generic => vec![
            b(0.0, 0.0, w, h, 0.0, 0.5),
            b(0.0, 0.0, w * 0.7, h * 0.7, 0.5, 1.0),
        ],
    };
    boxes(&blocks)
}

/// A terminal block: base, stepped body and the screw tier, one rib per
/// 6 mm of the wider side up to a limit, so a wide strip reads as terminals.
fn terminal_blocks(w: f32, h: f32, d: f32) -> Vec<Block> {
    let along_x = w >= h;
    let long = if along_x { w } else { h };
    let ribs = ((long / 6.0).round() as usize).clamp(1, MAX_RIBS);
    let pitch = long / ribs as f32;
    let gap = (pitch * 0.06).min(0.5);
    let mut out = vec![Block {
        cx: 0.0,
        cy: 0.0,
        sx: w,
        sy: h,
        z0: 0.0,
        z1: d * 0.5,
    }];
    for i in 0..ribs {
        let at = -long / 2.0 + pitch * (i as f32 + 0.5);
        let across = |f: f32| if along_x { h * f } else { w * f };
        let place = |len: f32, wide: f32, z0: f32, z1: f32, off: f32| {
            let (cx, cy, sx, sy) = if along_x {
                (at, off * h, len, wide)
            } else {
                (off * w, at, wide, len)
            };
            Block {
                cx,
                cy,
                sx,
                sy,
                z0: z0 * d,
                z1: z1 * d,
            }
        };
        let rib = pitch - 2.0 * gap;
        out.push(place(rib, across(0.84), 0.5, 0.78, 0.0));
        out.push(place(rib * 0.7, across(0.5), 0.78, 1.0, 0.0));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size() -> Vec3 {
        Vec3::new(18.0, 90.0, 70.0)
    }

    #[test]
    fn a_key_returns_its_family_and_size() {
        for class in FAMILIES {
            let k = key(class, Vec3::new(17.5, 90.0, 68.5)).unwrap();
            assert_eq!(spec(k), Some((class, Vec3::new(17.5, 90.0, 68.5))));
        }
    }

    #[test]
    fn sizes_are_shared_and_generic_has_no_shape() {
        let a = key(DeviceClass::Protection, size());
        assert_eq!(a, key(DeviceClass::Protection, size()));
        assert_ne!(a, key(DeviceClass::Switching, size()));
        assert!(key(DeviceClass::Generic, size()).is_none());
        assert!(key(DeviceClass::Protection, Vec3::new(1.0, 90.0, 70.0)).is_none());
        assert!(key(DeviceClass::Protection, Vec3::new(18.0, 9000.0, 70.0)).is_none());
    }

    #[test]
    fn a_maker_key_is_not_taken_for_a_generated_one() {
        assert!(spec(GeometryKey(0x1234_5678_9abc_def0)).is_none());
    }

    #[test]
    fn every_family_fills_its_size_exactly() {
        for class in FAMILIES {
            let m = mesh(class, size());
            let e = m.bounds.size();
            assert!((e - size()).abs().max_element() < 1e-3, "{class:?} {e}");
            assert!(m.bounds.min.x < 0.0 && m.bounds.min.z.abs() < 1e-4);
            assert!(m.triangles() > 12, "{class:?} is more than a box");
            assert!(m.triangles() < 600, "{class:?} stays light");
        }
    }

    #[test]
    fn a_wide_terminal_strip_gets_ribs_up_to_the_limit() {
        let few = mesh(DeviceClass::Terminal, Vec3::new(6.0, 50.0, 40.0)).triangles();
        let many = mesh(DeviceClass::Terminal, Vec3::new(120.0, 50.0, 40.0)).triangles();
        let capped = mesh(DeviceClass::Terminal, Vec3::new(2000.0, 50.0, 40.0)).triangles();
        assert!(few < many && many < capped);
        assert!(capped <= (1 + 2 * MAX_RIBS) * 12);
    }

    #[test]
    fn the_second_detail_level_is_the_plain_box() {
        let g = geometry(DeviceClass::Switching, size());
        assert_eq!(g.lods.len(), 2);
        assert_eq!(g.lods[1].triangles(), 12);
        assert!((g.lods[1].bounds.size() - size()).abs().max_element() < 1e-3);
    }

    fn scene_with(class: DeviceClass) -> Scene {
        use crate::scene::{Item, ItemId, ItemKind, PlateFrame, Pose};
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        let mut dev = Item::new(
            ItemId::new(ItemKind::Device, 1),
            Shape::Box,
            Pose::IDENTITY,
            size(),
        );
        if let Some(k) = key(class, size()) {
            dev.shape = Shape::Part(k);
        }
        s.push(dev);
        s
    }

    #[test]
    fn ensure_builds_what_is_missing_once_and_strip_restores_boxes() {
        let mut s = scene_with(DeviceClass::Protection);
        let mut lib = MeshLibrary::new();
        assert!(ensure(&mut lib, &s));
        assert!(!ensure(&mut lib, &s), "nothing left to build");
        let Shape::Part(k) = s.items[0].shape else {
            panic!("a part")
        };
        assert!(lib.get(k).is_some());
        let before = s.revision;
        strip(&mut s);
        assert_eq!(s.items[0].shape, Shape::Box);
        assert!(s.revision > before, "the renderer must re-upload");
    }

    #[test]
    fn a_maker_part_survives_strip() {
        let mut s = scene_with(DeviceClass::Generic);
        s.items[0].shape = Shape::Part(GeometryKey(42));
        strip(&mut s);
        assert_eq!(s.items[0].shape, Shape::Part(GeometryKey(42)));
    }
}
