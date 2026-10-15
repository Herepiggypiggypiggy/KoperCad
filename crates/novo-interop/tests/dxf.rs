//! DXF writer and reader (WS-24).

use novo_interop::dxf::{
    Drawing, DxfError, Entity, EntityKind, HAlign, Layer, Point, TextAlign, VAlign, read_dxf,
    write_dxf,
};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn sample() -> Drawing {
    let mut d = Drawing::default();
    d.layers.push(Layer::new("Frame", 5));
    d.layers.push(Layer {
        name: "Hidden".into(),
        color: 3,
        hidden: true,
    });
    d.push(Entity::new(
        "Frame",
        EntityKind::Line {
            a: p(0.0, 0.0),
            b: p(420.0, 0.0),
        },
    ));
    d.push(Entity::new(
        "Wires",
        EntityKind::Polyline {
            points: vec![p(10.0, 10.0), p(10.0, 50.5), p(30.25, 50.5)],
            closed: false,
        },
    ));
    d.push(Entity::new(
        "Wires",
        EntityKind::Polyline {
            points: vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 5.0)],
            closed: true,
        },
    ));
    d.push(Entity::new(
        "Symbols",
        EntityKind::Circle {
            center: p(100.0, 100.0),
            radius: 4.0,
        },
    ));
    d.push(Entity::new(
        "Symbols",
        EntityKind::Arc {
            center: p(50.0, 50.0),
            radius: 10.0,
            start_deg: 30.0,
            end_deg: 120.0,
        },
    ));
    d.push(Entity::new(
        "Text",
        EntityKind::Text {
            pos: p(12.0, 20.0),
            height: 2.5,
            rotation_deg: 90.0,
            text: "=A1+CAB-K1 ⌀ 12°".into(),
            align: TextAlign {
                h: HAlign::Center,
                v: VAlign::Middle,
            },
        },
    ));
    d
}

#[test]
fn round_trip_keeps_every_entity() {
    let d = sample();
    let text = write_dxf(&d);
    assert!(text.is_ascii(), "the file stays 7-bit");
    let back = read_dxf(text.as_bytes()).unwrap();
    assert!(back.skipped.is_empty());
    assert_eq!(back.drawing.entities, d.entities);
    assert_eq!(back.drawing.mm_per_unit, 1.0);
}

#[test]
fn layers_keep_colour_and_visibility() {
    let back = read_dxf(write_dxf(&sample()).as_bytes()).unwrap().drawing;
    assert_eq!(back.layer("Frame").unwrap().color, 5);
    assert!(!back.layer("Frame").unwrap().hidden);
    assert!(back.layer("Hidden").unwrap().hidden);
    assert!(back.layer("0").is_some(), "layer 0 always exists");
    assert!(back.layer("Wires").is_some(), "used layers are declared");
}

#[test]
fn output_is_deterministic() {
    assert_eq!(write_dxf(&sample()), write_dxf(&sample()));
}

#[test]
fn layer_names_are_made_legal() {
    let mut d = Drawing::default();
    d.push(Entity::new(
        "a/b:c",
        EntityKind::Line {
            a: p(0.0, 0.0),
            b: p(1.0, 1.0),
        },
    ));
    let back = read_dxf(write_dxf(&d).as_bytes()).unwrap().drawing;
    assert_eq!(back.entities[0].layer, "a_b_c");
}

#[test]
fn multi_line_text_becomes_one_text_per_line() {
    let mut d = Drawing::default();
    d.push(Entity::new(
        "Text",
        EntityKind::Text {
            pos: p(0.0, 0.0),
            height: 2.0,
            rotation_deg: 0.0,
            text: "one\ntwo".into(),
            align: TextAlign::default(),
        },
    ));
    let back = read_dxf(write_dxf(&d).as_bytes()).unwrap().drawing;
    assert_eq!(back.entities.len(), 2);
    match (&back.entities[0].kind, &back.entities[1].kind) {
        (
            EntityKind::Text {
                pos: a, text: ta, ..
            },
            EntityKind::Text {
                pos: b, text: tb, ..
            },
        ) => {
            assert_eq!((ta.as_str(), tb.as_str()), ("one", "two"));
            assert!(b.y < a.y, "the second line sits below the first");
        }
        _ => panic!("expected two texts"),
    }
}

#[test]
fn empty_drawing_is_valid() {
    let text = write_dxf(&Drawing::default());
    let back = read_dxf(text.as_bytes()).unwrap();
    assert!(back.drawing.entities.is_empty());
}

#[test]
fn bounds_cover_circles_by_their_extent() {
    let (min, max) = sample().bounds().unwrap();
    assert!(near(min.x, 0.0) && near(min.y, 0.0));
    assert!(near(max.x, 420.0) && near(max.y, 104.0));
    assert!(Drawing::default().bounds().is_none());
}

// ---- reading files other programs wrote ----------------------------------

fn dxf(entities: &str) -> String {
    format!("0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n")
}

