//! Motion for the user's own crane model (lane R3, part 3): the roles of the solids decide how each
//! one moves when the simulation drives the crane.
//!
//! - **Bridge** (girders, end trucks, the wheels that roll along the runway): travel along the runway.
//! - **Trolley** (the drum, the wheels that roll along the girder, whatever touches them): travel
//!   with the bridge and along the girder.
//! - **Hook** (hook block, hook, whatever touches them): travel with the trolley, rise and sink with
//!   the hook height and swing on the rope.
//! - **Frame** (rails, runway beams, whatever touches them only): stand still.
//!
//! Wheels turn with the distance they roll, the drum turns with the rope it pays out. A solid whose
//! role says nothing about motion (other, motor, gearbox) moves with the nearest solid that does.
//! The result is a guess from names and touching, shown in the parts window so it can be corrected
//! by changing a role. Pure data, like [`crate::crane_model`].

use crate::crane::{
    CraneState, boxed, push_fall, push_floor, push_hoist_flag, push_load, push_slack,
    push_travel_flag, size,
};
use crate::crane_model::{HIGHLIGHT, ModelPlan, PartRole, Placed};
use crate::math::Aabb;
use crate::scene::{Item, ItemId, ItemKind, PlateFrame, Pose, Scene, Shape};
use glam::{Quat, Vec3};
use std::f32::consts::TAU;

/// What a solid of the model moves with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Group {
    /// Stands still.
    Frame,
    /// Travels along the runway with the bridge.
    Bridge,
    /// Travels with the bridge and along the girder with the trolley.
    Trolley,
    /// Travels with the trolley, moves with the hook height and swings.
    Hook,
}

impl Group {
    /// A short label for lists.
    pub fn label(self) -> &'static str {
        match self {
            Group::Frame => "stands still",
            Group::Bridge => "bridge",
            Group::Trolley => "trolley",
            Group::Hook => "hook",
        }
    }
}

/// Which way an axle points in the picture: along the runway (x) or across the span (z).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axle {
    X,
    Z,
}

impl Axle {
    fn vector(self) -> Vec3 {
        match self {
            Axle::X => Vec3::X,
            Axle::Z => Vec3::Z,
        }
    }
}

/// How a solid turns.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Spin {
    /// A wheel of the bridge (axle across the span) or the trolley (axle along the runway): turns
    /// once per circumference rolled.
    Wheel {
        axle: Axle,
        radius_mm: f32,
        trolley: bool,
    },
    /// The rope drum: turns as the rope pays out, over `FALLS` falls.
    Drum { axle: Axle, radius_mm: f32 },
}

/// The block hangs on this many falls of rope: the drum pays out that many millimetres per
/// millimetre the hook sinks.
pub const FALLS: f32 = 2.0;

/// The model's motion worked out from its solids and their roles.
#[derive(Debug, Clone)]
pub struct Rig {
    /// What each placed solid moves with, in the order of the plan.
    pub groups: Vec<Group>,
    spins: Vec<Option<Spin>>,
    /// Where the bridge and the trolley start, in metres along their travel (0 is the end of the
    /// rails, or of the girder).
    pub rest_m: [f32; 2],
    /// How far each can travel, metres: the travel runs from 0 to this.
    pub extent_m: [f32; 2],
    /// Where the ropes leave the drum at rest, mm: the drum's middle in x and z, its underside.
    pivot: Option<Vec3>,
    /// The way the falls lie apart (the drum's axle).
    fall_axle: Axle,
    /// The box around the hook parts at rest, mm.
    hook: Aabb,
    /// The box around the rails, the girders, the bridge and the trolley, for flags and lamps.
    rails: Aabb,
    girders: Aabb,
    trolley: Aabb,
    bridge: Aabb,
}

fn gap(a: &Aabb, b: &Aabb) -> f32 {
    let d = |lo_a: f32, hi_a: f32, lo_b: f32, hi_b: f32| (lo_a - hi_b).max(lo_b - hi_a).max(0.0);
    Vec3::new(
        d(a.min.x, a.max.x, b.min.x, b.max.x),
        d(a.min.y, a.max.y, b.min.y, b.max.y),
        d(a.min.z, a.max.z, b.min.z, b.max.z),
    )
    .length()
}

fn volume(b: &Aabb) -> f32 {
    let s = b.size();
    s.x * s.y * s.z
}

/// The smallest of the three extents: the way a wheel's axle points.
fn thinnest(s: Vec3) -> usize {
    if s.x <= s.y && s.x <= s.z {
        0
    } else if s.y <= s.z {
        1
    } else {
        2
    }
}

fn union_of(plan: &ModelPlan, pick: impl Fn(usize, &Placed) -> bool) -> Aabb {
    plan.placed
        .iter()
        .enumerate()
        .filter(|(i, p)| pick(*i, p))
        .fold(Aabb::EMPTY, |b, (_, p)| b.union(&p.bounds))
}

