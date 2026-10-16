//! The Cabinet view as a canvas scene: a mounting plate, DIN rails, devices
//! as boxes with their tags, and the clearance around each device.
//!
//! Plain structs in, a [`Scene`] out: the canvas crate does not depend on
//! `novo-layout` or the model. The app turns its layout state into a
//! [`CabinetView`] and calls [`CabinetScene::set`]. Problems are shown by
//! colour and by a mark (a cross for a collision or a device off the plate,
//! a bang for a clearance that is too small), never by colour alone.

use crate::geom::Aabb;
use crate::scene::{Item, ItemId, Layer, LayerId, Scene};
use crate::shape::{Primitive, Shape};
use crate::style::{Color, Stroke};
use crate::text::{Anchor, Label};
use glam::Vec2;

/// The mounting plate, in mm with the origin at its top left, y down.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlateView {
    pub width: f32,
    pub height: f32,
    /// Unusable border along the plate's edge.
    pub margin: f32,
}

/// A horizontal DIN rail.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailView {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub length: f32,
}

/// What is wrong with a device's place, worst first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BoxState {
    #[default]
    Ok,
    /// Overlaps another device.
    Collision,
    /// Reaches past the usable area of the plate.
    Outside,
    /// Deeper than the room between the plate and the door: the door cannot
    /// close.
    TooDeep,
    /// Closer to a neighbour than its clearance allows.
    TooClose,
}

/// A device placed on the plate: its footprint (already rotated).
#[derive(Clone, Debug, PartialEq)]
pub struct BoxView {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub tag: String,
    /// The size is a default because the part has none: drawn dashed.
    pub guessed: bool,
    /// Free space wanted around the footprint, in mm.
    pub clearance: f32,
    pub state: BoxState,
}

/// Everything the Cabinet canvas draws.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CabinetView {
    pub plate: Option<PlateView>,
    pub rails: Vec<RailView>,
    pub boxes: Vec<BoxView>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CabinetStyle {
    pub plate_edge: Color,
    pub usable_edge: Color,
    pub rail: Color,
    pub device: Color,
    pub clearance: Color,
    pub error: Color,
    pub warning: Color,
    pub tag_height: f32,
}

impl Default for CabinetStyle {
    fn default() -> Self {
        Self {
            plate_edge: Color::rgb(150, 155, 165),
            usable_edge: Color::rgb(90, 96, 108),
            rail: Color::rgb(120, 126, 138),
            device: Color::rgb(70, 130, 200),
            clearance: Color::rgb(120, 130, 150),
            error: Color::rgb(215, 60, 60),
            warning: Color::rgb(190, 120, 20),
            tag_height: 3.5,
        }
    }
}

/// Layers of a cabinet scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CabinetLayers {
    pub plate: LayerId,
    pub clearance: LayerId,
    pub rails: LayerId,
    pub devices: LayerId,
}

/// Item id of the plate.
pub const PLATE_ITEM: ItemId = ItemId(1 << 40);

/// Item id of a rail.
pub fn rail_item(id: u64) -> ItemId {
    ItemId((2 << 40) | id)
}

/// Item id of a device box.
pub fn box_item(id: u64) -> ItemId {
    ItemId((3 << 40) | id)
}

/// Item id of a device's clearance outline.
pub fn clearance_item(id: u64) -> ItemId {
    ItemId((4 << 40) | id)
}

/// The device an item id belongs to, for picking.
pub fn box_of(item: ItemId) -> Option<u64> {
    (item.0 >> 40 == 3).then_some(item.0 & ((1 << 40) - 1))
}

pub struct CabinetScene {
    scene: Scene,
    layers: CabinetLayers,
    style: CabinetStyle,
}

