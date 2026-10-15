use novo_core::{
    DeviceTag, Direction, MountingPlate, PinRef, Placement, Point as CorePoint, Project, RailKind,
    StructureId, SymbolRole, Value,
};
use novo_routing::model::{Connection, route_connections};
use novo_routing::{Kind, Rules};

fn pin(
    tx: &mut novo_core::Tx<'_>,
    page: novo_core::PageId,
    device: novo_core::DeviceId,
    dir: Direction,
) -> PinRef {
    let symbol = tx
        .place_symbol(
            page,
            "iec/contacts/no",
            Placement::at(CorePoint::new(0.0, 0.0)),
            vec![novo_core::Pin::new("1", 0.0, 0.0).with_direction(dir)],
            SymbolRole::Device { device, main: true },
        )
        .unwrap();
    PinRef::new(symbol, 0)
}

fn project() -> (Project, StructureId, Vec<PinRef>) {
    let mut p = Project::new("t");
    let (out, _) = p
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
                ("=A1+CAB1-K3", 0.0, Direction::Up),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                // K3 stays off the plate.
                if y > 0.0 {
                    tx.place_device(d, cab, CorePoint::new(50.0, y), false, None)?;
                }
                pins.push(pin(tx, page, d, dir));
            }
            Ok((cab, pins))
        })
        .unwrap();
    (p, out.0, out.1)
}

#[test]
fn a_connection_is_routed_from_edge_to_edge_through_the_duct() {
    let (p, cab, pins) = project();
    let conns = [Connection {
        id: 7,
        from: pins[0],
        to: pins[1],
        diameter_mm: Some(3.0),
    }];
    let r = route_connections(&p, &conns, &Rules::default());
    assert!(r.unrouted.is_empty());
    let route = &r.routes[&7];
    assert_eq!(route.kind, Kind::Ducted);
    // K1's bottom edge midpoint (72.5, 140), K2's top (72.5, 250), the
    // duct at x=150: 77.5 across, 110 down the duct, 77.5 across, plus
    // 50 mm slack at each end.
    assert_eq!(route.length_mm, 77.5 + 110.0 + 77.5 + 100.0);
    assert_eq!(r.plans.len(), 1);
    assert_eq!(r.plans[0].0, cab);
}

#[test]
fn a_device_off_the_plate_leaves_its_connection_unrouted() {
    let (p, _, pins) = project();
    let conns = [Connection {
        id: 9,
        from: pins[0],
        to: pins[2],
        diameter_mm: None,
    }];
    let r = route_connections(&p, &conns, &Rules::default());
    assert_eq!(r.unrouted, vec![9]);
    assert!(r.routes.is_empty() && r.plans.is_empty());
}

#[test]
fn wires_between_pins_are_connections_with_a_diameter_from_their_size() {
    let (mut p, _, pins) = project();
    let (ids, _) = p
        .transact("wires", |tx| {
            let a = tx.connect(pins[0], pins[1])?;
            let b = tx.connect(pins[0], pins[2])?;
            tx.update::<novo_core::Wire>(a, |w| w.cross_section_mm2 = Some(1.5))?;
            Ok((a, b))
        })
        .unwrap();
    let list = novo_routing::model::wire_connections(&p);
    assert_eq!(list.len(), 2);
    let a = list.iter().find(|c| c.id == ids.0.raw()).unwrap();
    assert_eq!((a.from, a.to), (pins[0], pins[1]));
    // 1.5 mm2 is 2 * sqrt(1.5 / pi) + 1.6 = 2.98 mm.
    assert!((a.diameter_mm.unwrap() - 3.0).abs() < 0.05);
    assert_eq!(
        list.iter()
            .find(|c| c.id == ids.1.raw())
            .unwrap()
            .diameter_mm,
        None
    );
}

#[test]
fn route_project_routes_every_wire_between_placed_pins() {
    let (mut p, cab, pins) = project();
    p.transact("wire", |tx| tx.connect(pins[0], pins[1]))
        .unwrap();
    let r = novo_routing::model::route_project(&p);
    assert_eq!(r.routes.len(), 1);
    assert!(r.unrouted.is_empty());
    assert_eq!(r.plans[0].0, cab);
}
