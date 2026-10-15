//! Route all in the Cabinet and the wire-cut export from the app.

use novo_core::{
    DeviceTag, Direction, MountingPlate, Pin, PinRef, Placement, Point, RailKind, SymbolRole, Value,
};

use super::gate1_tests::Drive;
use crate::actions::Action;

/// Two placed devices joined by a wire, with a duct between them.
fn wired_cabinet(d: &mut Drive) {
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
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
            tx.set_property(part, "depth_mm", Some(Value::Float(75.0)))?;
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
            Ok(())
        })
        .unwrap();
}

#[test]
fn route_all_shows_the_cabinet_with_its_routes_and_toggles_off() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    wired_cabinet(&mut d);
    d.app.run_command(&ctx, "cabinet.routes", None);
    d.frame();
    assert!(d.app.cabinet_open && d.app.cabinet.routes);
    d.app.run_command(&ctx, "cabinet.routes", None);
    d.frame();
    assert!(!d.app.cabinet.routes);
}

#[test]
fn the_wire_cut_file_is_written_next_to_the_project_and_not_over_a_stranger() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    // Nothing to cut yet: no file.
    let none = dir.path().join("none.csv");
    assert_eq!(d.app.export_wire_cut_to(&none), None);
    assert!(!none.exists());

    wired_cabinet(&mut d);
    let path = dir.path().join("cut.csv");
    assert_eq!(d.app.export_wire_cut_to(&path), Some(path.clone()));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.starts_with('\u{feff}'), "Excel reads it right");
    let mut lines = text.trim_start_matches('\u{feff}').lines();
    assert!(lines.next().unwrap().starts_with("No;Wire no.;Qty;mm2;"));
    // Terminals 75 mm off the plate, wire at 30 mm in the duct: 45 + 45 up
    // and down, 77.5 + 110 + 77.5 in plan and 50 mm slack at each end = 455;
    // the cut adds 10 mm per end for the ferrule.
    let row = lines.next().expect("one wire");
    assert!(row.contains(";475;455;"), "{row}");

    let book = dir.path().join("production.xlsx");
    assert_eq!(d.app.export_wire_production_to(&book), Some(book.clone()));
    assert!(std::fs::metadata(&book).unwrap().len() > 0);
}
