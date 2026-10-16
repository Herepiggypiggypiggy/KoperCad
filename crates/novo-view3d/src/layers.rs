//! Layers of the cabinet view: what the user can switch off to see through or
//! tidy the picture. A layer is a filter over a built scene, so the project
//! and the builder do not know about it.

use crate::scene::{ItemKind, Scene};

/// Which parts of the scene are drawn. Everything on by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layers {
    /// The see-through enclosure box.
    pub shell: bool,
    /// The door (and the swing that goes with it).
    pub door: bool,
    /// The routed wires.
    pub wires: bool,
}

impl Default for Layers {
    fn default() -> Self {
        Layers {
            shell: true,
            door: true,
            wires: true,
        }
    }
}

impl Layers {
    /// The scene with the switched-off layers removed. Item order and ids of
    /// what stays are unchanged.
    pub fn apply(&self, mut scene: Scene) -> Scene {
        scene.items.retain(|i| match i.id.kind {
            ItemKind::Enclosure => self.shell,
            ItemKind::Door => self.door,
            _ => true,
        });
        if !self.door {
            let used: std::collections::HashSet<_> =
                scene.items.iter().filter_map(|i| i.group).collect();
            scene.groups.retain(|g| used.contains(&g.id));
        }
        if !self.wires {
            scene.wires.clear();
        }
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Group, GroupId, Item, ItemId, Mark, PlateFrame, Pose, Shape, Wire};
    use glam::Vec3;

    fn item(kind: ItemKind, id: u64) -> Item {
        Item::new(
            ItemId::new(kind, id),
            Shape::Box,
            Pose::at(Vec3::ZERO),
            Vec3::new(10.0, 10.0, 10.0),
        )
    }

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        s.push(item(ItemKind::Enclosure, 0));
        s.push(item(ItemKind::Plate, 0));
        s.push(item(ItemKind::Device, 7));
        let mut door = item(ItemKind::Door, 0);
        door.group = Some(GroupId(1));
        s.push(door);
        s.groups.push(Group {
            id: GroupId(1),
            pivot: Vec3::ZERO,
            axis: Vec3::Y,
            angle: 0.0,
        });
        s.wires.push(Wire {
            id: ItemId::new(ItemKind::Wire, 3),
            points: vec![Vec3::ZERO, Vec3::X],
            radius: 1.5,
            tint: None,
            stripe: None,
            mark: Mark::None,
            label: None,
        });
        s
    }

    fn has(scene: &Scene, kind: ItemKind) -> bool {
        scene.items.iter().any(|i| i.id.kind == kind)
    }

    #[test]
    fn all_layers_on_changes_nothing() {
        let s = scene();
        assert_eq!(Layers::default().apply(s.clone()), s);
    }

    #[test]
    fn each_layer_removes_only_its_own_things() {
        let s = scene();

        let no_shell = Layers {
            shell: false,
            ..Layers::default()
        }
        .apply(s.clone());
        assert!(!has(&no_shell, ItemKind::Enclosure));
        assert!(has(&no_shell, ItemKind::Door));
        assert_eq!(no_shell.wires.len(), 1);

        let no_door = Layers {
            door: false,
            ..Layers::default()
        }
        .apply(s.clone());
        assert!(!has(&no_door, ItemKind::Door));
        assert!(no_door.groups.is_empty(), "the hinge goes with the door");
        assert!(has(&no_door, ItemKind::Enclosure));

        let no_wires = Layers {
            wires: false,
            ..Layers::default()
        }
        .apply(s.clone());
        assert!(no_wires.wires.is_empty());
        assert_eq!(no_wires.items.len(), s.items.len());
    }
}
