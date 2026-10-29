//! A single-girder overhead crane as a scene, driven by plain numbers (lane L4, hoist half).
//!
//! The 3D view of the cabinet stays as it is; this builds a separate scene of the machine: floor,
//! two runway beams, the girder on its end trucks, the trolley with its drum, the rope, the hook
//! block, an optional load and a flag at the height of every bound limit switch. A caller feeds it
//! the bridge and trolley position and the hook height from the simulation, and gets a scene to
//! hand to the ordinary [`crate::widget::View3d`]. Pure data, no model dependency.
//!
//! What it is: a picture of the numbers the hoist loop computed, in the proportions of a generic
//! crane. What it is not: a structural model, a clearance check against a building, or a rope
//! model (the rope has a fixed length: the swing is a simple pendulum model, no stretch, no slack).
//!
//! World axes follow the scene contract: x along the runway (bridge travel), y up, z across the
//! span (trolley travel). Millimetres in the scene, metres in the inputs.

use crate::scene::{Item, ItemId, ItemKind, PlateFrame, Pose, Scene, Shape};
use glam::{Quat, Vec3};
use std::sync::Arc;

/// What the picture leaves out, shown next to it in the window.
pub const CRANE_PICTURE_LEAVES_OUT: &[&str] = &[
    "Generic crane proportions: the girder, trolley and hook block are not your crane's sizes",
    "The rope is a straight bar of fixed length: its swing is a simple pendulum model, not a measurement; stretch and slack rope are not modelled",
    "Bridge and trolley follow a travel drive if you pick one (forward is +x for the bridge, +z for the trolley) and its end limits stand as upright flags, red forward and amber reverse; otherwise they stand where you set them",
    "The swing is one pendulum per direction on a rope that keeps its length: no turning load, no push back on the drive",
    "No building, no clearance check, no structural model",
];

/// Size of the generic crane, in metres. Edited by the user; the defaults are a small workshop crane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CraneLayout {
    /// Length of the runway (bridge travel), m.
    pub runway_m: f32,
    /// Distance between the runway beams (the girder length), m.
    pub span_m: f32,
    /// Height of the runway beam tops above the floor, m.
    pub rail_m: f32,
    /// Height of the hook block's top above the floor at the start position (hook height 0), m.
    pub hook_start_m: f32,
    /// A load under the hook (width, height, depth), m; `None` for a bare hook.
    pub load_m: Option<[f32; 3]>,
}

impl Default for CraneLayout {
    fn default() -> Self {
        Self {
            runway_m: 14.0,
            span_m: 10.0,
            rail_m: 8.0,
            hook_start_m: 3.0,
            load_m: Some([1.2, 0.8, 0.8]),
        }
    }
}

/// Which way a limit switch cuts, for its flag colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitFlag {
    Upper,
    Lower,
}

/// One limit switch of the hoist: its height above the hook start, m.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LimitMark {
    pub height_m: f32,
    pub kind: LimitFlag,
}

/// One end limit of a travel axis: where along the runway (bridge) or across the span (trolley) it
/// sits, m. `Upper` is the forward end, `Lower` the reverse end, as for the hoist.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TravelMark {
    pub position_m: f32,
    pub kind: LimitFlag,
}

/// The moving numbers of one frame.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CraneState {
    /// Bridge position along the runway, m.
    pub bridge_m: f32,
    /// Trolley position across the span, m.
    pub trolley_m: f32,
    /// Hook height above its start position, m (negative is down).
    pub hook_m: f32,
    /// Rope angle from the vertical along the runway (x) and across the span (z), radians; 0 is
    /// a hook hanging straight down. Positive leans towards +x and +z.
    pub swing_rad: [f32; 2],
    /// The hoist reports a condition (a limit stopped it, the brake slipped, ...): the block turns red.
    pub warning: bool,
    pub limits: Vec<LimitMark>,
    /// End limits of the bridge's travel axis, along the runway. A limit outside the runway is not drawn.
    pub bridge_limits: Vec<TravelMark>,
    /// End limits of the trolley's travel axis, across the span. A limit outside the span is not drawn.
    pub trolley_limits: Vec<TravelMark>,
}

