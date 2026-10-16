//! The scene contract (`docs/design/3d-view.md` section 5): what a builder
//! produces and the renderer, picking and culling read. Plain data, no GPU,
//! no model dependency.

use crate::math::Aabb;
use crate::mesh::GeometryKey;
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

/// Version of this contract; bumped with every change to the types below.
pub const CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum ItemKind {
    Enclosure,
    Plate,
    Rail,
    Duct,
    Device,
    Door,
    Wire,
    Other,
}

impl ItemKind {
    pub const COUNT: usize = 8;

    /// Position in tables indexed by kind (`Look::surfaces`).
    pub fn index(self) -> usize {
        match self {
            ItemKind::Enclosure => 0,
            ItemKind::Plate => 1,
            ItemKind::Rail => 2,
            ItemKind::Duct => 3,
            ItemKind::Device => 4,
            ItemKind::Door => 5,
            ItemKind::Wire => 6,
            ItemKind::Other => 7,
        }
    }
}

/// Stable identity of a thing in the scene. `raw` is the model's raw id
/// (unique across kinds, as in novo-layout), so a click maps back to the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ItemId {
    pub kind: ItemKind,
    pub raw: u64,
}

impl ItemId {
    pub fn new(kind: ItemKind, raw: u64) -> Self {
        Self { kind, raw }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub u32);

/// World pose in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    pub translation: Vec3,
    pub rotation: Quat,
}

impl Pose {
    pub const IDENTITY: Pose = Pose {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
    };

    pub fn at(translation: Vec3) -> Self {
        Self {
            translation,
            rotation: Quat::IDENTITY,
        }
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.translation)
    }
}

/// Cross sections of rails and ducts. The section is drawn at its real size
/// in millimetres and extruded along local X by `Item::size.x`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Profile {
    /// 35 x 7.5 mm top-hat rail.
    TopHat35x7_5,
    /// 35 x 15 mm top-hat rail.
    TopHat35x15,
    /// Open wiring duct; width is `size.y`, height is `size.z`.
    Channel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Shape {
    /// Fills `Item::size`.
    Box,
    /// A rail or duct section extruded along local X.
    Profile(Profile),
    /// Maker or generated geometry from the `MeshLibrary`; drawn as a box
    /// until the library has it, so the view never waits.
    Part(GeometryKey),
}

/// A condition the builder found, drawn as a tint or edge colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum Mark {
    #[default]
    None,
    Collision,
    TooClose,
    Outside,
    GuessedSize,
    GeometryMismatch,
}

impl Mark {
    pub const COUNT: usize = 6;

    /// Position in tables indexed by mark (`Look::marks`).
    pub fn index(self) -> usize {
        match self {
            Mark::None => 0,
            Mark::Collision => 1,
            Mark::TooClose => 2,
            Mark::Outside => 3,
            Mark::GuessedSize => 4,
            Mark::GeometryMismatch => 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: ItemId,
    pub shape: Shape,
    pub pose: Pose,
    /// Extents of the local box, mm: x in [-sx/2, sx/2], y in [-sy/2, sy/2],
    /// z in [0, sz]. The origin is the middle of the back face.
    pub size: Vec3,
    /// Maker colour, if the part has one.
    pub tint: Option<[u8; 3]>,
    pub mark: Mark,
    pub label: Option<Arc<str>>,
    /// Doors and covers: drawn see-through, after the opaque items.
    pub translucent: bool,
    pub group: Option<GroupId>,
}

impl Item {
    pub fn new(id: ItemId, shape: Shape, pose: Pose, size: Vec3) -> Self {
        Self {
            id,
            shape,
            pose,
            size,
            tint: None,
            mark: Mark::None,
            label: None,
            translucent: false,
            group: None,
        }
    }

    /// The local box.
    pub fn local_bounds(&self) -> Aabb {
        Aabb::new(
            Vec3::new(-self.size.x * 0.5, -self.size.y * 0.5, 0.0),
            Vec3::new(self.size.x * 0.5, self.size.y * 0.5, self.size.z),
        )
    }
}

/// A hinge: its items rotate by `angle` about `axis` through `pivot`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Group {
    pub id: GroupId,
    pub pivot: Vec3,
    pub axis: Vec3,
    pub angle: f32,
}

impl Group {
    pub fn matrix(&self) -> Mat4 {
        let axis = self.axis.try_normalize().unwrap_or(Vec3::Y);
        Mat4::from_translation(self.pivot)
            * Mat4::from_axis_angle(axis, self.angle)
            * Mat4::from_translation(-self.pivot)
    }
}

/// Conversion between the 2D plate of `cabinet-layout.md` (origin top left,
/// y down) and the 3D world (origin lower left, y up, z out of the plate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateFrame {
    pub width: f32,
    pub height: f32,
}

impl PlateFrame {
    /// A 2D plate point and a height above the plate surface, in the world.
    pub fn world(&self, x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(x as f32, self.height - y as f32, z as f32)
    }

    /// The 2D plate point (x, y down) under a world point.
    pub fn plate(&self, p: Vec3) -> (f64, f64) {
        (p.x as f64, (self.height - p.y) as f64)
    }
}

