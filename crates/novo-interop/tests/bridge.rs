//! Page scene to DXF (WS-24).

use std::f32::consts::FRAC_PI_2;

use novo_canvas::model::{ModelStyle, PlaceholderArt};
use novo_canvas::{
    Anchor, Color, Item, ItemId, Label, Layer, Primitive, Scene, Shape, Stroke, Vec2,
};
use novo_core::Document;
use novo_interop::bridge::{page_drawing, scene_to_drawing};
use novo_interop::dxf::{EntityKind, HAlign, VAlign, read_dxf, write_dxf};
use novo_tests::motor_starter;

fn black() -> Stroke {
    Stroke::world(0.25, Color::BLACK)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-4
}

fn scene_with(item: impl FnOnce(&mut Item)) -> Scene {
    let mut scene = Scene::new();
    let layer = scene.add_layer(Layer::new("Wires", 0));
    let mut it = Item::new(layer);
    item(&mut it);
    scene.insert(ItemId(1), it);
    scene
}

#[test]
fn y_is_mirrored_about_the_page_height() {
    let scene = scene_with(|it| {
        it.primitives.push(Primitive::stroked(
            Shape::line(Vec2::new(0.0, 0.0), Vec2::new(10.0, 20.0)),
            black(),
        ))
    });
    let d = scene_to_drawing(&scene, 100.0);
    match &d.entities[0].kind {
        EntityKind::Line { a, b } => {
            assert!(near(a.x, 0.0) && near(a.y, 100.0));
            assert!(near(b.x, 10.0) && near(b.y, 80.0));
        }
        other => panic!("expected a line, got {other:?}"),
    }
    assert_eq!(d.entities[0].layer, "Wires");
    assert_eq!(d.layer("Wires").unwrap().color, 3);
}

#[test]
fn a_clockwise_quarter_arc_becomes_counter_clockwise_after_the_flip() {
    // From +x towards +y (down the page) is 3 o'clock to 6 o'clock; with y
    // up that is 0 degrees back to -90, so CCW from 270 to 0.
    let scene = scene_with(|it| {
        it.primitives.push(Primitive::stroked(
            Shape::Arc {
                center: Vec2::new(50.0, 50.0),
                radius: 5.0,
                start: 0.0,
                sweep: FRAC_PI_2,
            },
            black(),
        ))
    });
    let d = scene_to_drawing(&scene, 100.0);
    match &d.entities[0].kind {
        EntityKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            assert!(near(center.x, 50.0) && near(center.y, 50.0) && near(*radius, 5.0));
            assert!(near(*start_deg, 270.0), "start {start_deg}");
            assert!(near(end_deg.rem_euclid(360.0), 0.0), "end {end_deg}");
        }
        other => panic!("expected an arc, got {other:?}"),
    }
}

#[test]
fn rects_become_closed_polylines_and_invisible_things_are_skipped() {
    let scene = scene_with(|it| {
        it.primitives.push(Primitive::stroked(
            Shape::Rect {
                min: Vec2::new(0.0, 0.0),
                max: Vec2::new(4.0, 2.0),
            },
            black(),
        ));
        // Fully transparent stroke, no fill: nothing to draw.
        it.primitives.push(Primitive::stroked(
            Shape::line(Vec2::ZERO, Vec2::ONE),
            Stroke::world(0.25, Color([0, 0, 0, 0])),
        ));
        // A filled dot with no outline still shows (junction dots).
        it.primitives.push(Primitive {
            shape: Shape::circle(Vec2::new(1.0, 1.0), 0.5),
            stroke: None,
            fill: Some(Color::BLACK),
        });
    });
    let d = scene_to_drawing(&scene, 10.0);
    assert_eq!(d.entities.len(), 2);
    assert!(matches!(
        &d.entities[0].kind,
        EntityKind::Polyline { points, closed: true } if points.len() == 4
    ));
    assert!(matches!(d.entities[1].kind, EntityKind::Circle { .. }));
}

