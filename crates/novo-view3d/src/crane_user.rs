//! Pieces of the user's own crane model inside the generic crane picture.
//!
//! A model file often holds only a hoist unit: a drum with its motor and gearbox, a hook block and
//! a hook, and no rails or girders. Its hook and hoist solids then take the place of the generic
//! ones, and the generic crane (runway, bridge, trolley, rope) stays around them. The hook
//! solids hang from the rope's end at the size the file gives, the hoist solids sit where the
//! generic drum sits. Which solid belongs to which is the motion rig's grouping, so a changed role
//! changes it. Pure data, like [`crate::crane_model`].

use crate::crane_model::{ModelPlan, PartRole};
use crate::crane_motion::{Group, Rig};
use crate::mesh::GeometryKey;
use crate::scene::{Item, ItemId, ItemKind, Pose, Shape};
use glam::{Quat, Vec3};

/// One solid of a piece: its mesh, the size of its box and where its box stands relative to the
/// piece's anchor (the box is centred in x and y, z up from 0 in the plan's own axes).
#[derive(Debug, Clone, PartialEq)]
pub struct UserPart {
    pub key: GeometryKey,
    pub role: PartRole,
    pub size: Vec3,
    pub offset: Vec3,
}

/// The solids that replace the generic hook and hoist. Either may be empty (the generic one stays).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserCrane {
    /// Anchored at the top middle of the hook solids' box: that point hangs from the rope's end.
    pub hook: Vec<UserPart>,
    /// How tall the hook solids are, mm.
    pub hook_height: f32,
    /// Anchored at the middle of the hoist solids' box: that point sits at the generic drum's.
    pub hoist: Vec<UserPart>,
}

/// Whether a model is a hoist unit only: it has hook or drum solids and no rails or girders.
pub fn is_hoist_unit(plan: &ModelPlan) -> bool {
    let has = |f: &dyn Fn(PartRole) -> bool| plan.placed.iter().any(|p| f(p.role));
    !has(&|r| matches!(r, PartRole::Rail | PartRole::Girder))
        && has(&|r| matches!(r, PartRole::Hook | PartRole::HookBlock | PartRole::Drum))
}

/// The pieces of a hoist-unit model: the hook group's solids and the trolley group's (the drum, the
/// motor, the gearbox and what touches them), wheels left out.
pub fn from_plan(plan: &ModelPlan, rig: &Rig) -> UserCrane {
    let mut hook = crate::math::Aabb::EMPTY;
    let mut hoist = crate::math::Aabb::EMPTY;
    for (p, g) in plan.placed.iter().zip(&rig.groups) {
        match g {
            Group::Hook => hook = hook.union(&p.bounds),
            Group::Trolley if p.role != PartRole::Wheel => hoist = hoist.union(&p.bounds),
            _ => {}
        }
    }
    let part = |p: &crate::crane_model::Placed, anchor: Vec3| UserPart {
        key: p.key,
        role: p.role,
        size: p.bounds.size(),
        offset: Vec3::new(p.bounds.center().x, p.bounds.center().y, p.bounds.min.z) - anchor,
    };
    let mut out = UserCrane::default();
    if !hook.is_empty() {
        let anchor = Vec3::new(hook.center().x, hook.max.y, hook.center().z);
        out.hook_height = hook.size().y;
        for (p, g) in plan.placed.iter().zip(&rig.groups) {
            if *g == Group::Hook {
                out.hook.push(part(p, anchor));
            }
        }
    }
    if !hoist.is_empty() {
        let anchor = hoist.center();
        for (p, g) in plan.placed.iter().zip(&rig.groups) {
            if *g == Group::Trolley && p.role != PartRole::Wheel {
                out.hoist.push(part(p, anchor));
            }
        }
    }
    out
}

