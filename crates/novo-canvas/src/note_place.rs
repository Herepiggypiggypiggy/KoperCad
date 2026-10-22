//! Where a note goes so it does not sit on a wire or a symbol.
//!
//! A note (a cross reference such as `95-96  2.1`) has a preferred place next
//! to its symbol (see [`crate::model::NoteAnchor`]). When wires, other
//! symbols or other text cover that place, the note moves to the first free
//! place of a fixed list: the same drawing always gives the same answer, and
//! the answer changes only when the drawing around the note does. A note
//! the user moved by hand is never searched for; it stays where they put it.

use crate::scene::{Item, ItemId, Scene};
use crate::shape::{Primitive, Shape};
use crate::text::{Anchor, Label};
use crate::{Aabb, Vec2, geom};

/// Clear space kept around a note, in mm.
pub const PAD: f32 = 0.4;

/// Extra distance from the symbol, in mm, tried in this order.
const RINGS: [f32; 6] = [0.0, 1.5, 3.0, 5.0, 8.0, 12.0];

/// The farthest ring, in mm.
pub const MAX_RING: f32 = 12.0;

/// Slides along a side, in text heights, tried in this order.
const SLIDES: [f32; 5] = [0.0, 1.0, -1.0, 2.0, -2.0];

/// The places tried for a note, best first: the preferred one, then rings
/// of growing distance around the symbol's `ink` (right, below, left, above,
/// each slid a little along its side). `height` is the note's text height.
pub fn candidates(preferred: (Vec2, Anchor), ink: &Aabb, height: f32) -> Vec<(Vec2, Anchor)> {
    let mut out = vec![preferred];
    let c = ink.center();
    for ring in RINGS {
        for slide in SLIDES {
            let along = slide * height;
            out.push((
                Vec2::new(ink.max.x + 1.0 + ring, c.y + along),
                Anchor::CenterLeft,
            ));
        }
        for slide in SLIDES {
            let along = slide * height * 2.0;
            out.push((
                Vec2::new(ink.min.x + along, ink.max.y + 1.0 + ring),
                Anchor::TopLeft,
            ));
        }
        for slide in SLIDES {
            let along = slide * height;
            out.push((
                Vec2::new(ink.min.x - 1.0 - ring, c.y + along),
                Anchor::CenterRight,
            ));
        }
        for slide in SLIDES {
            let along = slide * height * 2.0;
            out.push((
                Vec2::new(ink.min.x + along, ink.min.y - 0.5 - ring),
                Anchor::BottomLeft,
            ));
        }
    }
    out
}

/// What a note must keep clear of: every selectable item of the scene but
/// its own, and the parts of its own item that are not the note.
pub struct Obstacles<'a> {
    pub scene: &'a Scene,
    /// The note's own item. Only its first `keep` primitives and labels
    /// count: what comes after is the note itself.
    pub own: &'a Item,
    pub own_id: ItemId,
    pub keep: (usize, usize),
    /// For another item that carries a note placed after this one: how many
    /// of its primitives and labels are not that note. Notes go down one
    /// symbol at a time in a fixed order and a note keeps clear only of the
    /// notes before it, so the result does not depend on where the others
    /// happen to be drawn at the moment.
    pub later: &'a dyn Fn(ItemId) -> Option<(usize, usize)>,
}

impl Obstacles<'_> {
    /// True when the note, drawn as `primitives` and `labels`, would touch
    /// a wire, a symbol or other text.
    pub fn blocks(&self, primitives: &[Primitive], labels: &[Label]) -> bool {
        let boxes = primitives
            .iter()
            .map(|p| p.shape.bounds())
            .chain(labels.iter().map(Label::bounds));
        boxes.into_iter().any(|b| self.blocks_box(&b.expand(PAD)))
    }

    fn blocks_box(&self, rect: &Aabb) -> bool {
        let own = &self.own;
        let primitives = &own.primitives[..self.keep.0.min(own.primitives.len())];
        let labels = &own.labels[..self.keep.1.min(own.labels.len())];
        if parts_touch(primitives, labels, rect) {
            return true;
        }
        self.scene.query(rect).into_iter().any(|id| {
            id != self.own_id
                && self
                    .scene
                    .get(id)
                    .is_some_and(|item| item.selectable && self.item_blocks(id, item, rect))
        })
    }

    fn item_blocks(&self, id: ItemId, item: &Item, rect: &Aabb) -> bool {
        match (self.later)(id) {
            Some((p, l)) => parts_touch(
                &item.primitives[..p.min(item.primitives.len())],
                &item.labels[..l.min(item.labels.len())],
                rect,
            ),
            None => item_touches(item, rect),
        }
    }
}