#[test]
fn hidden_layers_are_left_out() {
    let mut scene = scene_with(|it| {
        it.primitives.push(Primitive::stroked(
            Shape::line(Vec2::ZERO, Vec2::ONE),
            black(),
        ))
    });
    let id = scene
        .layers()
        .iter()
        .position(|l| l.name == "Wires")
        .map(|i| novo_canvas::LayerId(i as u16))
        .unwrap();
    scene.layer_mut(id).unwrap().visible = false;
    let d = scene_to_drawing(&scene, 10.0);
    assert!(d.entities.is_empty());
    assert!(d.layer("Wires").is_none());
}

#[test]
fn labels_become_one_text_per_line_keeping_their_anchor() {
    let scene = scene_with(|it| {
        let mut l = Label::new(Vec2::new(10.0, 10.0), "-K1\n24 V", 2.0);
        l.anchor = Anchor::Center;
        it.labels.push(l);
    });
    let d = scene_to_drawing(&scene, 100.0);
    assert_eq!(d.entities.len(), 2);
    let texts: Vec<_> = d
        .entities
        .iter()
        .map(|e| match &e.kind {
            EntityKind::Text {
                pos,
                height,
                text,
                align,
                rotation_deg,
            } => (pos.y, *height, text.clone(), *align, *rotation_deg),
            other => panic!("expected text, got {other:?}"),
        })
        .collect();
    assert_eq!(texts[0].2, "-K1");
    assert_eq!(texts[1].2, "24 V");
    // Centre anchored: the two lines straddle the anchor, first one higher
    // on the page (larger y in DXF).
    assert!(near(texts[0].0, 100.0 - 10.0 + 1.0), "{}", texts[0].0);
    assert!(near(texts[1].0, 100.0 - 10.0 - 1.0), "{}", texts[1].0);
    assert_eq!(texts[0].3.h, HAlign::Center);
    assert_eq!(texts[0].3.v, VAlign::Middle);
    assert!(near(texts[0].4, 0.0));
}

#[test]
fn rotated_labels_read_counter_clockwise_in_dxf() {
    // -90 degrees on screen reads bottom to top: 90 degrees CCW in DXF.
    let scene = scene_with(|it| {
        let mut l = Label::new(Vec2::new(5.0, 5.0), "L1", 2.0);
        l.rotation = -FRAC_PI_2;
        it.labels.push(l);
    });
    let d = scene_to_drawing(&scene, 50.0);
    match &d.entities[0].kind {
        EntityKind::Text { rotation_deg, .. } => assert!(near(*rotation_deg, 90.0)),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn the_motor_starter_pages_export_and_read_back() {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).unwrap();
    let project = doc.project();
    let page = project.page(h.main_page).unwrap();
    let (w, ph) = page.format.size_mm();

    let d = page_drawing(project, h.main_page, &PlaceholderArt, ModelStyle::default()).unwrap();
    assert!(d.entities.len() > 50, "frame, symbols, wires and texts");
    for name in ["Frame", "Wires", "Symbols", "Texts"] {
        assert!(d.layer(name).is_some(), "layer {name}");
    }
    // Everything sits on the sheet.
    let (min, max) = d.bounds().unwrap();
    assert!(min.x >= -1.0 && min.y >= -1.0, "{min:?}");
    assert!(max.x <= w + 1.0 && max.y <= ph + 1.0, "{max:?}");
    // Some text carries a device tag.
    assert!(d.entities.iter().any(|e| matches!(
        &e.kind,
        EntityKind::Text { text, .. } if text.contains("K1")
    )));

    // The file reads back with the same entities.
    let text = write_dxf(&d);
    let back = read_dxf(text.as_bytes()).unwrap();
    assert!(back.skipped.is_empty());
    assert_eq!(back.drawing.entities.len(), d.entities.len());
    // Deterministic.
    let again = page_drawing(project, h.main_page, &PlaceholderArt, ModelStyle::default()).unwrap();
    assert_eq!(write_dxf(&again), text);
}

#[test]
fn an_unknown_page_gives_nothing() {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).unwrap();
    let other = Document::default();
    assert!(
        page_drawing(
            other.project(),
            h.main_page,
            &PlaceholderArt,
            ModelStyle::default()
        )
        .is_none()
    );
}
