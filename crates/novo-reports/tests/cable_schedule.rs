#[test]
fn cable_length_comes_from_the_cabinet_layout_unless_set() {
    use novo_core::{
        Cable, CableCore, DeviceTag, Direction, MountingPlate, PinRef, Placement, Point, Project,
        RailKind, SymbolRole, Value,
    };
    let mut p = Project::new("t");
    let (cable, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            let duct = tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
            tx.update::<novo_core::Rail>(duct, |r| r.vertical = true)?;
            tx.set_rail_size(duct, 200.0, 40.0, 60.0)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, y, dir) in [
                ("=A1+CAB1-K1", 50.0, Direction::Down),
                ("=A1+CAB1-K2", 250.0, Direction::Up),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, Point::new(50.0, y), false, None)?;
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(Point::new(0.0, 0.0)),
                    vec![novo_core::Pin::new("1", 0.0, 0.0).with_direction(dir)],
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                pins.push(PinRef::new(symbol, 0));
            }
            let wire = tx.connect(pins[0], pins[1])?;
            let cable =
                tx.add_cable(DeviceTag::parse("-W1").unwrap(), vec![CableCore::new("1")])?;
            tx.assign_core(cable, 0, Some(wire))?;
            Ok(cable)
        })
        .unwrap();
    let length = |p: &Project| {
        let out = novo_reports::CableSchedule.generate(p);
        let rows = out.sections[0].text_rows();
        (rows[0][2].clone(), rows[0][8].clone())
    };
    use novo_reports::Report;
    let (calculated, note) = length(&p);
    assert!(calculated.parse::<f64>().unwrap() > 0.2, "{calculated}");
    assert_eq!(note, "calc");
    p.transact("set length", |tx| {
        tx.update::<Cable>(cable, |c| c.length_m = Some(5.0))
    })
    .unwrap();
    assert_eq!(length(&p), ("5".to_owned(), String::new()));
}
