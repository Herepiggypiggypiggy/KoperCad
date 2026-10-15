use novo_core::{DeviceTag, MountingPlate, Project, Value};
use novo_interop::dxf::{EntityKind, read_dxf, write_dxf};
use novo_interop::plate::plate_dxf;
use novo_layout::model::{apply_plan, candidates, load, plate_documents, set_plate};

fn docs() -> Vec<novo_layout::plate_drawing::PlateDocument> {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(90)))?;
            tx.set_property(part, "hole_pattern", Some(Value::Text("5,5,4.2".into())))?;
            let d = tx.add_device(DeviceTag::parse("=A1+CAB1-K1").unwrap(), "")?;
            tx.assign_part(d, part)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 600.0,
                    height: 800.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    plate_documents(&p, cab).unwrap().0
}

#[test]
fn the_plate_dxf_round_trips_with_holes_dimensions_and_tables() {
    let docs = docs();
    let text = write_dxf(&plate_dxf(&docs[0]));
    let back = read_dxf(text.as_bytes()).unwrap().drawing;

    // Plate edge: a closed rectangle 600 x 800 below the x axis.
    let plate = back
        .entities
        .iter()
        .find(|e| e.layer == "PLATE")
        .expect("plate outline");
    let EntityKind::Polyline { points, closed } = &plate.kind else {
        panic!("plate is a polyline");
    };
    assert!(*closed);
    let ys: Vec<f64> = points.iter().map(|p| p.y).collect();
    assert_eq!(ys.iter().cloned().fold(f64::MIN, f64::max), 0.0);
    assert_eq!(ys.iter().cloned().fold(f64::MAX, f64::min), -800.0);

    let holes = back
        .entities
        .iter()
        .filter(|e| e.layer == "HOLES" && matches!(e.kind, EntityKind::Circle { .. }));
    // One device hole and the rail's holes.
    assert!(holes.count() >= 3);
    let texts: Vec<String> = back
        .entities
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.iter().any(|t| t == "600"));
    assert!(texts.iter().any(|t| t == "800"));
    assert!(texts.iter().any(|t| t == "Drill holes"));
    assert!(texts.iter().any(|t| t.contains("1:5")));
}

#[test]
fn the_same_documents_give_the_same_bytes() {
    let docs = docs();
    assert_eq!(
        write_dxf(&plate_dxf(&docs[0])),
        write_dxf(&plate_dxf(&docs[0]))
    );
}

#[test]
fn dxf_coordinates_are_x_and_minus_y_of_the_table() {
    let docs = docs();
    let (x, y) = {
        let h = docs[0]
            .drawing
            .marks
            .shapes
            .iter()
            .find_map(|(l, s)| match s {
                novo_layout::plate_drawing::Shape::Circle { centre, radius }
                    if *l == novo_layout::plate_drawing::Layer::Holes && *radius < 2.5 =>
                {
                    Some(*centre)
                }
                _ => None,
            });
        h.expect("a 4.2 mm hole")
    };
    let back = read_dxf(write_dxf(&plate_dxf(&docs[0])).as_bytes())
        .unwrap()
        .drawing;
    assert!(back.entities.iter().any(|e| matches!(
        e.kind,
        EntityKind::Circle { center, .. } if (center.x - x).abs() < 1e-6 && (center.y + y).abs() < 1e-6
    )));
}