/// Sizes of the generic crane parts, mm. One place so the tests and the builder agree.
pub mod size {
    pub const BEAM: f32 = 400.0;
    pub const GIRDER_DEPTH: f32 = 800.0;
    pub const GIRDER_WIDTH: f32 = 300.0;
    pub const TRUCK_LEN: f32 = 900.0;
    pub const TRUCK_HEIGHT: f32 = 300.0;
    pub const TROLLEY: [f32; 3] = [1400.0, 500.0, 1100.0];
    pub const DRUM: [f32; 3] = [500.0, 500.0, 900.0];
    pub const ROPE: f32 = 24.0;
    pub const BLOCK: [f32; 3] = [380.0, 520.0, 260.0];
    pub const HOOK: [f32; 3] = [90.0, 260.0, 90.0];
    pub const FLAG: [f32; 3] = [260.0, 40.0, 40.0];
    /// An upright flag of a travel end limit: along the travel, up, across.
    pub const TRAVEL_FLAG: [f32; 3] = [400.0, 1400.0, 60.0];
    /// The rope hangs beside the girder: this far from its centre line.
    pub const ROPE_OFFSET: f32 = 650.0;
    /// Gap between the hook tip and the top of the load (the sling).
    pub const SLING: f32 = 500.0;
}

const RAW_FLOOR: u64 = 1;
const RAW_BEAM: u64 = 2;
const RAW_GIRDER: u64 = 4;
const RAW_TRUCK: u64 = 5;
const RAW_TROLLEY: u64 = 7;
const RAW_DRUM: u64 = 8;
const RAW_ROPE: u64 = 9;
const RAW_BLOCK: u64 = 10;
const RAW_HOOK: u64 = 11;
const RAW_LOAD: u64 = 12;
const RAW_SLING: u64 = 13;
const RAW_FLAG: u64 = 20;
const RAW_BRIDGE_FLAG: u64 = 1000;
const RAW_TROLLEY_FLAG: u64 = 2000;

/// Length of the rope from the drum's underside to the top of the block, mm.
pub fn rope_length_mm(layout: &CraneLayout, hook_m: f32) -> f32 {
    let (top, _) = block_top_mm(layout, hook_m);
    (drum_underside_mm(layout) - top).max(0.0)
}

/// Offsets of the block from straight under the drum for a swing, mm: `(dx, drop, dz)` where
/// `drop` is how far the block top hangs below the drum. The rope keeps its length.
pub fn swung_offset_mm(rope_mm: f32, swing_rad: [f32; 2]) -> (f32, f32, f32) {
    let dx = rope_mm * swing_rad[0].sin();
    let dz = rope_mm * swing_rad[1].sin();
    // Keep the horizontal reach inside the rope, so a wild angle cannot lift the block above the drum.
    let reach = (dx * dx + dz * dz).sqrt();
    let limit = rope_mm * 0.99;
    let k = if reach > limit && reach > 0.0 {
        limit / reach
    } else {
        1.0
    };
    let (dx, dz) = (dx * k, dz * k);
    let drop = (rope_mm * rope_mm - dx * dx - dz * dz).max(0.0).sqrt();
    (dx, drop, dz)
}

/// Where the hook block's top is in the world, mm, and whether the drum stops it there.
///
/// The block cannot rise above the underside of the drum: a hook height that asks for more is
/// clamped and reported, so a limit switch bound too high shows as a hook pressed against the drum.
pub fn block_top_mm(layout: &CraneLayout, hook_m: f32) -> (f32, bool) {
    let wanted = (layout.hook_start_m + hook_m) * 1000.0;
    let floor = size::BLOCK[1];
    // A rail lower than the block is tall leaves no room: the ceiling never drops below the floor.
    let ceiling = drum_underside_mm(layout).max(floor);
    let top = wanted.clamp(floor, ceiling);
    (top, (wanted - top).abs() > 0.5)
}

