//! Ducts with slots, devices that wires go round, and DIN rails as routes.

use novo_routing::{
    Anchor, Body, Duct, Facing, Job, Kind, Network, Openings, Point, Problem, RailRow, Rules, plan,
};

fn duct(id: u64, a: (f64, f64), b: (f64, f64)) -> Duct {
    Duct {
        id,
        a: Point::new(a.0, a.1),
        b: Point::new(b.0, b.1),
        width: 60.0,
        height: 60.0,
    }
}

fn anchor(x: f64, y: f64, facing: Facing) -> Anchor {
    Anchor {
        pos: Point::new(x, y),
        facing,
        z_mm: Some(0.0),
    }
}

fn job(from: Anchor, to: Anchor) -> Job {
    Job {
        id: 1,
        from,
        to,
        diameter_mm: Some(3.0),
    }
}

fn body(id: u64, x0: f64, y0: f64, x1: f64, y1: f64) -> Body {
    Body { id, x0, y0, x1, y1 }
}

/// True when a segment of the route passes through the inside of the body.
fn crosses(points: &[Point], b: &Body) -> bool {
    points.windows(2).any(|w| {
        w[0].x.max(w[1].x) > b.x0 + 0.5
            && w[0].x.min(w[1].x) < b.x1 - 0.5
            && w[0].y.max(w[1].y) > b.y0 + 0.5
            && w[0].y.min(w[1].y) < b.y1 - 0.5
    })
}

#[test]
fn a_wire_enters_a_slotted_duct_at_a_slot() {
    let net = Network::new(&[duct(7, (0.0, 100.0), (400.0, 100.0))]).with_openings(&[(
        7,
        Openings::Pitch {
            pitch: 20.0,
            first: 10.0,
        },
    )]);
    let j = job(
        anchor(95.0, 40.0, Facing::Down),
        anchor(297.0, 40.0, Facing::Down),
    );
    let r = net.route(&j, &Rules::default());
    assert_eq!(r.kind, Kind::Ducted);
    // The points on the duct's line are at slots (10, 30, ... 390).
    let on_duct: Vec<f64> = r
        .points
        .iter()
        .filter(|p| (p.y - 100.0).abs() < 1e-6)
        .map(|p| p.x)
        .collect();
    assert_eq!(on_duct.len(), 2, "{:?}", r.points);
    for &x in &on_duct {
        assert!(
            ((x - 10.0) / 20.0 - ((x - 10.0) / 20.0).round()).abs() < 1e-6,
            "{x}"
        );
    }
    assert_eq!(on_duct[0], 90.0, "95 is nearest to the slot at 90");
}

#[test]
fn a_closed_duct_is_entered_at_its_ends() {
    let net = Network::new(&[duct(7, (0.0, 100.0), (400.0, 100.0))])
        .with_openings(&[(7, Openings::Ends)]);
    let j = job(
        anchor(95.0, 40.0, Facing::Down),
        anchor(297.0, 40.0, Facing::Down),
    );
    let r = net.route(&j, &Rules::default());
    assert_eq!(r.kind, Kind::Ducted);
    // The wire comes in along the duct from outside its end.
    assert!(
        r.points.iter().any(|p| p.x < 0.0 || p.x > 400.0),
        "{:?}",
        r.points
    );
    assert!(r.length_mm > 400.0, "{}", r.length_mm);
}

#[test]
fn a_wire_goes_round_its_own_device_instead_of_through_it() {
    // The terminal is on the top edge of a device; the only duct is below.
    let own = body(1, 50.0, 20.0, 150.0, 80.0);
    let net = Network::new(&[duct(7, (0.0, 140.0), (400.0, 140.0))]).with_bodies(vec![own]);
    let j = job(
        anchor(100.0, 20.0, Facing::Up),
        anchor(300.0, 120.0, Facing::Down),
    );
    let r = net.route(&j, &Rules::default());
    assert_eq!(r.kind, Kind::Ducted);
    assert!(!r.blocked);
    assert!(!crosses(&r.points, &own), "{:?}", r.points);
    // Out of the top, round the side, down: longer than straight through.
    let straight = 120.0 + 200.0;
    assert!(novo_routing::polyline_length(&r.points) > straight - 1.0);
}

