use novo_routing::{
    Anchor, Duct, Facing, Job, Kind, Network, Point, Rules, polyline_length, route_all,
};

fn duct(id: u64, ax: f64, ay: f64, bx: f64, by: f64) -> Duct {
    Duct {
        id,
        a: Point::new(ax, ay),
        b: Point::new(bx, by),
        width: 40.0,
        height: 60.0,
    }
}

fn anchor(x: f64, y: f64, facing: Facing) -> Anchor {
    Anchor {
        pos: Point::new(x, y),
        facing,
    }
}

fn job(id: u64, from: Anchor, to: Anchor) -> Job {
    Job { id, from, to }
}

const NO_SLACK: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 150.0,
};

#[test]
fn two_terminals_above_one_duct_run_down_along_and_up() {
    let net = Network::new(&[duct(1, 0.0, 100.0, 400.0, 100.0)]);
    let j = job(
        1,
        anchor(50.0, 40.0, Facing::Down),
        anchor(350.0, 40.0, Facing::Down),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(r.kind, Kind::Ducted);
    assert_eq!(r.ducts, vec![1]);
    assert_eq!(r.length_mm, 60.0 + 300.0 + 60.0);
    assert_eq!(r.detour_mm, 0.0);
    assert_eq!(polyline_length(&r.points), r.length_mm);
}

#[test]
fn slack_is_added_once_per_end() {
    let net = Network::new(&[duct(1, 0.0, 100.0, 400.0, 100.0)]);
    let j = job(
        1,
        anchor(50.0, 40.0, Facing::Down),
        anchor(350.0, 40.0, Facing::Down),
    );
    let r = net.route(
        &j,
        &Rules {
            slack_mm: 25.0,
            ..NO_SLACK
        },
    );
    assert_eq!(r.length_mm, 420.0 + 50.0);
}

#[test]
fn a_wire_changes_duct_where_ducts_cross() {
    // H duct along y=100 and V duct along x=300 cross at (300,100).
    let net = Network::new(&[
        duct(1, 0.0, 100.0, 400.0, 100.0),
        duct(2, 300.0, 0.0, 300.0, 300.0),
    ]);
    let j = job(
        1,
        anchor(50.0, 60.0, Facing::Down),
        anchor(340.0, 250.0, Facing::Left),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(r.kind, Kind::Ducted);
    assert_eq!(r.ducts, vec![1, 2]);
    // 40 down, 250 along duct 1 to x=300, 150 down duct 2, 40 across.
    assert_eq!(r.length_mm, 40.0 + 250.0 + 150.0 + 40.0);
}

#[test]
fn a_duct_ending_on_another_joins_it() {
    // T junction: duct 2 ends on the side of duct 1.
    let net = Network::new(&[
        duct(1, 0.0, 100.0, 400.0, 100.0),
        duct(2, 200.0, 100.0, 200.0, 300.0),
    ]);
    let j = job(
        1,
        anchor(20.0, 70.0, Facing::Down),
        anchor(230.0, 280.0, Facing::Left),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(r.ducts, vec![1, 2]);
    assert_eq!(r.length_mm, 30.0 + 180.0 + 180.0 + 30.0);
}

#[test]
fn the_shorter_of_two_ways_round_wins() {
    // Two parallel runs between the same two vertical ducts; the lower
    // path is the longer way round.
    let net = Network::new(&[
        duct(1, 0.0, 0.0, 0.0, 400.0),
        duct(2, 300.0, 0.0, 300.0, 400.0),
        duct(3, 0.0, 0.0, 300.0, 0.0),
        duct(4, 0.0, 400.0, 300.0, 400.0),
    ]);
    let j = job(
        1,
        anchor(-30.0, 100.0, Facing::Right),
        anchor(330.0, 100.0, Facing::Left),
    );
    let r = net.route(&j, &NO_SLACK);
    // Along the top: 100 up, 300 across, 100 down, plus two 30 mm stubs.
    assert_eq!(r.length_mm, 30.0 + 100.0 + 300.0 + 100.0 + 30.0);
    assert!(r.ducts.contains(&3) && !r.ducts.contains(&4));
}

#[test]
fn both_ends_on_the_same_duct_never_leave_it() {
    let net = Network::new(&[duct(1, 0.0, 100.0, 400.0, 100.0)]);
    let j = job(
        1,
        anchor(100.0, 80.0, Facing::Down),
        anchor(200.0, 120.0, Facing::Up),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(r.ducts, vec![1]);
    assert_eq!(r.length_mm, 20.0 + 100.0 + 20.0);
}

#[test]
fn no_ducts_gives_a_flagged_free_air_route_never_zero() {
    let net = Network::new(&[]);
    let j = job(
        1,
        anchor(0.0, 0.0, Facing::Down),
        anchor(100.0, 200.0, Facing::Up),
    );
    let r = net.route(&j, &Rules::default());
    assert_eq!(r.kind, Kind::FreeAir);
    assert!(r.ducts.is_empty());
    assert_eq!(r.length_mm, 300.0 + 100.0);
    assert_eq!(r.points.first(), Some(&Point::new(0.0, 0.0)));
    assert_eq!(r.points.last(), Some(&Point::new(100.0, 200.0)));
}

#[test]
fn disconnected_networks_fall_back_to_free_air() {
    let net = Network::new(&[
        duct(1, 0.0, 100.0, 100.0, 100.0),
        duct(2, 500.0, 100.0, 600.0, 100.0),
    ]);
    let j = job(
        1,
        anchor(50.0, 60.0, Facing::Down),
        anchor(550.0, 60.0, Facing::Down),
    );
    assert_eq!(net.route(&j, &NO_SLACK).kind, Kind::FreeAir);
}

#[test]
fn a_terminal_facing_away_from_the_duct_pays_a_detour() {
    let net = Network::new(&[duct(1, 0.0, 100.0, 400.0, 100.0)]);
    let j = job(
        1,
        anchor(50.0, 60.0, Facing::Up),
        anchor(350.0, 60.0, Facing::Down),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(r.detour_mm, 80.0);
    assert_eq!(r.length_mm, 40.0 + 300.0 + 40.0 + 80.0);
}

#[test]
fn diagonal_and_zero_length_ducts_are_ignored_and_reported() {
    let net = Network::new(&[
        duct(1, 0.0, 0.0, 100.0, 100.0),
        duct(2, 5.0, 5.0, 5.0, 5.0),
        duct(3, 0.0, 100.0, 400.0, 100.0),
    ]);
    assert_eq!(net.ignored, vec![1, 2]);
    let j = job(
        1,
        anchor(0.0, 60.0, Facing::Down),
        anchor(400.0, 60.0, Facing::Down),
    );
    assert_eq!(net.route(&j, &NO_SLACK).ducts, vec![3]);
}

#[test]
fn routing_is_deterministic_and_ignores_duct_input_order() {
    let ducts = [
        duct(1, 0.0, 100.0, 400.0, 100.0),
        duct(2, 0.0, 120.0, 400.0, 120.0),
    ];
    // Both ducts are equally near, so the tie goes to the lower id.
    let jobs = [job(
        1,
        anchor(50.0, 110.0, Facing::Any),
        anchor(350.0, 110.0, Facing::Any),
    )];
    let a = route_all(&Network::new(&ducts), &jobs, &NO_SLACK);
    let rev = [ducts[1].clone(), ducts[0].clone()];
    let b = route_all(&Network::new(&rev), &jobs, &NO_SLACK);
    assert_eq!(a, b);
    assert_eq!(a[0].ducts, vec![1]);
}

#[test]
fn route_all_keeps_job_order() {
    let net = Network::new(&[duct(1, 0.0, 100.0, 400.0, 100.0)]);
    let jobs = [
        job(
            9,
            anchor(0.0, 80.0, Facing::Down),
            anchor(100.0, 80.0, Facing::Down),
        ),
        job(
            3,
            anchor(0.0, 80.0, Facing::Down),
            anchor(300.0, 80.0, Facing::Down),
        ),
    ];
    let r = route_all(&net, &jobs, &NO_SLACK);
    assert_eq!(r.iter().map(|r| r.job).collect::<Vec<_>>(), vec![9, 3]);
}

#[test]
fn non_finite_ducts_are_ignored_not_a_panic() {
    let net = Network::new(&[
        duct(1, f64::NAN, 100.0, f64::NAN, 100.0),
        duct(2, 0.0, f64::NAN, 400.0, f64::NAN),
        duct(3, 0.0, 100.0, 400.0, 100.0),
    ]);
    assert_eq!(net.ignored, vec![1, 2]);
    let j = job(
        1,
        anchor(50.0, 40.0, Facing::Down),
        anchor(350.0, 40.0, Facing::Down),
    );
    assert_eq!(net.route(&j, &NO_SLACK).ducts, vec![3]);
}

#[test]
fn route_points_keep_only_bends_and_ends() {
    // Crossing ducts: the wire runs straight through the crossing at (300,100).
    let net = Network::new(&[
        duct(1, 0.0, 100.0, 500.0, 100.0),
        duct(2, 300.0, 80.0, 300.0, 120.0),
    ]);
    let j = job(
        1,
        anchor(50.0, 60.0, Facing::Down),
        anchor(450.0, 60.0, Facing::Down),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(
        r.points,
        vec![
            Point::new(50.0, 60.0),
            Point::new(50.0, 100.0),
            Point::new(450.0, 100.0),
            Point::new(450.0, 60.0),
        ]
    );
    assert_eq!(r.length_mm, 40.0 + 400.0 + 40.0);
}

#[test]
fn a_wire_does_not_cross_the_plate_in_free_air_to_save_duct_length() {
    // Ducts 1 and 2 touch; the far end of duct 2 is 250 mm from each
    // terminal in a straight line, which would beat 400 mm along duct 1.
    let net = Network::new(&[
        duct(1, 0.0, 100.0, 500.0, 100.0),
        duct(2, 300.0, 80.0, 300.0, 120.0),
    ]);
    let j = job(
        1,
        anchor(50.0, 60.0, Facing::Down),
        anchor(450.0, 60.0, Facing::Down),
    );
    let r = net.route(&j, &NO_SLACK);
    assert_eq!(r.length_mm, 480.0);
    let far = Rules {
        max_stub_mm: 1000.0,
        ..NO_SLACK
    };
    assert!(net.route(&j, &far).length_mm < 480.0);
}
