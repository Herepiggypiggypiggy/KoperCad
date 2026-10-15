use novo_core::{
    DeviceTag, Direction, MountingPlate, Pin, PinRef, Placement, Point as CorePoint, Project,
    RailKind, Rotation, SymbolRole, Value,
};
use novo_routing::Rules;
use novo_routing::model::{Connection, route_connections};

/// Two devices on a plate with a vertical duct between them. K1's symbol is
/// drawn turned 90 degrees on the page; its pin leaves downwards in the
/// library.
fn project(rotation: Rotation) -> (Project, PinRef, PinRef) {
    let mut p = Project::new("t");
    let (pins, _) = p
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
            tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, y, dir, rot) in [
                ("=A1+CAB1-K1", 50.0, Direction::Down, rotation),
                ("=A1+CAB1-K2", 250.0, Direction::Up, Rotation::R0),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, CorePoint::new(50.0, y), false, None)?;
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement {
                        rotation: rot,
                        ..Placement::at(CorePoint::new(0.0, 0.0))
                    },
                    vec![Pin::new("1", 0.0, 0.0).with_direction(dir)],
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                pins.push(PinRef::new(symbol, 0));
            }
            Ok(pins)
        })
        .unwrap();
    (p, pins[0], pins[1])
}

#[test]
fn the_side_a_wire_leaves_does_not_depend_on_how_the_symbol_is_drawn() {
    let (upright, a, b) = project(Rotation::R0);
    let (sideways, c, d) = project(Rotation::R90);
    let conn = |from, to| {
        [Connection {
            id: 1,
            from,
            to,
            diameter_mm: None,
        }]
    };
    let one = route_connections(&upright, &conn(a, b), &Rules::default());
    let two = route_connections(&sideways, &conn(c, d), &Rules::default());
    assert_eq!(one.routes[&1].points, two.routes[&1].points);
    assert_eq!(one.routes[&1].length_mm, two.routes[&1].length_mm);
}

#[test]
fn connections_sharing_an_id_are_all_left_unrouted() {
    let (p, a, b) = project(Rotation::R0);
    let conns = [
        Connection {
            id: 4,
            from: a,
            to: b,
            diameter_mm: None,
        },
        Connection {
            id: 4,
            from: b,
            to: a,
            diameter_mm: None,
        },
        Connection {
            id: 5,
            from: a,
            to: b,
            diameter_mm: None,
        },
    ];
    let r = route_connections(&p, &conns, &Rules::default());
    assert_eq!(r.unrouted, vec![4, 4]);
    assert_eq!(r.routes.keys().copied().collect::<Vec<_>>(), vec![5]);
}
