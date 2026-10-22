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

/// A DIN rail: its centre line starts at `(x, y)` and runs for `length` to
/// the right, or down when `vertical`; the 35 mm band is centred on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailView {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub length: f32,
    pub vertical: bool,
}

/// A wiring duct: a filled strip, `width` across, running from `(x, y)` for
/// `length` to the right, or down when `vertical`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DuctView {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub length: f32,
    pub width: f32,
    pub vertical: bool,
}

impl DuctView {
    /// Corners of the strip. `(x, y)` is the start of the centre line, as
    /// for a rail; the strip is centred across the run.
    fn corners(&self) -> (Vec2, Vec2) {
        let half = self.width / 2.0;
        if self.vertical {
            (
                Vec2::new(self.x - half, self.y),
                Vec2::new(self.x + half, self.y + self.length),
            )
        } else {
            (
                Vec2::new(self.x, self.y - half),
                Vec2::new(self.x + self.length, self.y + half),
            )
        }
    }
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
    pub ducts: Vec<DuctView>,
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
            // 35 mm DIN rail (TH35): a faint band between two edges 35 mm
            // apart, with the 25 mm wide mounting slot down its middle. The
            // slot reads as "rail" where a plain outline read as a box.
            let (a, b, across) = if r.vertical {
                (
                    Vec2::new(r.x, r.y),
                    Vec2::new(r.x, r.y + r.length),
                    Vec2::new(1.0, 0.0),
                )
            } else {
                (
                    Vec2::new(r.x, r.y),
                    Vec2::new(r.x + r.length, r.y),
                    Vec2::new(0.0, 1.0),
                )
            };
            let (top, bottom) = (across * -17.5, across * 17.5);
            let stroke = Stroke::world(0.4, s.rail);
            let slot = across * 2.5;
            let item = Item::new(l.rails)
                .with(Primitive::filled(
                    Shape::rect(a + top, b + bottom),
                    s.rail.with_alpha(26),
                ))
                .with(Primitive::stroked(Shape::rect(a + top, b + bottom), stroke))
                .with(Primitive::stroked(
                    Shape::rect(a - slot, b + slot),
                    Stroke::world(0.25, s.rail.with_alpha(150)),
                ))
                .unselectable();
            out.push((rail_item(r.id), item));
        }
        for d in &view.ducts {
            let (min, max) = d.corners();
            // A wiring duct: a filled strip with the comb of finger slots
            // along both long sides, so it never reads as a second rail.
            let mut item = Item::new(l.rails)
                .with(Primitive::filled(
                    Shape::rect(min, max),
                    s.rail.with_alpha(48),
                ))
                .with(Primitive::stroked(
                    Shape::rect(min, max),
                    Stroke::world(0.4, s.rail),
                ))
                .unselectable();
            let slats = Stroke::world(0.2, s.rail.with_alpha(160));
            let inset = (d.width * 0.28).clamp(2.0, 8.0);
            let pitch = 8.0;
            let n = ((d.length / pitch) as usize).min(400);
            for i in 1..n {
                let t = i as f32 * pitch;
                let (p0, p1, p2, p3) = if d.vertical {
                    (
                        Vec2::new(min.x, min.y + t),
                        Vec2::new(min.x + inset, min.y + t),
                        Vec2::new(max.x - inset, min.y + t),
                        Vec2::new(max.x, min.y + t),
                    )
                } else {
                    (
                        Vec2::new(min.x + t, min.y),
                        Vec2::new(min.x + t, min.y + inset),
                        Vec2::new(min.x + t, max.y - inset),
                        Vec2::new(min.x + t, max.y),
                    )
                };
                item.primitives
                    .push(Primitive::stroked(Shape::line(p0, p1), slats));
                item.primitives
                    .push(Primitive::stroked(Shape::line(p2, p3), slats));
            }
            out.push((rail_item(d.id), item));
        }
        // On a big plate fitted to the window a 3.5 mm tag is under 2 points
        // high and is dropped as unreadable, so the tag grows with the plate.
        let tag_height = view.plate.map_or(s.tag_height, |p| {
            s.tag_height.max(p.width.max(p.height) / 200.0)
        });
        for b in &view.boxes {
            out.push((box_item(b.id), self.device(b, tag_height)));
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

    fn device(&self, b: &BoxView, tag_height: f32) -> Item {
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
        item.primitives.extend(front_details(
            BoxKind::of_tag(&b.tag),
            min,
            max,
            Stroke::world(0.25, color.with_alpha(170)),
            color.with_alpha(170),
        ));
        if !b.tag.is_empty() {
            // A tall device carries its tag along its height: at "fit" the
            // box is a few dozen points wide, so a horizontal tag meets the
            // neighbour's and is hidden, a vertical one has the room.
            let tall = b.height > b.width * 1.2;
            let label = Label::new((min + max) * 0.5, b.tag.clone(), tag_height)
                .anchored(Anchor::Center)
                .colored(color);
            item.labels.push(if tall {
                label.rotated(-std::f32::consts::FRAC_PI_2)
            } else {
                label
            });
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

/// The kind of device a box is, read from the letter code of its tag
/// (IEC 81346): what the typical front drawn on the box looks like. It is a
/// typical front of the family, not the maker's drawing; the box itself is the
/// part's real footprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxKind {
    /// Circuit breakers, fuses and isolators (`Q`, `F`).
    Breaker,
    /// Contactors, relays and timers (`K`).
    Contactor,
    /// Terminal strips (`X`).
    Terminal,
    /// Power supplies and transformers (`G`, `T`).
    Supply,
    /// PLC parts and drives (`A`, `U`, `V`).
    Controller,
    /// Anything else: a plain box.
    Plain,
}

impl BoxKind {
    /// The kind of a device from its tag (`=A1+CAB-K1`, `-Q2`): the letters
    /// after the last `-`.
    pub fn of_tag(tag: &str) -> Self {
        let name = tag.rsplit('-').next().unwrap_or(tag);
        let letters: String = name
            .chars()
            .take_while(|c| c.is_ascii_alphabetic())
            .collect::<String>()
            .to_ascii_uppercase();
        match letters.as_str() {
            "Q" | "F" => Self::Breaker,
            "K" | "KA" | "KM" | "KT" => Self::Contactor,
            "X" => Self::Terminal,
            "G" | "T" => Self::Supply,
            "A" | "U" | "V" => Self::Controller,
            _ => Self::Plain,
        }
    }
}

/// The typical front of a device inside its box: terminal screws, a lever or a
/// label window, vent slots, drawn from the box's own size so it never leaves
/// it. Boxes too small for details get none.
fn front_details(kind: BoxKind, min: Vec2, max: Vec2, line: Stroke, fill: Color) -> Vec<Primitive> {
    let (w, h) = (max.x - min.x, max.y - min.y);
    if kind == BoxKind::Plain || w < 12.0 || h < 20.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let centre = (min + max) * 0.5;
    let screw = |at: Vec2, r: f32, out: &mut Vec<Primitive>| {
        out.push(Primitive::stroked(Shape::circle(at, r), line));
        out.push(Primitive::stroked(
            Shape::line(at - Vec2::new(r * 0.7, 0.0), at + Vec2::new(r * 0.7, 0.0)),
            line,
        ));
    };
    // A row of terminal screws across the top and bottom, one per `pitch`.
    let screw_rows = |pitch: f32, inset: f32, out: &mut Vec<Primitive>| {
        let n = ((w / pitch).round() as usize).max(1);
        let r = (pitch * 0.22).clamp(1.2, 3.0);
        for i in 0..n {
            let x = min.x + w * (i as f32 + 0.5) / n as f32;
            screw(Vec2::new(x, min.y + inset), r, out);
            screw(Vec2::new(x, max.y - inset), r, out);
        }
    };
    match kind {
        BoxKind::Breaker => {
            // One pole per 18 mm: screws top and bottom, a stepped front
            // with the lever of the handle in the middle.
            screw_rows(18.0, 6.0, &mut out);
            let lever = Vec2::new((w * 0.55).min(24.0), 12.0_f32.min(h * 0.2)) * 0.5;
            out.push(Primitive::stroked(
                Shape::rect(centre - lever, centre + lever),
                line,
            ));
            out.push(Primitive::filled(
                Shape::rect(
                    Vec2::new(centre.x - lever.x * 0.9, centre.y - lever.y * 0.9),
                    Vec2::new(centre.x, centre.y + lever.y * 0.9),
                ),
                fill.with_alpha(60),
            ));
            let step = h * 0.2;
            for y in [min.y + step, max.y - step] {
                out.push(Primitive::stroked(
                    Shape::line(Vec2::new(min.x, y), Vec2::new(max.x, y)),
                    line,
                ));
            }
        }
        BoxKind::Contactor => {
            // Three power poles over a label window and a coil row.
            screw_rows(15.0, 6.0, &mut out);
            let win = Vec2::new(w * 0.34, h * 0.12);
            out.push(Primitive::stroked(
                Shape::rect(centre - win, centre + win),
                line,
            ));
            let test = Vec2::splat((w * 0.09).max(1.5));
            let at = Vec2::new(max.x - w * 0.2, centre.y + h * 0.28);
            out.push(Primitive::stroked(Shape::rect(at - test, at + test), line));
            for y in [min.y + h * 0.17, max.y - h * 0.17] {
                out.push(Primitive::stroked(
                    Shape::line(Vec2::new(min.x, y), Vec2::new(max.x, y)),
                    line,
                ));
            }
        }
        BoxKind::Terminal => {
            // Blocks side by side, 6 mm each: a rib between two, a screw on each.
            let n = ((w / 6.0).round() as usize).clamp(1, 40);
            for i in 0..n {
                let x0 = min.x + w * i as f32 / n as f32;
                let x1 = min.x + w * (i as f32 + 1.0) / n as f32;
                if i > 0 {
                    out.push(Primitive::stroked(
                        Shape::line(Vec2::new(x0, min.y), Vec2::new(x0, max.y)),
                        line,
                    ));
                }
                let r = ((x1 - x0) * 0.22).clamp(0.6, 2.0);
                screw(Vec2::new((x0 + x1) * 0.5, min.y + h * 0.28), r, &mut out);
                screw(Vec2::new((x0 + x1) * 0.5, max.y - h * 0.28), r, &mut out);
            }
        }
        BoxKind::Supply => {
            // Terminals at both ends and vent slots across the body.
            screw_rows(14.0, 6.0, &mut out);
            let slots = 5;
            for i in 0..slots {
                let y = min.y + h * (0.3 + 0.4 * i as f32 / (slots - 1) as f32);
                out.push(Primitive::stroked(
                    Shape::line(Vec2::new(min.x + w * 0.2, y), Vec2::new(max.x - w * 0.2, y)),
                    line,
                ));
            }
        }
        BoxKind::Controller => {
            // A status LED column and a label window between two terminal rows.
            screw_rows(10.0, 5.0, &mut out);
            for i in 0..4 {
                let y = min.y + h * (0.28 + 0.1 * i as f32);
                out.push(Primitive::stroked(
                    Shape::circle(Vec2::new(min.x + w * 0.22, y), (w * 0.05).clamp(0.8, 1.6)),
                    line,
                ));
            }
            let win = Vec2::new(w * 0.3, h * 0.1);
            let at = Vec2::new(centre.x + w * 0.08, centre.y + h * 0.18);
            out.push(Primitive::stroked(Shape::rect(at - win, at + win), line));
        }
        BoxKind::Plain => {}
    }
    out
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
        let (min, max) = if r.vertical {
            (
                Vec2::new(r.x - 17.5, r.y),
                Vec2::new(r.x + 17.5, r.y + r.length),
            )
        } else {
            (
                Vec2::new(r.x, r.y - 17.5),
                Vec2::new(r.x + r.length, r.y + 17.5),
            )
        };
        b = b.union(&Aabb::new(min, max));
    }
    for d in &view.ducts {
        let (min, max) = d.corners();
        b = b.union(&Aabb::new(min, max));
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
                vertical: false,
            }],
            ducts: vec![DuctView {
                id: 9,
                x: 40.0,
                y: 300.0,
                length: 500.0,
                width: 30.0,
                vertical: false,
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
        assert_eq!(s.iter().count(), 1 + 1 + 1 + 2 + 2);
        // The plate, rails and clearances never take a click.
        assert!(!s.get(PLATE_ITEM).unwrap().selectable);
        assert!(!s.get(rail_item(1)).unwrap().selectable);
        assert!(!s.get(clearance_item(1)).unwrap().selectable);
        assert!(s.get(box_item(1)).unwrap().selectable);
        // The tag is the box's label.
        assert_eq!(s.get(box_item(2)).unwrap().labels[0].text, "-K2");
    }

    #[test]
    fn a_tall_device_carries_its_tag_along_its_height() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        let mut v = view();
        v.boxes[0].width = 90.0;
        v.boxes[0].height = 45.0;
        c.set(&v);
        let wide = &c.scene().get(box_item(v.boxes[0].id)).unwrap().labels[0];
        assert_eq!(wide.rotation, 0.0);
        let tall = &c.scene().get(box_item(v.boxes[1].id)).unwrap().labels[0];
        assert!(tall.rotation < 0.0);
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

    #[test]
    fn a_duct_is_an_unselectable_strip_and_widens_the_bounds() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        let mut v = view();
        c.set(&v);
        let duct = c.scene().get(rail_item(9)).unwrap();
        assert!(!duct.selectable);
        // Centred on its line: the horizontal one at y 300 spans 285..315.
        assert_eq!(v.ducts[0].corners().0.y, 285.0);
        // A vertical duct swaps its sides and may stick out of the plate.
        v.ducts[0].vertical = true;
        v.ducts[0].length = 900.0;
        let b = view_bounds(&v);
        assert_eq!(b.max.y, 1200.0);
        assert_eq!(
            v.ducts[0].corners(),
            (Vec2::new(25.0, 300.0), Vec2::new(55.0, 1200.0))
        );
    }

    #[test]
    fn a_rail_has_a_slot_and_a_duct_a_comb_of_slats() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        let v = view();
        c.set(&v);
        let rail = c.scene().get(rail_item(1)).unwrap();
        // Fill, outline and the mounting slot.
        assert_eq!(rail.primitives.len(), 3);
        let duct = c.scene().get(rail_item(9)).unwrap();
        // Fill and outline, then two slats per 8 mm of length.
        let slats = ((v.ducts[0].length / 8.0) as usize).saturating_sub(1) * 2;
        assert_eq!(duct.primitives.len(), 2 + slats);
    }

    #[test]
    fn a_very_long_duct_is_capped_at_four_hundred_slats() {
        let mut c = CabinetScene::new(CabinetStyle::default());
        let mut v = view();
        v.ducts[0].length = 100_000.0;
        c.set(&v);
        let duct = c.scene().get(rail_item(9)).unwrap();
        assert_eq!(duct.primitives.len(), 2 + 399 * 2);
    }

    #[test]
    fn the_kind_comes_from_the_letters_of_the_tag() {
        assert_eq!(BoxKind::of_tag("=A1+CAB-K1"), BoxKind::Contactor);
        assert_eq!(BoxKind::of_tag("-Q12"), BoxKind::Breaker);
        assert_eq!(BoxKind::of_tag("-F3"), BoxKind::Breaker);
        assert_eq!(BoxKind::of_tag("-X1"), BoxKind::Terminal);
        assert_eq!(BoxKind::of_tag("-T1"), BoxKind::Supply);
        assert_eq!(BoxKind::of_tag("-S0"), BoxKind::Plain);
        assert_eq!(BoxKind::of_tag(""), BoxKind::Plain);
    }

    #[test]
    fn a_front_stays_inside_its_box() {
        let line = Stroke::world(0.25, Color::rgb(1, 2, 3));
        for kind in [
            BoxKind::Breaker,
            BoxKind::Contactor,
            BoxKind::Terminal,
            BoxKind::Supply,
            BoxKind::Controller,
        ] {
            for (w, h) in [
                (18.0_f32, 90.0_f32),
                (45.0, 85.0),
                (54.0, 90.0),
                (90.0, 110.0),
            ] {
                let (min, max) = (Vec2::new(100.0, 200.0), Vec2::new(100.0 + w, 200.0 + h));
                let prims = front_details(kind, min, max, line, Color::rgb(1, 2, 3));
                assert!(!prims.is_empty(), "{kind:?} {w}x{h} has a front");
                for p in &prims {
                    let b = p.shape.bounds();
                    assert!(
                        b.min.x >= min.x - 1e-3
                            && b.min.y >= min.y - 1e-3
                            && b.max.x <= max.x + 1e-3
                            && b.max.y <= max.y + 1e-3,
                        "{kind:?} {w}x{h}: {b:?}"
                    );
                }
            }
        }
        // Too small for details, or a plain box: nothing.
        let small = front_details(
            BoxKind::Breaker,
            Vec2::ZERO,
            Vec2::new(8.0, 30.0),
            line,
            Color::rgb(1, 2, 3),
        );
        assert!(small.is_empty());
    }

    #[test]
    fn a_vertical_rail_is_a_band_down_the_plate() {
        let mut v = view();
        v.rails[0].vertical = true;
        v.rails[0].x = 100.0;
        v.rails[0].y = 50.0;
        v.rails[0].length = 400.0;
        let b = view_bounds(&v);
        assert!(b.min.x <= 100.0 - 17.5 + 1e-3 && b.max.y >= 450.0);
        let mut c = CabinetScene::new(CabinetStyle::default());
        c.set(&v);
        assert!(c.scene().get(rail_item(1)).is_some());
    }
}