/// A wire or cable path through the cabinet, in world millimetres, drawn as
/// a thin tube. Wires are not boxes, so they live beside the items; their
/// `ItemId` has kind `ItemKind::Wire` so selection and picking work the same.
#[derive(Debug, Clone, PartialEq)]
pub struct Wire {
    pub id: ItemId,
    pub points: Vec<Vec3>,
    /// Tube radius, mm.
    pub radius: f32,
    pub tint: Option<[u8; 3]>,
    /// A second colour in bands along the tube (the green-yellow of a
    /// protective conductor); `None` for a plain wire.
    pub stripe: Option<[u8; 3]>,
    pub mark: Mark,
    pub label: Option<Arc<str>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub frame: PlateFrame,
    pub items: Vec<Item>,
    pub wires: Vec<Wire>,
    pub groups: Vec<Group>,
    /// Bumped by the builder when items change; the renderer re-uploads.
    pub revision: u64,
    /// Bumped when only group angles change (door animation).
    pub pose_revision: u64,
}

impl Scene {
    pub fn new(frame: PlateFrame) -> Self {
        Self {
            frame,
            items: Vec::new(),
            wires: Vec::new(),
            groups: Vec::new(),
            revision: 0,
            pose_revision: 0,
        }
    }

    pub fn push(&mut self, item: Item) {
        self.items.push(item);
    }

    pub fn get(&self, id: ItemId) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// Sets a hinge angle and bumps `pose_revision`.
    pub fn set_group_angle(&mut self, id: GroupId, angle: f32) {
        if let Some(g) = self.groups.iter_mut().find(|g| g.id == id)
            && g.angle != angle
        {
            g.angle = angle;
            self.pose_revision += 1;
        }
    }

    /// Local-to-world transform of an item, hinge included.
    pub fn world_matrix(&self, item: &Item) -> Mat4 {
        let m = item.pose.matrix();
        match item.group.and_then(|g| self.group(g)) {
            Some(g) => g.matrix() * m,
            None => m,
        }
    }

    pub fn world_bounds(&self, item: &Item) -> Aabb {
        item.local_bounds().transformed(&self.world_matrix(item))
    }

    /// Bounds of everything.
    pub fn bounds(&self) -> Aabb {
        let mut b = Aabb::EMPTY;
        for i in &self.items {
            b = b.union(&self.world_bounds(i));
        }
        for w in &self.wires {
            for p in &w.points {
                b.extend(*p);
            }
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plate_frame_round_trips() {
        let f = PlateFrame {
            width: 600.0,
            height: 800.0,
        };
        let p = f.world(100.0, 50.0, 7.5);
        assert_eq!(p, Vec3::new(100.0, 750.0, 7.5));
        assert_eq!(f.plate(p), (100.0, 50.0));
    }

    #[test]
    fn hinge_rotates_about_its_pivot() {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        s.groups.push(Group {
            id: GroupId(1),
            pivot: Vec3::new(0.0, 0.0, 0.0),
            axis: Vec3::Y,
            angle: 0.0,
        });
        let mut door = Item::new(
            ItemId::new(ItemKind::Door, 1),
            Shape::Box,
            Pose::at(Vec3::new(50.0, 50.0, 10.0)),
            Vec3::new(100.0, 100.0, 2.0),
        );
        door.group = Some(GroupId(1));
        s.push(door);
        let before = s.bounds();
        s.set_group_angle(GroupId(1), std::f32::consts::FRAC_PI_2);
        assert_eq!(s.pose_revision, 1);
        let after = s.bounds();
        // Opened 90 degrees about Y at x = 0: the door now extends along z.
        assert!(after.size().z > before.size().z + 50.0);
        assert!(after.size().x < before.size().x);
    }

    #[test]
    fn wires_count_towards_the_bounds() {
        let mut s = Scene::new(PlateFrame {
            width: 10.0,
            height: 10.0,
        });
        s.wires.push(Wire {
            id: ItemId::new(ItemKind::Wire, 1),
            points: vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 50.0, 20.0)],
            radius: 1.0,
            tint: None,
            stripe: None,
            mark: Mark::None,
            label: None,
        });
        assert_eq!(s.bounds().max, Vec3::new(100.0, 50.0, 20.0));
    }

    #[test]
    fn kind_and_mark_indices_are_dense() {
        use std::collections::HashSet;
        let kinds = [
            ItemKind::Enclosure,
            ItemKind::Plate,
            ItemKind::Rail,
            ItemKind::Duct,
            ItemKind::Device,
            ItemKind::Door,
            ItemKind::Wire,
            ItemKind::Other,
        ];
        let set: HashSet<usize> = kinds.iter().map(|k| k.index()).collect();
        assert_eq!(set.len(), ItemKind::COUNT);
        assert!(set.iter().all(|&i| i < ItemKind::COUNT));
        let marks = [
            Mark::None,
            Mark::Collision,
            Mark::TooClose,
            Mark::Outside,
            Mark::GuessedSize,
            Mark::GeometryMismatch,
        ];
        let set: HashSet<usize> = marks.iter().map(|m| m.index()).collect();
        assert_eq!(set.len(), Mark::COUNT);
    }

    #[test]
    fn bounds_cover_the_local_box() {
        let mut s = Scene::new(PlateFrame {
            width: 10.0,
            height: 10.0,
        });
        s.push(Item::new(
            ItemId::new(ItemKind::Device, 1),
            Shape::Box,
            Pose::at(Vec3::new(10.0, 20.0, 5.0)),
            Vec3::new(4.0, 6.0, 8.0),
        ));
        let b = s.bounds();
        assert_eq!(b.min, Vec3::new(8.0, 17.0, 5.0));
        assert_eq!(b.max, Vec3::new(12.0, 23.0, 13.0));
    }
}
