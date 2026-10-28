//! Route all on the shipped example: every wire leaves a terminal on the edge
//! of its device, runs inside the ducts and arrives at a terminal; what no
//! duct reaches is listed, not drawn.

use super::gate1_tests::Drive;
use crate::actions::Action;
use novo_core::{CabinetPlacement, Project, Rail, RailKind};
use novo_routing::Kind;
use novo_routing::model::route_project;

fn placed_example() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.run_command(&ctx, "cabinet.place-all", None);
    d.frame();
    (dir, d)
}

/// The ducts of the plate as rectangles `(x0, y0, x1, y1)`.
fn duct_rects(p: &Project) -> Vec<(f64, f64, f64, f64)> {
    p.all::<Rail>()
        .filter(|r| r.kind == RailKind::Duct)
        .map(|r| {
            let h = r.width / 2.0;
            if r.vertical {
                (r.x - h, r.y, r.x + h, r.y + r.length)
            } else {
                (r.x, r.y - h, r.x + r.length, r.y + h)
            }
        })
        .collect()
}

fn inside(rects: &[(f64, f64, f64, f64)], p: novo_routing::Point) -> bool {
    let e = 0.5;
    rects
        .iter()
        .any(|r| p.x >= r.0 - e && p.x <= r.2 + e && p.y >= r.1 - e && p.y <= r.3 + e)
}

#[test]
fn every_routed_wire_stays_in_the_ducts_between_its_two_terminals() {
    let (_dir, d) = placed_example();
    let p = d.doc().project();
    let routed = route_project(p);
    let ducts = duct_rects(p);
    assert!(routed.routes.len() >= 8, "{} routes", routed.routes.len());
    let footprints: Vec<(f64, f64, f64, f64)> = p
        .all::<CabinetPlacement>()
        .map(|pl| {
            let dev = p.device(pl.device).unwrap();
            let s = novo_layout::model::device_size(p, dev);
            (pl.x, pl.y, pl.x + s.width, pl.y + s.height)
        })
        .collect();
    for (id, r) in &routed.routes {
        assert_eq!(r.kind, Kind::Ducted, "wire {id} has no duct path");
        let pts = &r.points;
        assert!(pts.len() >= 2, "wire {id}");
        // Both ends are terminals of a placed device, on its footprint (a
        // terminal of a strip sits along the strip, a pin of a device on the
        // edge it leaves through).
        for end in [pts[0], pts[pts.len() - 1]] {
            let on_device = footprints.iter().any(|f| {
                end.x >= f.0 - 0.5 && end.x <= f.2 + 0.5 && end.y >= f.1 - 0.5 && end.y <= f.3 + 0.5
            });
            assert!(
                on_device,
                "wire {id} ends at ({}, {}), on no device",
                end.x, end.y
            );
        }
        // Between the two stubs the wire lies in the ducts: every point but
        // the first and last two is inside a duct.
        let n = pts.len();
        for (i, pt) in pts.iter().enumerate() {
            if i >= 2 && i + 2 < n {
                assert!(
                    inside(&ducts, *pt),
                    "wire {id} point {i} ({}, {}) is outside the ducts",
                    pt.x,
                    pt.y
                );
            }
        }
        // The cut length is at least the path.
        assert!(
            r.length_mm >= novo_routing::polyline_length(pts),
            "wire {id}"
        );
    }
    // The terminals of one device are not one point: wires 1 3 5 of the
    // breaker start at different places.
    let starts: std::collections::BTreeSet<(i64, i64)> = routed
        .routes
        .values()
        .map(|r| (r.points[0].x.round() as i64, r.points[0].y.round() as i64))
        .collect();
    assert!(starts.len() >= 6, "{} distinct wire ends", starts.len());
}

#[test]
fn the_cabinet_says_what_it_estimated_and_what_leaves_the_cabinet() {
    let (_dir, d) = placed_example();
    let p = d.doc().project();
    let cab = p.structures().find(|s| s.enclosure).unwrap().id;
    let o = super::cabinet_routes::overlay(p, cab);
    assert!(o.routes.iter().all(|r| !r.free_air));
    assert!(
        o.notes
            .iter()
            .any(|n| n.contains("estimated terminal position")),
        "{:?}",
        o.notes
    );
    assert!(
        o.notes.iter().any(|n| n.contains("no entry point")),
        "{:?}",
        o.notes
    );
}
