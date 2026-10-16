use novo_core::{DeviceTag, MountingPlate, Project, StructureId, Value};
use novo_layout::drill::{
    CutoutShape, Surface, drill_table, features_from_text, parse_cutout, parse_holes, rail_holes,
};
use novo_layout::model::{apply_plan, candidates, load, plate_documents, set_plate};
use novo_layout::plate_drawing::{choose_scale, plate_drawing, scale_text, tables};
use novo_layout::state::CabinetLayout;
use novo_layout::{Margins, Placement, Plate, Rail, Size};

fn layout() -> CabinetLayout {
    let mut l = CabinetLayout::new(Plate {
        width: 600.0,
        height: 800.0,
        margin: 10.0,
    });
    l.rails.push(Rail {
        id: 1,
        x: 10.0,
        y: 100.0,
        length: 580.0,
        vertical: false,
    });
    l.placements.push(Placement {
        id: 7,
        x: 100.0,
        y: 120.0,
        rotated: false,
        size: Size::from_part(Some(40.0), Some(80.0)),
        clearance: None,
        margins: Margins::default(),
        rail: Some(1),
    });
    l
}

#[test]
fn hole_patterns_are_read_and_bad_entries_reported() {
    let (holes, bad) = parse_holes("5,5,4.2; 35,75,4.2;\nnope; 1,2,0");
    assert_eq!(holes.len(), 2);
    assert_eq!(holes[1].dy, 75.0);
    assert_eq!(bad.len(), 2);
}

#[test]
fn cutouts_are_read() {
    let c = parse_cutout("rect 22.5x22.5").unwrap();
    assert_eq!(c.shape, CutoutShape::Rect { w: 22.5, h: 22.5 });
    assert_eq!(c.centre, None);
    let c = parse_cutout("Round 30.5 @10, 20").unwrap();
    assert_eq!(c.shape, CutoutShape::Round { dia: 30.5 });
    assert_eq!(c.centre, Some((10.0, 20.0)));
    assert!(parse_cutout("oval 3").is_err());
    assert!(parse_cutout("rect 0x4").is_err());
}

#[test]
fn rail_holes_sit_at_both_ends_and_at_most_200_apart() {
    let r = Rail {
        id: 1,
        x: 10.0,
        y: 100.0,
        length: 580.0,
        vertical: false,
    };
    let h = rail_holes(&r);
    assert_eq!(h.first().unwrap().0, 20.0);
    assert_eq!(h.last().unwrap().0, 580.0);
    assert!(h.windows(2).all(|w| w[1].0 - w[0].0 <= 200.0 + 1e-9));
    assert_eq!(h.len(), 4);
    let short = Rail { length: 15.0, ..r };
    assert_eq!(rail_holes(&short), vec![(17.5, 100.0)]);
}

#[test]
fn holes_follow_the_device_and_its_rotation() {
    let mut l = layout();
    let f = features_from_text(7, "-K1", None, Some("5,5,4.2;35,75,4.2"), None);
    let t = drill_table(&l, std::slice::from_ref(&f));
    // 4 rail holes (5.5) after the two 4.2 holes, numbered by diameter.
    assert_eq!(t.holes.len(), 6);
    assert_eq!(t.holes[0].dia, 4.2);
    assert_eq!((t.holes[0].x, t.holes[0].y), (105.0, 125.0));
    assert_eq!((t.holes[1].x, t.holes[1].y), (135.0, 195.0));
    assert_eq!(
        t.holes.iter().map(|h| h.no).collect::<Vec<_>>(),
        [1, 2, 3, 4, 5, 6]
    );
    assert_eq!(t.holes[2].source, "Rail 1");

    // Rotated 90 degrees clockwise: footprint 80 x 40, (5,5) -> (80-5, 5).
    l.placements[0].rotated = true;
    let t = drill_table(&l, &[f]);
    assert_eq!((t.holes[0].x, t.holes[0].y), (175.0, 125.0));
}

#[test]
fn door_devices_go_to_the_door_and_cutouts_default_to_the_centre() {
    let l = layout();
    let f = features_from_text(7, "-P1", Some("door_cutout"), None, Some("rect 22.5x22.5"));
    let t = drill_table(&l, &[f]);
    assert_eq!(t.cutouts.len(), 1);
    assert_eq!(t.cutouts[0].surface, Surface::Door);
    assert_eq!((t.cutouts[0].x, t.cutouts[0].y), (120.0, 160.0));
    // Only the rail holes stay on the plate.
    assert!(t.holes.iter().all(|h| h.surface == Surface::Plate));
    assert_eq!(tables(&t, Surface::Door).len(), 1);
}

#[test]
fn unreadable_patterns_are_listed_as_problems() {
    let l = layout();
    let f = features_from_text(7, "-K1", None, Some("1,2"), Some("blob"));
    let t = drill_table(&l, &[f]);
    assert_eq!(t.problems.len(), 2);
    assert!(t.problems[0].starts_with("-K1: "));
}