/// The group a solid belongs to by its role alone, and how it turns. `None` for a role that says
/// nothing about motion.
fn by_role(p: &Placed) -> (Option<Group>, Option<Spin>) {
    let s = p.bounds.size();
    match p.role {
        PartRole::Rail => (Some(Group::Frame), None),
        PartRole::Girder => (Some(Group::Bridge), None),
        PartRole::Hook | PartRole::HookBlock => (Some(Group::Hook), None),
        PartRole::Drum => {
            // The axle lies along the longer horizontal side; the radius is the mean of the other two.
            let (axle, along, across) = if s.x > s.z {
                (Axle::X, s.x, s.z)
            } else {
                (Axle::Z, s.z, s.x)
            };
            let _ = along;
            (
                Some(Group::Trolley),
                Some(Spin::Drum {
                    axle,
                    radius_mm: (across + s.y) * 0.25,
                }),
            )
        }
        PartRole::Wheel => match thinnest(s) {
            // Axle across the span: rolls along the runway with the bridge.
            2 => (
                Some(Group::Bridge),
                Some(Spin::Wheel {
                    axle: Axle::Z,
                    radius_mm: (s.x + s.y) * 0.25,
                    trolley: false,
                }),
            ),
            // Axle along the runway: rolls across the span with the trolley.
            0 => (
                Some(Group::Trolley),
                Some(Spin::Wheel {
                    axle: Axle::X,
                    radius_mm: (s.y + s.z) * 0.25,
                    trolley: true,
                }),
            ),
            // A wheel on a vertical axle (a guide roller) does not roll on a rail.
            _ => (Some(Group::Frame), None),
        },
        _ => (None, None),
    }
}

/// The group of the solid `i` taken from the nearest of the `anchors` (solids whose role decided).
fn nearest_group(
    plan: &ModelPlan,
    i: usize,
    anchors: &[usize],
    decided: &[Option<Group>],
) -> Group {
    let b = &plan.placed[i].bounds;
    let mut best: Option<(f32, f32, Group)> = None;
    for &a in anchors {
        let ab = &plan.placed[a].bounds;
        let (g, v) = (gap(b, ab), volume(ab));
        let take = match best {
            None => true,
            Some((bg, bv, _)) => {
                if (g - bg).abs() <= 1.0 {
                    v > bv
                } else {
                    g < bg
                }
            }
        };
        if take {
            let nearest = best.map_or(g, |(bg, _, _)| bg.min(g));
            best = Some((nearest, v, decided[a].unwrap_or(Group::Frame)));
        }
    }
    best.map_or(Group::Frame, |x| x.2)
}

/// How far a group of solids can move along one axis between two walls (`lo..hi` of the track,
/// `group` its own extent), and where it stands now: `(rest, extent)` in metres with 0 at the end
/// of the track. `fallback` is the track to use when the model has none.
fn track(track_lo: f32, track_hi: f32, group_lo: f32, group_hi: f32) -> (f32, f32) {
    let back = (group_lo - track_lo).max(0.0);
    let fwd = (track_hi - group_hi).max(0.0);
    (back / 1000.0, (back + fwd) / 1000.0)
}

