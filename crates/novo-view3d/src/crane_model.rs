//! A crane model of the user's own in the crane picture (lane R3, parts 2 and 3).
//!
//! The model is a STEP file split into its solids by the caller (`novo-interop`, the reader merged
//! with `kopercad-cli parts step-parts`); this module takes them as plain meshes and turns them into
//! a [`ModelPlan`]: every solid with the role it plays on the crane (wheel, drum, hook block, ...),
//! stood on the floor in the picture's axes (x along the runway, y up, z across the span), with the
//! mesh for each solid ready for the renderer. Pure data, no file access and no GPU.
//!
//! What it is: the user's own geometry, coloured by role. What it is not: a measurement. The role of
//! a solid is guessed from its name and the user can change it; the orientation of the file (which
//! axis is up, which way the runway runs) is chosen by the user.

use crate::crane::push_floor;
use crate::math::Aabb;
use crate::mesh::{GeometryKey, MeshData, MeshLibrary, PartGeometry};
use crate::scene::{Item, ItemId, ItemKind, PlateFrame, Pose, Scene, Shape};
use glam::{Quat, Vec3};
use std::f32::consts::FRAC_PI_2;
use std::sync::atomic::{AtomicU64, Ordering};

/// What the picture leaves out when it shows a crane model, shown behind the window's "?".
pub const MODEL_PICTURE_LEAVES_OUT: &[&str] = &[
    "Your crane model is drawn as the file gives it, in one colour per role: its own colours, materials and textures are not read",
    "A solid's role (wheel, drum, hook block, ...) is a guess from its name until you set it; a wrong role gives a wrong colour and, in the moving picture, a wrong motion",
    "The file's orientation is your choice: pick which axis is up and turn the model until the runway lies along the picture's long side",
    "The floor and its yellow line are drawn around the model's footprint; the rope is not part of a model file and is drawn only where the model has none",
    "Each solid is simplified to a few thousand triangles, so small details (threads, text) are lost",
];

/// What a solid of the model is on the crane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PartRole {
    Wheel,
    Drum,
    HookBlock,
    Hook,
    Girder,
    Rail,
    Motor,
    Gearbox,
    Other,
}

impl PartRole {
    /// Every role, in the order a list offers them.
    pub const ALL: [PartRole; 9] = [
        PartRole::Wheel,
        PartRole::Drum,
        PartRole::HookBlock,
        PartRole::Hook,
        PartRole::Girder,
        PartRole::Rail,
        PartRole::Motor,
        PartRole::Gearbox,
        PartRole::Other,
    ];

    /// A short label for lists.
    pub fn label(self) -> &'static str {
        match self {
            PartRole::Wheel => "wheel",
            PartRole::Drum => "drum",
            PartRole::HookBlock => "hook block",
            PartRole::Hook => "hook",
            PartRole::Girder => "girder",
            PartRole::Rail => "rail",
            PartRole::Motor => "motor",
            PartRole::Gearbox => "gearbox",
            PartRole::Other => "other",
        }
    }

    /// The word the project file keeps for the role.
    pub fn code(self) -> &'static str {
        match self {
            PartRole::HookBlock => "hookblock",
            r => r.label(),
        }
    }

    /// The role a stored word names.
    pub fn from_code(code: &str) -> Option<PartRole> {
        PartRole::ALL.into_iter().find(|r| r.code() == code.trim())
    }

    /// The colour of the role in the picture.
    pub fn tint(self) -> [u8; 3] {
        match self {
            PartRole::Wheel => [104, 110, 120],
            PartRole::Drum => [120, 130, 145],
            PartRole::HookBlock => [235, 150, 40],
            PartRole::Hook => [210, 214, 222],
            PartRole::Girder => [60, 110, 170],
            PartRole::Rail => [150, 156, 166],
            PartRole::Motor => [48, 62, 92],
            PartRole::Gearbox => [92, 98, 108],
            PartRole::Other => [235, 190, 40],
        }
    }
}

/// Which axis of the file points up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    /// The file's y axis is up (the picture's own convention).
    Y,
    /// The file's z axis is up (most CAD programs).
    #[default]
    Z,
}

/// How the file's axes lie in the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Orientation {
    pub up: UpAxis,
    /// Quarter turns about the vertical axis, 0 to 3, counter-clockwise seen from above.
    pub turns: u8,
}

