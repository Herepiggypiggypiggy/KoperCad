//! DXF as a page background (WS-24).

use novo_canvas::Shape;
use novo_core::{Properties, Value};
use novo_interop::dxf::{Drawing, Entity, EntityKind, Point, write_dxf};
use novo_interop::underlay::{self, import_underlay, underlay_of};

fn props(import: &underlay::UnderlayImport) -> Properties {
    let mut p = Properties::default();
    for (k, v) in &import.properties {
        p.insert((*k).to_string(), v.clone());
    }
    p
}

fn sample() -> Vec<u8> {
    let mut d = Drawing::default();
    // Far from the origin, y up: a 100 x 50 box corner to corner.
    d.push(Entity::new(
        "Wall",
        EntityKind::Line {
            a: Point::new(1000.0, 2000.0),
            b: Point::new(1100.0, 2050.0),
        },
    ));
    d.push(Entity::new(
        "Wall",
        EntityKind::Text {
            pos: Point::new(1000.0, 2000.0),
            height: 5.0,
            rotation_deg: 0.0,
            text: "Door".into(),
            align: Default::default(),
        },
    ));
    write_dxf(&d).into_bytes()
}

#[test]
fn import_moves_the_drawing_to_the_page_corner_with_y_down() {
    let import = import_underlay(&sample(), "plant.dxf").unwrap();
    assert_eq!(import.entities, 2);
    let p = props(&import);
    assert_eq!(underlay::stored_name(&p), Some("plant.dxf"));
    let u = underlay_of(&p).unwrap();
    let Shape::Line { a, b } = u.primitives[0].shape else {
        panic!("a line");
    };
    // The DXF's lower-left (1000, 2000) is the bottom-left of the box at
    // dx, dy + height; its upper-right is the top-right.
    assert!(
        (a.x - 10.0).abs() < 1e-3 && (a.y - 60.0).abs() < 1e-3,
        "{a:?}"
    );
    assert!(
        (b.x - 110.0).abs() < 1e-3 && (b.y - 10.0).abs() < 1e-3,
        "{b:?}"
    );
    assert_eq!(u.labels.len(), 1);
}

#[test]
fn offset_scale_and_visibility_follow_the_properties() {
    let import = import_underlay(&sample(), "plant.dxf").unwrap();
    let mut p = props(&import);
    let key = underlay::cache_key(&p).unwrap();
    p.insert(underlay::KEY_DX.into(), Value::Float(20.0));
    p.insert(underlay::KEY_SCALE.into(), Value::Float(2.0));
    assert_ne!(underlay::cache_key(&p), Some(key), "a move redraws");
    let u = underlay_of(&p).unwrap();
    let Shape::Line { a, b } = u.primitives[0].shape else {
        panic!("a line");
    };
    assert!((a.x - 20.0).abs() < 1e-3 && (b.x - 220.0).abs() < 1e-3);
    p.insert(underlay::KEY_VISIBLE.into(), Value::Bool(false));
    assert!(underlay_of(&p).is_none() && underlay::cache_key(&p).is_none());
}

#[test]
fn nothing_to_show_and_too_much_are_refused() {
    assert!(import_underlay(b"0\r\nEOF\r\n", "x.dxf").is_err());
    let mut d = Drawing::default();
    for i in 0..60_000 {
        d.push(Entity::new(
            "0",
            EntityKind::Line {
                a: Point::new(f64::from(i), 0.0),
                b: Point::new(f64::from(i), 1.5),
            },
        ));
    }
    let err = import_underlay(write_dxf(&d).as_bytes(), "big.dxf").unwrap_err();
    assert!(
        matches!(err, underlay::UnderlayError::TooLarge { .. }),
        "{err}"
    );
}

#[test]
fn a_page_without_an_underlay_has_none() {
    assert!(underlay_of(&Properties::default()).is_none());
}
