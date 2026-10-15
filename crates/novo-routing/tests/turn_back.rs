use novo_routing::{Anchor, Duct, Facing, Job, Network, Point, Rules, polyline_length};

#[test]
fn a_wire_that_turns_back_keeps_its_turning_point() {
    // Both terminals face the duct but sit on one vertical line: the wire
    // runs down to the duct and back up, and the length counts both legs.
    let net = Network::new(&[Duct {
        id: 1,
        a: Point::new(0.0, 100.0),
        b: Point::new(400.0, 100.0),
        width: 40.0,
        height: 60.0,
    }]);
    let j = Job {
        id: 1,
        from: Anchor {
            pos: Point::new(50.0, 20.0),
            facing: Facing::Down,
        },
        to: Anchor {
            pos: Point::new(50.0, 60.0),
            facing: Facing::Down,
        },
        diameter_mm: None,
    };
    let r = net.route(
        &j,
        &Rules {
            slack_mm: 0.0,
            max_stub_mm: 150.0,
            fill_limit: 0.5,
        },
    );
    assert_eq!(r.length_mm, 80.0 + 40.0);
    assert_eq!(polyline_length(&r.points), r.length_mm);
    assert!(r.points.contains(&Point::new(50.0, 100.0)));
}