impl Orientation {
    /// The turn that takes a point of the file to the picture's axes.
    pub fn rotation(self) -> Quat {
        let fix = match self.up {
            UpAxis::Y => Quat::IDENTITY,
            // z up becomes y up: (x, y, z) -> (x, z, -y).
            UpAxis::Z => Quat::from_rotation_x(-FRAC_PI_2),
        };
        Quat::from_rotation_y(f32::from(self.turns % 4) * FRAC_PI_2) * fix
    }
}

/// One solid of the file, in the file's axes, millimetres.
#[derive(Debug, Clone)]
pub struct RawPart {
    pub name: String,
    pub mesh: MeshData,
}

/// A crane model as read: the solids in file order.
#[derive(Debug, Clone)]
pub struct CraneModel {
    pub parts: Vec<RawPart>,
}

/// One solid in the picture: where it stands and what it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    pub key: GeometryKey,
    pub role: PartRole,
    /// The box around the solid in the picture's axes, millimetres.
    pub bounds: Aabb,
}

/// The model ready to draw: every solid placed, and the meshes to hand to the renderer.
#[derive(Debug, Clone)]
pub struct ModelPlan {
    pub placed: Vec<Placed>,
    pub library: MeshLibrary,
    /// The box around the whole model.
    pub bounds: Aabb,
}

/// Top byte of a model part key. Maker keys are hashes, device shapes use `0xD5`, crane parts `0xC7`.
const TAG: u64 = 0xC8;
static NEXT_PLAN: AtomicU64 = AtomicU64::new(1);

fn rotated_bounds(b: &Aabb, q: Quat) -> Aabb {
    let mut out = Aabb::EMPTY;
    for c in b.corners() {
        out.extend(q * c);
    }
    out
}

/// The box around positions, `EMPTY` for none.
fn bounds_of(positions: &[[f32; 3]]) -> Aabb {
    let mut b = Aabb::EMPTY;
    for p in positions {
        b.extend(Vec3::from_array(*p));
    }
    b
}

impl CraneModel {
    /// A model of these solids. Each solid's box is taken from its triangles.
    pub fn new(mut parts: Vec<RawPart>) -> Self {
        for p in &mut parts {
            p.mesh.bounds = bounds_of(&p.mesh.positions);
        }
        Self { parts }
    }

    /// The box around the whole model in the file's axes.
    pub fn bounds(&self) -> Aabb {
        self.parts
            .iter()
            .fold(Aabb::EMPTY, |b, p| b.union(&p.mesh.bounds))
    }

    /// The triangles of all solids.
    pub fn triangles(&self) -> usize {
        self.parts.iter().map(|p| p.mesh.triangles()).sum()
    }
}

/// Places the model: turned to `orient`, stood on the floor with its lowest point at height 0 and
/// its nearest corner at x = 0, z = 0, one solid per entry of `roles` (a missing entry is `Other`).
/// Solids without triangles are left out.
pub fn plan(model: &CraneModel, orient: Orientation, roles: &[PartRole]) -> ModelPlan {
    let q = orient.rotation();
    let turned: Vec<Aabb> = model
        .parts
        .iter()
        .map(|p| rotated_bounds(&p.mesh.bounds, q))
        .collect();
    let whole = turned
        .iter()
        .zip(&model.parts)
        .filter(|(_, p)| p.mesh.triangles() > 0)
        .fold(Aabb::EMPTY, |b, (t, _)| b.union(t));
    let shift = if whole.is_empty() {
        Vec3::ZERO
    } else {
        -whole.min
    };
    let id = NEXT_PLAN.fetch_add(1, Ordering::Relaxed) & 0xFF_FFFF;
    let mut library = MeshLibrary::new();
    let mut placed = Vec::new();
    for (i, part) in model.parts.iter().enumerate() {
        if part.mesh.triangles() == 0 {
            continue;
        }
        let b = Aabb::new(turned[i].min + shift, turned[i].max + shift);
        let size = b.size();
        // The renderer's local box is centred in x and y and runs from 0 to the depth along z; the
        // item stands where that box's back-face centre belongs, with no turn of its own.
        let origin = Vec3::new(b.center().x, b.center().y, b.min.z);
        let local = |p: [f32; 3]| (q * Vec3::from_array(p) + shift - origin).to_array();
        let m = &part.mesh;
        let lo = Vec3::new(-size.x * 0.5, -size.y * 0.5, 0.0);
        let mesh = MeshData {
            positions: m.positions.iter().map(|p| local(*p)).collect(),
            normals: m
                .normals
                .iter()
                .map(|n| (q * Vec3::from_array(*n)).to_array())
                .collect(),
            indices: m.indices.clone(),
            edges: m.edges.clone(),
            bounds: Aabb::new(lo, lo + size),
        };
        // The far level is the plain box of the same extent.
        let mut plain = MeshData::from_boxes(&[(lo, lo + size)]);
        plain.bounds = mesh.bounds;
        let key = GeometryKey(TAG << 56 | id << 32 | i as u64);
        library.insert(
            key,
            PartGeometry {
                lods: vec![mesh, plain],
            },
        );
        placed.push(Placed {
            key,
            role: roles.get(i).copied().unwrap_or(PartRole::Other),
            bounds: b,
        });
    }
    let bounds = placed
        .iter()
        .fold(Aabb::EMPTY, |acc, p| acc.union(&p.bounds));
    ModelPlan {
        placed,
        library,
        bounds,
    }
}

