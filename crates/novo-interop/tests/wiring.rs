use novo_core::{
    DeviceTag, Direction, MountingPlate, Pin, PinRef, Placement, Point, Project, RailKind,
    StructureId, SymbolRole, Value,
};
use novo_interop::dxf::{EntityKind, read_dxf, write_dxf};
use novo_interop::wiring::wiring_dxf;

fn cabinet(duct: bool) -> (Project, StructureId) {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            if duct {
                let d = tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
                tx.update::<novo_core::Rail>(d, |r| r.vertical = true)?;
                tx.set_rail_size(d, 200.0, 40.0, 60.0)?;
            }
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, y, dir) in [
                ("=A1+CAB1-K1", 50.0, Direction::Down),
                ("=A1+CAB1-K2", 250.0, Direction::Up),
            ] {
                let device = tx.add_device(DeviceTag::parse(tag)?, "")?;
                tx.assign_part(device, part)?;
                tx.place_device(device, cab, Point::new(50.0, y), false, None)?;
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(Point::new(0.0, 0.0)),
                    vec![Pin::new("1", 0.0, 0.0).with_direction(dir)],
                    SymbolRole::Device { device, main: true },
                )?;
                pins.push(PinRef::new(symbol, 0));
            }
            tx.connect(pins[0], pins[1])?;
            Ok(cab)
        })
        .unwrap();
    (p, cab)
}

fn on<'a>(d: &'a novo_interop::dxf::Drawing, layer: &str) -> Vec<&'a novo_interop::dxf::Entity> {
    d.entities.iter().filter(|e| e.layer == layer).collect()
}

#[test]
fn the_plan_draws_plate_ducts_devices_routes_and_a_table() {
    let (p, cab) = cabinet(true);
    let d = wiring_dxf(&p, cab).expect("a plate");
    assert_eq!(on(&d, "PLATE").len(), 2, "plate and margin");
    assert_eq!(on(&d, "DUCTS").len(), 1, "the outline");
    assert_eq!(on(&d, "TEXT").len(), 1, "its fill");
    assert_eq!(on(&d, "DEVICES").len(), 4, "two boxes and two tags");
    let routes = on(&d, "ROUTES");
    assert_eq!(routes.len(), 2, "the polyline and its label");
    assert!(on(&d, "FREEAIR").is_empty());
    // y points up: everything on the plate is at or below the x axis.
    let EntityKind::Polyline { points, .. } = &routes[0].kind else {
        panic!("polyline first")
    };
    assert!(points.iter().all(|p| p.y <= 0.0));
    // 77.5 + 110 + 77.5 in plan, 50 mm slack at each end and 30 mm down to
    // the middle of the duct at each end (the parts have no depth).
    let table: Vec<String> = on(&d, "TABLE")
        .iter()
        .filter_map(|e| match &e.kind {
            EntityKind::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert!(
        table[1].contains("=A1+CAB1-K1:1 > =A1+CAB1-K2:1"),
        "{table:?}"
    );
    assert!(
        table[1].contains(" 365 ") || table[1].contains("425"),
        "{table:?}"
    );
    // It is valid DXF that reads back.
    let text = write_dxf(&d);
    assert!(read_dxf(text.as_bytes()).is_ok());
}

#[test]
fn a_wire_no_duct_reaches_is_on_its_own_layer() {
    let (p, cab) = cabinet(false);
    let d = wiring_dxf(&p, cab).unwrap();
    assert!(on(&d, "ROUTES").is_empty());
    assert_eq!(on(&d, "FREEAIR").len(), 2);
}

#[test]
fn a_panel_without_a_plate_has_no_plan() {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("c", |tx| {
            tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)
        })
        .unwrap();
    assert!(wiring_dxf(&p, cab).is_none());
}
