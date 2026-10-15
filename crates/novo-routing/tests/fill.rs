use novo_routing::{Anchor, Duct, Facing, Job, Network, Point, Problem, Rules, plan};

fn duct(id: u64, ay: f64, w: f64, h: f64) -> Duct {
    Duct {
        id,
        a: Point::new(0.0, ay),
        b: Point::new(400.0, ay),
        width: w,
        height: h,
    }
}

fn job(id: u64, x0: f64, x1: f64, y: f64, d: Option<f64>) -> Job {
    let anchor = |x| Anchor {
        pos: Point::new(x, y),
        facing: Facing::Any,
    };
    Job {
        id,
        from: anchor(x0),
        to: anchor(x1),
        diameter_mm: d,
    }
}

const RULES: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 150.0,
    fill_limit: 0.5,
};

fn wire_area(d: f64) -> f64 {
    std::f64::consts::FRAC_PI_4 * d * d
}

#[test]
fn fill_adds_up_wire_cross_sections_against_the_allowed_share() {
    let net = Network::new(&[duct(1, 100.0, 40.0, 60.0)]);
    let jobs = [
        job(1, 50.0, 350.0, 90.0, Some(4.0)),
        job(2, 60.0, 340.0, 90.0, Some(2.0)),
    ];
    let p = plan(&net, &jobs, &RULES);
    let f = &p.fill[0];
    assert_eq!(f.duct, 1);
    assert!((f.used_mm2 - (wire_area(4.0) + wire_area(2.0))).abs() < 1e-9);
    assert_eq!(f.capacity_mm2, 40.0 * 60.0 * 0.5);
    assert!((f.ratio - f.used_mm2 / 1200.0).abs() < 1e-12);
    assert_eq!(f.jobs, vec![1, 2]);
    assert!(p.problems.is_empty());
}

#[test]
fn a_duct_past_its_limit_is_reported_with_the_wires_in_it() {
    // 10 x 10 duct: 100 mm2, 50 mm2 allowed; eight 3 mm wires need 56.5.
    let net = Network::new(&[duct(1, 100.0, 10.0, 10.0)]);
    let jobs: Vec<Job> = (1..=8)
        .map(|i| job(i, 50.0, 350.0, 90.0, Some(3.0)))
        .collect();
    let p = plan(&net, &jobs, &RULES);
    let [Problem::OverfullDuct { duct, ratio, jobs }] = p.problems.as_slice() else {
        panic!("expected one overfull duct, got {:?}", p.problems);
    };
    assert_eq!(*duct, 1);
    assert!(*ratio > 1.0);
    assert_eq!(jobs.len(), 8);
}

#[test]
fn wires_spread_to_a_second_duct_before_the_first_overflows() {
    // Two equally near ducts of 200 mm2 (100 allowed each). Twenty 3 mm
    // wires (141 mm2) fit across both but not in the first alone.
    let net = Network::new(&[duct(1, 100.0, 10.0, 20.0), duct(2, 110.0, 10.0, 20.0)]);
    let jobs: Vec<Job> = (1..=20)
        .map(|i| job(i, 50.0, 350.0, 105.0, Some(3.0)))
        .collect();
    let p = plan(&net, &jobs, &RULES);
    assert!(
        p.problems.is_empty(),
        "unexpected problems: {:?}",
        p.problems
    );
    assert!(p.fill.iter().all(|f| !f.jobs.is_empty()));
    assert!(p.fill.iter().all(|f| f.ratio <= 1.0));
}

#[test]
fn the_longest_wire_is_routed_first_whatever_the_input_order() {
    let net = Network::new(&[duct(1, 100.0, 40.0, 60.0)]);
    let short = job(1, 100.0, 150.0, 90.0, Some(3.0));
    let long = job(2, 10.0, 390.0, 90.0, Some(3.0));
    let a = plan(&net, &[short.clone(), long.clone()], &RULES);
    let b = plan(&net, &[long.clone(), short.clone()], &RULES);
    assert_eq!(a.fill[0].jobs, vec![2, 1]);
    assert_eq!(b.fill[0].jobs, vec![2, 1]);
    // Routes come back in input order.
    assert_eq!(
        a.routes.iter().map(|r| r.job).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(
        b.routes.iter().map(|r| r.job).collect::<Vec<_>>(),
        vec![2, 1]
    );
    assert_eq!(a.fill, b.fill);
}

#[test]
fn free_air_and_guessed_wires_are_reported() {
    let net = Network::new(&[]);
    let jobs = [
        job(7, 0.0, 100.0, 0.0, Some(3.0)),
        job(3, 0.0, 100.0, 50.0, None),
    ];
    let p = plan(&net, &jobs, &RULES);
    assert_eq!(
        p.problems,
        vec![
            Problem::FreeAir { job: 7 },
            Problem::FreeAir { job: 3 },
            Problem::GuessedWire { job: 3 },
        ]
    );
    assert!(p.fill.is_empty());
}
