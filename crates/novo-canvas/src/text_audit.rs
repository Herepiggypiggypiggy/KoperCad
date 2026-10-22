//! Measuring text conflicts on a page (docs/TEXT-PLACEMENT.md, stage 0).
//!
//! Read only: finds where one label's box covers another label, a symbol's
//! drawing or a wire. Label boxes are the estimated boxes of
//! [`Label::corners`], so a count is a close estimate, not a pixel result.
//! The scene dump, the QA reports and the optional `text.conflict` check all
//! read the same numbers.
//!
//! Not counted: a label over the drawing of its own item (the `M` inside a
//! motor circle is meant to be there), and ink on the page frame and underlay
//! layers. Their text does count as text a label must not sit on, and a
//! label there is compared with other labels like any other. Rotated labels
//! are measured by their rotated box against labels, symbols and wires.

use glam::Vec2;

use crate::geom::Aabb;
use crate::note_place::shape_touches;
use crate::scene::{ItemId, Scene};
use crate::shape::{Placement, Shape};
use crate::text::Label;

/// Overlaps smaller than this, in mm, are rounding noise of the estimated boxes.
const TOLERANCE: f32 = 0.05;

/// Layer names the scene's model adapter uses.
const WIRES: &str = "Wires";
const FRAME: &str = "Frame";
const UNDERLAY: &str = "Underlay";

/// One label in a conflict: where it lives and what it says.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    pub item: ItemId,
    pub label: usize,
    pub text: String,
}

/// A label that covers something.
#[derive(Clone, Debug, PartialEq)]
pub struct Conflict {
    pub label: Subject,
    /// The other label for text-on-text, the item for ink and wire.
    pub other: Option<Subject>,
    pub other_item: ItemId,
    /// Centre of the label's box, mm.
    pub at: Vec2,
}

/// Counts and pairs for one page.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextAudit {
    pub labels: usize,
    pub text_on_text: Vec<Conflict>,
    pub text_on_ink: Vec<Conflict>,
    pub text_on_wire: Vec<Conflict>,
}

impl TextAudit {
    pub fn total(&self) -> usize {
        self.text_on_text.len() + self.text_on_ink.len() + self.text_on_wire.len()
    }

    /// One line per conflict, for reports.
    pub fn describe(&self) -> Vec<String> {
        let line = |kind: &str, c: &Conflict| match &c.other {
            Some(o) => format!(
                "{kind}: \"{}\" ({}) and \"{}\" ({}) at {:.1}, {:.1} mm",
                c.label.text, c.label.item.0, o.text, o.item.0, c.at.x, c.at.y
            ),
            None => format!(
                "{kind}: \"{}\" ({}) over item {} at {:.1}, {:.1} mm",
                c.label.text, c.label.item.0, c.other_item.0, c.at.x, c.at.y
            ),
        };
        let mut out = Vec::new();
        out.extend(self.text_on_text.iter().map(|c| line("text on text", c)));
        out.extend(self.text_on_ink.iter().map(|c| line("text on symbol", c)));
        out.extend(self.text_on_wire.iter().map(|c| line("text on wire", c)));
        out
    }
}

/// Measures every visible label of `scene`. Deterministic: items are visited
/// in id order and each pair is reported once.
pub fn audit(scene: &Scene) -> TextAudit {
    let mut items: Vec<_> = scene
        .iter()
        .filter(|(_, it)| scene.layer(it.layer).is_some_and(|l| l.visible))
        .collect();
    items.sort_by_key(|(id, _)| *id);
    let layer_name = |it: &crate::scene::Item| {
        scene
            .layer(it.layer)
            .map_or("", |l| l.name.as_str())
            .to_string()
    };
    let mut out = TextAudit::default();
    for (id, item) in &items {
        let name = layer_name(item);
        for (i, label) in item
            .labels
            .iter()
            .enumerate()
            .filter(|(_, l)| !l.text.is_empty())
        {
            out.labels += 1;
            let page_furniture = name == FRAME || name == UNDERLAY;
            let b = label.bounds();
            let probe = b.expand(TOLERANCE);
            let me = subject(*id, i, label);
            for other_id in sorted(scene.query(&probe)) {
                let Some(other) = scene.get(other_id) else {
                    continue;
                };
                if !scene.layer(other.layer).is_some_and(|l| l.visible) {
                    continue;
                }
                let other_name = layer_name(other);
                for (j, ol) in other
                    .labels
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| !l.text.is_empty())
                {
                    // Each pair once: the lower (item, label) reports.
                    if (other_id, j) <= (*id, i) {
                        continue;
                    }
                    if boxes_overlap(label, ol) {
                        out.text_on_text.push(Conflict {
                            label: me.clone(),
                            other: Some(subject(other_id, j, ol)),
                            other_item: other_id,
                            at: b.center(),
                        });
                    }
                }
                if page_furniture
                    || other_id == *id
                    || other_name == FRAME
                    || other_name == UNDERLAY
                {
                    continue;
                }
                if other
                    .primitives
                    .iter()
                    .any(|p| touches(&p.shape, label, &b))
                {
                    let c = Conflict {
                        label: me.clone(),
                        other: None,
                        other_item: other_id,
                        at: b.center(),
                    };
                    if other_name == WIRES {
                        out.text_on_wire.push(c);
                    } else {
                        out.text_on_ink.push(c);
                    }
                }
            }
        }
    }
    out
}

