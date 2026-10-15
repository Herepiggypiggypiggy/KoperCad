//! Scene builder: turns a cabinet (plate, rails, ducts, devices, door and
//! routed wires) into a [`Scene`]. Pure geometry over `novo-layout` types;
//! [`model`] reads the project into the inputs. Contract and axes:
//! `docs/design/3d-view.md` sections 4 and 5.
//!
//! Heights (mm, off the plate surface): a 35 x 7.5 rail stands 7.5; a device
//! on a rail has its back on the rail's top, 7.5, and off a rail at 0. A
//! missing device depth is 60 mm and a missing duct section 40 x 60 mm; both
//! are marked `GuessedSize` instead of being drawn as zero.

pub mod model;

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;
use std::sync::Arc;

use glam::{Quat, Vec3};
use novo_layout::state::CabinetLayout;

use crate::scene::{
    Group, GroupId, Item, ItemId, ItemKind, Mark, PlateFrame, Pose, Profile, Scene, Shape, Wire,
};

/// Thickness of the mounting plate, behind the plate surface (z < 0).
pub const PLATE_THICKNESS_MM: f64 = 3.0;
/// Standard 35 x 7.5 mm top-hat rail.
pub const DIN_RAIL_WIDTH_MM: f64 = 35.0;
pub const DIN_RAIL_HEIGHT_MM: f64 = 7.5;
/// Section assumed for a duct whose width or height is unknown (width, height).
pub const DEFAULT_DUCT_MM: (f64, f64) = (40.0, 60.0);
/// Depth assumed for a device whose part has none.
pub const DEFAULT_DEPTH_MM: f64 = 60.0;
/// Height off the plate of a wire in free air.
pub const FREE_AIR_Z_MM: f64 = 15.0;
/// Thickness of the door panel.
pub const DOOR_THICKNESS_MM: f64 = 2.0;
/// Depth of the enclosure when nothing else says so.
pub const DEFAULT_ENCLOSURE_DEPTH_MM: f64 = 250.0;
/// Drawn diameter of a wire with no cross-section.
pub const WIRE_DIAMETER_MM: f64 = 3.0;
/// The hinge group of the door.
pub const DOOR_HINGE: GroupId = GroupId(1);

/// A device's depth, from its part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Depth {
    pub mm: f64,
    pub guessed: bool,
}

impl Depth {
    pub fn from_part(depth: Option<f64>) -> Depth {
        match depth {
            Some(d) if d > 0.0 => Depth {
                mm: d,
                guessed: false,
            },
            _ => Depth {
                mm: DEFAULT_DEPTH_MM,
                guessed: true,
            },
        }
    }
}

/// Which side the door is hinged on, seen from the front (from the door).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HingeSide {
    #[default]
    Left,
    Right,
}

/// The door as far as the model knows it. `open_deg` is 0 for closed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DoorSpec {
    pub hinge: HingeSide,
    pub open_deg: f64,
}

/// The enclosure shell. The interior is taken to be as wide and high as the
/// plate; depth is from the plate surface to the inside of the door.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shell {
    pub depth: f64,
    pub door: Option<DoorSpec>,
}

impl Default for Shell {
    fn default() -> Shell {
        Shell {
            depth: DEFAULT_ENCLOSURE_DEPTH_MM,
            door: Some(DoorSpec {
                hinge: HingeSide::Left,
                open_deg: 0.0,
            }),
        }
    }
}

/// How a wire was routed; the same split as the routing engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireKind {
    Ducted,
    FreeAir,
}

/// A routed wire: the plate path of a route and the ids of the ducts it lies
/// in. A route of `novo-routing` converts 1:1. Paths are horizontal or
/// vertical runs, as the routing engine makes them.
#[derive(Debug, Clone, PartialEq)]
pub struct WireInput {
    pub id: u64,
    pub kind: WireKind,
    pub points: Vec<(f64, f64)>,
    pub ducts: Vec<u64>,
    pub diameter_mm: Option<f64>,
}

/// A duct on the plate. `(x, y)` is the start, as on the model's `Rail`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuctShape {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub length: f64,
    pub vertical: bool,
    /// Width across the plate and height off it; 0 while unknown.
    pub width: f64,
    pub height: f64,
}

/// What the scene needs beyond the 2D layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Inputs {
    pub depths: BTreeMap<u64, Depth>,
    pub ducts: Vec<DuctShape>,
    /// Layout rails that run down the plate instead of across.
    pub vertical_rails: Vec<u64>,
    /// Device labels, for example `-K1`.
    pub labels: BTreeMap<u64, Arc<str>>,
}

/// Builder options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    pub shell: Shell,
    pub wires: Vec<WireInput>,
}