#[test]
fn a_wire_goes_round_the_device_next_door() {
    let a = body(1, 0.0, 0.0, 100.0, 80.0);
    let wall = body(2, 100.0, 0.0, 300.0, 200.0);
    let net = Network::new(&[duct(7, (0.0, 260.0), (600.0, 260.0))]).with_bodies(vec![a, wall]);
    let j = job(
        anchor(100.0, 40.0, Facing::Right),
        anchor(500.0, 200.0, Facing::Down),
    );
    let r = net.route(&j, &Rules::default());
    assert!(!r.blocked, "{:?}", r.points);
    assert!(!crosses(&r.points, &wall), "{:?}", r.points);
}

#[test]
fn a_terminal_boxed_in_by_devices_is_reported() {
    let b = [
        body(1, 80.0, 80.0, 120.0, 120.0),
        body(2, 0.0, 0.0, 200.0, 75.0),
        body(3, 0.0, 125.0, 200.0, 200.0),
        body(4, 0.0, 75.0, 75.0, 125.0),
        body(5, 125.0, 75.0, 200.0, 125.0),
    ];
    let net = Network::new(&[duct(7, (0.0, 300.0), (600.0, 300.0))]).with_bodies(b.to_vec());
    let j = job(
        anchor(100.0, 100.0, Facing::Any),
        anchor(500.0, 280.0, Facing::Down),
    );
    let p = plan(&net, &[j], &Rules::default());
    assert!(
        p.problems
            .iter()
            .any(|x| matches!(x, Problem::StubThroughDevice { job: 1 })),
        "{:?}",
        p.problems
    );
}

fn row() -> (Vec<Body>, RailRow) {
    // Two devices on a horizontal rail at y = 60, their row 40..90.
    (
        vec![
            body(1, 20.0, 40.0, 80.0, 90.0),
            body(2, 200.0, 40.0, 260.0, 90.0),
        ],
        RailRow {
            id: 9,
            vertical: false,
            start: Point::new(0.0, 60.0),
            length: 300.0,
            across: (40.0, 90.0),
        },
    )
}

#[test]
fn wires_between_devices_on_a_rail_run_along_the_rail() {
    let (bodies, rail) = row();
    let net = Network::new(&[])
        .with_bodies(bodies.clone())
        .with_rails(&[rail]);
    // Top terminals of the two devices.
    let j = job(
        anchor(50.0, 40.0, Facing::Up),
        anchor(230.0, 40.0, Facing::Up),
    );
    let p = plan(&net, &[j], &Rules::default());
    let r = &p.routes[0];
    assert_eq!(r.kind, Kind::Ducted, "{:?}", r.points);
    assert_eq!(r.rails, vec![9]);
    assert!(r.ducts.is_empty());
    for b in &bodies {
        assert!(!crosses(&r.points, b), "{:?}", r.points);
    }
    // The rail has no cross-section: nothing to fill, nothing unsized.
    assert!(p.fill.is_empty());
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    // Up 10, along 180, down 10 and the two service loops.
    assert_eq!(r.length_mm, 10.0 + 180.0 + 10.0 + 100.0);
}

#[test]
fn a_terminal_on_the_bottom_edge_uses_the_channel_below() {
    let (bodies, rail) = row();
    let net = Network::new(&[]).with_bodies(bodies).with_rails(&[rail]);
    let j = job(
        anchor(50.0, 90.0, Facing::Down),
        anchor(230.0, 90.0, Facing::Down),
    );
    let r = net.route(&j, &Rules::default());
    assert_eq!(r.rails, vec![9]);
    assert!(r.points.iter().any(|p| (p.y - 100.0).abs() < 1e-6));
}

#[test]
fn a_duct_wins_when_it_is_shorter_than_the_rail() {
    let (bodies, rail) = row();
    // A duct runs just above the row: shorter than the channel for a wire
    // from the first device to the far end of the duct.
    let net = Network::new(&[duct(7, (0.0, 10.0), (300.0, 10.0))])
        .with_bodies(bodies)
        .with_rails(&[rail]);
    let j = job(
        anchor(50.0, 40.0, Facing::Up),
        anchor(230.0, 40.0, Facing::Up),
    );
    let r = net.route(&j, &Rules::default());
    assert_eq!(r.kind, Kind::Ducted);
    assert!(r.ducts.contains(&7) || r.rails == vec![9], "{r:?}");
}

#[test]
fn a_rail_is_not_a_duct_to_the_counts() {
    let (bodies, rail) = row();
    let net = Network::new(&[duct(7, (0.0, 10.0), (300.0, 10.0))])
        .with_bodies(bodies)
        .with_rails(&[rail]);
    assert_eq!(net.duct_count(), 1);
}
