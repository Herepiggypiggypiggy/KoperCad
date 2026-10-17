//! Hiding, isolating and selecting by type: filters over a built scene, like
//! the layers, so the project and the builder do not know about them.

use std::collections::BTreeSet;

use crate::scene::{ItemId, ItemKind, Scene};

/// The kinds a user hides, isolates and selects one by one. The enclosure,
/// plate and door are layers (the toolbar's switches), not items.
pub fn is_part(kind: ItemKind) -> bool {
    matches!(kind, ItemKind::Device | ItemKind::Rail | ItemKind::Duct)
}

/// The scene without the hidden items. Order and ids of what stays are
/// unchanged; a hidden id the scene does not have is ignored.
pub fn without(mut scene: Scene, hidden: &BTreeSet<ItemId>) -> Scene {
    if !hidden.is_empty() {
        scene.items.retain(|i| !hidden.contains(&i.id));
    }
    scene
}

/// Every item of one kind, to select "all rails" or "all devices".
pub fn of_kind(scene: &Scene, kind: ItemKind) -> BTreeSet<ItemId> {
    scene
        .items
        .iter()
        .filter(|i| i.id.kind == kind)
        .map(|i| i.id)
        .collect()
}

/// What to hide so only `keep` of the parts stays: every device, rail and
/// duct that is not kept. The shell, plate and door are left to the layers.
pub fn all_but(scene: &Scene, keep: &BTreeSet<ItemId>) -> BTreeSet<ItemId> {
    scene
        .items
        .iter()
        .filter(|i| is_part(i.id.kind) && !keep.contains(&i.id))
        .map(|i| i.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, PlateFrame, Pose, Shape};
    use glam::Vec3;

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        for (kind, raw) in [
            (ItemKind::Plate, 0),
            (ItemKind::Device, 1),
            (ItemKind::Device, 2),
            (ItemKind::Rail, 3),
            (ItemKind::Duct, 4),
        ] {
            s.push(Item::new(
                ItemId::new(kind, raw),
                Shape::Box,
                Pose::at(Vec3::ZERO),
                Vec3::splat(10.0),
            ));
        }
        s
    }

    #[test]
    fn hidden_items_leave_the_scene_and_the_rest_keeps_its_order() {
        let hidden = BTreeSet::from([ItemId::new(ItemKind::Device, 1)]);
        let out = without(scene(), &hidden);
        let ids: Vec<u64> = out.items.iter().map(|i| i.id.raw).collect();
        assert_eq!(ids, vec![0, 2, 3, 4]);
    }

    #[test]
    fn select_by_type_takes_every_item_of_the_kind() {
        let s = scene();
        assert_eq!(of_kind(&s, ItemKind::Device).len(), 2);
        assert_eq!(of_kind(&s, ItemKind::Rail).len(), 1);
        assert_eq!(of_kind(&s, ItemKind::Duct).len(), 1);
        assert!(of_kind(&s, ItemKind::Door).is_empty());
    }

    #[test]
    fn isolate_hides_the_other_parts_but_not_the_plate() {
        let s = scene();
        let keep = BTreeSet::from([ItemId::new(ItemKind::Rail, 3)]);
        let hide = all_but(&s, &keep);
        assert_eq!(hide.len(), 3);
        assert!(!hide.contains(&ItemId::new(ItemKind::Plate, 0)));
        assert!(!hide.contains(&ItemId::new(ItemKind::Rail, 3)));
        let shown = without(s, &hide);
        assert_eq!(shown.items.len(), 2);
    }
}