fn line_item(
    id: ItemId,
    profile: Profile,
    (x, y): (f64, f64),
    length: f64,
    vertical: bool,
    section: (f64, f64),
    frame: &PlateFrame,
) -> Item {
    // The item's local X runs along the line; the origin is the middle of the
    // back face, so the centre of the line on the plate surface.
    let (cx, cy) = if vertical {
        (x, y + length / 2.0)
    } else {
        (x + length / 2.0, y)
    };
    let rotation = if vertical {
        Quat::from_rotation_z(FRAC_PI_2)
    } else {
        Quat::IDENTITY
    };
    Item::new(
        id,
        Shape::Profile(profile),
        Pose {
            translation: frame.world(cx, cy, 0.0),
            rotation,
        },
        Vec3::new(length as f32, section.0 as f32, section.1 as f32),
    )
}

/// Builds the scene of one cabinet. Item order is fixed (shell, plate,
/// rails, ducts, devices, door), each group by id; wires follow in the given
/// order. Ids are unique per kind, which is asserted in debug builds.
pub fn from_layout(layout: &CabinetLayout, inputs: &Inputs, options: &Options) -> Scene {
    let plate = &layout.plate;
    let frame = PlateFrame {
        width: plate.width as f32,
        height: plate.height as f32,
    };
    let mut scene = Scene::new(frame);
    let depth = options.shell.depth.max(0.0);
    let centre = |z: f64| frame.world(plate.width / 2.0, plate.height / 2.0, z);

    let mut shell = Item::new(
        ItemId::new(ItemKind::Enclosure, 0),
        Shape::Box,
        Pose::at(centre(0.0)),
        Vec3::new(frame.width, frame.height, depth as f32),
    );
    shell.translucent = true;
    scene.push(shell);
    scene.push(Item::new(
        ItemId::new(ItemKind::Plate, 0),
        Shape::Box,
        Pose::at(centre(-PLATE_THICKNESS_MM)),
        Vec3::new(frame.width, frame.height, PLATE_THICKNESS_MM as f32),
    ));

    let mut rails: Vec<_> = layout.rails.iter().collect();
    rails.sort_by_key(|r| r.id);
    for r in rails {
        scene.push(line_item(
            ItemId::new(ItemKind::Rail, r.id),
            Profile::TopHat35x7_5,
            (r.x, r.y),
            r.length,
            inputs.vertical_rails.contains(&r.id),
            (DIN_RAIL_WIDTH_MM, DIN_RAIL_HEIGHT_MM),
            &frame,
        ));
    }

    let mut ducts = inputs.ducts.clone();
    ducts.sort_by_key(|d| d.id);
    for d in &ducts {
        let guessed = d.width <= 0.0 || d.height <= 0.0;
        let section = if guessed {
            DEFAULT_DUCT_MM
        } else {
            (d.width, d.height)
        };
        let mut item = line_item(
            ItemId::new(ItemKind::Duct, d.id),
            Profile::Channel,
            (d.x, d.y),
            d.length,
            d.vertical,
            section,
            &frame,
        );
        if guessed {
            item.mark = Mark::GuessedSize;
        }
        scene.push(item);
    }

    let report = layout.report();
    let mut devices: Vec<_> = layout.placements.iter().collect();
    devices.sort_by_key(|p| p.id);
    for p in devices {
        let d = inputs
            .depths
            .get(&p.id)
            .copied()
            .unwrap_or_else(|| Depth::from_part(None));
        let z0 = match p.rail {
            Some(r) if layout.rails.iter().any(|x| x.id == r) => DIN_RAIL_HEIGHT_MM,
            _ => 0.0,
        };
        let f = p.footprint();
        let mut item = Item::new(
            ItemId::new(ItemKind::Device, p.id),
            Shape::Box,
            Pose {
                translation: frame.world(f.x + f.w / 2.0, f.y + f.h / 2.0, z0),
                // R turns the part clockwise seen from the door.
                rotation: if p.rotated {
                    Quat::from_rotation_z(-FRAC_PI_2)
                } else {
                    Quat::IDENTITY
                },
            },
            // Local extents are the unrotated part; the pose turns them.
            Vec3::new(p.size.width as f32, p.size.height as f32, d.mm as f32),
        );
        item.mark = if report.collisions.iter().any(|c| c.0 == p.id || c.1 == p.id) {
            Mark::Collision
        } else if report.outside.contains(&p.id) {
            Mark::Outside
        } else if report.too_close.iter().any(|c| c.0 == p.id || c.1 == p.id) {
            Mark::TooClose
        } else if p.size.guessed || d.guessed {
            Mark::GuessedSize
        } else {
            Mark::None
        };
        item.label = inputs.labels.get(&p.id).cloned();
        scene.push(item);
    }

    if let Some(door) = options.shell.door {
        let (pivot_x, angle) = match door.hinge {
            // Seen from the front +X is to the right. A left-hinged door
            // swings out (+Z) with a negative turn about +Y.
            HingeSide::Left => (0.0, -door.open_deg.to_radians()),
            HingeSide::Right => (plate.width, door.open_deg.to_radians()),
        };
        let mut item = Item::new(
            ItemId::new(ItemKind::Door, 0),
            Shape::Box,
            Pose::at(centre(depth)),
            Vec3::new(frame.width, frame.height, DOOR_THICKNESS_MM as f32),
        );
        item.translucent = true;
        item.group = Some(DOOR_HINGE);
        scene.push(item);
        scene.groups.push(Group {
            id: DOOR_HINGE,
            pivot: Vec3::new(pivot_x as f32, 0.0, depth as f32),
            axis: Vec3::Y,
            angle: angle as f32,
        });
    }

    for w in &options.wires {
        push_wire(&mut scene, w, &ducts);
    }
    debug_assert!(unique_ids(&scene), "duplicate id in the scene");
    scene
}