/// Height of the drum's underside, mm: the highest the block's top can be.
pub fn drum_underside_mm(layout: &CraneLayout) -> f32 {
    girder_bottom_mm(layout) + size::GIRDER_DEPTH * 0.25 - size::DRUM[1] * 0.5
}

fn girder_bottom_mm(layout: &CraneLayout) -> f32 {
    layout.rail_m * 1000.0 + size::TRUCK_HEIGHT
}

/// An axis-aligned box from its lowest corner and its extents (x, y up, z), mm.
fn boxed(raw: u64, kind: ItemKind, min: Vec3, extent: Vec3, tint: [u8; 3]) -> Item {
    // The scene's local box is centred in x and y and runs from 0 to size.z along local z; turning
    // local z to world y makes local y the world's negative z.
    let pose = Pose {
        translation: Vec3::new(min.x + extent.x * 0.5, min.y, min.z + extent.z * 0.5),
        rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
    };
    let mut item = Item::new(
        ItemId::new(kind, raw),
        Shape::Box,
        pose,
        Vec3::new(extent.x, extent.z, extent.y),
    );
    item.tint = Some(tint);
    item
}

/// A bar of square section `thick` from `a` to `b`, mm.
fn bar(raw: u64, a: Vec3, b: Vec3, thick: f32, tint: [u8; 3]) -> Item {
    let d = b - a;
    let len = d.length().max(1.0);
    let mut item = Item::new(
        ItemId::new(ItemKind::Other, raw),
        Shape::Box,
        Pose {
            translation: a,
            rotation: Quat::from_rotation_arc(Vec3::Z, d / len),
        },
        Vec3::new(thick, thick, len),
    );
    item.tint = Some(tint);
    item
}

const STEEL: [u8; 3] = [120, 130, 145];
const GIRDER: [u8; 3] = [60, 110, 170];
const YELLOW: [u8; 3] = [235, 190, 40];
const BLOCK_OK: [u8; 3] = [235, 150, 40];
const BLOCK_WARN: [u8; 3] = [220, 60, 50];
const FLOOR: [u8; 3] = [70, 72, 78];
const LOAD: [u8; 3] = [150, 120, 90];
const UPPER_FLAG: [u8; 3] = [220, 60, 50];
const LOWER_FLAG: [u8; 3] = [230, 180, 50];

