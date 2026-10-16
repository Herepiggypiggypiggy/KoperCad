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

fn job(id: u64, y: f64) -> Job {
    let anchor = |x| Anchor {
        pos: Point::new(x, y),
        facing: Facing::Any,
        z_mm: None,
    };
    Job {
        id,
        from: anchor(50.0),
        to: anchor(350.0),
        diameter_mm: Some(3.0),
    }
}

#[test]
fn an_unsized_duct_is_not_avoided_by_the_unloaded_route() {
    // Duct 1 has no cross-section yet (0 x 0) and is the near one; duct 2
    // is sized but 30 mm further. The geometric route must use duct 1.
    let net = Network::new(&[duct(1, 100.0, 0.0, 0.0), duct(2, 130.0, 40.0, 60.0)]);
    let r = net.route(&job(1, 90.0), &Rules::default());
    assert_eq!(r.ducts, vec![1]);
}

#[test]
fn an_unsized_duct_is_reported_not_called_overfull() {
    let net = Network::new(&[duct(1, 100.0, 0.0, 0.0)]);
    let p = plan(&net, &[job(1, 90.0), job(2, 90.0)], &Rules::default());
    assert_eq!(p.routes[0].ducts, vec![1]);
    assert_eq!(p.fill[0].ratio, 0.0);
    assert_eq!(
        p.problems,
        vec![Problem::UnsizedDuct {
            duct: 1,
            jobs: vec![1, 2]
        }]
    );
}

#[test]
fn a_repeated_duct_id_is_ignored_and_the_first_one_counts() {
    let net = Network::new(&[
        duct(5, 100.0, 40.0, 60.0),
        duct(5, 300.0, 40.0, 60.0),
        duct(6, 200.0, 40.0, 60.0),
    ]);
    assert_eq!(net.ignored, vec![5]);
    assert_eq!(net.duct_count(), 2);
    let p = plan(&net, &[job(1, 95.0)], &Rules::default());
    assert_eq!(p.routes[0].ducts, vec![5]);
    assert_eq!(
        p.fill.iter().map(|f| f.duct).collect::<Vec<_>>(),
        vec![5, 6]
    );
    assert_eq!(p.fill[0].jobs, vec![1]);
}