/// Works out the motion of a placed model.
pub fn rig(plan: &ModelPlan) -> Rig {
    let n = plan.placed.len();
    let mut groups: Vec<Option<Group>> = Vec::with_capacity(n);
    let mut spins: Vec<Option<Spin>> = Vec::with_capacity(n);
    for p in &plan.placed {
        let (g, s) = by_role(p);
        groups.push(g);
        spins.push(s);
    }
    // Everything else moves with the nearest solid whose role decided: the closest by the gap
    // between boxes, and of those that touch (within a millimetre) the biggest.
    let anchors: Vec<usize> = (0..n).filter(|i| groups[*i].is_some()).collect();
    let decided = groups.clone();
    for (i, g) in groups.iter_mut().enumerate() {
        if g.is_none() {
            *g = Some(nearest_group(plan, i, &anchors, &decided));
        }
    }
    let groups: Vec<Group> = groups
        .into_iter()
        .map(|g| g.unwrap_or(Group::Frame))
        .collect();

    let rails = union_of(plan, |_, p| p.role == PartRole::Rail);
    let girders = union_of(plan, |_, p| p.role == PartRole::Girder);
    let bridge = union_of(plan, |i, _| groups[i] == Group::Bridge);
    let trolley = union_of(plan, |i, _| groups[i] == Group::Trolley);
    let hook = union_of(plan, |i, _| groups[i] == Group::Hook);
    let model = plan.bounds;

    // The bridge runs between the ends of the rails; without rails it may go one half model length
    // each way. The trolley runs along the girders (or along the bridge when no part is a girder).
    let (b_ref, b_group) = (
        rails,
        if bridge.is_empty() {
            None
        } else {
            Some(bridge)
        },
    );
    let bridge_track = match b_group {
        None => (0.0, 0.0),
        Some(g) if !b_ref.is_empty() => track(b_ref.min.x, b_ref.max.x, g.min.x, g.max.x),
        Some(g) => {
            let half = model.size().x * 0.5;
            track(g.min.x - half, g.max.x + half, g.min.x, g.max.x)
        }
    };
    let t_group = if trolley.is_empty() { hook } else { trolley };
    let t_ref = if !girders.is_empty() { girders } else { bridge };
    let trolley_track = if t_group.is_empty() {
        (0.0, 0.0)
    } else if !t_ref.is_empty() {
        track(t_ref.min.z, t_ref.max.z, t_group.min.z, t_group.max.z)
    } else {
        let half = model.size().z * 0.5;
        track(
            t_group.min.z - half,
            t_group.max.z + half,
            t_group.min.z,
            t_group.max.z,
        )
    };

    // The ropes leave the underside of the biggest drum.
    let drum = plan
        .placed
        .iter()
        .enumerate()
        .filter(|(_, p)| p.role == PartRole::Drum)
        .max_by(|(_, a), (_, b)| volume(&a.bounds).total_cmp(&volume(&b.bounds)))
        .map(|(i, p)| (i, p.bounds));
    let pivot = drum.map(|(_, b)| Vec3::new(b.center().x, b.min.y, b.center().z));
    let fall_axle = drum
        .and_then(|(i, _)| match spins[i] {
            Some(Spin::Drum { axle, .. }) => Some(axle),
            _ => None,
        })
        .unwrap_or(Axle::Z);
    Rig {
        groups,
        spins,
        rest_m: [bridge_track.0, trolley_track.0],
        extent_m: [bridge_track.1, trolley_track.1],
        pivot,
        fall_axle,
        hook,
        rails,
        girders,
        trolley,
        bridge,
    }
}

impl Rig {
    /// The ropes' length at rest and at `hook_m`: from the underside of the drum to the top of the
    /// hook parts. Zero when the model has no drum or no hook.
    pub fn rope_length_mm(&self, hook_m: f32) -> f32 {
        match self.pivot {
            Some(p) if !self.hook.is_empty() => {
                (p.y - (self.hook.max.y + self.hook_shift(hook_m).0)).max(0.0)
            }
            _ => 0.0,
        }
    }

    /// How far the hook parts move up for `hook_m`, mm, held between the drum and the floor, and
    /// whether it was held.
    fn hook_shift(&self, hook_m: f32) -> (f32, bool) {
        let want = hook_m * 1000.0;
        if self.hook.is_empty() {
            return (0.0, false);
        }
        let lo = -self.hook.min.y;
        let hi = self
            .pivot
            .map_or(f32::MAX, |p| p.y - self.hook.max.y)
            .max(lo);
        let dy = want.clamp(lo, hi);
        (dy, (dy - want).abs() > 0.5)
    }

    /// The box a camera frames to look at the hook with its ropes and the drum above it.
    pub fn hook_view(&self, hook_m: f32) -> Option<Aabb> {
        if self.hook.is_empty() {
            return None;
        }
        let (dy, _) = self.hook_shift(hook_m);
        let mut b = Aabb::new(self.hook.min + Vec3::Y * dy, self.hook.max + Vec3::Y * dy);
        if let Some(p) = self.pivot {
            b.extend(p);
        }
        let pad = Vec3::splat(1200.0);
        Some(Aabb::new(b.min - pad, b.max + pad))
    }
}

const RAW_PART: u64 = 10_000;
const RAW_LAMP: u64 = 130;
const BLOCK_WARN: [u8; 3] = [220, 60, 50];

/// The scene of the placed model for one frame of `state`: every solid moved by its group, the
/// wheels and the drum turned, the ropes between the drum and the hook parts, the load under the
/// hook, and the flags and lamps the generic crane has. `state.bridge_m` and `state.trolley_m`
/// are positions along the travel (0 to `rig.extent_m`), as `rig.rest_m` is.
pub fn scene(plan: &ModelPlan, rig: &Rig, state: &CraneState, load: Option<[f32; 3]>) -> Scene {
    scene_with(plan, rig, state, load, None)
}