impl CabinetScene {
    pub fn new(style: CabinetStyle) -> Self {
        let mut scene = Scene::new();
        let layers = CabinetLayers {
            plate: scene.add_layer(Layer::new("Plate", -10)),
            clearance: scene.add_layer(Layer::new("Clearance", 0)),
            rails: scene.add_layer(Layer::new("Rails", 5)),
            devices: scene.add_layer(Layer::new("Devices", 10)),
        };
        Self {
            scene,
            layers,
            style,
        }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn layers(&self) -> CabinetLayers {
        self.layers
    }

    pub fn style(&self) -> &CabinetStyle {
        &self.style
    }

    /// Replaces the drawing with `view`. Items that did not change keep
    /// their place in the scene.
    pub fn set(&mut self, view: &CabinetView) {
        let wanted = self.items(view);
        let stale: Vec<ItemId> = self
            .scene
            .iter()
            .map(|(id, _)| id)
            .filter(|id| !wanted.iter().any(|(w, _)| w == id))
            .collect();
        for id in stale {
            self.scene.remove(id);
        }
        for (id, item) in wanted {
            if self.scene.get(id) != Some(&item) {
                self.scene.insert(id, item);
            }
        }
    }

    fn items(&self, view: &CabinetView) -> Vec<(ItemId, Item)> {
        let s = &self.style;
        let l = &self.layers;
        let mut out = Vec::new();
        if let Some(p) = view.plate {
            let size = Vec2::new(p.width, p.height);
            let mut item = Item::new(l.plate)
                .with(Primitive::stroked(
                    Shape::rect(Vec2::ZERO, size),
                    Stroke::world(0.6, s.plate_edge),
                ))
                .unselectable();
            if p.margin > 0.0 {
                item.primitives.push(Primitive::stroked(
                    Shape::rect(Vec2::splat(p.margin), size - Vec2::splat(p.margin)),
                    Stroke::world(0.25, s.usable_edge),
                ));
            }
            out.push((PLATE_ITEM, item));
        }
        for r in &view.rails {
            let a = Vec2::new(r.x, r.y);
            let b = Vec2::new(r.x + r.length, r.y);
            // 35 mm DIN rail: two edges 35 mm apart, centred on the line.
            let (top, bottom) = (Vec2::new(0.0, -17.5), Vec2::new(0.0, 17.5));
            let stroke = Stroke::world(0.4, s.rail);
            let item = Item::new(l.rails)
                .with(Primitive::stroked(Shape::rect(a + top, b + bottom), stroke))
                .with(Primitive::stroked(
                    Shape::line(a, b),
                    Stroke::world(0.2, s.rail),
                ))
                .unselectable();
            out.push((rail_item(r.id), item));
        }
        for b in &view.boxes {
            out.push((box_item(b.id), self.device(b)));
            if b.clearance > 0.0 {
                out.push((clearance_item(b.id), self.clearance(b)));
            }
        }
        out
    }

    fn state_color(&self, state: BoxState) -> Color {
        match state {
            BoxState::Ok => self.style.device,
            BoxState::Collision | BoxState::Outside | BoxState::TooDeep => self.style.error,
            BoxState::TooClose => self.style.warning,
        }
    }

    fn device(&self, b: &BoxView) -> Item {
        let s = &self.style;
        let min = Vec2::new(b.x, b.y);
        let max = min + Vec2::new(b.width, b.height);
        let color = self.state_color(b.state);
        let stroke = Stroke::world(0.5, color);
        let mut item = Item::new(self.layers.devices);
        // The fill is a faint tint of the state colour.
        item.primitives.push(Primitive::filled(
            Shape::rect(min, max),
            color.with_alpha(40),
        ));
        if b.guessed {
            item.primitives
                .extend(dashed_rect(min, max, 3.0, 2.0, stroke));
        } else {
            item.primitives
                .push(Primitive::stroked(Shape::rect(min, max), stroke));
        }
        if !b.tag.is_empty() {
            item.labels.push(
                Label::new((min + max) * 0.5, b.tag.clone(), s.tag_height)
                    .anchored(Anchor::Center)
                    .colored(color),
            );
        }
        // The mark in the top right corner, so state never rests on colour.
        let m = (b.width.min(b.height) * 0.25).clamp(2.0, 6.0);
        let corner = Vec2::new(max.x - m * 1.5, min.y + m * 1.5);
        let mark = Stroke::world(0.6, color);
        match b.state {
            BoxState::Ok => {}
            BoxState::Collision | BoxState::Outside => {
                let d = Vec2::splat(m * 0.5);
                item.primitives.push(Primitive::stroked(
                    Shape::line(corner - d, corner + d),
                    mark,
                ));
                item.primitives.push(Primitive::stroked(
                    Shape::line(corner + Vec2::new(-d.x, d.y), corner + Vec2::new(d.x, -d.y)),
                    mark,
                ));
            }
            BoxState::TooDeep => {
                // A triangle pointing at the viewer: it sticks out toward the door.
                let (l, r, t) = (
                    corner + Vec2::new(-m * 0.5, -m * 0.45),
                    corner + Vec2::new(m * 0.5, -m * 0.45),
                    corner + Vec2::new(0.0, m * 0.55),
                );
                for (a, b) in [(l, r), (r, t), (t, l)] {
                    item.primitives
                        .push(Primitive::stroked(Shape::line(a, b), mark));
                }
            }
            BoxState::TooClose => {
                item.primitives.push(Primitive::stroked(
                    Shape::line(
                        corner + Vec2::new(0.0, -m * 0.6),
                        corner + Vec2::new(0.0, m * 0.2),
                    ),
                    mark,
                ));
                item.primitives.push(Primitive::filled(
                    Shape::circle(corner + Vec2::new(0.0, m * 0.55), m * 0.12),
                    color,
                ));
            }
        }
        item
    }

    /// The free space wanted around a device: its footprint grown by the
    /// clearance, dotted and faint, and not selectable.
    fn clearance(&self, b: &BoxView) -> Item {
        let grow = Vec2::splat(b.clearance);
        let min = Vec2::new(b.x, b.y) - grow;
        let max = Vec2::new(b.x + b.width, b.y + b.height) + grow;
        let mut item = Item::new(self.layers.clearance).unselectable();
        item.primitives.extend(dashed_rect(
            min,
            max,
            1.0,
            1.5,
            Stroke::world(0.2, self.style.clearance),
        ));
        item
    }
}

/// A rectangle's outline cut into dashes.
fn dashed_rect(min: Vec2, max: Vec2, dash: f32, gap: f32, stroke: Stroke) -> Vec<Primitive> {
    let c = [min, Vec2::new(max.x, min.y), max, Vec2::new(min.x, max.y)];
    let mut out = Vec::new();
    for i in 0..4 {
        let (a, b) = (c[i], c[(i + 1) % 4]);
        let len = a.distance(b);
        if len <= 0.0 {
            continue;
        }
        let dir = (b - a) / len;
        let mut t = 0.0;
        while t < len {
            let e = (t + dash).min(len);
            out.push(Primitive::stroked(
                Shape::line(a + dir * t, a + dir * e),
                stroke,
            ));
            t += dash + gap;
        }
    }
    out
}

/// Bounds of the plate and everything on it, for fitting the view.
pub fn view_bounds(view: &CabinetView) -> Aabb {
    let mut b = Aabb::EMPTY;
    if let Some(p) = view.plate {
        b = b.union(&Aabb::new(Vec2::ZERO, Vec2::new(p.width, p.height)));
    }
    for r in &view.rails {
        b = b.union(&Aabb::new(
            Vec2::new(r.x, r.y - 17.5),
            Vec2::new(r.x + r.length, r.y + 17.5),
        ));
    }
    for d in &view.boxes {
        b = b.union(&Aabb::new(
            Vec2::new(d.x, d.y),
            Vec2::new(d.x + d.width, d.y + d.height),
        ));
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: u64, x: f32, state: BoxState) -> BoxView {
        BoxView {
            id,
            x,
            y: 100.0,
            width: 45.0,
            height: 90.0,
            tag: format!("-K{id}"),
            guessed: false,
            clearance: 10.0,
            state,
        }
    }

    fn view() -> CabinetView {
        CabinetView {
            plate: Some(PlateView {
                width: 600.0,
                height: 800.0,
                margin: 20.0,
            }),
            rails: vec![RailView {
                id: 1,
                x: 40.0,
                y: 150.0,
                length: 500.0,
            }],
            boxes: vec![
                device(1, 60.0, BoxState::Ok),
                device(2, 120.0, BoxState::Ok),
            ],
        }
    }

    #[test]
    fn plate_rails_devices_and_clearances_are_items() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        c.set(&view());
        let s = c.scene();
        assert!(s.get(PLATE_ITEM).is_some());
        assert!(s.get(rail_item(1)).is_some());
        for id in [1, 2] {
            assert!(s.get(box_item(id)).is_some());
            assert!(s.get(clearance_item(id)).is_some());
        }
        assert_eq!(s.iter().count(), 1 + 1 + 2 + 2);
        // The plate, rails and clearances never take a click.
        assert!(!s.get(PLATE_ITEM).unwrap().selectable);
        assert!(!s.get(rail_item(1)).unwrap().selectable);
        assert!(!s.get(clearance_item(1)).unwrap().selectable);
        assert!(s.get(box_item(1)).unwrap().selectable);
        // The tag is the box's label.
        assert_eq!(s.get(box_item(2)).unwrap().labels[0].text, "-K2");
    }