fn sorted(mut ids: Vec<ItemId>) -> Vec<ItemId> {
    ids.sort();
    ids
}

fn subject(item: ItemId, label: usize, l: &Label) -> Subject {
    Subject {
        item,
        label,
        text: l.text.clone(),
    }
}

/// The label's box shrunk by the tolerance, so touching edges do not count.
fn inset(b: &Aabb) -> Aabb {
    Aabb::new(
        b.min + Vec2::splat(TOLERANCE),
        b.max - Vec2::splat(TOLERANCE),
    )
}

/// True when `shape` touches the label's box. A rotated label is tested in
/// its own frame, where the box is axis aligned, so a label at an angle is
/// not measured by the bigger axis-aligned box around it.
fn touches(shape: &Shape, label: &Label, bounds: &Aabb) -> bool {
    if label.rotation.abs() < 1e-6 {
        return shape_touches(shape, &inset(bounds));
    }
    let back = Vec2::from_angle(-label.rotation);
    let to_local = Placement {
        offset: -back.rotate(label.pos),
        rotation: -label.rotation,
        mirror: false,
        scale: 1.0,
    };
    let mut upright = label.clone();
    upright.pos = Vec2::ZERO;
    upright.rotation = 0.0;
    shape_touches(&shape.transformed(&to_local), &inset(&upright.bounds()))
}

/// True when the two (possibly rotated) boxes overlap by more than the tolerance.
fn boxes_overlap(a: &Label, b: &Label) -> bool {
    let (ca, cb) = (a.corners(), b.corners());
    for poly in [&ca, &cb] {
        for i in 0..4 {
            let edge = poly[(i + 1) % 4] - poly[i];
            let axis = Vec2::new(-edge.y, edge.x).normalize_or_zero();
            if axis == Vec2::ZERO {
                continue;
            }
            let (a0, a1) = project(&ca, axis);
            let (b0, b1) = project(&cb, axis);
            if a1 - b0 <= TOLERANCE || b1 - a0 <= TOLERANCE {
                return false;
            }
        }
    }
    true
}

