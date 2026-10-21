//! Duct fill in the 3D view: each duct body takes the colour of its fill tone
//! (the same tones as the plate's route screen), so an over-full duct is
//! visible from across the panel. Colour is a cue only; the tooltip and the
//! route screen carry the numbers.

use super::cabinet_routes::{DuctFill, Tone, tone};

/// Colour of a tone, `None` for a duct that keeps its own colour.
fn rgb(t: Tone) -> Option<[u8; 3]> {
    let p = crate::theme::ui_palette();
    let c = match t {
        Tone::Idle => return None,
        Tone::NoSize => p.text_secondary,
        Tone::Ok => p.success,
        Tone::Busy => p.warning,
        Tone::Over => p.error,
    };
    Some([c.r(), c.g(), c.b()])
}

/// Colours the duct items of `scene` by `fills`; ducts without an entry keep
/// their colour. Bumps the revision so the view redraws.
pub(super) fn tint(scene: &mut novo_view3d::Scene, fills: &[DuctFill]) {
    for item in &mut scene.items {
        if item.id.kind != novo_view3d::ItemKind::Duct {
            continue;
        }
        if let Some(f) = fills.iter().find(|f| f.rail == item.id.raw) {
            item.tint = rgb(tone(f));
        }
    }
    scene.revision = scene.revision.wrapping_add(0x5DF1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_view3d::{Item, ItemId, ItemKind, PlateFrame, Pose, Scene, Shape};

    fn fill(rail: u64, ratio: Option<f64>, wires: usize) -> DuctFill {
        DuctFill {
            rail,
            rect: (0.0, 0.0, 10.0, 10.0),
            ratio,
            wires,
        }
    }

    fn scene() -> Scene {
        let mut s = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        for (kind, raw) in [
            (ItemKind::Duct, 1),
            (ItemKind::Duct, 2),
            (ItemKind::Rail, 1),
        ] {
            let mut i = Item::new(
                ItemId::new(kind, raw),
                Shape::Box,
                Pose::IDENTITY,
                glam::Vec3::ONE,
            );
            i.tint = None;
            s.items.push(i);
        }
        s
    }

    #[test]
    fn only_ducts_are_tinted_by_tone() {
        let mut s = scene();
        let rev = s.revision;
        tint(&mut s, &[fill(1, Some(1.2), 3), fill(2, None, 0)]);
        let p = crate::theme::ui_palette();
        assert_eq!(
            s.items[0].tint,
            Some([p.error.r(), p.error.g(), p.error.b()])
        );
        assert_eq!(s.items[1].tint, None, "idle duct keeps its colour");
        assert_eq!(s.items[2].tint, None, "rails are not ducts");
        assert_ne!(s.revision, rev);
    }

    #[test]
    fn tones_have_distinct_colours() {
        let ok = rgb(Tone::Ok);
        let busy = rgb(Tone::Busy);
        let over = rgb(Tone::Over);
        assert!(ok != busy && busy != over && ok != over);
    }
}
