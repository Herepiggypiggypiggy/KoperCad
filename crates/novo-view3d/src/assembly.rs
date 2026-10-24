//! Build order in the picture: the cabinet shown as it stands after each step
//! of the workshop's build order (`novo_layout::assembly`: rails, ducts,
//! devices, then the wires). Position 0 is the enclosure and plate alone;
//! position `k` adds the part of step `k`; a step with no part (the wires)
//! shows everything that is left.
//!
//! The order is the layout's, the same one the printed assembly pack lists,
//! so the picture and the sheet agree. It reads the cabinet the project
//! holds; it does not plan the work.

use crate::scene::{ItemId, ItemKind, Scene};
use std::collections::BTreeSet;

/// One step of the sequence. `part` is the rail, duct or device it puts on the
/// plate; `None` is the closing step that adds everything still missing (the
/// wires, and any part the order does not name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub part: Option<ItemId>,
    /// Short name: R1, D2, a device tag, or "Wires".
    pub mark: String,
    /// What the workshop does in this step.
    pub text: String,
}

/// Kinds that are there from the start: the shell, the plate and anything
/// else that is not mounted in the order.
fn fixed(kind: ItemKind) -> bool {
    !matches!(
        kind,
        ItemKind::Rail | ItemKind::Duct | ItemKind::Device | ItemKind::Wire
    )
}

/// Whether the first `position` steps include the closing one.
fn complete(steps: &[Step], position: usize) -> bool {
    steps.iter().take(position).any(|s| s.part.is_none())
}

/// The scene as it stands after `position` steps. Item order and ids of what
/// stays are unchanged.
pub fn at(steps: &[Step], position: usize, mut scene: Scene) -> Scene {
    if complete(steps, position) {
        return scene;
    }
    let done: BTreeSet<ItemId> = steps.iter().take(position).filter_map(|s| s.part).collect();
    scene
        .items
        .retain(|i| fixed(i.id.kind) || done.contains(&i.id));
    scene.wires.clear();
    scene
}

/// What the step just taken adds, for the soft highlight: its part, or for
/// the closing step the wires. Nothing at position 0.
pub fn added(steps: &[Step], position: usize, scene: &Scene) -> BTreeSet<ItemId> {
    let Some(step) = position.checked_sub(1).and_then(|i| steps.get(i)) else {
        return BTreeSet::new();
    };
    match step.part {
        Some(part) => BTreeSet::from([part]),
        None => scene.wires.iter().map(|w| w.id).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, Mark, PlateFrame, Pose, Shape, Wire};
    use glam::Vec3;

    fn id(kind: ItemKind, raw: u64) -> ItemId {
        ItemId::new(kind, raw)
    }

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        for (kind, raw) in [
            (ItemKind::Enclosure, 0),
            (ItemKind::Plate, 0),
            (ItemKind::Door, 0),
            (ItemKind::Rail, 1),
            (ItemKind::Duct, 3),
            (ItemKind::Device, 4),
            (ItemKind::Device, 5),
            (ItemKind::Device, 6),
        ] {
            s.push(Item::new(
                id(kind, raw),
                Shape::Box,
                Pose::at(Vec3::ZERO),
                Vec3::splat(10.0),
            ));
        }
        s.wires.push(Wire {
            id: id(ItemKind::Wire, 9),
            points: vec![Vec3::ZERO, Vec3::X],
            radius: 1.0,
            tint: None,
            stripe: None,
            mark: Mark::None,
            label: None,
        });
        s
    }

    fn step(part: Option<ItemId>, mark: &str) -> Step {
        Step {
            part,
            mark: mark.into(),
            text: String::new(),
        }
    }

    /// Rail, duct, two devices (device 6 is not named), then the closing step.
    fn steps() -> Vec<Step> {
        vec![
            step(Some(id(ItemKind::Rail, 1)), "R1"),
            step(Some(id(ItemKind::Duct, 3)), "D1"),
            step(Some(id(ItemKind::Device, 4)), "-K1"),
            step(Some(id(ItemKind::Device, 5)), "-K2"),
            step(None, "Wires"),
        ]
    }

    fn ids(s: &Scene, kind: ItemKind) -> Vec<u64> {
        s.items
            .iter()
            .filter(|i| i.id.kind == kind)
            .map(|i| i.id.raw)
            .collect()
    }

    #[test]
    fn the_start_is_the_shell_and_plate_alone() {
        let s = at(&steps(), 0, scene());
        assert_eq!(s.items.len(), 3);
        assert!(s.wires.is_empty());
    }

    #[test]
    fn each_step_adds_exactly_its_part() {
        let steps = steps();
        let two = at(&steps, 2, scene());
        assert_eq!(ids(&two, ItemKind::Rail), [1]);
        assert_eq!(ids(&two, ItemKind::Duct), [3]);
        assert!(ids(&two, ItemKind::Device).is_empty());
        let three = at(&steps, 3, scene());
        assert_eq!(ids(&three, ItemKind::Device), [4]);
        assert!(three.wires.is_empty());
    }

    #[test]
    fn the_closing_step_shows_the_whole_cabinet_even_parts_the_order_misses() {
        let steps = steps();
        let all = at(&steps, steps.len(), scene());
        assert_eq!(all, scene());
        assert_eq!(ids(&all, ItemKind::Device), [4, 5, 6]);
    }

    #[test]
    fn a_step_highlights_only_what_it_adds() {
        let steps = steps();
        let s = scene();
        assert!(added(&steps, 0, &s).is_empty());
        assert_eq!(
            added(&steps, 3, &s),
            BTreeSet::from([id(ItemKind::Device, 4)])
        );
        assert_eq!(
            added(&steps, 5, &s),
            BTreeSet::from([id(ItemKind::Wire, 9)])
        );
    }

    #[test]
    fn an_empty_order_shows_the_plate_and_no_steps_to_take() {
        let s = at(&[], 0, scene());
        assert_eq!(s.items.len(), 3);
    }
}