    #[test]
    fn a_click_on_a_device_picks_it() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        c.set(&view());
        let hit = c.scene().pick(Vec2::new(80.0, 120.0), 1.0, 1.0).unwrap();
        assert_eq!(box_of(hit.id), Some(1));
        assert_eq!(box_of(PLATE_ITEM), None);
    }

    #[test]
    fn problems_show_a_mark_as_well_as_a_colour() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        let mut v = view();
        v.boxes[0].state = BoxState::Collision;
        v.boxes[1].state = BoxState::TooClose;
        let mut deep = v.boxes[0].clone();
        deep.id = 3;
        deep.x = 200.0;
        deep.state = BoxState::TooDeep;
        v.boxes.push(deep);
        c.set(&v);
        let ok = {
            let mut c = CabinetScene::new(CabinetStyle::default());
            c.set(&view());
            c.scene().get(box_item(1)).unwrap().primitives.len()
        };
        let s = c.scene();
        // A cross is two lines, a bang a line and a dot, over fill + outline.
        assert_eq!(s.get(box_item(1)).unwrap().primitives.len(), ok + 2);
        assert_eq!(s.get(box_item(2)).unwrap().primitives.len(), ok + 2);
        // Too deep is a triangle: three lines.
        assert_eq!(s.get(box_item(3)).unwrap().primitives.len(), ok + 3);
        let colour = |id| s.get(box_item(id)).unwrap().labels[0].color;
        assert_ne!(colour(1), colour(2));
        assert_eq!(colour(1), CabinetStyle::default().error);
        assert_eq!(colour(2), CabinetStyle::default().warning);
        assert_eq!(colour(3), CabinetStyle::default().error);
    }

    #[test]
    fn a_guessed_size_is_dashed() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        let mut v = view();
        v.boxes[1].guessed = true;
        c.set(&v);
        let s = c.scene();
        assert!(
            s.get(box_item(2)).unwrap().primitives.len()
                > s.get(box_item(1)).unwrap().primitives.len() + 10
        );
    }

    #[test]
    fn clearance_grows_the_footprint() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        c.set(&view());
        let b = c.scene().item_bounds(clearance_item(1)).unwrap();
        assert!((b.min - Vec2::new(50.0, 90.0)).length() < 0.5);
        assert!((b.max - Vec2::new(115.0, 200.0)).length() < 0.5);
    }

    #[test]
    fn set_updates_in_place_and_drops_what_left() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        c.set(&view());
        let rev = c.scene().revision();
        c.set(&view());
        assert_eq!(c.scene().revision(), rev, "an equal view changes nothing");
        let mut v = view();
        v.boxes.remove(1);
        v.boxes[0].x = 70.0;
        c.set(&v);
        assert!(c.scene().get(box_item(2)).is_none());
        assert!(c.scene().get(clearance_item(2)).is_none());
        assert!(c.scene().revision() > rev);
        assert_eq!(c.scene().get(box_item(1)).unwrap().labels[0].pos.x, 92.5);
    }

    #[test]
    fn view_bounds_cover_plate_rails_and_devices() {
        let b = view_bounds(&view());
        assert_eq!((b.min, b.max), (Vec2::ZERO, Vec2::new(600.0, 800.0)));
        assert!(view_bounds(&CabinetView::default()).is_empty());
    }
}