const RAW_PART: u64 = 10_000;

/// The colour of the solid the person points at in the parts list.
pub const HIGHLIGHT: [u8; 3] = [255, 70, 200];

/// The scene of a placed model: the floor under its footprint and every solid in the colour of
/// its role, the one at `highlight` (an index into `plan.placed`) in the highlight colour.
pub fn scene(plan: &ModelPlan, highlight: Option<usize>) -> Scene {
    let size = if plan.bounds.is_empty() {
        Vec3::ZERO
    } else {
        plan.bounds.size()
    };
    let mut scene = Scene::new(PlateFrame {
        width: size.x,
        height: size.y,
    });
    push_floor(&mut scene, size.x, size.z);
    for (i, p) in plan.placed.iter().enumerate() {
        let s = p.bounds.size();
        let origin = Vec3::new(p.bounds.center().x, p.bounds.center().y, p.bounds.min.z);
        let mut item = Item::new(
            ItemId::new(ItemKind::Other, RAW_PART + i as u64),
            Shape::Part(p.key),
            Pose {
                translation: origin,
                rotation: Quat::IDENTITY,
            },
            s,
        );
        item.tint = Some(if highlight == Some(i) {
            HIGHLIGHT
        } else {
            p.role.tint()
        });
        scene.push(item);
    }
    scene.revision = 1;
    scene
}