/// The scene item of `part` with the piece's anchor at `anchor` (world: x, y up, z).
pub fn item(raw: u64, part: &UserPart, anchor: Vec3, tint: [u8; 3]) -> Item {
    let mut it = Item::new(
        ItemId::new(ItemKind::Other, raw),
        Shape::Part(part.key),
        Pose {
            translation: anchor + part.offset,
            rotation: Quat::IDENTITY,
        },
        part.size,
    );
    it.tint = Some(tint);
    it
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crane_model::{CraneModel, Orientation, RawPart, UpAxis, plan};
    use crate::crane_motion::rig;
    use crate::mesh::MeshData;

    fn unit() -> (ModelPlan, Rig) {
        let boxes: &[(&str, [f32; 3], [f32; 3])] = &[
            ("drum", [0.0, 1000.0, 0.0], [800.0, 1400.0, 400.0]),
            ("motor", [800.0, 1000.0, 0.0], [1200.0, 1300.0, 300.0]),
            ("block", [300.0, 0.0, 100.0], [500.0, 300.0, 300.0]),
            ("hook", [350.0, -400.0, 150.0], [450.0, 0.0, 250.0]),
        ];
        let model = CraneModel::new(
            boxes
                .iter()
                .map(|(n, lo, hi)| RawPart {
                    name: (*n).into(),
                    mesh: MeshData::from_boxes(&[(Vec3::from_array(*lo), Vec3::from_array(*hi))]),
                })
                .collect(),
        );
        let p = plan(
            &model,
            Orientation {
                up: UpAxis::Y,
                turns: 0,
            },
            &[
                PartRole::Drum,
                PartRole::Motor,
                PartRole::HookBlock,
                PartRole::Hook,
            ],
        );
        let r = rig(&p);
        (p, r)
    }

    #[test]
    fn a_hoist_unit_without_rails_or_girders_is_recognised() {
        let (p, _) = unit();
        assert!(is_hoist_unit(&p));
    }

    #[test]
    fn the_hook_solids_hang_from_their_top_and_the_hoist_solids_centre_on_their_box() {
        let (p, r) = unit();
        let u = from_plan(&p, &r);
        assert_eq!(u.hook.len(), 2, "{u:?}");
        assert_eq!(u.hoist.len(), 2, "{u:?}");
        assert!((u.hook_height - 700.0).abs() < 1e-3);
        // The anchor is the top middle of the hook solids (y = 300 in the file): the block's box
        // centre is 150 below it, the hook's 500 below.
        let block = u
            .hook
            .iter()
            .find(|x| x.role == PartRole::HookBlock)
            .unwrap();
        let hook = u.hook.iter().find(|x| x.role == PartRole::Hook).unwrap();
        assert!((block.offset.y + 150.0).abs() < 1e-3, "{block:?}");
        assert!((hook.offset.y + 500.0).abs() < 1e-3, "{hook:?}");
        // Across the span the item's origin is the box's low face, 100 mm from the middle plane.
        assert!((block.offset.z - (100.0 - 200.0)).abs() < 1e-3, "{block:?}");
    }

    #[test]
    fn in_the_generic_crane_the_user_solids_replace_the_hook_and_hoist_and_hang_from_the_rope() {
        use crate::crane::{CraneLayout, CraneState, build};
        let (p, r) = unit();
        let layout = CraneLayout::default();
        let generic = build(&layout, &CraneState::default());
        let state = CraneState {
            user: Some(from_plan(&p, &r)),
            ..CraneState::default()
        };
        let s = build(&layout, &state);
        let has = |sc: &crate::scene::Scene, raw: u64| sc.items.iter().any(|i| i.id.raw == raw);
        // The generic hook (11), sheaves (15, 16) and hoist drive (70, 71) are gone, the drum (8) too.
        for gone in [8, 11, 15, 16, 70, 71] {
            assert!(has(&generic, gone) && !has(&s, gone), "{gone}");
        }
        assert!(has(&s, 21_000) && has(&s, 21_001) && has(&s, 20_000) && has(&s, 20_001));
        // The top of the user's hook solids is where the generic block's top is.
        let top = |sc: &crate::scene::Scene, raws: &[u64]| {
            raws.iter()
                .map(|r| {
                    sc.world_bounds(sc.items.iter().find(|i| i.id.raw == *r).unwrap())
                        .max
                        .y
                })
                .fold(f32::MIN, f32::max)
        };
        let (a, b) = (top(&s, &[21_000, 21_001]), top(&generic, &[10]));
        assert!((a - b).abs() < 1.0, "{a} {b}");
    }
}