fn project(c: &[Vec2; 4], axis: Vec2) -> (f32, f32) {
    c.iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), p| {
            let d = p.dot(axis);
            (lo.min(d), hi.max(d))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Item, Layer, LayerId};
    use crate::shape::{Primitive, Shape};
    use crate::style::{Color, Stroke};
    use crate::text::Anchor;

    fn scene() -> (Scene, LayerId, LayerId) {
        let mut s = Scene::new();
        let wires = s.add_layer(Layer::new(WIRES, 0));
        let symbols = s.add_layer(Layer::new("Symbols", 10));
        (s, wires, symbols)
    }

    fn label(x: f32, y: f32, text: &str) -> Label {
        Label::new(Vec2::new(x, y), text, 2.5).anchored(Anchor::CenterLeft)
    }

    fn line(a: Vec2, b: Vec2) -> Primitive {
        Primitive::stroked(
            Shape::line(a, b),
            Stroke::world(0.25, Color::rgb(255, 255, 255)),
        )
    }

    #[test]
    fn two_tags_on_top_of_each_other_are_one_conflict() {
        let (mut s, _, sym) = scene();
        s.insert(ItemId(1), Item::new(sym).with_label(label(0.0, 0.0, "-K1")));
        s.insert(ItemId(2), Item::new(sym).with_label(label(2.0, 0.0, "-K2")));
        let a = audit(&s);
        assert_eq!(a.labels, 2);
        assert_eq!(a.text_on_text.len(), 1);
        assert_eq!(a.total(), 1);
    }

    #[test]
    fn labels_that_only_touch_or_are_apart_do_not_count() {
        let (mut s, _, sym) = scene();
        let w = label(0.0, 0.0, "-K1").bounds().max.x;
        s.insert(ItemId(1), Item::new(sym).with_label(label(0.0, 0.0, "-K1")));
        s.insert(ItemId(2), Item::new(sym).with_label(label(w, 0.0, "-K2")));
        s.insert(
            ItemId(3),
            Item::new(sym).with_label(label(w + 10.0, 0.0, "-K3")),
        );
        assert_eq!(audit(&s).total(), 0);
    }

    #[test]
    fn a_rotated_label_is_measured_by_its_rotated_box() {
        let (mut s, _, sym) = scene();
        let v = label(0.0, 0.0, "12345").rotated(-std::f32::consts::FRAC_PI_2);
        s.insert(ItemId(1), Item::new(sym).with_label(v));
        // Beside the vertical text: clear. Its axis-aligned box would not be.
        s.insert(ItemId(2), Item::new(sym).with_label(label(4.0, 0.0, "x")));
        assert_eq!(audit(&s).total(), 0);
        s.insert(ItemId(3), Item::new(sym).with_label(label(-0.5, -4.0, "x")));
        assert_eq!(audit(&s).text_on_text.len(), 1);
    }

    #[test]
    fn text_over_a_wire_and_over_a_symbol_are_counted_apart() {
        let (mut s, wires, sym) = scene();
        let mut w = Item::new(wires);
        w.primitives
            .push(line(Vec2::new(-5.0, 0.0), Vec2::new(20.0, 0.0)));
        s.insert(ItemId(1), w);
        let mut body = Item::new(sym);
        body.primitives
            .push(line(Vec2::new(30.0, -5.0), Vec2::new(30.0, 5.0)));
        s.insert(ItemId(2), body);
        s.insert(ItemId(3), Item::new(sym).with_label(label(1.0, 0.0, "-K1")));
        s.insert(
            ItemId(4),
            Item::new(sym).with_label(label(28.0, 0.0, "-K2")),
        );
        let a = audit(&s);
        assert_eq!((a.text_on_wire.len(), a.text_on_ink.len()), (1, 1));
        assert_eq!(a.describe().len(), 2);
    }

    #[test]
    fn a_label_over_its_own_symbol_is_meant() {
        let (mut s, _, sym) = scene();
        let mut m = Item::new(sym).with_label(label(-1.0, 0.0, "M"));
        m.primitives
            .push(line(Vec2::new(0.0, -5.0), Vec2::new(0.0, 5.0)));
        s.insert(ItemId(1), m);
        assert_eq!(audit(&s).total(), 0);
    }

    #[test]
    fn two_labels_of_one_item_are_compared_and_the_result_is_stable() {
        let (mut s, _, sym) = scene();
        let item = Item::new(sym)
            .with_label(label(0.0, 0.0, "-X1"))
            .with_label(label(1.0, 0.0, "13"));
        s.insert(ItemId(7), item);
        let first = audit(&s);
        assert_eq!(first.text_on_text.len(), 1);
        assert_eq!(audit(&s), first);
    }

    #[test]
    fn page_text_and_a_label_conflict_whatever_the_ids() {
        for (frame_id, label_id) in [(1, 2), (2, 1)] {
            let (mut s, _, sym) = scene();
            let frame = s.add_layer(Layer::new(FRAME, -10));
            s.insert(
                ItemId(frame_id),
                Item::new(frame).with_label(label(0.0, 0.0, "Motor starter")),
            );
            s.insert(
                ItemId(label_id),
                Item::new(sym).with_label(label(2.0, 0.0, "-K1")),
            );
            assert_eq!(
                audit(&s).text_on_text.len(),
                1,
                "ids {frame_id}, {label_id}"
            );
        }
    }

    #[test]
    fn a_label_at_an_angle_is_measured_by_its_own_box() {
        let (mut s, wires, sym) = scene();
        let slanted = label(0.0, 0.0, "12345678").rotated(std::f32::consts::FRAC_PI_4);
        s.insert(ItemId(1), Item::new(sym).with_label(slanted));
        // The box of the slanted text is far from this wire, though the
        // axis-aligned box around it reaches it.
        let mut w = Item::new(wires);
        w.primitives
            .push(line(Vec2::new(8.0, 0.0), Vec2::new(8.0, 3.0)));
        s.insert(ItemId(2), w);
        assert_eq!(audit(&s).text_on_wire.len(), 0);
        // A wire through the text does count.
        let mut through = Item::new(wires);
        through
            .primitives
            .push(line(Vec2::new(3.0, 3.0), Vec2::new(4.0, 4.0)));
        s.insert(ItemId(3), through);
        assert_eq!(audit(&s).text_on_wire.len(), 1);
    }
}
