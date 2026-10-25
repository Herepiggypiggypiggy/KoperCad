use std::collections::BTreeMap;

use novo_routing::{Anchor, Duct, Facing, Job, Kind, Network, Point, Rules, plan, plan_avoiding};

fn duct(id: u64, y: f64) -> Duct {
    Duct {
        id,
        a: Point::new(0.0, y),
        b: Point::new(400.0, y),
        width: 40.0,
        height: 60.0,
    }
}

fn job() -> Job {
    let anchor = |x| Anchor {
        pos: Point::new(x, 120.0),
        facing: Facing::Any,
        z_mm: None,
    };
    Job {
        id: 7,
        from: anchor(50.0),
        to: anchor(350.0),
        diameter_mm: Some(3.0),
    }
}

const RULES: Rules = Rules {
    slack_mm: 0.0,
    max_stub_mm: 150.0,
    fill_limit: 0.5,
    bend_mm: 0.0,
};

fn avoid(ducts: &[u64]) -> BTreeMap<u64, Vec<u64>> {
    BTreeMap::from([(7, ducts.to_vec())])
}

#[test]
fn a_wire_asked_to_stay_out_of_a_duct_takes_the_other_one() {
    let net = Network::new(&[duct(1, 100.0), duct(2, 150.0)]);
    let jobs = [job()];
    let plain = plan(&net, &jobs, &RULES);
    assert_eq!(plain.routes[0].ducts, vec![1], "the nearer duct by default");
    let p = plan_avoiding(&net, &jobs, &RULES, &avoid(&[1]));
    assert_eq!(p.routes[0].kind, Kind::Ducted);
    assert_eq!(p.routes[0].ducts, vec![2]);
    // Asking for nothing is the plain plan.
    assert_eq!(plan_avoiding(&net, &jobs, &RULES, &BTreeMap::new()), plain);
}

#[test]
fn with_one_duct_only_the_wire_still_goes_through_it() {
    let net = Network::new(&[duct(1, 100.0)]);
    let p = plan_avoiding(&net, &[job()], &RULES, &avoid(&[1]));
    assert_eq!(p.routes[0].kind, Kind::Ducted);
    assert_eq!(p.routes[0].ducts, vec![1]);
}
