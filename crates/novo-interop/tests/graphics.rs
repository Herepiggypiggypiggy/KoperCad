//! Lines, boxes and the cabinet layout of a project export, on a synthetic
//! fixture (the real export is checked opt-in in `crane_real`).

use novo_core::{DeviceTag, Document, Value};
use novo_interop::eplan::pxf::{PxfOptions, ShapeKind, plan_pxf, read_pxf_str};
use novo_interop::underlay;

const LAYOUT: &str = include_str!("fixtures/pxf/synthetic-layout.epj");

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

#[test]
fn shapes_are_read_with_their_geometry() {
    let f = read_pxf_str(LAYOUT).unwrap();
    let count = |pred: fn(&ShapeKind) -> bool| f.shapes.iter().filter(|s| pred(&s.kind)).count();
    assert_eq!(count(|k| matches!(k, ShapeKind::Line { .. })), 3);
    assert_eq!(count(|k| matches!(k, ShapeKind::Rect { .. })), 8);
    assert_eq!(count(|k| matches!(k, ShapeKind::Arc { .. })), 2);
    assert_eq!(count(|k| matches!(k, ShapeKind::Polygon { .. })), 1);
    let b = f
        .shapes
        .iter()
        .find_map(|s| match &s.kind {
            ShapeKind::LocationBox { a, b, location, .. } => Some((*a, *b, location.clone())),
            _ => None,
        })
        .expect("a location box");
    assert_eq!(b, ((40.0, 42.0), (212.0, 112.0), "+CAB".to_string()));
    assert!(f.shapes.iter().any(|s| s.dashed));
    assert!(!f.skipped.contains_key(&31));
}

#[test]
fn the_pages_get_their_lines_and_boxes_as_a_background_drawing() {
    let f = read_pxf_str(LAYOUT).unwrap();
    let mut doc = Document::default();
    let plan = plan_pxf(doc.project(), &f, &PxfOptions::default());
    let r = plan.report();
    assert!(r.text().contains("location boxes"), "{}", r.text());
    assert!(r.text().contains("lines outside the sheet"), "{}", r.text());
    plan.apply(&mut doc).unwrap();
    let page = doc
        .project()
        .pages()
        .find(|p| p.name.starts_with("=A1+CAB1/1"))
        .expect("the schematic page");
    let text = underlay::stored_text(&page.properties).expect("a background drawing");
    assert!(text.contains("+CAB"), "the location box carries its name");
    assert_eq!(
        page.properties.get(underlay::KEY_HEIGHT),
        Some(&Value::Float(297.0))
    );
    let under = underlay::underlay_of(&page.properties).expect("the canvas can draw it");
    assert!(!under.primitives.is_empty());
    assert!(!under.labels.is_empty());
}

#[test]
fn the_mounting_plate_sheet_becomes_a_cabinet_with_rails_ducts_and_placements() {
    let f = read_pxf_str(LAYOUT).unwrap();
    let mut doc = Document::default();
    let plan = plan_pxf(doc.project(), &f, &PxfOptions::default());
    assert!(plan.report().text().contains("Cabinet +CAB1"));
    assert!(!plan.report().text().contains("could not be placed"));
    // The footprints and rails come from drawn outlines: said so, not exact.
    let report = plan.report();
    assert!(
        report
            .guessed
            .iter()
            .any(|g| g.what == "device footprints on +CAB1" && g.count > 0),
        "{}",
        report.text()
    );
    assert!(
        report
            .guessed
            .iter()
            .any(|g| g.what == "rails and ducts on +CAB1")
    );
    assert!(!report.is_exact());
    plan.apply(&mut doc).unwrap();
    let p = doc.project();
    let s = p.structure_by_tag(&tag("+CAB1")).expect("the location");
    assert!(s.enclosure);
    let plate = s.plate.expect("a plate");
    assert_eq!((plate.width, plate.height), (600.0, 760.0));
    let rails: Vec<_> = p
        .all::<novo_core::Rail>()
        .filter(|r| r.structure == s.id)
        .collect();
    let din: Vec<_> = rails
        .iter()
        .filter(|r| r.kind == novo_core::RailKind::DinRail)
        .collect();
    let ducts: Vec<_> = rails
        .iter()
        .filter(|r| r.kind == novo_core::RailKind::Duct)
        .collect();
    assert_eq!((din.len(), ducts.len()), (1, 1));
    // The rail's centre line is 600 mm above the plate's bottom: 160 from the top.
    assert!((din[0].y - 160.0).abs() < 0.5, "{}", din[0].y);
    assert!((din[0].x - 15.0).abs() < 0.5 && (din[0].length - 570.0).abs() < 0.5);
    assert!((ducts[0].width - 25.0).abs() < 0.5);
    let q = p.devices_by_tag(&tag("=A1+CAB1-Q011"))[0];
    let placed = p
        .all::<novo_core::CabinetPlacement>()
        .find(|c| c.device == q)
        .expect("Q011 is placed");
    assert!((placed.x - 60.0).abs() < 0.5, "{}", placed.x);
    // 570..630 up, i.e. 130..190 from the top.
    assert!((placed.y - 130.0).abs() < 0.5, "{}", placed.y);
    assert_eq!(placed.rail, Some(din[0].id));
    let w = p.device(q).unwrap().properties.get("width_mm").cloned();
    assert_eq!(w, Some(Value::Float(40.0)));
    let x1 = p.devices_by_tag(&tag("=A1+CAB1-X1"))[0];
    assert!(
        p.all::<novo_core::CabinetPlacement>()
            .any(|c| c.device == x1)
    );
    assert!(doc.undo().unwrap().is_some());
    assert_eq!(
        doc.project().all::<novo_core::CabinetPlacement>().count(),
        0
    );
}

#[test]
fn other_readers_can_ask_for_a_background_from_plain_shapes() {
    use novo_interop::eplan::pxf::{PxfShape, background_properties};
    let line = PxfShape {
        page: None,
        kind: ShapeKind::Line {
            a: (10.0, 10.0),
            b: (50.0, 10.0),
        },
        pen: None,
        dashed: false,
    };
    let props = background_properties(&[line]).expect("a drawing");
    assert!(props.iter().any(|(k, _)| *k == underlay::KEY_DXF));
    assert!(background_properties(&[]).is_none());
}

#[test]
fn shapes_with_huge_coordinates_are_dropped_not_drawn() {
    use novo_interop::eplan::pxf::{PxfShape, background_properties};
    let far = 1e12;
    let shape = |kind| PxfShape {
        page: None,
        kind,
        pen: None,
        dashed: true,
    };
    let shapes = [
        shape(ShapeKind::Line {
            a: (0.0, 0.0),
            b: (far, 0.0),
        }),
        shape(ShapeKind::Rect {
            a: (0.0, 0.0),
            b: (far, far),
        }),
        shape(ShapeKind::LocationBox {
            a: (0.0, 0.0),
            b: (far, far),
            function: String::new(),
            location: "K1".into(),
        }),
    ];
    // Returns at once with nothing to draw.
    assert!(background_properties(&shapes).is_none());
}
