//! Terminal positions from the part move the wire ends: the wire list's
//! lengths change when a part knows where its terminals are.

use novo_core::{
    DeviceTag, Direction, MountingPlate, PinRef, Placement, Point as CorePoint, Project, RailKind,
    SymbolRole, Value,
};
use novo_routing::Rules;
use novo_routing::model::{Connection, Routed, route_connections};

const NO_SLACK: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 150.0,
    fill_limit: 0.5,
    bend_mm: 0.0,
};

/// Two 45 x 90 devices, one left of a vertical duct and one right, same part.
fn project(positions: Option<[[f64; 3]; 2]>, rotated: bool) -> (Project, Vec<PinRef>) {
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
            let duct = tx.add_rail(cab, 200.0, 20.0, 360.0, RailKind::Duct)?;
            tx.update::<novo_core::Rail>(duct, |r| r.vertical = true)?;
            tx.set_rail_size(duct, 360.0, 40.0, 60.0)?;
            let part = tx.add_part("ACME", "C1")?;
            for (k, v) in [("width_mm", 45.0), ("height_mm", 90.0), ("depth_mm", 70.0)] {
                tx.set_property(part, k, Some(Value::Float(v)))?;
            }
            if let Some(pos) = positions {
                for (name, at) in [("A", pos[0]), ("B", pos[1])] {
                    for (axis, v) in ["x", "y", "z"].iter().zip(at) {
                        tx.set_property(
                            part,
                            format!("pin.{name}.{axis}_mm"),
                            Some(Value::Float(v)),
                        )?;
                    }
                }
            }
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, x, name) in [("=A1+CAB1-K1", 50.0, "A"), ("=A1+CAB1-K2", 300.0, "B")] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, CorePoint::new(x, 100.0), rotated, None)?;
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(CorePoint::new(0.0, 0.0)),
                    vec![novo_core::Pin::new(name, 0.0, 0.0).with_direction(Direction::Up)],
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
    (p, pins)
}

fn run(p: &Project, pins: &[PinRef]) -> Routed {
    let c = [Connection {
        id: 1,
        from: pins[0],
        to: pins[1],
        diameter_mm: Some(3.0),
    }];
    route_connections(p, &c, &NO_SLACK)
}

#[test]
fn a_known_terminal_moves_the_wire_end_and_its_height() {
    let (p, pins) = project(None, false);
    let base = run(&p, &pins);
    // Terminal A at the bottom-left of the front, B at the top-right.
    let (p, pins) = project(Some([[5.0, 10.0, 60.0], [40.0, 80.0, 60.0]]), false);
    let routed = run(&p, &pins);
    let (a, b) = (&routed.routes[&1], &base.routes[&1]);
    assert_ne!(a.length_mm, b.length_mm);
    let first = a.points3.first().unwrap();
    // x 50 + 5, y 100 + 90 - 10, z from the part.
    assert_eq!((first.x, first.y, first.z), (55.0, 180.0, 60.0));
    let last = a.points3.last().unwrap();
    assert_eq!((last.x, last.y, last.z), (340.0, 110.0, 60.0));
}

#[test]
fn a_turned_device_turns_its_terminals_clockwise() {
    // Footprint 90 x 45 at (50, 100): centre (95, 122.5). A is at the left
    // edge, 10 above the bottom; turned clockwise it is on the top edge.
    let (p, pins) = project(Some([[0.0, 10.0, 60.0], [45.0, 90.0, 60.0]]), true);
    let routed = run(&p, &pins);
    let first = routed.routes[&1].points3.first().unwrap();
    assert_eq!((first.x, first.y), (95.0 + 10.0 - 45.0, 122.5 - 22.5));
}

#[test]
fn a_pin_the_part_does_not_know_keeps_the_edge_anchor() {
    // Only A has a position; B (facing up) keeps the middle of its top edge
    // at the part depth.
    let (mut p, pins) = project(Some([[5.0, 10.0, 60.0], [40.0, 80.0, 60.0]]), false);
    let part = p.parts().next().unwrap().id;
    p.transact("drop B", |tx| {
        for axis in ["x", "y", "z"] {
            tx.set_property(part, format!("pin.B.{axis}_mm"), None)?;
        }
        Ok(())
    })
    .unwrap();
    let routed = run(&p, &pins);
    let last = routed.routes[&1].points3.last().unwrap();
    assert_eq!((last.x, last.y, last.z), (322.5, 100.0, 70.0));
}