#[test]
fn the_drawing_carries_dimensions_and_every_hole() {
    let l = layout();
    let f = features_from_text(7, "-K1", None, Some("5,5,4.2"), None);
    let t = drill_table(&l, std::slice::from_ref(&f));
    let d = plate_drawing(&l, &[f], &t, Surface::Plate, 5.0);
    let texts: Vec<_> = d.marks.texts.iter().map(|x| x.text.as_str()).collect();
    assert!(texts.contains(&"600") && texts.contains(&"800"));
    assert!(texts.contains(&"-K1"));
    let circles = d
        .marks
        .shapes
        .iter()
        .filter(|(_, s)| matches!(s, novo_layout::plate_drawing::Shape::Circle { .. }))
        .count();
    assert_eq!(circles, t.holes.len());
}

#[test]
fn scale_is_the_smallest_standard_one_that_fits() {
    assert_eq!(choose_scale(200.0, 150.0, (275.0, 250.0)), 1.0);
    assert_eq!(choose_scale(600.0, 800.0, (275.0, 250.0)), 5.0);
    assert_eq!(choose_scale(1e6, 1e6, (275.0, 250.0)), 100.0);
    assert_eq!(scale_text(2.5), "1:2.5");
}

fn project() -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "hole_pattern", Some(Value::Text("5,5,4.2".into())))?;
            let disp = tx.add_part("ACME", "D1")?;
            tx.set_property(disp, "width_mm", Some(Value::Float(72.0)))?;
            tx.set_property(disp, "height_mm", Some(Value::Float(72.0)))?;
            tx.set_property(disp, "mounting", Some(Value::Text("door_cutout".into())))?;
            tx.set_property(disp, "cutout", Some(Value::Text("round 22.5".into())))?;
            let k = tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "")?;
            tx.assign_part(k, part)?;
            let h = tx.add_device(DeviceTag::parse("=A1+CAB1-P1").unwrap(), "")?;
            tx.assign_part(h, disp)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let layout = load(&p, cab).unwrap();
    let plan = layout.preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    (p, cab)
}

#[test]
fn documents_read_the_part_keys_from_the_project() {
    let (p, cab) = project();
    let (docs, table) = plate_documents(&p, cab).unwrap();
    assert_eq!(docs.len(), 2); // plate and door
    assert_eq!(docs[0].title, "+CAB1");
    assert_eq!(table.cutouts.len(), 1);
    assert_eq!(table.cutouts[0].surface, Surface::Door);
    assert!(
        table
            .holes
            .iter()
            .any(|h| h.dia == 4.2 && h.source.contains("K1"))
    );
    assert_eq!(docs[0].drawing.scale, docs[1].drawing.scale);
}

#[test]
fn no_plate_no_documents() {
    let mut p = Project::new("t");
    let (s, _) = p
        .transact("s", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB2").unwrap(), "", true)
        })
        .unwrap();
    assert!(plate_documents(&p, s).is_none());
}

#[test]
fn vertical_rails_get_holes_down_their_length_and_are_drawn_vertical() {
    let r = Rail {
        id: 1,
        x: 100.0,
        y: 50.0,
        length: 500.0,
        vertical: true,
    };
    let h = rail_holes(&r);
    assert_eq!(h.first(), Some(&(100.0, 60.0)));
    assert_eq!(h.last(), Some(&(100.0, 540.0)));
    assert!(h.iter().all(|p| p.0 == 100.0));

    let mut l = layout();
    l.rails = vec![r];
    let t = drill_table(&l, &[]);
    assert!(t.holes.iter().all(|h| h.x == 100.0));
    let d = plate_drawing(&l, &[], &t, Surface::Plate, 5.0);
    let rail_lines: Vec<_> = d
        .marks
        .shapes
        .iter()
        .filter(|(layer, _)| *layer == novo_layout::plate_drawing::Layer::Rails)
        .collect();
    assert_eq!(rail_lines.len(), 2);
    for (_, s) in rail_lines {
        let novo_layout::plate_drawing::Shape::Line { a, b } = s else {
            panic!()
        };
        assert_eq!(a.0, b.0);
        assert_eq!(b.1 - a.1, 500.0);
    }
}

#[test]
fn the_door_sheet_takes_its_size_from_the_enclosure() {
    use novo_layout::enclosure::Enclosure;
    let mut l = layout();
    let f = features_from_text(7, "-P1", Some("door_cutout"), None, Some("rect 22.5x22.5"));
    let t = drill_table(&l, std::slice::from_ref(&f));
    let door = |l: &CabinetLayout, scale| {
        plate_drawing(l, std::slice::from_ref(&f), &t, Surface::Door, scale)
    };
    assert_eq!((door(&l, 5.0).width, door(&l, 5.0).height), (600.0, 800.0));

    l.enclosure = Some(Enclosure {
        width: 650.0,
        height: 850.0,
        depth: 250.0,
        plate_setback: 20.0,
        door: None,
        front_clearance: None,
        hinge_side_clearance: None,
    });
    let d = door(&l, 5.0);
    assert_eq!((d.width, d.height), (650.0, 850.0));
    let plate = plate_drawing(&l, std::slice::from_ref(&f), &t, Surface::Plate, 5.0);
    assert_eq!((plate.width, plate.height), (600.0, 800.0));
}