/// The scene for one frame.
pub fn build(layout: &CraneLayout, state: &CraneState) -> Scene {
    let l = layout;
    let runway = l.runway_m * 1000.0;
    let span = l.span_m * 1000.0;
    let rail = l.rail_m * 1000.0;
    let bridge = state.bridge_m.clamp(0.0, l.runway_m) * 1000.0;
    let trolley = state.trolley_m.clamp(0.0, l.span_m) * 1000.0;
    let mut scene = Scene::new(PlateFrame {
        width: runway,
        height: rail,
    });
    let mut push = |item: Item| scene.push(item);

    // Floor, a little larger than the runway.
    push(boxed(
        RAW_FLOOR,
        ItemKind::Plate,
        Vec3::new(-1500.0, -60.0, -1500.0),
        Vec3::new(runway + 3000.0, 60.0, span + 3000.0),
        FLOOR,
    ));
    // Runway beams along x at both ends of the span.
    for (k, z) in [0.0, span].into_iter().enumerate() {
        push(boxed(
            RAW_BEAM + k as u64 * 100,
            ItemKind::Other,
            Vec3::new(0.0, rail - size::BEAM, z - size::BEAM * 0.5),
            Vec3::new(runway, size::BEAM, size::BEAM),
            STEEL,
        ));
    }
    // End trucks and the girder across the span.
    let girder_bottom = girder_bottom_mm(l);
    for (k, z) in [0.0, span].into_iter().enumerate() {
        push(boxed(
            RAW_TRUCK + k as u64 * 100,
            ItemKind::Other,
            Vec3::new(bridge - size::TRUCK_LEN * 0.5, rail, z - size::BEAM * 0.5),
            Vec3::new(size::TRUCK_LEN, size::TRUCK_HEIGHT, size::BEAM),
            YELLOW,
        ));
    }
    push(boxed(
        RAW_GIRDER,
        ItemKind::Other,
        Vec3::new(bridge - size::GIRDER_WIDTH * 0.5, girder_bottom, 0.0),
        Vec3::new(size::GIRDER_WIDTH, size::GIRDER_DEPTH, span),
        GIRDER,
    ));
    // Trolley on top of the girder and the drum on its side, where the rope leaves.
    let girder_top = girder_bottom + size::GIRDER_DEPTH;
    push(boxed(
        RAW_TROLLEY,
        ItemKind::Other,
        Vec3::new(
            bridge - size::TROLLEY[0] * 0.5,
            girder_top,
            trolley - size::TROLLEY[2] * 0.5,
        ),
        Vec3::new(size::TROLLEY[0], size::TROLLEY[1], size::TROLLEY[2]),
        YELLOW,
    ));
    let rope_x = bridge + size::ROPE_OFFSET;
    let drum_bottom = drum_underside_mm(l);
    push(boxed(
        RAW_DRUM,
        ItemKind::Other,
        Vec3::new(
            rope_x - size::DRUM[0] * 0.5,
            drum_bottom,
            trolley - size::DRUM[2] * 0.5,
        ),
        Vec3::new(size::DRUM[0], size::DRUM[1], size::DRUM[2]),
        STEEL,
    ));

    // Hook block, hook, load and rope.
    let (still_top, clamped) = block_top_mm(l, state.hook_m);
    let warn = state.warning || clamped;
    let block_tint = if warn { BLOCK_WARN } else { BLOCK_OK };
    // The block hangs from the drum on a rope of constant length and may swing out of plumb.
    let rope_mm = (drum_bottom - still_top).max(0.0);
    let (dx, drop, dz) = swung_offset_mm(rope_mm, state.swing_rad);
    let top = drum_bottom - drop;
    let (hook_x, hook_z) = (rope_x + dx, trolley + dz);
    let block_min = Vec3::new(
        hook_x - size::BLOCK[0] * 0.5,
        top - size::BLOCK[1],
        hook_z - size::BLOCK[2] * 0.5,
    );
    push(bar(
        RAW_ROPE,
        Vec3::new(rope_x, drum_bottom, trolley),
        Vec3::new(hook_x, top, hook_z),
        size::ROPE,
        STEEL,
    ));
    push(boxed(
        RAW_BLOCK,
        ItemKind::Other,
        block_min,
        Vec3::new(size::BLOCK[0], size::BLOCK[1], size::BLOCK[2]),
        block_tint,
    ));
    let hook_bottom = block_min.y - size::HOOK[1];
    push(boxed(
        RAW_HOOK,
        ItemKind::Other,
        Vec3::new(
            hook_x - size::HOOK[0] * 0.5,
            hook_bottom,
            hook_z - size::HOOK[2] * 0.5,
        ),
        Vec3::new(size::HOOK[0], size::HOOK[1], size::HOOK[2]),
        STEEL,
    ));
    if let Some([w, h, d]) = l.load_m {
        let (w, h, d) = (w * 1000.0, h * 1000.0, d * 1000.0);
        let load_top = hook_bottom - size::SLING;
        push(bar(
            RAW_SLING,
            Vec3::new(hook_x, hook_bottom, hook_z),
            Vec3::new(hook_x, load_top, hook_z),
            size::ROPE * 0.6,
            STEEL,
        ));
        push(boxed(
            RAW_LOAD,
            ItemKind::Other,
            Vec3::new(hook_x - w * 0.5, load_top - h, hook_z - d * 0.5),
            Vec3::new(w, h, d),
            LOAD,
        ));
    }

    // A flag at the height of each bound limit switch, on the far side of the rope's path.
    for (k, mark) in state.limits.iter().enumerate() {
        let y = (l.hook_start_m + mark.height_m) * 1000.0;
        let tint = match mark.kind {
            LimitFlag::Upper => UPPER_FLAG,
            LimitFlag::Lower => LOWER_FLAG,
        };
        push(boxed(
            RAW_FLAG + k as u64,
            ItemKind::Other,
            Vec3::new(
                rope_x + size::BLOCK[0] * 0.5 + 60.0,
                y - size::FLAG[1] * 0.5,
                trolley - size::FLAG[2] * 0.5,
            ),
            Vec3::new(size::FLAG[0], size::FLAG[1], size::FLAG[2]),
            tint,
        ));
    }
    // Travel end limits stand on the runway beam (bridge) and on the girder (trolley), upright so
    // they are not taken for the horizontal hoist flags.
    let tint_of = |kind: LimitFlag| match kind {
        LimitFlag::Upper => UPPER_FLAG,
        LimitFlag::Lower => LOWER_FLAG,
    };
    for (k, mark) in state.bridge_limits.iter().enumerate() {
        if !(0.0..=l.runway_m).contains(&mark.position_m) {
            continue;
        }
        push(boxed(
            RAW_BRIDGE_FLAG + k as u64,
            ItemKind::Other,
            Vec3::new(
                mark.position_m * 1000.0 - size::TRAVEL_FLAG[0] * 0.5,
                rail,
                -size::TRAVEL_FLAG[2] * 0.5,
            ),
            Vec3::new(
                size::TRAVEL_FLAG[0],
                size::TRAVEL_FLAG[1],
                size::TRAVEL_FLAG[2],
            ),
            tint_of(mark.kind),
        ));
    }
    for (k, mark) in state.trolley_limits.iter().enumerate() {
        if !(0.0..=l.span_m).contains(&mark.position_m) {
            continue;
        }
        push(boxed(
            RAW_TROLLEY_FLAG + k as u64,
            ItemKind::Other,
            Vec3::new(
                bridge - size::TRAVEL_FLAG[2] * 0.5,
                girder_top,
                mark.position_m * 1000.0 - size::TRAVEL_FLAG[0] * 0.5,
            ),
            Vec3::new(
                size::TRAVEL_FLAG[2],
                size::TRAVEL_FLAG[1],
                size::TRAVEL_FLAG[0],
            ),
            tint_of(mark.kind),
        ));
    }
    scene.revision = 1;
    scene
}

