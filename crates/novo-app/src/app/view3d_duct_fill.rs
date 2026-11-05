//! Duct fill in the 3D view: each duct body takes the colour of its fill tone
//! (the same tones as the plate's route screen), so an over-full duct is
//! visible from across the panel. Colour is never the only cue: an over-full
//! duct (red) and a busy one (yellow) also carry a problem triangle and the
//! fill percentage, drawn with the shared 3D flag painter. The route screen
//! carries the wires. (Lane T4 turns these into flags of the store.)

use super::cabinet_routes::{DuctFill, Tone, tone};
use eframe::egui;

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

/// A duct that needs a triangle: over its allowed fill, or busy.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Cue {
    pub rail: u64,
    pub red: bool,
    /// Fill, percent of the allowed fill.
    pub percent: u32,
}

/// The ducts with a cue, worst first.
pub(super) fn cues(fills: &[DuctFill]) -> Vec<Cue> {
    let mut out: Vec<Cue> = fills
        .iter()
        .filter_map(|f| {
            let red = match tone(f) {
                Tone::Over => true,
                Tone::Busy => false,
                _ => return None,
            };
            Some(Cue {
                rail: f.rail,
                red,
                percent: (f.ratio? * 100.0).round() as u32,
            })
        })
        .collect();
    out.sort_by_key(|c| (!c.red, std::cmp::Reverse(c.percent)));
    out
}

/// Draws the cues on the ducts: the triangle and the percentage beside it.
pub(super) fn paint(ui: &egui::Ui, layer: &super::view3d_flags::Layer, cues: &[Cue]) {
    let pal = crate::theme::ui_palette();
    let painter = ui.painter().with_clip_rect(layer.rect);
    let font = egui::FontId::proportional(12.0);
    for c in cues {
        let anchor = novo_flags::Anchor::Duct(c.rail.to_string());
        let Some(world) = super::view3d_flags::world_of(layer.scene, layer.extra, &anchor) else {
            continue;
        };
        let Some(at) = layer.view.project(layer.rect, layer.scene, world) else {
            continue;
        };
        if !layer.rect.contains(at) {
            continue;
        }
        novo_theme::widgets::flag_triangle(
            &painter,
            at,
            super::view3d_flags::SIZE,
            c.red,
            false,
            pal,
        );
        let galley =
            painter.layout_no_wrap(format!("{} %", c.percent), font.clone(), pal.text_primary);
        let chip = egui::Rect::from_min_size(
            at + egui::vec2(
                super::view3d_flags::SIZE * 0.6,
                -galley.size().y * 0.5 - 2.0,
            ),
            galley.size() + egui::vec2(8.0, 4.0),
        );
        painter.rect_filled(chip, 4.0, pal.raised.gamma_multiply(0.92));
        painter.galley(chip.min + egui::vec2(4.0, 2.0), galley, pal.text_primary);
    }
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
    fn only_over_and_busy_ducts_get_a_triangle_and_a_percentage() {
        let c = cues(&[
            fill(1, Some(0.5), 3),
            fill(2, Some(0.9), 3),
            fill(3, Some(1.2), 3),
            fill(4, None, 3),
            fill(5, Some(2.0), 0),
        ]);
        assert_eq!(
            c,
            vec![
                Cue {
                    rail: 3,
                    red: true,
                    percent: 120
                },
                Cue {
                    rail: 2,
                    red: false,
                    percent: 90
                },
            ]
        );
    }

    #[test]
    fn tones_have_distinct_colours() {
        let ok = rgb(Tone::Ok);
        let busy = rgb(Tone::Busy);
        let over = rgb(Tone::Over);
        assert!(ok != busy && busy != over && ok != over);
    }
}
