//! Generated demo content: schematic-like pages for the stress test, the
//! app's placeholder page and benchmarks. Not real symbols; WS-05 owns those.

use crate::scene::{Item, ItemId, Layer, LayerId, Scene, SnapPoint};
use crate::shape::{Placement, Primitive, Shape};
use crate::style::{Color, Stroke};
use glam::Vec2;
use std::f32::consts::{FRAC_PI_2, PI};

const INK: Color = Color::rgb(20, 24, 32);
const WIRE: Color = Color::rgb(30, 70, 200);
const FRAME: Color = Color::rgb(90, 90, 90);

fn ink() -> Stroke {
    Stroke::world(0.35, INK)
}

/// Symbol shapes in local coordinates, about 8 x 16 mm, pins at (0, 0) and
/// (0, 16).
fn symbol(kind: usize) -> (Vec<Primitive>, Vec<Vec2>) {
    let pins = vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 16.0)];
    let s = ink();
    let p = match kind % 5 {
        // Coil: box with leads.
        0 => vec![
            Primitive::stroked(Shape::line(Vec2::ZERO, Vec2::new(0.0, 5.0)), s),
            Primitive::stroked(Shape::rect(Vec2::new(-4.0, 5.0), Vec2::new(4.0, 11.0)), s),
            Primitive::stroked(Shape::line(Vec2::new(0.0, 11.0), Vec2::new(0.0, 16.0)), s),
        ],
        // Normally open contact.
        1 => vec![
            Primitive::stroked(Shape::line(Vec2::ZERO, Vec2::new(0.0, 5.0)), s),
            Primitive::stroked(Shape::line(Vec2::new(-3.0, 6.0), Vec2::new(0.0, 11.0)), s),
            Primitive::stroked(Shape::line(Vec2::new(0.0, 11.0), Vec2::new(0.0, 16.0)), s),
        ],
        // Normally closed contact.
        2 => vec![
            Primitive::stroked(Shape::line(Vec2::ZERO, Vec2::new(0.0, 5.0)), s),
            Primitive::stroked(Shape::line(Vec2::new(3.0, 5.0), Vec2::new(-1.0, 11.0)), s),
            Primitive::stroked(Shape::line(Vec2::new(0.0, 5.0), Vec2::new(3.0, 5.0)), s),
            Primitive::stroked(Shape::line(Vec2::new(0.0, 11.0), Vec2::new(0.0, 16.0)), s),
        ],
        // Motor.
        3 => vec![
            Primitive::stroked(Shape::line(Vec2::ZERO, Vec2::new(0.0, 3.0)), s),
            Primitive::stroked(Shape::circle(Vec2::new(0.0, 8.0), 5.0), s)
                .with_fill(Color::rgb(245, 247, 250)),
            Primitive::stroked(
                Shape::polyline(
                    [
                        Vec2::new(-2.0, 10.0),
                        Vec2::new(-2.0, 6.0),
                        Vec2::new(0.0, 8.5),
                        Vec2::new(2.0, 6.0),
                        Vec2::new(2.0, 10.0),
                    ],
                    false,
                ),
                s,
            ),
            Primitive::stroked(Shape::line(Vec2::new(0.0, 13.0), Vec2::new(0.0, 16.0)), s),
        ],
        // Lamp with an arc-drawn cross.
        _ => vec![
            Primitive::stroked(Shape::line(Vec2::ZERO, Vec2::new(0.0, 4.0)), s),
            Primitive::stroked(
                Shape::Arc {
                    center: Vec2::new(0.0, 8.0),
                    radius: 4.0,
                    start: -FRAC_PI_2,
                    sweep: PI * 2.0,
                },
                s,
            ),
            Primitive::stroked(Shape::line(Vec2::new(-2.8, 5.2), Vec2::new(2.8, 10.8)), s),
            Primitive::stroked(Shape::line(Vec2::new(2.8, 5.2), Vec2::new(-2.8, 10.8)), s),
            Primitive::stroked(Shape::line(Vec2::new(0.0, 12.0), Vec2::new(0.0, 16.0)), s),
        ],
    };
    (p, pins)
}

/// Layers used by the demo pages.
pub struct DemoLayers {
    pub frame: LayerId,
    pub wires: LayerId,
    pub symbols: LayerId,
}

fn add_layers(scene: &mut Scene) -> DemoLayers {
    DemoLayers {
        frame: scene.add_layer(Layer::new("Frame", -10)),
        wires: scene.add_layer(Layer::new("Wires", 0)),
        symbols: scene.add_layer(Layer::new("Symbols", 10)),
    }
}