fn item_touches(item: &Item, rect: &Aabb) -> bool {
    parts_touch(&item.primitives, &item.labels, rect)
}

fn parts_touch(primitives: &[Primitive], labels: &[Label], rect: &Aabb) -> bool {
    primitives.iter().any(|p| shape_touches(&p.shape, rect))
        || labels.iter().any(|l| l.intersects(rect))
}

/// Lines count along their length, so the corner of an L-shaped wire does
/// not block the empty space inside the L.
pub(crate) fn shape_touches(shape: &Shape, rect: &Aabb) -> bool {
    match shape {
        Shape::Line { a, b } => geom::segment_intersects_aabb(*a, *b, rect),
        Shape::Polyline { points, closed } => {
            let n = points.len();
            (0..n.saturating_sub(1))
                .any(|i| geom::segment_intersects_aabb(points[i], points[i + 1], rect))
                || (*closed
                    && n > 2
                    && geom::segment_intersects_aabb(points[n - 1], points[0], rect))
        }
        other => other.bounds().intersects(rect),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Layer, LayerId};
    use crate::{Primitive, Stroke};

    fn scene_with(items: Vec<Item>) -> Scene {
        let mut scene = Scene::new();
        let layer = scene.add_layer(Layer::new("test", 0));
        for (i, mut item) in items.into_iter().enumerate() {
            item.layer = layer;
            scene.insert(ItemId(1 + i as u64), item);
        }
        scene
    }

    fn wire(a: (f32, f32), b: (f32, f32)) -> Item {
        let mut item = Item::new(LayerId(0));
        item.primitives.push(Primitive::stroked(
            Shape::line(Vec2::new(a.0, a.1), Vec2::new(b.0, b.1)),
            Stroke::world(0.25, crate::Color::rgb(255, 255, 255)),
        ));
        item
    }

    /// The first free place for a one-label note.
    fn place_label(base: &Label, ink: &Aabb, ob: &Obstacles) -> Label {
        for (pos, anchor) in candidates((base.pos, base.anchor), ink, base.height) {
            let mut label = base.clone();
            label.pos = pos;
            label.anchor = anchor;
            if !ob.blocks(&[], std::slice::from_ref(&label)) {
                return label;
            }
        }
        base.clone()
    }

    fn note_at(pos: Vec2, anchor: Anchor) -> Label {
        Label::new(pos, "95-96 2.1", 2.5).anchored(anchor)
    }

    fn own() -> Item {
        let mut item = Item::new(LayerId(0));
        item.primitives.push(Primitive::stroked(
            Shape::rect(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0)),
            Stroke::world(0.25, crate::Color::rgb(255, 255, 255)),
        ));
        item
    }

    #[test]
    fn a_free_preferred_place_is_kept() {
        let scene = scene_with(vec![]);
        let own = own();
        let ob = Obstacles {
            scene: &scene,
            own: &own,
            own_id: ItemId(99),
            keep: (1, 0),
            later: &|_| None,
        };
        let ink = Aabb::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let base = note_at(Vec2::new(11.0, 5.0), Anchor::CenterLeft);
        assert_eq!(place_label(&base, &ink, &ob), base);
    }

    #[test]
    fn a_wire_through_the_note_moves_it_to_the_nearest_free_place() {
        // A wire runs down through the preferred place, right of the symbol.
        let scene = scene_with(vec![wire((14.0, -20.0), (14.0, 30.0))]);
        let own = own();
        let ob = Obstacles {
            scene: &scene,
            own: &own,
            own_id: ItemId(99),
            keep: (1, 0),
            later: &|_| None,
        };
        let ink = Aabb::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let base = note_at(Vec2::new(11.0, 5.0), Anchor::CenterLeft);
        assert!(
            ob.blocks(&[], std::slice::from_ref(&base)),
            "the preferred place is crossed"
        );
        let placed = place_label(&base, &ink, &ob);
        assert_ne!(placed.pos, base.pos);
        assert!(
            !ob.blocks(&[], std::slice::from_ref(&placed)),
            "the new place is free"
        );
        // And it is deterministic.
        assert_eq!(place_label(&base, &ink, &ob), placed);
    }

    #[test]
    fn the_inside_of_an_l_shaped_wire_is_free() {
        // An L around the corner: its bounding box covers the note, its
        // lines do not.
        let mut l = Item::new(LayerId(0));
        l.primitives.push(Primitive::stroked(
            Shape::polyline(
                [
                    Vec2::new(30.0, -30.0),
                    Vec2::new(30.0, 30.0),
                    Vec2::new(60.0, 30.0),
                ],
                false,
            ),
            Stroke::world(0.25, crate::Color::rgb(255, 255, 255)),
        ));
        let scene = scene_with(vec![l]);
        let own = own();
        let ob = Obstacles {
            scene: &scene,
            own: &own,
            own_id: ItemId(99),
            keep: (1, 0),
            later: &|_| None,
        };
        let base = note_at(Vec2::new(11.0, 5.0), Anchor::CenterLeft);
        assert!(!ob.blocks(&[], std::slice::from_ref(&base)));
    }

    #[test]
    fn the_symbols_own_ink_and_other_text_are_avoided() {
        let mut other = Item::new(LayerId(0));
        other
            .labels
            .push(Label::new(Vec2::new(11.0, 5.0), "-K2", 2.5).anchored(Anchor::CenterLeft));
        let scene = scene_with(vec![other]);
        let own = own();
        let ob = Obstacles {
            scene: &scene,
            own: &own,
            own_id: ItemId(99),
            keep: (1, 0),
            later: &|_| None,
        };
        let on_ink = note_at(Vec2::new(2.0, 5.0), Anchor::CenterLeft);
        assert!(
            ob.blocks(&[], std::slice::from_ref(&on_ink)),
            "on the symbol"
        );
        let on_text = note_at(Vec2::new(11.0, 5.0), Anchor::CenterLeft);
        assert!(
            ob.blocks(&[], std::slice::from_ref(&on_text)),
            "on another tag"
        );
    }

    #[test]
    fn a_box_that_cannot_be_cleared_stays_at_the_preferred_place() {
        // Walls of wire all around, far beyond the search rings.
        let mut walls = Vec::new();
        for i in -40..=40 {
            let d = i as f32 * 1.0;
            walls.push(wire((-60.0, d), (70.0, d)));
        }
        let scene = scene_with(walls);
        let own = own();
        let ob = Obstacles {
            scene: &scene,
            own: &own,
            own_id: ItemId(99),
            keep: (1, 0),
            later: &|_| None,
        };
        let ink = Aabb::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let base = note_at(Vec2::new(11.0, 5.0), Anchor::CenterLeft);
        assert_eq!(place_label(&base, &ink, &ob), base);
    }

    #[test]
    fn the_search_order_is_fixed() {
        let ink = Aabb::from_corners(Vec2::new(0.0, 0.0), Vec2::new(10.0, 10.0));
        let pref = (Vec2::new(11.0, 5.0), Anchor::CenterLeft);
        let a = candidates(pref, &ink, 2.5);
        let b = candidates(pref, &ink, 2.5);
        assert_eq!(a, b);
        assert_eq!(a[0], pref);
        assert_eq!(a.len(), 1 + 6 * 4 * 5);
    }
}