/// The box around the drum, the rope, the hook block and the load: what a camera frames to look at
/// the hook rather than at the whole crane.
pub fn hook_bounds(layout: &CraneLayout, state: &CraneState) -> crate::math::Aabb {
    let (top, _) = block_top_mm(layout, state.hook_m);
    let rope_x = state.bridge_m.clamp(0.0, layout.runway_m) * 1000.0 + size::ROPE_OFFSET;
    let z = state.trolley_m.clamp(0.0, layout.span_m) * 1000.0;
    let bottom = top
        - size::BLOCK[1]
        - size::HOOK[1]
        - layout
            .load_m
            .map_or(0.0, |[_, h, _]| size::SLING + h * 1000.0);
    let pad = 1500.0 + rope_length_mm(layout, state.hook_m) * 0.3;
    crate::math::Aabb::new(
        Vec3::new(rope_x - pad, bottom.min(top) - 300.0, z - pad),
        Vec3::new(
            rope_x + pad,
            drum_underside_mm(layout) + size::DRUM[1],
            z + pad,
        ),
    )
}

/// The scene as the shared handle the widget takes.
pub fn shared(layout: &CraneLayout, state: &CraneState) -> Arc<Scene> {
    Arc::new(build(layout, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(scene: &Scene, raw: u64) -> &Item {
        scene
            .items
            .iter()
            .find(|i| i.id.raw == raw)
            .unwrap_or_else(|| panic!("no item {raw}"))
    }

    #[test]
    fn a_box_item_covers_the_world_box_it_was_made_from() {
        let it = boxed(
            1,
            ItemKind::Other,
            Vec3::new(100.0, 200.0, 300.0),
            Vec3::new(10.0, 20.0, 30.0),
            [1, 2, 3],
        );
        let s = Scene::new(PlateFrame {
            width: 1.0,
            height: 1.0,
        });
        let b = s.world_bounds(&it);
        for (got, want) in [
            (b.min, Vec3::new(100.0, 200.0, 300.0)),
            (b.max, Vec3::new(110.0, 220.0, 330.0)),
        ] {
            assert!((got - want).length() < 1e-3, "{got} vs {want}");
        }
    }

    #[test]
    fn the_hook_block_rises_one_millimetre_per_millimetre_of_hook_height() {
        let l = CraneLayout::default();
        let at = |h: f32| {
            let s = build(
                &l,
                &CraneState {
                    hook_m: h,
                    ..Default::default()
                },
            );
            let b = s.world_bounds(item(&s, RAW_BLOCK));
            b.max.y
        };
        assert!((at(0.0) - l.hook_start_m * 1000.0).abs() < 1e-2);
        assert!((at(1.5) - at(0.0) - 1500.0).abs() < 1e-2);
        assert!((at(0.0) - at(-1.0) - 1000.0).abs() < 1e-2);
    }

    #[test]
    fn the_rope_runs_from_the_drum_to_the_top_of_the_block() {
        let l = CraneLayout::default();
        let s = build(
            &l,
            &CraneState {
                hook_m: 1.0,
                ..Default::default()
            },
        );
        let rope = s.world_bounds(item(&s, RAW_ROPE));
        let block = s.world_bounds(item(&s, RAW_BLOCK));
        assert!((rope.min.y - block.max.y).abs() < 1.0);
        assert!((rope.max.y - drum_underside_mm(&l)).abs() < 1.0);
    }

    #[test]
    fn a_hook_height_above_the_drum_is_held_there_and_reported() {
        let l = CraneLayout::default();
        let (top, clamped) = block_top_mm(&l, 100.0);
        assert!(clamped);
        assert!((top - drum_underside_mm(&l)).abs() < 1e-3);
        let (_, clamped) = block_top_mm(&l, 0.5);
        assert!(!clamped);
        // And below the floor: the block rests on it.
        let (top, clamped) = block_top_mm(&l, -100.0);
        assert!(clamped);
        assert!((top - size::BLOCK[1]).abs() < 1e-3);
    }

    #[test]
    fn a_clamped_hook_turns_the_block_red_like_a_warning() {
        let l = CraneLayout::default();
        let tint = |hook_m: f32, warning: bool| {
            let s = build(
                &l,
                &CraneState {
                    hook_m,
                    warning,
                    ..Default::default()
                },
            );
            item(&s, RAW_BLOCK).tint
        };
        assert_eq!(tint(0.0, false), Some(BLOCK_OK));
        assert_eq!(tint(0.0, true), Some(BLOCK_WARN));
        assert_eq!(tint(100.0, false), Some(BLOCK_WARN));
    }

    #[test]
    fn bridge_and_trolley_move_the_girder_and_the_trolley() {
        let l = CraneLayout::default();
        let s = build(
            &l,
            &CraneState {
                bridge_m: 6.0,
                trolley_m: 2.5,
                ..Default::default()
            },
        );
        let girder = s.world_bounds(item(&s, RAW_GIRDER));
        assert!(((girder.min.x + girder.max.x) * 0.5 - 6000.0).abs() < 1e-2);
        assert!((girder.max.z - girder.min.z - l.span_m * 1000.0).abs() < 1e-2);
        let trolley = s.world_bounds(item(&s, RAW_TROLLEY));
        assert!(((trolley.min.z + trolley.max.z) * 0.5 - 2500.0).abs() < 1e-2);
        // The rope follows the trolley.
        let block = s.world_bounds(item(&s, RAW_BLOCK));
        assert!(((block.min.z + block.max.z) * 0.5 - 2500.0).abs() < 1e-2);
    }

    #[test]
    fn positions_outside_the_runway_are_held_at_its_ends() {
        let l = CraneLayout::default();
        let s = build(
            &l,
            &CraneState {
                bridge_m: -4.0,
                trolley_m: 99.0,
                ..Default::default()
            },
        );
        let girder = s.world_bounds(item(&s, RAW_GIRDER));
        assert!(((girder.min.x + girder.max.x) * 0.5).abs() < 1e-2);
        let trolley = s.world_bounds(item(&s, RAW_TROLLEY));
        assert!(((trolley.min.z + trolley.max.z) * 0.5 - l.span_m * 1000.0).abs() < 1e-2);
    }

    #[test]
    fn travel_limits_stand_on_the_beam_and_on_the_girder_and_outside_ones_are_skipped() {
        let l = CraneLayout::default();
        let s = build(
            &l,
            &CraneState {
                bridge_m: 4.0,
                bridge_limits: vec![
                    TravelMark {
                        position_m: 9.0,
                        kind: LimitFlag::Upper,
                    },
                    TravelMark {
                        position_m: l.runway_m + 1.0,
                        kind: LimitFlag::Lower,
                    },
                ],
                trolley_limits: vec![TravelMark {
                    position_m: 2.5,
                    kind: LimitFlag::Lower,
                }],
                ..Default::default()
            },
        );
        let b = s.world_bounds(item(&s, RAW_BRIDGE_FLAG));
        assert!(((b.min.x + b.max.x) * 0.5 - 9000.0).abs() < 1e-2);
        assert!((b.min.y - l.rail_m * 1000.0).abs() < 1e-2);
        assert_eq!(item(&s, RAW_BRIDGE_FLAG).tint, Some(UPPER_FLAG));
        assert!(
            s.items.iter().all(|i| i.id.raw != RAW_BRIDGE_FLAG + 1),
            "a limit beyond the runway is not drawn"
        );
        let t = s.world_bounds(item(&s, RAW_TROLLEY_FLAG));
        assert!(((t.min.z + t.max.z) * 0.5 - 2500.0).abs() < 1e-2);
        assert!(((t.min.x + t.max.x) * 0.5 - 4000.0).abs() < 1e-2);
        assert_eq!(item(&s, RAW_TROLLEY_FLAG).tint, Some(LOWER_FLAG));
    }

    #[test]
    fn every_limit_gets_a_flag_at_its_height() {
        let l = CraneLayout::default();
        let s = build(
            &l,
            &CraneState {
                limits: vec![
                    LimitMark {
                        height_m: 2.0,
                        kind: LimitFlag::Upper,
                    },
                    LimitMark {
                        height_m: -1.0,
                        kind: LimitFlag::Lower,
                    },
                ],
                ..Default::default()
            },
        );
        let up = s.world_bounds(item(&s, RAW_FLAG));
        let down = s.world_bounds(item(&s, RAW_FLAG + 1));
        assert!(((up.min.y + up.max.y) * 0.5 - (l.hook_start_m + 2.0) * 1000.0).abs() < 1e-2);
        assert!(((down.min.y + down.max.y) * 0.5 - (l.hook_start_m - 1.0) * 1000.0).abs() < 1e-2);
        assert_eq!(item(&s, RAW_FLAG).tint, Some(UPPER_FLAG));
        assert_eq!(item(&s, RAW_FLAG + 1).tint, Some(LOWER_FLAG));
    }

    #[test]
    fn the_load_hangs_below_the_hook_and_goes_when_switched_off() {
        let mut l = CraneLayout::default();
        let s = build(&l, &CraneState::default());
        let hook = s.world_bounds(item(&s, RAW_HOOK));
        let load = s.world_bounds(item(&s, RAW_LOAD));
        assert!(load.max.y < hook.min.y);
        l.load_m = None;
        let s = build(&l, &CraneState::default());
        assert!(s.items.iter().all(|i| i.id.raw != RAW_LOAD));
    }

    #[test]
    fn the_hook_box_holds_the_block_the_load_and_the_drum() {
        let l = CraneLayout::default();
        let st = CraneState {
            hook_m: 1.0,
            ..Default::default()
        };
        let s = build(&l, &st);
        let hb = hook_bounds(&l, &st);
        for raw in [RAW_BLOCK, RAW_LOAD, RAW_DRUM, RAW_HOOK] {
            let b = s.world_bounds(item(&s, raw));
            assert!(
                hb.min.y <= b.min.y && hb.max.y >= b.max.y,
                "item {raw} outside the hook box"
            );
        }
        // It is far smaller than the crane.
        let all = s.bounds();
        assert!(hb.max.x - hb.min.x < (all.max.x - all.min.x) * 0.5);
    }

    #[test]
    fn a_swing_moves_the_block_along_the_travel_direction_and_keeps_the_rope_length() {
        let l = CraneLayout::default();
        let rope = rope_length_mm(&l, 0.0);
        let at = |swing: [f32; 2]| {
            let s = build(
                &l,
                &CraneState {
                    swing_rad: swing,
                    ..Default::default()
                },
            );
            let b = s.world_bounds(item(&s, RAW_BLOCK));
            (
                (b.min.x + b.max.x) * 0.5,
                b.max.y,
                (b.min.z + b.max.z) * 0.5,
            )
        };
        let (x0, y0, z0) = at([0.0, 0.0]);
        let (x1, y1, z1) = at([0.2, 0.0]);
        let (x2, _, z2) = at([0.0, -0.2]);
        assert!((x1 - x0 - rope * 0.2_f32.sin()).abs() < 1.0);
        assert!((z1 - z0).abs() < 1e-2);
        assert!(y1 > y0, "a swung block hangs higher");
        assert!((x2 - x0).abs() < 1e-2);
        assert!((z2 - z0 + rope * 0.2_f32.sin()).abs() < 1.0);
        // The distance from the drum to the block top is the rope length at any angle.
        let drum = drum_underside_mm(&l);
        let reach = ((x1 - x0).powi(2) + (drum - y1).powi(2)).sqrt();
        assert!((reach - rope).abs() < 1.0, "{reach} vs {rope}");
    }

    #[test]
    fn a_wild_swing_never_lifts_the_block_above_the_drum() {
        let (dx, drop, dz) = swung_offset_mm(2000.0, [1.5, 1.5]);
        assert!(drop > 0.0 && drop.is_finite());
        assert!((dx * dx + drop * drop + dz * dz).sqrt() <= 2001.0);
        let (_, drop, _) = swung_offset_mm(0.0, [0.5, 0.5]);
        assert_eq!(drop, 0.0);
    }

    #[test]
    fn a_very_low_rail_does_not_panic() {
        let l = CraneLayout {
            rail_m: 0.1,
            ..Default::default()
        };
        let (top, _) = block_top_mm(&l, 1.0);
        assert!(top.is_finite());
        let _ = build(&l, &CraneState::default());
    }

    #[test]
    fn the_whole_crane_has_bounds_and_unique_ids() {
        let s = build(&CraneLayout::default(), &CraneState::default());
        assert!(!s.bounds().is_empty());
        let mut ids: Vec<_> = s.items.iter().map(|i| (i.id.kind, i.id.raw)).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n);
    }
}