/// [`scene`] with the `highlight`th placed solid in the highlight colour (the one the person points
/// at in the parts list).
pub fn scene_with(
    plan: &ModelPlan,
    rig: &Rig,
    state: &CraneState,
    load: Option<[f32; 3]>,
    highlight: Option<usize>,
) -> Scene {
    let size3 = if plan.bounds.is_empty() {
        Vec3::ZERO
    } else {
        plan.bounds.size()
    };
    let mut scene = Scene::new(PlateFrame {
        width: size3.x,
        height: size3.y,
    });
    push_floor(&mut scene, size3.x, size3.z);
    let pos_b = state.bridge_m.clamp(0.0, rig.extent_m[0]);
    let pos_t = state.trolley_m.clamp(0.0, rig.extent_m[1]);
    let dx = (pos_b - rig.rest_m[0]) * 1000.0;
    let dz = (pos_t - rig.rest_m[1]) * 1000.0;
    let (dy, held) = rig.hook_shift(state.hook_m);
    // The hook parts swing about the point the ropes leave the drum (or the top of the hook
    // parts when the model has no drum), which travels with the trolley.
    let pivot = rig
        .pivot
        .unwrap_or_else(|| Vec3::new(rig.hook.center().x, rig.hook.max.y, rig.hook.center().z))
        + Vec3::new(dx, 0.0, dz);
    let swing =
        Quat::from_rotation_z(state.swing_rad[0]) * Quat::from_rotation_x(-state.swing_rad[1]);
    let warn = state.warning || held;
    // Where the wheel and the drum are in their turn: distance rolled over radius, kept below one
    // turn so a long travel does not lose the angle to float rounding.
    let turn = |a: f32| a.rem_euclid(TAU);
    for (i, p) in plan.placed.iter().enumerate() {
        let g = rig.groups[i];
        let shift = match g {
            Group::Frame => Vec3::ZERO,
            Group::Bridge => Vec3::new(dx, 0.0, 0.0),
            Group::Trolley => Vec3::new(dx, 0.0, dz),
            Group::Hook => Vec3::new(dx, dy, dz),
        };
        let mut centre = p.bounds.center() + shift;
        let mut rot = Quat::IDENTITY;
        match rig.spins[i] {
            Some(Spin::Wheel {
                axle,
                radius_mm,
                trolley,
            }) => {
                // Rolling forward turns a wheel with the top ahead: about -z for the bridge going
                // +x, about +x for the trolley going +z.
                let angle = if trolley { dz } else { -dx } / radius_mm.max(1.0);
                rot = Quat::from_axis_angle(axle.vector(), turn(angle));
            }
            Some(Spin::Drum { axle, radius_mm }) => {
                let angle = -FALLS * state.hook_m * 1000.0 / radius_mm.max(1.0);
                rot = Quat::from_axis_angle(axle.vector(), turn(angle));
            }
            None => {}
        }
        if g == Group::Hook {
            centre = pivot + swing * (centre - pivot);
            rot = swing;
        }
        let s = p.bounds.size();
        let mut item = Item::new(
            ItemId::new(ItemKind::Other, RAW_PART + i as u64),
            Shape::Part(p.key),
            Pose {
                translation: centre - rot * Vec3::new(0.0, 0.0, s.z * 0.5),
                rotation: rot,
            },
            s,
        );
        item.tint = Some(if highlight == Some(i) {
            HIGHLIGHT
        } else if warn && p.role == PartRole::HookBlock {
            BLOCK_WARN
        } else {
            p.role.tint()
        });
        scene.push(item);
    }

    // The top and the tip of the hook parts where they are now, and the ropes between the drum and
    // the top.
    if !rig.hook.is_empty() {
        let at = |y: f32| {
            let c = rig.hook.center();
            pivot + swing * (Vec3::new(c.x + dx, y + dy, c.z + dz) - pivot)
        };
        let (top, tip) = (at(rig.hook.max.y), at(rig.hook.min.y));
        if let Some(p) = rig.pivot {
            let a = p + Vec3::new(dx, 0.0, dz);
            let half = (size::FALL_SPACING * 0.5).min(rig.fall_extent() * 0.4);
            let lateral = rig.fall_axle.vector() * half;
            for (k, s) in [-1.0f32, 1.0].into_iter().enumerate() {
                push_fall(&mut scene, k, a + lateral * s, top + swing * (lateral * s));
            }
        }
        if let Some(l) = load {
            push_load(&mut scene, tip.x, tip.y, tip.z, l);
        }
        if state.slack_m > 0.0 {
            push_slack(&mut scene, state.slack_m, top.x, top.z);
        }
        // A flag at the height of each bound limit switch, beside the hook's path.
        let flag_x = rig.hook.center().x + dx + rig.hook.size().x * 0.5 + 60.0;
        for (k, mark) in state.limits.iter().enumerate() {
            let y = rig.hook.max.y + mark.height_m * 1000.0;
            push_hoist_flag(
                &mut scene,
                k,
                mark.kind,
                Vec3::new(flag_x, y, rig.hook.center().z + dz),
            );
        }
    }
    // Travel end limits stand on the rail beside the runway (bridge) and on the girder (trolley).
    let rail_top = if rig.rails.is_empty() {
        plan.bounds.min.y
    } else {
        rig.rails.max.y
    };
    let girder_top = if rig.girders.is_empty() {
        rig.bridge.max.y.max(rail_top)
    } else {
        rig.girders.max.y
    };
    let x_start = if rig.rails.is_empty() {
        rig.bridge.min.x - rig.rest_m[0] * 1000.0
    } else {
        rig.rails.min.x
    };
    let z_start = if rig.girders.is_empty() {
        rig.bridge.min.z
    } else {
        rig.girders.min.z
    };
    for (k, mark) in state.bridge_limits.iter().enumerate() {
        if !(0.0..=rig.extent_m[0]).contains(&mark.position_m) {
            continue;
        }
        let x = x_start + mark.position_m * 1000.0;
        let z = if rig.rails.is_empty() {
            plan.bounds.min.z
        } else {
            rig.rails.min.z
        } - size::TRAVEL_FLAG[2]
            - 100.0;
        push_travel_flag(&mut scene, true, k, mark.kind, Vec3::new(x, rail_top, z));
    }
    for (k, mark) in state.trolley_limits.iter().enumerate() {
        if !(0.0..=rig.extent_m[1]).contains(&mark.position_m) {
            continue;
        }
        let x = rig.bridge.center().x + dx;
        let z = z_start + mark.position_m * 1000.0;
        push_travel_flag(&mut scene, false, k, mark.kind, Vec3::new(x, girder_top, z));
    }
    // The two state lamps sit on top of the trolley.
    if !rig.trolley.is_empty() {
        let top = rig.trolley.max.y;
        let c = rig.trolley.center() + Vec3::new(dx, 0.0, dz);
        for (k, lamp) in state.lamps.iter().enumerate() {
            scene.push(boxed(
                RAW_LAMP + k as u64,
                ItemKind::Other,
                Vec3::new(c.x - 45.0, top, c.z + (k as f32 - 0.5) * 160.0 - 45.0),
                Vec3::new(90.0, 90.0, 90.0),
                lamp.tint(),
            ));
        }
    }
    scene.revision = 1;
    scene
}