/// A grid of symbols with wires between neighbours: `objects` items in
/// total (half symbols, half wires, plus a frame), laid out on 4 mm grid.
pub fn stress_scene(objects: usize) -> Scene {
    let mut scene = Scene::new();
    let layers = add_layers(&mut scene);
    let symbols = objects.div_ceil(2).max(1);
    let cols = (symbols as f32).sqrt().ceil() as usize;
    let pitch = Vec2::new(24.0, 32.0);
    let mut id = 0u64;
    let mut next = || {
        id += 1;
        ItemId(id)
    };
    let mut placed = 0;
    let mut wires = 0;
    let wire_budget = objects.saturating_sub(symbols);
    for i in 0..symbols {
        let (c, r) = (i % cols, i / cols);
        let origin = Vec2::new(c as f32 * pitch.x, r as f32 * pitch.y);
        let (prims, pins) = symbol(i);
        let place = Placement {
            offset: origin,
            mirror: (i / 7) % 2 == 1,
            ..Default::default()
        };
        let mut item = Item::new(layers.symbols);
        item.primitives = prims.iter().map(|p| p.transformed(&place)).collect();
        item.snap_points = pins
            .iter()
            .map(|p| SnapPoint::connection(place.affine().transform_point2(*p)))
            .collect();
        scene.insert(next(), item);
        placed += 1;
        // Wire from this symbol's bottom pin to the next row's top pin,
        // with a dog-leg to the right so it is a real polyline.
        if wires < wire_budget {
            let a = origin + Vec2::new(0.0, 16.0);
            let b = origin + Vec2::new(0.0, pitch.y);
            let mid = (a.y + b.y) * 0.5;
            let pts = if c + 1 < cols && i % 3 == 0 {
                vec![
                    a,
                    Vec2::new(a.x, mid),
                    Vec2::new(a.x + pitch.x, mid),
                    b + Vec2::X * pitch.x,
                ]
            } else {
                vec![a, b]
            };
            let end = *pts.last().unwrap();
            let mut w = Item::new(layers.wires).with(Primitive::stroked(
                Shape::polyline(pts, false),
                Stroke::world(0.35, WIRE),
            ));
            w.snap_points = vec![SnapPoint::connection(a), SnapPoint::connection(end)];
            if i % 3 == 0 && c + 1 < cols {
                w.primitives.push(Primitive::filled(
                    Shape::circle(Vec2::new(a.x, mid), 0.8),
                    WIRE,
                ));
            }
            scene.insert(next(), w);
            wires += 1;
        }
    }
    let _ = placed;
    let b = scene.bounds().expand(12.0);
    scene.insert(
        next(),
        Item::new(layers.frame)
            .with(Primitive::stroked(
                Shape::rect(b.min, b.max),
                Stroke::world(0.7, FRAME),
            ))
            .with(Primitive::stroked(
                Shape::rect(b.min + Vec2::splat(5.0), b.max - Vec2::splat(5.0)),
                Stroke::world(0.25, FRAME),
            ))
            .unselectable(),
    );
    scene
}

/// A small A3 page with a few symbols: what the app shows until the
/// schematic editor (WS-06) draws real pages.
pub fn sample_page() -> Scene {
    let mut scene = Scene::new();
    let layers = add_layers(&mut scene);
    let page = Vec2::new(420.0, 297.0);
    scene.insert(
        ItemId(1),
        Item::new(layers.frame)
            .with(Primitive::stroked(
                Shape::rect(Vec2::ZERO, page),
                Stroke::world(0.5, FRAME),
            ))
            .with(Primitive::stroked(
                Shape::rect(Vec2::splat(10.0), page - Vec2::splat(10.0)),
                Stroke::world(0.35, FRAME),
            ))
            .with(Primitive::stroked(
                Shape::rect(Vec2::new(250.0, 247.0), page - Vec2::splat(10.0)),
                Stroke::world(0.35, FRAME),
            ))
            .unselectable(),
    );
    let mut id = 10;
    for (k, x) in [60.0, 100.0, 140.0, 180.0, 220.0].into_iter().enumerate() {
        let (prims, pins) = symbol(k);
        let place = Placement::at(Vec2::new(x, 100.0));
        let mut item = Item::new(layers.symbols);
        item.primitives = prims.iter().map(|p| p.transformed(&place)).collect();
        item.snap_points = pins
            .iter()
            .map(|p| SnapPoint::connection(place.affine().transform_point2(*p)))
            .collect();
        scene.insert(ItemId(id), item);
        id += 1;
        for (a, b) in [
            (Vec2::new(x, 60.0), Vec2::new(x, 100.0)),
            (Vec2::new(x, 116.0), Vec2::new(x, 160.0)),
        ] {
            scene.insert(
                ItemId(id),
                Item::new(layers.wires)
                    .with(Primitive::stroked(
                        Shape::line(a, b),
                        Stroke::world(0.35, WIRE),
                    ))
                    .with_snap(SnapPoint::connection(a))
                    .with_snap(SnapPoint::connection(b)),
            );
            id += 1;
        }
    }
    for y in [60.0, 160.0] {
        scene.insert(
            ItemId(id),
            Item::new(layers.wires).with(Primitive::stroked(
                Shape::line(Vec2::new(40.0, y), Vec2::new(240.0, y)),
                Stroke::world(0.35, WIRE),
            )),
        );
        id += 1;
    }
    scene
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_scene_has_requested_count() {
        let s = stress_scene(10_000);
        // 10,000 objects plus the frame.
        assert_eq!(s.len(), 10_001);
        assert!(!s.bounds().is_empty());
    }

    #[test]
    fn sample_page_fits_a3() {
        let s = sample_page();
        let b = s.bounds();
        assert!(b.max.x <= 420.5 && b.max.y <= 297.5);
    }
}