#[test]
fn binary_dxf_is_refused() {
    let mut bytes = b"AutoCAD Binary DXF\r\n\x1a\0".to_vec();
    bytes.extend_from_slice(&[0, 1, 2, 3]);
    assert_eq!(read_dxf(&bytes).unwrap_err(), DxfError::Binary);
}

#[test]
fn text_that_is_not_dxf_is_refused() {
    assert_eq!(
        read_dxf(b"hello\nworld\n").unwrap_err(),
        DxfError::BadCode {
            line: 1,
            text: "hello".into()
        }
    );
    assert_eq!(read_dxf(b"999\ncomment\n").unwrap_err(), DxfError::NotDxf);
}

#[test]
fn crlf_and_blank_trailing_lines_are_fine() {
    let text = dxf("0\nLINE\n8\n0\n10\n1\n20\n2\n11\n3\n21\n4\n").replace('\n', "\r\n");
    let back = read_dxf(format!("{text}\r\n\r\n").as_bytes()).unwrap();
    assert_eq!(back.drawing.entities.len(), 1);
}

#[test]
fn lwpolyline_reads_as_polyline() {
    let r = read_dxf(
        dxf("0\nLWPOLYLINE\n8\nW\n90\n3\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n5\n")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        r.drawing.entities[0].kind,
        EntityKind::Polyline {
            points: vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0)],
            closed: true
        }
    );
}

#[test]
fn bulge_becomes_an_arc() {
    // A half circle from (0,0) to (10,0); a negative bulge runs clockwise,
    // so it bulges up.
    let r = read_dxf(
        dxf("0\nLWPOLYLINE\n8\n0\n90\n2\n70\n0\n10\n0\n20\n0\n42\n-1\n10\n10\n20\n0\n").as_bytes(),
    )
    .unwrap();
    match &r.drawing.entities[0].kind {
        EntityKind::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            assert!(near(center.x, 5.0) && near(center.y, 0.0));
            assert!(near(*radius, 5.0));
            // Counter-clockwise from 0 to 180 is the upper half.
            assert!(near(start_deg.rem_euclid(360.0), 0.0) && near(*end_deg, 180.0));
        }
        other => panic!("expected an arc, got {other:?}"),
    }
}