impl Rig {
    /// The length of the drum along its axle: the falls are never further apart than this allows.
    fn fall_extent(&self) -> f32 {
        // Fall spacing is capped by the hook parts' own size across the falls.
        let s = self.hook.size();
        match self.fall_axle {
            Axle::X => s.x,
            Axle::Z => s.z,
        }
        .max(60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crane::{LimitFlag, LimitMark, TravelMark};
    use crate::crane_model::{CraneModel, Orientation, RawPart, UpAxis, plan};
    use crate::mesh::MeshData;

    /// A model of boxes in the picture's own axes (y up): name, role, low corner, high corner, mm.
    /// A small workshop crane: two rails, a girder across them on two end trucks with a wheel each,
    /// a trolley wheel, a drum with a motor beside it, a hook block and a hook, and a support.
    fn crane() -> (CraneModel, Vec<PartRole>) {
        use PartRole::*;
        let parts: &[(&str, PartRole, [f32; 3], [f32; 3])] = &[
            ("rail A", Rail, [0.0, 7000.0, 0.0], [12000.0, 7060.0, 60.0]),
            (
                "rail B",
                Rail,
                [0.0, 7000.0, 7940.0],
                [12000.0, 7060.0, 8000.0],
            ),
            (
                "support A",
                Other,
                [0.0, 6700.0, 0.0],
                [12000.0, 7000.0, 300.0],
            ),
            (
                "girder",
                Girder,
                [2100.0, 7400.0, 0.0],
                [2400.0, 8000.0, 8000.0],
            ),
            (
                "truck A",
                Other,
                [1700.0, 7220.0, 0.0],
                [2800.0, 7400.0, 400.0],
            ),
            (
                "bridge wheel",
                Wheel,
                [1740.0, 7060.0, 0.0],
                [2060.0, 7380.0, 80.0],
            ),
            (
                "trolley wheel",
                Wheel,
                [2150.0, 8000.0, 3500.0],
                [2210.0, 8090.0, 3590.0],
            ),
            (
                "drum",
                Drum,
                [2030.0, 8080.0, 3600.0],
                [2470.0, 8520.0, 4400.0],
            ),
            (
                "motor",
                Motor,
                [2050.0, 8200.0, 4500.0],
                [2450.0, 8500.0, 4900.0],
            ),
            (
                "block",
                HookBlock,
                [2100.0, 6700.0, 3900.0],
                [2400.0, 7100.0, 4100.0],
            ),
            (
                "hook",
                Hook,
                [2200.0, 6300.0, 3950.0],
                [2300.0, 6700.0, 4050.0],
            ),
        ];
        let model = CraneModel::new(
            parts
                .iter()
                .map(|(name, _, lo, hi)| RawPart {
                    name: (*name).into(),
                    mesh: MeshData::from_boxes(&[(Vec3::from_array(*lo), Vec3::from_array(*hi))]),
                })
                .collect(),
        );
        (model, parts.iter().map(|p| p.1).collect())
    }

    fn placed() -> (ModelPlan, Rig) {
        let (m, roles) = crane();
        let p = plan(
            &m,
            Orientation {
                up: UpAxis::Y,
                turns: 0,
            },
            &roles,
        );
        // The model's lowest point is the support's underside, so nothing moved: the picture's
        // coordinates are the file's, less the corner.
        let r = rig(&p);
        (p, r)
    }

    fn index(name: &str) -> usize {
        crane().0.parts.iter().position(|p| p.name == name).unwrap()
    }

    /// Where the middle of a part's box is in the scene.
    fn middle(s: &Scene, i: usize) -> Vec3 {
        let it = s
            .items
            .iter()
            .find(|it| it.id.raw == RAW_PART + i as u64)
            .unwrap();
        it.pose.translation + it.pose.rotation * Vec3::new(0.0, 0.0, it.size.z * 0.5)
    }

    fn rest_state(r: &Rig) -> CraneState {
        CraneState {
            bridge_m: r.rest_m[0],
            trolley_m: r.rest_m[1],
            ..Default::default()
        }
    }

    #[test]
    fn roles_and_touching_decide_what_moves_with_what() {
        let (_, r) = placed();
        let g = |n: &str| r.groups[index(n)];
        assert_eq!(g("rail A"), Group::Frame);
        assert_eq!(g("girder"), Group::Bridge);
        assert_eq!(g("bridge wheel"), Group::Bridge);
        assert_eq!(g("trolley wheel"), Group::Trolley);
        assert_eq!(g("drum"), Group::Trolley);
        assert_eq!(g("block"), Group::Hook);
        assert_eq!(g("hook"), Group::Hook);
        // Touching decides the rest: the support sits on a rail, the truck on the girder, the
        // motor beside the drum.
        assert_eq!(g("support A"), Group::Frame);
        assert_eq!(g("truck A"), Group::Bridge);
        assert_eq!(g("motor"), Group::Trolley);
    }

    #[test]
    fn the_model_with_nothing_to_move_stands_still() {
        let (p, r) = placed();
        let s = scene(&p, &r, &rest_state(&r), None);
        for (i, pl) in p.placed.iter().enumerate() {
            assert!((middle(&s, i) - pl.bounds.center()).length() < 1e-2, "{i}");
        }
    }

    #[test]
    fn travel_runs_between_the_ends_of_the_rails_and_the_girder() {
        let (_, r) = placed();
        // The bridge stands between 1.7 m and 2.8 m of a 12 m rail.
        assert!((r.rest_m[0] - 1.7).abs() < 1e-3, "{:?}", r.rest_m);
        assert!(
            (r.extent_m[0] - (12.0 - 1.1)).abs() < 1e-3,
            "{:?}",
            r.extent_m
        );
        // The trolley (the wheel, the drum and the motor) stands between 3.5 m and 4.9 m of an 8 m girder.
        assert!((r.rest_m[1] - 3.5).abs() < 1e-3);
        assert!((r.extent_m[1] - (8.0 - 1.4)).abs() < 1e-3);
    }

    #[test]
    fn the_bridge_takes_its_parts_along_the_runway_and_leaves_the_rails() {
        let (p, r) = placed();
        let at = |b: f32, t: f32| CraneState {
            bridge_m: b,
            trolley_m: t,
            ..Default::default()
        };
        let rest = scene(&p, &r, &rest_state(&r), None);
        let moved = scene(&p, &r, &at(r.rest_m[0] + 2.0, r.rest_m[1]), None);
        for (name, dx) in [
            ("girder", 2000.0),
            ("truck A", 2000.0),
            ("drum", 2000.0),
            ("block", 2000.0),
            ("rail A", 0.0),
            ("support A", 0.0),
        ] {
            let i = index(name);
            let d = middle(&moved, i) - middle(&rest, i);
            assert!((d - Vec3::new(dx, 0.0, 0.0)).length() < 1e-2, "{name}: {d}");
        }
        // Past the end it holds at the end of the rails.
        let far = scene(&p, &r, &at(1000.0, 0.0), None);
        let i = index("girder");
        let end = middle(&far, i).x + 150.0;
        assert!(
            (end - 12000.0 + 400.0).abs() < 1.0 || end <= 12000.0,
            "{end}"
        );
    }

    #[test]
    fn the_trolley_takes_the_drum_and_the_hook_along_the_girder_and_leaves_the_bridge() {
        let (p, r) = placed();
        let rest = scene(&p, &r, &rest_state(&r), None);
        let mut s = rest_state(&r);
        s.trolley_m += 1.5;
        let moved = scene(&p, &r, &s, None);
        for (name, dz) in [
            ("drum", 1500.0),
            ("trolley wheel", 1500.0),
            ("block", 1500.0),
            ("hook", 1500.0),
            ("girder", 0.0),
            ("bridge wheel", 0.0),
        ] {
            let i = index(name);
            let d = middle(&moved, i) - middle(&rest, i);
            assert!((d - Vec3::new(0.0, 0.0, dz)).length() < 1e-2, "{name}: {d}");
        }
    }

    #[test]
    fn the_hook_parts_rise_with_the_hook_and_stop_under_the_drum_and_above_the_floor() {
        let (p, r) = placed();
        let rest = scene(&p, &r, &rest_state(&r), None);
        let mut s = rest_state(&r);
        s.hook_m = 0.5;
        let up = scene(&p, &r, &s, None);
        for name in ["block", "hook"] {
            let i = index(name);
            let d = middle(&up, i) - middle(&rest, i);
            assert!(
                (d - Vec3::new(0.0, 500.0, 0.0)).length() < 1e-2,
                "{name}: {d}"
            );
        }
        assert!((middle(&up, index("drum")) - middle(&rest, index("drum"))).length() < 1e-2);
        // The rope is as long as the gap under the drum and shortens as the hook rises.
        let l0 = r.rope_length_mm(0.0);
        assert!((l0 - (8080.0 - 7100.0)).abs() < 1e-2, "{l0}");
        assert!((r.rope_length_mm(0.5) - (l0 - 500.0)).abs() < 1e-2);
        // Up to the drum and no further; down to the floor and no further.
        s.hook_m = 50.0;
        let held = scene(&p, &r, &s, None);
        let top = middle(&held, index("block")).y + 200.0;
        // The plan puts the lowest point (the hook tip at 6300) on the floor, so the drum is at 1780.
        assert!((top - 1780.0).abs() < 1e-2, "{top}");
        s.hook_m = -50.0;
        let low = scene(&p, &r, &s, None);
        let tip = middle(&low, index("hook")).y - 200.0;
        assert!(tip.abs() < 1e-2, "{tip}");
        // Held parts turn the block red.
        let block = |sc: &Scene| {
            sc.items
                .iter()
                .find(|it| it.id.raw == RAW_PART + index("block") as u64)
                .unwrap()
                .tint
        };
        assert_eq!(block(&held), Some(BLOCK_WARN));
        assert_eq!(block(&rest), Some(PartRole::HookBlock.tint()));
    }

    #[test]
    fn two_falls_of_rope_join_the_drum_to_the_block() {
        let (p, r) = placed();
        let s = scene(&p, &r, &rest_state(&r), None);
        let ropes: Vec<&Item> = s
            .items
            .iter()
            .filter(|i| i.id.raw == 9 || i.id.raw == 14)
            .collect();
        assert_eq!(ropes.len(), 2);
        // Each is as long as the rope at rest.
        for rope in ropes {
            assert!(
                (rope.size.z - r.rope_length_mm(0.0)).abs() < 1.0,
                "{}",
                rope.size.z
            );
        }
    }

    #[test]
    fn wheels_turn_with_the_distance_they_roll_and_the_drum_with_the_rope() {
        let (p, r) = placed();
        let spun = |s: &Scene, name: &str| {
            s.items
                .iter()
                .find(|it| it.id.raw == RAW_PART + index(name) as u64)
                .unwrap()
                .pose
                .rotation
        };
        let mut st = rest_state(&r);
        st.bridge_m += 1.0;
        st.trolley_m += 0.5;
        st.hook_m = 0.25;
        let s = scene(&p, &r, &st, None);
        // A bridge wheel of 320 mm (radius 160) after 1000 mm: about z by -1000/160 rad.
        let want = Quat::from_axis_angle(Vec3::Z, (-1000.0f32 / 160.0).rem_euclid(TAU));
        assert!(spun(&s, "bridge wheel").angle_between(want) < 1e-3);
        // A trolley wheel of 90 mm: about x by +500/45 rad.
        let want = Quat::from_axis_angle(Vec3::X, (500.0f32 / 45.0).rem_euclid(TAU));
        assert!(spun(&s, "trolley wheel").angle_between(want) < 1e-3);
        // The drum pays out two millimetres per millimetre: radius 300 (440 x 440 -> 220... mean).
        let radius = (440.0 + 440.0) * 0.25;
        let want = Quat::from_axis_angle(Vec3::Z, (-FALLS * 250.0 / radius).rem_euclid(TAU));
        assert!(spun(&s, "drum").angle_between(want) < 1e-3);
        // The frame, the girder and the hook do not turn about their axes.
        assert!(spun(&s, "girder").angle_between(Quat::IDENTITY) < 1e-5);
        assert!(spun(&s, "rail A").angle_between(Quat::IDENTITY) < 1e-5);
    }

    #[test]
    fn a_swing_leans_the_hook_parts_about_the_drum_and_nothing_else() {
        let (p, r) = placed();
        let rest = scene(&p, &r, &rest_state(&r), None);
        let mut s = rest_state(&r);
        s.swing_rad = [0.1, 0.0];
        let swung = scene(&p, &r, &s, None);
        let i = index("block");
        // Leaning towards +x: the block goes that way and a little up.
        let d = middle(&swung, i) - middle(&rest, i);
        assert!(d.x > 50.0 && d.y > 0.0 && d.z.abs() < 1e-2, "{d}");
        let j = index("girder");
        assert!((middle(&swung, j) - middle(&rest, j)).length() < 1e-2);
        s.swing_rad = [0.0, 0.1];
        let across = middle(&scene(&p, &r, &s, None), i) - middle(&rest, i);
        assert!(across.z > 50.0 && across.x.abs() < 1e-2, "{across}");
    }

    #[test]
    fn a_load_flags_and_lamps_follow_the_model() {
        let (p, r) = placed();
        let mut s = rest_state(&r);
        s.limits = vec![LimitMark {
            height_m: 1.0,
            kind: LimitFlag::Upper,
        }];
        s.bridge_limits = vec![TravelMark {
            position_m: 5.0,
            kind: LimitFlag::Lower,
        }];
        s.trolley_limits = vec![TravelMark {
            position_m: 2.0,
            kind: LimitFlag::Upper,
        }];
        s.slack_m = 0.4;
        let with = scene(&p, &r, &s, Some([1.2, 0.8, 0.8]));
        let without = scene(&p, &r, &rest_state(&r), None);
        assert!(
            with.items.len() > without.items.len() + 6,
            "{} {}",
            with.items.len(),
            without.items.len()
        );
        // The load hangs under the hook's tip.
        let load = with.items.iter().find(|i| i.id.raw == 12).unwrap();
        let hook_tip = p.placed[index("hook")].bounds.min.y;
        assert!(with.world_bounds(load).max.y < hook_tip);
        // A travel limit beyond the travel is not drawn.
        s.bridge_limits[0].position_m = 500.0;
        let outside = scene(&p, &r, &s, Some([1.2, 0.8, 0.8]));
        assert_eq!(outside.items.len(), with.items.len() - 1);
    }

    #[test]
    fn a_model_without_drum_or_hook_still_stands_and_has_no_rope() {
        use PartRole::*;
        let model = CraneModel::new(vec![
            RawPart {
                name: "rail".into(),
                mesh: MeshData::from_boxes(&[(Vec3::ZERO, Vec3::new(5000.0, 60.0, 60.0))]),
            },
            RawPart {
                name: "girder".into(),
                mesh: MeshData::from_boxes(&[(
                    Vec3::new(1000.0, 60.0, 0.0),
                    Vec3::new(1300.0, 500.0, 60.0),
                )]),
            },
        ]);
        let p = plan(
            &model,
            Orientation {
                up: UpAxis::Y,
                turns: 0,
            },
            &[Rail, Girder],
        );
        let r = rig(&p);
        assert_eq!(r.rope_length_mm(0.3), 0.0);
        assert!(r.hook_view(0.0).is_none());
        let s = scene(
            &p,
            &r,
            &CraneState {
                bridge_m: 1.0,
                ..Default::default()
            },
            Some([1.0; 3]),
        );
        assert!(s.items.iter().all(|i| i.id.raw != 9 && i.id.raw != 12));
        // The bridge moves 1 m of its 4.7 m of rail (5 m less the girder).
        assert!((r.extent_m[0] - 4.7).abs() < 1e-3, "{:?}", r.extent_m);
        // No trolley and no hook: the trolley has nowhere to go.
        assert_eq!(r.extent_m[1], 0.0);
    }

    #[test]
    fn a_model_with_no_rails_travels_half_its_length_each_way() {
        use PartRole::*;
        let model = CraneModel::new(vec![RawPart {
            name: "girder".into(),
            mesh: MeshData::from_boxes(&[(
                Vec3::new(4000.0, 0.0, 0.0),
                Vec3::new(4400.0, 500.0, 6000.0),
            )]),
        }]);
        let p = plan(
            &model,
            Orientation {
                up: UpAxis::Y,
                turns: 0,
            },
            &[Girder],
        );
        let r = rig(&p);
        // The model is 400 mm long, so the bridge has 200 mm each way.
        assert!(
            r.extent_m[0] > 0.0 && r.extent_m[0] < 1.0,
            "{:?}",
            r.extent_m
        );
    }

    #[test]
    fn the_hook_view_frames_the_hook_parts_and_the_drum() {
        let (_, r) = placed();
        let v = r.hook_view(0.0).unwrap();
        assert!(v.min.y < 0.0 && v.max.y > 1780.0);
        assert!(v.size().x < 12000.0);
    }
}
