use std::collections::BTreeMap;

use novo_routing::{
    Anchor, Clearance, Duct, Facing, Job, Kind, Network, Point, Problem, Rules, plan,
    plan_constrained,
};

const RULES: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 400.0,
    fill_limit: 0.5,
    bend_mm: 0.0,
};

fn duct(id: u64, y: f64) -> Duct {
    Duct {
        id,
        a: Point::new(0.0, y),
        b: Point::new(400.0, y),
        width: 40.0,
        height: 60.0,
    }
}

fn job(id: u64, y: f64, d: f64) -> Job {
    let anchor = |x| Anchor {
        pos: Point::new(x, y),
        facing: Facing::Any,
        z_mm: None,
    };
    Job {
        id,
        from: anchor(50.0),
        to: anchor(350.0),
        diameter_mm: Some(d),
    }
}

fn keep(job: u64, other: u64, mm: f64) -> BTreeMap<u64, Vec<Clearance>> {
    BTreeMap::from([(job, vec![Clearance { other, mm }])])
}

/// Ducts at y = 100 and 300, 40 wide: 160 mm apart edge to edge, the wires between them.
fn net() -> Network {
    Network::new(&[duct(1, 100.0), duct(2, 300.0)])
}

#[test]
fn duct_gaps_are_edge_to_edge() {
    let n = net();
    assert_eq!(n.duct_gap(1, 2), Some(160.0));
    assert_eq!(n.duct_gap(2, 1), Some(160.0));
    assert_eq!(n.duct_gap(1, 1), Some(0.0));
    assert_eq!(n.duct_gap(1, 99), None);
}

#[test]
fn a_wire_keeps_out_of_ducts_closer_than_the_clearance_to_its_partner() {
    let jobs = [job(1, 200.0, 3.0), job(2, 200.0, 3.0)];
    let plain = plan(&net(), &jobs, &RULES);
    assert_eq!(plain.routes[0].ducts, vec![1]);
    assert_eq!(plain.routes[1].ducts, vec![1], "both in the nearest duct");
    // 100 mm apart: duct 2 is 160 from duct 1.
    let p = plan_constrained(&net(), &jobs, &RULES, &BTreeMap::new(), &keep(2, 1, 100.0));
    assert!(p.problems.is_empty(), "{:?}", p.problems);
    assert_eq!(p.routes[0].ducts, vec![1]);
    assert_eq!(p.routes[1].kind, Kind::Ducted);
    assert_eq!(p.routes[1].ducts, vec![2]);
}

#[test]
fn it_does_not_matter_which_of_the_two_asked() {
    let jobs = [job(1, 200.0, 3.0), job(2, 200.0, 3.0)];
    let a = plan_constrained(&net(), &jobs, &RULES, &BTreeMap::new(), &keep(2, 1, 100.0));
    let b = plan_constrained(&net(), &jobs, &RULES, &BTreeMap::new(), &keep(1, 2, 100.0));
    assert_eq!(a, b);
}

#[test]
fn a_clearance_nothing_can_meet_routes_the_wire_as_if_unasked_and_says_so() {
    let jobs = [job(1, 200.0, 3.0), job(2, 200.0, 3.0)];
    // 160 mm is all there is: every duct is forbidden for 400.
    let p = plan_constrained(&net(), &jobs, &RULES, &BTreeMap::new(), &keep(2, 1, 400.0));
    assert_eq!(p.routes[1].kind, Kind::Ducted, "never left unrouted");
    assert_eq!(p.routes[1].ducts, vec![1]);
    let [
        Problem::ClearanceUnmet {
            job,
            other,
            mm,
            gap_mm,
        },
    ] = p.problems.as_slice()
    else {
        panic!("{:?}", p.problems);
    };
    assert_eq!((*job, *other, *mm, *gap_mm), (2, 1, 400.0, 0.0));
}

#[test]
fn no_clearance_is_the_plain_plan() {
    let jobs = [job(1, 200.0, 3.0), job(2, 200.0, 3.0)];
    assert_eq!(
        plan_constrained(&net(), &jobs, &RULES, &BTreeMap::new(), &BTreeMap::new()),
        plan(&net(), &jobs, &RULES)
    );
}
