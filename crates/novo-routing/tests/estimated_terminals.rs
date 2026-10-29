//! A part without terminal data still gets a wire end per terminal: on the
//! edge of the device the pin leaves through, in marking order, flagged as an
//! estimate, and the wire runs in the duct on that side.

use novo_core::{
    DeviceTag, MountingPlate, Pin, PinRef, Placement, Point as CorePoint, Project, RailKind,
    SymbolRole, Value,
};
use novo_routing::model::{Connection, Routed, route_connections};
use novo_routing::{Kind, Rules};

const NO_SLACK: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 150.0,
    fill_limit: 0.5,
    bend_mm: 0.0,
};

/// Two 45 x 90 contactors (pins 1 3 5 above, 2 4 6 below, no library
/// direction) at x 50 and x 250, a horizontal duct above and below them and
/// a vertical trunk at the left that is nearer to the terminals than either.
fn project(with_positions: bool) -> (Project, Vec<[PinRef; 6]>) {
    let mut p = Project::new("t");
    let (devices, _) = p
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
            for (x, y, length, vertical) in [
                (20.0, 60.0, 360.0, false),
                (20.0, 230.0, 360.0, false),
                (40.0, 20.0, 360.0, true),
            ] {
                let d = tx.add_rail(cab, x, y, length, RailKind::Duct)?;
                tx.update::<novo_core::Rail>(d, |r| r.vertical = vertical)?;
                tx.set_rail_size(d, length, 30.0, 60.0)?;
            }
            let part = tx.add_part("ACME", "C1")?;
            for (k, v) in [("width_mm", 45.0), ("height_mm", 90.0), ("depth_mm", 70.0)] {
                tx.set_property(part, k, Some(Value::Float(v)))?;
            }
            if with_positions {
                for (i, name) in ["1", "3", "5", "2", "4", "6"].iter().enumerate() {
                    let (x, y) = (5.0 + 17.0 * (i % 3) as f64, if i < 3 { 85.0 } else { 5.0 });
                    for (axis, v) in ["x", "y", "z"].iter().zip([x, y, 60.0]) {
                        tx.set_property(
                            part,
                            format!("pin.{name}.{axis}_mm"),
                            Some(Value::Float(v)),
                        )?;
                    }
                }
            }
            let page = tx.add_page("1")?;
            let mut devices = Vec::new();
            for (tag, x) in [("=A1+CAB1-K1", 50.0), ("=A1+CAB1-K2", 250.0)] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, CorePoint::new(x, 100.0), false, None)?;
                let pins = ["1", "3", "5", "2", "4", "6"]
                    .iter()
                    .enumerate()
                    .map(|(i, n)| {
                        Pin::new(*n, 10.0 * (i % 3) as f64, if i < 3 { 0.0 } else { 20.0 })
                    })
                    .collect();
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(CorePoint::new(0.0, 0.0)),
                    pins,
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )?;
                let mut six = [PinRef::new(symbol, 0); 6];
                for (i, slot) in six.iter_mut().enumerate() {
                    *slot = PinRef::new(symbol, i as u16);
                }
                devices.push(six);
            }
            Ok(devices)
        })
        .unwrap();
    (p, devices)
}

fn run(p: &Project, d: &[[PinRef; 6]]) -> Routed {
    // 1 to 1, 3 to 3, 5 to 5 across the top; 2 to 2 below.
    let c: Vec<Connection> = [0usize, 1, 2, 3]
        .iter()
        .map(|&i| Connection {
            id: i as u64 + 1,
            from: d[0][i],
            to: d[1][i],
            diameter_mm: Some(3.0),
        })
        .collect();
    route_connections(p, &c, &NO_SLACK)
}

#[test]
fn each_terminal_has_its_own_place_on_the_edge_it_leaves_through() {
    let (p, d) = project(false);
    let routed = run(&p, &d);
    let start = |id: u64| routed.routes[&id].points[0];
    // Pins 1, 3, 5 on the top edge (y 100) of K1 (x 50..95), spread evenly in
    // marking order; pin 2 on the bottom edge (y 190).
    let xs: Vec<f64> = (1..=3).map(|id| start(id).x).collect();
    assert_eq!(
        xs,
        vec![
            50.0 + 45.0 / 6.0,
            50.0 + 45.0 / 2.0,
            50.0 + 45.0 * 5.0 / 6.0
        ]
    );
    assert!((1..=3).all(|id| start(id).y == 100.0));
    assert_eq!(start(4).y, 190.0);
    // Every wire says its ends are estimates.
    assert_eq!(routed.estimated, vec![1, 2, 3, 4]);
}

#[test]
fn a_wire_leaves_by_the_duct_on_its_own_side_not_along_the_row() {
    let (p, d) = project(false);
    let routed = run(&p, &d);
    for id in 1..=3u64 {
        let r = &routed.routes[&id];
        assert_eq!(r.kind, Kind::Ducted);
        // The duct above (the first rail added), not the trunk at the left:
        // the wire never runs sideways past its neighbours.
        assert_eq!(r.ducts.len(), 1, "wire {id}: {:?}", r.ducts);
        assert!(r.points.iter().all(|pt| pt.x >= 50.0), "{:?}", r.points);
    }
}

#[test]
fn a_part_that_knows_its_terminals_is_not_an_estimate() {
    let (p, d) = project(true);
    let routed = run(&p, &d);
    assert!(routed.estimated.is_empty(), "{:?}", routed.estimated);
}
