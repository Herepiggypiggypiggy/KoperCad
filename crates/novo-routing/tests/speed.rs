//! A big plate routes in reasonable time: 360 devices, 1500 wires.

use std::time::Instant;

use novo_routing::{Anchor, Body, Duct, Facing, Job, Network, Point, Rules, plan};

fn big() -> (Network, Vec<Job>) {
    // 12 rows of 30 devices (45 x 90 mm), a duct above each row and a trunk.
    let mut bodies = Vec::new();
    let mut ducts = Vec::new();
    for row in 0..12 {
        let y = 150.0 + row as f64 * 200.0;
        ducts.push(Duct {
            id: 100 + row,
            a: Point::new(0.0, y - 40.0),
            b: Point::new(1600.0, y - 40.0),
            width: 40.0,
            height: 60.0,
        });
        for col in 0..30 {
            bodies.push(Body {
                id: row * 30 + col,
                x0: 20.0 + col as f64 * 52.0,
                y0: y,
                x1: 65.0 + col as f64 * 52.0,
                y1: y + 90.0,
            });
        }
    }
    ducts.push(Duct {
        id: 1,
        a: Point::new(10.0, 0.0),
        b: Point::new(10.0, 2500.0),
        width: 40.0,
        height: 60.0,
    });
    let net = Network::new(&ducts).with_bodies(bodies.clone());
    let jobs = (0..1500u64)
        .map(|n| {
            let (a, b) = (
                &bodies[(n * 7 % 360) as usize],
                &bodies[((n * 13 + 5) % 360) as usize],
            );
            let at = |b: &Body| Anchor {
                pos: Point::new((b.x0 + b.x1) / 2.0, b.y0),
                facing: Facing::Up,
                z_mm: Some(0.0),
            };
            Job {
                id: n,
                from: at(a),
                to: at(b),
                diameter_mm: Some(3.0),
            }
        })
        .collect();
    (net, jobs)
}

#[test]
fn fifteen_hundred_wires_on_three_hundred_sixty_devices_route_in_seconds() {
    let (net, jobs) = big();
    let t = Instant::now();
    let p = plan(&net, &jobs, &Rules::default());
    let took = t.elapsed().as_secs_f64();
    eprintln!("routed {} wires in {took:.2} s", p.routes.len());
    assert_eq!(p.routes.len(), 1500);
    // Unoptimised test build on a shared runner (0.5 s here): generous, but a
    // stub path worked out for every duct and rail of the plate would not fit.
    assert!(took < 6.0, "took {took:.1} s");
}
