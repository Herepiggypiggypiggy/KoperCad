//! The wiring rule that holds in every wiring mode (DESIGN §4 "Wiring
//! modes"): two segments of different conductors never share length. They
//! may cross at a point or meet at a pin or a junction.
//!
//! The mode-specific journeys (Auto, Draw, the toggle) wait for the
//! implementation; this file holds the invariant, which is the same in both.
//! Tests that fail on main today are ignored and name the gap.

use novo_core::*;
use novo_editor::edit::{self, Transform};
use novo_editor::geometry::wire_points;
use novo_tests::motor_starter;

const EPS: f64 = 1e-6;

/// Whether two segments lie on one line and share a stretch of positive
/// length.
fn share_length(a: (Point, Point), b: (Point, Point)) -> bool {
    let d = a.1 - a.0;
    let cross = |p: Point| d.x * (p.y - a.0.y) - d.y * (p.x - a.0.x);
    let len = (d.x * d.x + d.y * d.y).sqrt();
    if len < EPS || cross(b.0).abs() > EPS * len || cross(b.1).abs() > EPS * len {
        return false;
    }
    // Project both ends of `b` onto `a` and intersect the intervals.
    let t = |p: Point| ((p.x - a.0.x) * d.x + (p.y - a.0.y) * d.y) / len;
    let (lo, hi) = (t(b.0).min(t(b.1)), t(b.0).max(t(b.1)));
    lo.max(0.0) + EPS < hi.min(len)
}

/// Every pair of wires on one page that are in different nets and share
/// length, as `(wire, wire)`.
fn overlaps(project: &Project, page: PageId) -> Vec<(WireId, WireId)> {
    let nets = project.netlist();
    let wires: Vec<_> = project
        .wires()
        .filter(|w| w.page == page)
        .map(|w| (w.id, nets.net_of(w.from)))
        .collect();
    let mut found = Vec::new();
    for (i, (wa, na)) in wires.iter().enumerate() {
        for (wb, nb) in &wires[i + 1..] {
            if na == nb {
                continue;
            }
            let (pa, pb) = (
                wire_points(project, *wa).unwrap(),
                wire_points(project, *wb).unwrap(),
            );
            let hit = pa.windows(2).any(|sa| {
                pb.windows(2)
                    .any(|sb| share_length((sa[0], sa[1]), (sb[0], sb[1])))
            });
            if hit {
                found.push((*wa, *wb));
            }
        }
    }
    found
}

#[test]
fn the_overlap_check_sees_what_it_should() {
    let p = Point::new;
    // Collinear and overlapping, either direction.
    assert!(share_length(
        (p(0., 0.), p(10., 0.)),
        (p(5., 0.), p(15., 0.))
    ));
    assert!(share_length(
        (p(0., 0.), p(10., 0.)),
        (p(8., 0.), p(2., 0.))
    ));
    // Collinear but only touching at an end.
    assert!(!share_length(
        (p(0., 0.), p(10., 0.)),
        (p(10., 0.), p(20., 0.))
    ));
    // A crossing is not an overlap.
    assert!(!share_length(
        (p(0., 0.), p(10., 0.)),
        (p(5., -5.), p(5., 5.))
    ));
    // Parallel, one step apart.
    assert!(!share_length(
        (p(0., 0.), p(10., 0.)),
        (p(0., 2.5), p(10., 2.5))
    ));
}

#[test]
fn the_motor_starter_has_no_overlapping_wires() {
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    for page in [s.main_page, s.control_page] {
        assert_eq!(overlaps(doc.project(), page), vec![], "page {page:?}");
    }
}

/// Moves the starter's symbols about at random, one move per transaction,
/// and checks the rule after every move. A small deterministic generator
/// keeps a failure reproducible.
#[test]
#[ignore = "no overlap rule yet: wires of different conductors can share length after a move (see TEST_REPORT Round 14)"]
fn gap_random_moves_never_leave_overlapping_wires() {
    let mut doc = Document::default();
    let s = motor_starter(&mut doc).unwrap();
    let symbols: Vec<_> = [s.f1_sym, s.k1_main, s.f2_main, s.m1_sym, s.f3_sym]
        .into_iter()
        .chain(s.x1_syms)
        .collect();
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = move |n: u64| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state % n
    };
    for step in 0..200 {
        let sym = symbols[next(symbols.len() as u64) as usize];
        // Moves are whole grid steps of 2.5 mm, up to 20 mm each way.
        let delta = Point::new((next(17) as f64 - 8.0) * 2.5, (next(17) as f64 - 8.0) * 2.5);
        doc.transact("Move", |tx| {
            edit::transform(tx, &[sym.into()], Transform::Move(delta))
        })
        .unwrap();
        for page in [s.main_page, s.control_page] {
            let found = overlaps(doc.project(), page);
            assert!(
                found.is_empty(),
                "step {step}: moving {sym:?} by {delta:?} left {found:?} overlapping"
            );
        }
    }
}