fn unique_ids(scene: &Scene) -> bool {
    let mut seen = std::collections::HashSet::new();
    scene.items.iter().map(|i| i.id).all(|id| seen.insert(id))
        && scene.wires.iter().map(|w| w.id).all(|id| seen.insert(id))
}

/// Height of a wire: halfway up the shallowest duct it lies in, else free air.
fn wire_height(w: &WireInput, ducts: &[DuctShape]) -> f64 {
    match w.kind {
        WireKind::FreeAir => FREE_AIR_Z_MM,
        WireKind::Ducted => w
            .ducts
            .iter()
            .filter_map(|id| ducts.iter().find(|d| d.id == *id))
            .map(|d| {
                if d.height > 0.0 {
                    d.height
                } else {
                    DEFAULT_DUCT_MM.1
                }
            })
            .reduce(f64::min)
            .map_or(FREE_AIR_Z_MM, |h| h / 2.0),
    }
}

/// A wire is a path of points at one height with a tube radius.
fn push_wire(scene: &mut Scene, w: &WireInput, ducts: &[DuctShape]) {
    let z = wire_height(w, ducts);
    let dia = w
        .diameter_mm
        .filter(|d| *d > 0.0)
        .unwrap_or(WIRE_DIAMETER_MM);
    let frame = scene.frame;
    scene.wires.push(Wire {
        id: ItemId::new(ItemKind::Wire, w.id),
        points: w
            .points
            .iter()
            .map(|&(x, y)| frame.world(x, y, z))
            .collect(),
        radius: (dia / 2.0) as f32,
        tint: None,
        // A wire in free air found no duct: flag it like the routing engine does.
        mark: if w.kind == WireKind::FreeAir {
            Mark::Outside
        } else {
            Mark::None
        },
        label: None,
    });
}

/// Devices whose back-to-front extent reaches the inside of the door, with
/// the overshoot in millimetres: the 2D answer to "does the door close".
pub fn reaching_door(scene: &Scene) -> Vec<(u64, f64)> {
    let Some(door) = scene.get(ItemId::new(ItemKind::Door, 0)) else {
        return Vec::new();
    };
    let inner = door.pose.translation.z as f64;
    let mut out: Vec<(u64, f64)> = scene
        .items
        .iter()
        .filter(|i| i.id.kind == ItemKind::Device)
        .filter_map(|i| {
            let over = (i.pose.translation.z + i.size.z) as f64 - inner;
            (over >= 0.0).then_some((i.id.raw, over))
        })
        .collect();
    out.sort_by_key(|v| v.0);
    out
}

/// Replaces `current` with `next` and bumps the revision counters the
/// renderer watches: `revision` when items changed, `pose_revision` when only
/// a hinge angle did. An identical scene changes nothing.
pub fn refresh(current: &mut Scene, mut next: Scene) {
    let items_same = current.frame == next.frame
        && current.items == next.items
        && current.wires == next.wires
        && current.groups.len() == next.groups.len()
        && current
            .groups
            .iter()
            .zip(&next.groups)
            .all(|(a, b)| a.id == b.id && a.pivot == b.pivot && a.axis == b.axis);
    let angles_same = items_same
        && current
            .groups
            .iter()
            .zip(&next.groups)
            .all(|(a, b)| a.angle == b.angle);
    next.revision = current.revision + u64::from(!items_same);
    next.pose_revision = current.pose_revision + u64::from(items_same && !angles_same);
    *current = next;
}

#[cfg(test)]
mod tests;
