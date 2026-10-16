//! The third dimension: terminals stand off the plate, wires lie in the duct.

use novo_core::{
    DeviceTag, Direction, MountingPlate, PinRef, Placement, Point as CorePoint, Project, RailKind,
    SymbolRole, Value,
};
use novo_routing::model::{Connection, route_connections};
use novo_routing::{Anchor, Duct, Facing, Job, Kind, Network, Point, Point3, Rules};

fn duct(height: f64) -> Duct {
    Duct {
        id: 1,
        a: Point::new(0.0, 100.0),
        b: Point::new(400.0, 100.0),
        width: 40.0,
        height,
    }
}

fn job(za: Option<f64>, zb: Option<f64>) -> Job {
    let anchor = |x, z_mm| Anchor {
        pos: Point::new(x, 70.0),
        facing: Facing::Down,
        z_mm,
    };
    Job {
        id: 1,
        from: anchor(50.0, za),
        to: anchor(350.0, zb),
        diameter_mm: None,
    }
}

const NO_SLACK: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 150.0,
    fill_limit: 0.5,
    bend_mm: 0.0,
};

#[test]
fn the_wire_rises_from_the_duct_to_the_terminal_at_both_ends() {
    let net = Network::new(&[duct(60.0)]);
    let r = net.route(&job(Some(50.0), Some(40.0)), &NO_SLACK);
    assert_eq!(r.kind, Kind::Ducted);
    // 30 down + 300 along + 30 up in plan; the duct holds the wire at 30 mm,
    // so it falls 20 at one end and rises 10 at the other.
    assert_eq!(r.rise_mm, 30.0);
    assert_eq!(r.length_mm, 360.0 + 30.0);
    assert_eq!(r.points3.first(), Some(&Point3::new(50.0, 70.0, 50.0)));
    assert_eq!(r.points3.last(), Some(&Point3::new(350.0, 70.0, 40.0)));
    assert!(r.points3.iter().any(|p| p.z == 30.0));
}

#[test]
fn an_unknown_height_is_flat_and_a_known_one_is_never_shorter() {
    let net = Network::new(&[duct(60.0)]);
    let flat = net.route(&job(None, None), &NO_SLACK);
    let high = net.route(&job(Some(80.0), Some(80.0)), &NO_SLACK);
    assert_eq!(
        flat.rise_mm, 60.0,
        "a flat terminal still drops to the duct"
    );
    assert_eq!(high.rise_mm, 100.0);
    assert!(high.length_mm > flat.length_mm);
}

#[test]
fn an_unsized_duct_holds_the_wire_on_the_plate() {
    let net = Network::new(&[duct(0.0)]);
    let r = net.route(&job(Some(25.0), Some(25.0)), &NO_SLACK);
    assert_eq!(r.rise_mm, 50.0);
}

#[test]
fn free_air_runs_at_the_higher_terminal() {
    let net = Network::new(&[]);
    let r = net.route(&job(Some(20.0), Some(45.0)), &NO_SLACK);
    assert_eq!(r.kind, Kind::FreeAir);
    // Plan 300 mm, up to 45 mm at the start and back down 0 at the far end.
    assert_eq!(r.rise_mm, 25.0 + 0.0);
    assert!(r.points3.iter().all(|p| p.z <= 45.0));
}

#[test]
fn each_bend_costs_its_allowance() {
    let net = Network::new(&[duct(60.0)]);
    let plain = net.route(&job(Some(30.0), Some(30.0)), &NO_SLACK);
    let rules = Rules {
        bend_mm: 15.0,
        ..NO_SLACK
    };
    let bent = net.route(&job(Some(30.0), Some(30.0)), &rules);
    assert_eq!(plain.bends, 2);
    assert_eq!(bent.length_mm, plain.length_mm + 2.0 * 15.0);
}

fn project(depth: Option<f64>) -> (Project, Vec<PinRef>) {
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
            let duct = tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
            tx.update::<novo_core::Rail>(duct, |r| r.vertical = true)?;
            tx.set_rail_size(duct, 200.0, 40.0, 60.0)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
            if let Some(d) = depth {
                tx.set_property(part, "depth_mm", Some(Value::Float(d)))?;
            }
            let page = tx.add_page("1")?;
            let mut pins = Vec::new();
            for (tag, y, dir) in [
                ("=A1+CAB1-K1", 50.0, Direction::Down),
                ("=A1+CAB1-K2", 250.0, Direction::Up),
            ] {
                let d = tx.add_device(DeviceTag::parse(tag).unwrap(), "")?;
                tx.assign_part(d, part)?;
                tx.place_device(d, cab, CorePoint::new(50.0, y), false, None)?;
                let symbol = tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(CorePoint::new(0.0, 0.0)),
                    vec![novo_core::Pin::new("1", 0.0, 0.0).with_direction(dir)],
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

fn run(p: &Project, pins: &[PinRef]) -> novo_routing::model::Routed {
    let c = [Connection {
        id: 1,
        from: pins[0],
        to: pins[1],
        diameter_mm: Some(3.0),
    }];
    route_connections(p, &c, &NO_SLACK)
}

#[test]
fn the_part_depth_is_the_terminal_height() {
    let (p, pins) = project(Some(75.0));
    let r = run(&p, &pins);
    assert!(r.unknown_height.is_empty());
    // 75 mm terminals, wires at 30 mm in the duct: 45 mm each end.
    assert_eq!(r.routes[&1].rise_mm, 90.0);
    assert_eq!(r.routes[&1].length_mm, 77.5 + 110.0 + 77.5 + 90.0);
}

#[test]
fn a_part_without_depth_is_reported_not_guessed() {
    let (p, pins) = project(None);
    let r = run(&p, &pins);
    assert_eq!(r.unknown_height, vec![1]);
    assert_eq!(r.routes[&1].rise_mm, 60.0);
}