#[test]
fn old_style_polyline_with_vertices() {
    let r = read_dxf(
        dxf("0\nPOLYLINE\n8\n0\n66\n1\n70\n0\n0\nVERTEX\n8\n0\n10\n0\n20\n0\n0\nVERTEX\n8\n0\n10\n4\n20\n3\n0\nSEQEND\n8\n0\n")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(
        r.drawing.entities[0].kind,
        EntityKind::Polyline {
            points: vec![p(0.0, 0.0), p(4.0, 3.0)],
            closed: false
        }
    );
}

#[test]
fn unsupported_entities_are_counted() {
    let r = read_dxf(
        dxf("0\nSPLINE\n8\n0\n0\nSPLINE\n8\n0\n0\nHATCH\n8\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n1\n")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(r.drawing.entities.len(), 1);
    assert_eq!(r.skipped.get("SPLINE"), Some(&2));
    assert_eq!(r.skipped.get("HATCH"), Some(&1));
}

#[test]
fn mtext_loses_its_formatting() {
    let r = read_dxf(
        dxf("0\nMTEXT\n8\n0\n10\n5\n20\n6\n40\n3\n71\n5\n1\n{\\fArial|b1;Motor}\\Pline 2 %%c12\n")
            .as_bytes(),
    )
    .unwrap();
    match &r.drawing.entities[0].kind {
        EntityKind::Text { text, align, .. } => {
            assert_eq!(text, "Motor\nline 2 ⌀12");
            assert_eq!(align.h, HAlign::Center);
            assert_eq!(align.v, VAlign::Middle);
        }
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn text_shortcuts_and_unicode_escapes() {
    let r = read_dxf(
        dxf("0\nTEXT\n8\n0\n10\n0\n20\n0\n40\n2.5\n1\n45%%d \\U+00E9t\\U+00E9\n").as_bytes(),
    )
    .unwrap();
    match &r.drawing.entities[0].kind {
        EntityKind::Text { text, .. } => assert_eq!(text, "45° été"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn old_code_page_files_decode_as_windows_1252() {
    let mut bytes = dxf("0\nTEXT\n8\n0\n10\n0\n20\n0\n40\n2.5\n1\nCaf\u{1}\n").into_bytes();
    let at = bytes.iter().position(|&b| b == 1).unwrap();
    bytes[at] = 0xE9; // é in Windows-1252, invalid as UTF-8
    let r = read_dxf(&bytes).unwrap();
    match &r.drawing.entities[0].kind {
        EntityKind::Text { text, .. } => assert_eq!(text, "Café"),
        other => panic!("expected text, got {other:?}"),
    }
}

#[test]
fn aligned_text_uses_the_alignment_point() {
    let r = read_dxf(
        dxf("0\nTEXT\n8\n0\n10\n1\n20\n1\n11\n7\n21\n8\n72\n2\n73\n0\n40\n2\n1\nR\n").as_bytes(),
    )
    .unwrap();
    match &r.drawing.entities[0].kind {
        EntityKind::Text { pos, align, .. } => {
            assert_eq!(*pos, p(7.0, 8.0));
            assert_eq!(align.h, HAlign::Right);
        }
        other => panic!("expected text, got {other:?}"),
    }
}

fn with_block(block: &str, entities: &str, header: &str) -> String {
    format!(
        "0\nSECTION\n2\nHEADER\n{header}0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n{block}0\nENDSEC\n0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n"
    )
}

#[test]
fn inserts_are_exploded_with_offset_scale_and_rotation() {
    // Block: a line from its base point (1,1) to (3,1); inserted at (10,10),
    // scaled 2 and turned 90 degrees.
    let block =
        "0\nBLOCK\n2\nB\n10\n1\n20\n1\n0\nLINE\n8\n0\n10\n1\n20\n1\n11\n3\n21\n1\n0\nENDBLK\n";
    let ins = "0\nINSERT\n8\nPLACED\n2\nB\n10\n10\n20\n10\n41\n2\n50\n90\n";
    let r = read_dxf(with_block(block, ins, "").as_bytes()).unwrap();
    assert_eq!(r.drawing.entities.len(), 1);
    let e = &r.drawing.entities[0];
    assert_eq!(
        e.layer, "PLACED",
        "layer 0 content takes the insert's layer"
    );
    match &e.kind {
        EntityKind::Line { a, b } => {
            assert!(near(a.x, 10.0) && near(a.y, 10.0));
            assert!(near(b.x, 10.0) && near(b.y, 14.0));
        }
        other => panic!("expected line, got {other:?}"),
    }
}

#[test]
fn mirrored_insert_flips_arc_direction() {
    // Quarter arc 0..90 deg mirrored in x becomes 90..180.
    let block = "0\nBLOCK\n2\nA\n10\n0\n20\n0\n0\nARC\n8\n0\n10\n0\n20\n0\n40\n5\n50\n0\n51\n90\n0\nENDBLK\n";
    let ins = "0\nINSERT\n8\n0\n2\nA\n10\n0\n20\n0\n41\n-1\n42\n1\n";
    let r = read_dxf(with_block(block, ins, "").as_bytes()).unwrap();
    match &r.drawing.entities[0].kind {
        EntityKind::Arc {
            start_deg, end_deg, ..
        } => assert!(near(*start_deg, 90.0) && near(*end_deg, 180.0)),
        other => panic!("expected arc, got {other:?}"),
    }
}

#[test]
fn missing_and_recursive_blocks_warn_instead_of_hanging() {
    let ins = "0\nINSERT\n8\n0\n2\nNOPE\n";
    let r = read_dxf(with_block("", ins, "").as_bytes()).unwrap();
    assert_eq!(r.warnings.len(), 1);

    let block = "0\nBLOCK\n2\nLOOP\n10\n0\n20\n0\n0\nINSERT\n8\n0\n2\nLOOP\n0\nENDBLK\n";
    let ins = "0\nINSERT\n8\n0\n2\nLOOP\n";
    let r = read_dxf(with_block(block, ins, "").as_bytes()).unwrap();
    assert!(r.drawing.entities.is_empty());
    assert!(r.warnings.iter().any(|w| w.contains("nested too deep")));
}

#[test]
fn units_scale_to_millimetres() {
    // $INSUNITS 1 = inches.
    let text = with_block(
        "",
        "0\nLINE\n8\n0\n10\n0\n20\n0\n11\n2\n21\n0\n0\nCIRCLE\n8\n0\n10\n0\n20\n0\n40\n1\n",
        "9\n$INSUNITS\n70\n1\n",
    );
    let r = read_dxf(text.as_bytes()).unwrap();
    assert!(near(r.drawing.mm_per_unit, 25.4));
    let mm = r.drawing.scaled_to_mm();
    assert!(near(mm.mm_per_unit, 1.0));
    match (&mm.entities[0].kind, &mm.entities[1].kind) {
        (EntityKind::Line { b, .. }, EntityKind::Circle { radius, .. }) => {
            assert!(near(b.x, 50.8) && near(*radius, 25.4));
        }
        _ => panic!("unexpected entities"),
    }
}

#[test]
fn layer_table_gives_colour_and_off_state() {
    let text = "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n2\n0\nLAYER\n2\nA\n70\n0\n62\n1\n6\nCONTINUOUS\n0\nLAYER\n2\nB\n70\n1\n62\n-4\n6\nCONTINUOUS\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
    let d = read_dxf(text.as_bytes()).unwrap().drawing;
    assert_eq!(d.layer("A").unwrap().color, 1);
    assert!(!d.layer("A").unwrap().hidden);
    assert_eq!(d.layer("B").unwrap().color, 4);
    assert!(d.layer("B").unwrap().hidden);
}