/// The box a camera frames to look at the hook of the model: the hook and its block with some
/// room, or the whole model when it has neither.
pub fn hook_bounds(plan: &ModelPlan) -> Aabb {
    let hook = plan
        .placed
        .iter()
        .filter(|p| matches!(p.role, PartRole::Hook | PartRole::HookBlock))
        .fold(Aabb::EMPTY, |b, p| b.union(&p.bounds));
    if hook.is_empty() {
        return plan.bounds;
    }
    let pad = Vec3::splat(1500.0);
    Aabb::new(hook.min - pad, hook.max + pad)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(lo: Vec3, hi: Vec3) -> MeshData {
        MeshData::from_boxes(&[(lo, hi)])
    }

    fn part(name: &str, lo: [f32; 3], hi: [f32; 3]) -> RawPart {
        RawPart {
            name: name.into(),
            mesh: block(Vec3::from_array(lo), Vec3::from_array(hi)),
        }
    }

    #[test]
    fn roles_round_trip_through_their_stored_word() {
        for r in PartRole::ALL {
            assert_eq!(PartRole::from_code(r.code()), Some(r));
        }
        assert_eq!(PartRole::from_code("nonsense"), None);
        assert_eq!(
            PartRole::from_code(" hookblock "),
            Some(PartRole::HookBlock)
        );
    }

    #[test]
    fn z_up_lays_the_files_z_axis_up_and_a_turn_keeps_it_there() {
        let up = Orientation::default().rotation() * Vec3::Z;
        assert!((up - Vec3::Y).length() < 1e-5, "{up}");
        let o = Orientation {
            up: UpAxis::Z,
            turns: 1,
        };
        assert!((o.rotation() * Vec3::Z - Vec3::Y).length() < 1e-5);
        // A quarter turn about the vertical takes the file's x axis (after z-up: x) to -z.
        assert!((o.rotation() * Vec3::X - Vec3::NEG_Z).length() < 1e-5);
        let y_up = Orientation {
            up: UpAxis::Y,
            turns: 0,
        };
        assert!((y_up.rotation() * Vec3::Y - Vec3::Y).length() < 1e-6);
    }

    #[test]
    fn the_model_stands_on_the_floor_with_its_corner_at_the_origin() {
        // z up: a 4000 x 2000 x 1000 slab placed far from the origin and below it.
        let model = CraneModel::new(vec![part(
            "Slab",
            [10_000.0, -500.0, -300.0],
            [14_000.0, 1_500.0, 700.0],
        )]);
        let p = plan(&model, Orientation::default(), &[PartRole::Girder]);
        let b = p.bounds;
        assert!(b.min.length() < 1e-3, "{:?}", b.min);
        // x stays x, the file's z (1000) is the height, the file's y (2000) lies along -z.
        assert!((b.size() - Vec3::new(4000.0, 1000.0, 2000.0)).length() < 1e-2);
    }

    #[test]
    fn a_part_mesh_is_centred_in_x_and_y_and_starts_at_z_zero() {
        let model = CraneModel::new(vec![
            part("A", [0.0, 0.0, 0.0], [100.0, 100.0, 100.0]),
            part("B", [300.0, 0.0, 0.0], [500.0, 40.0, 60.0]),
        ]);
        let p = plan(
            &model,
            Orientation {
                up: UpAxis::Y,
                turns: 0,
            },
            &[],
        );
        assert_eq!(p.placed.len(), 2);
        let g = p.library.get(p.placed[1].key).unwrap();
        let m = &g.lods[0];
        let (lo, hi) = (m.bounds.min, m.bounds.max);
        assert!((lo - Vec3::new(-100.0, -20.0, 0.0)).length() < 1e-3, "{lo}");
        assert!((hi - Vec3::new(100.0, 20.0, 60.0)).length() < 1e-3, "{hi}");
        // The positions fill the stated bounds.
        let real = bounds_of(&m.positions);
        assert!((real.min - lo).length() < 1e-3 && (real.max - hi).length() < 1e-3);
        assert_eq!(g.lods.len(), 2);
    }

    #[test]
    fn the_scene_has_the_floor_and_one_tinted_item_per_part() {
        let model = CraneModel::new(vec![
            part("Wheel", [0.0, 0.0, 0.0], [100.0, 100.0, 100.0]),
            part("Hook", [200.0, 0.0, 0.0], [260.0, 100.0, 100.0]),
        ]);
        let roles = [PartRole::Wheel, PartRole::Hook];
        let p = plan(&model, Orientation::default(), &roles);
        let s = scene(&p, Some(1));
        let parts: Vec<&Item> = s
            .items
            .iter()
            .filter(|i| matches!(i.shape, Shape::Part(_)))
            .collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].tint, Some(PartRole::Wheel.tint()));
        // The highlighted solid is drawn in the highlight colour, the others keep their role's.
        assert_eq!(parts[1].tint, Some(HIGHLIGHT));
        let plain = scene(&p, None);
        let hook = plain
            .items
            .iter()
            .find(|i| i.shape == Shape::Part(p.placed[1].key))
            .unwrap();
        assert_eq!(hook.tint, Some(PartRole::Hook.tint()));
        assert!(s.items.iter().any(|i| i.id.kind == ItemKind::Plate));
        // An item stands where its box is.
        let w = s.world_bounds(parts[1]);
        assert!((w.min - p.placed[1].bounds.min).length() < 1e-2, "{w:?}");
        assert!((w.max - p.placed[1].bounds.max).length() < 1e-2);
    }

    #[test]
    fn parts_without_triangles_are_left_out_and_keys_differ_between_plans() {
        let empty = RawPart {
            name: "Empty".into(),
            mesh: MeshData {
                positions: vec![],
                normals: vec![],
                indices: vec![],
                edges: vec![],
                bounds: Aabb::EMPTY,
            },
        };
        let model = CraneModel::new(vec![empty, part("A", [0.0; 3], [1.0; 3])]);
        let a = plan(&model, Orientation::default(), &[]);
        let b = plan(&model, Orientation::default(), &[]);
        assert_eq!(a.placed.len(), 1);
        assert_ne!(a.placed[0].key, b.placed[0].key);
        assert!(
            plan(&CraneModel::new(vec![]), Orientation::default(), &[])
                .placed
                .is_empty()
        );
    }

    #[test]
    fn the_hook_view_frames_the_hook_and_falls_back_to_the_whole_model() {
        let model = CraneModel::new(vec![
            part("Girder", [0.0, 0.0, 0.0], [8000.0, 100.0, 100.0]),
            part("Hook", [4000.0, 0.0, 0.0], [4100.0, 100.0, 100.0]),
        ]);
        let with = plan(
            &model,
            Orientation::default(),
            &[PartRole::Girder, PartRole::Hook],
        );
        assert!(hook_bounds(&with).size().x < with.bounds.size().x);
        let without = plan(&model, Orientation::default(), &[]);
        assert_eq!(hook_bounds(&without), without.bounds);
    }
}
