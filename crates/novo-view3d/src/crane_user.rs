//! Pieces of the user's own crane model in the crane picture.
//!
//! A caller turns a STEP model into up to two meshes (see `novo-interop`'s `crane_model`), puts
//! them in the mesh library under [`key`] and tells the crane scene about them with a
//! [`UserCrane`]. The generic hoist (drum, motor, gearbox) and hook (block, sheaves, hook) are
//! then left out and the pieces are drawn instead, at the size the file gives.

use crate::mesh::GeometryKey;
use crate::scene::{Item, ItemId, ItemKind, Pose, Shape};
use glam::{Quat, Vec3};

/// Top byte of a user piece key; crane parts use `0xC7`, device shapes `0xD5`.
const TAG: u64 = 0xC8;

/// Which piece of the picture a user mesh replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Hook,
    Hoist,
}

/// The library key of a user piece. `token` tells one model from another, so a new model does not
/// show the old one's mesh.
pub fn key(slot: Slot, token: u32) -> GeometryKey {
    let s = match slot {
        Slot::Hook => 1u64,
        Slot::Hoist => 2,
    };
    GeometryKey((TAG << 56) | (s << 40) | u64::from(token))
}

/// One user mesh and the extents of its local box: x and y centred, z up from 0, millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UserPiece {
    pub key: GeometryKey,
    pub size: Vec3,
}

/// The pieces that replace the generic picture. A `None` keeps the generic one.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UserCrane {
    pub hook: Option<UserPiece>,
    pub hoist: Option<UserPiece>,
}

/// A scene item for `piece` with its bottom centre at `bottom_centre` (world: x, y up, z).
pub fn item(
    raw: u64,
    kind: ItemKind,
    piece: UserPiece,
    bottom_centre: Vec3,
    tint: [u8; 3],
) -> Item {
    // Local z (up in the piece) lies on world y; local y becomes world -z.
    let mut it = Item::new(
        ItemId::new(kind, raw),
        Shape::Part(piece.key),
        Pose {
            translation: bottom_centre,
            rotation: Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2),
        },
        piece.size,
    );
    it.tint = Some(tint);
    it
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_differ_by_slot_and_by_model_and_stay_out_of_the_other_tags() {
        assert_ne!(key(Slot::Hook, 1), key(Slot::Hoist, 1));
        assert_ne!(key(Slot::Hook, 1), key(Slot::Hook, 2));
        assert_eq!(key(Slot::Hook, 1).0 >> 56, TAG);
        assert!(crate::crane_parts::Part::from_key(key(Slot::Hook, 1)).is_none());
    }
}
