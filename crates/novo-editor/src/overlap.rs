//! Wires never share a stretch of line (docs/DESIGN.md §4, "Wiring modes").
//!
//! This module is the read-only half of the rule: finding the stretches two
//! routes share. It is pure geometry over the model, so the editor, the
//! checks (`wire.overlap`) and tests all ask the same question. Routes are
//! orthogonal in this version; a stretch is a piece of line of positive
//! length where two segments lie on the same horizontal or vertical line.
//!
//! Touching at a point, crossing and a wire ending on the side of another are
//! not overlaps. A wire that ends *along* another is: its last segment
//! shares a stretch with it.

use std::collections::HashMap;

use novo_core::{Direction, PageId, Point, Project, WireId};

use crate::geometry::{EPS, route_offset, route_through, simplify, wire_points};

/// How far the router may move a cross-over, in snap steps (DESIGN: up to 3).
pub const MAX_NUDGE_STEPS: u32 = 3;

/// One snap step in millimetres.
pub const NUDGE_STEP: f64 = 2.5;

/// Bucket size of the segment index, in millimetres. Large enough that a
/// typical segment spans a few buckets, small enough that a dense page does
/// not put every wire in one.
const CELL: f64 = 50.0;

/// A stretch of line shared with an existing wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Overlap {
    /// The existing wire.
    pub wire: WireId,
    /// Ends of the shared stretch, ordered by x then y.
    pub from: Point,
    pub to: Point,
}

impl Overlap {
    pub fn length(&self) -> f64 {
        self.from.distance(self.to)
    }
}

/// The stretch two segments share, if it has positive length.
pub fn segment_overlap(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<(Point, Point)> {
    let horizontal = |p: Point, q: Point| (p.y - q.y).abs() < EPS && (p.x - q.x).abs() >= EPS;
    let vertical = |p: Point, q: Point| (p.x - q.x).abs() < EPS && (p.y - q.y).abs() >= EPS;
    if horizontal(a1, a2) && horizontal(b1, b2) && (a1.y - b1.y).abs() < EPS {
        let lo = a1.x.min(a2.x).max(b1.x.min(b2.x));
        let hi = a1.x.max(a2.x).min(b1.x.max(b2.x));
        (hi - lo > EPS).then(|| (Point::new(lo, a1.y), Point::new(hi, a1.y)))
    } else if vertical(a1, a2) && vertical(b1, b2) && (a1.x - b1.x).abs() < EPS {
        let lo = a1.y.min(a2.y).max(b1.y.min(b2.y));
        let hi = a1.y.max(a2.y).min(b1.y.max(b2.y));
        (hi - lo > EPS).then(|| (Point::new(a1.x, lo), Point::new(a1.x, hi)))
    } else {
        None
    }
}

fn cell(v: f64) -> i64 {
    (v / CELL).floor() as i64
}

/// The segments of every wire on a page, bucketed by grid cell so a drag
/// asks only about its neighbours.
pub struct SegmentIndex {
    segments: Vec<(WireId, Point, Point)>,
    buckets: HashMap<(i64, i64), Vec<usize>>,
}

impl SegmentIndex {
    pub fn new(project: &Project, page: PageId) -> Self {
        let mut index = SegmentIndex {
            segments: Vec::new(),
            buckets: HashMap::new(),
        };
        for wire in project.wires().filter(|w| w.page == page) {
            let Some(points) = wire_points(project, wire.id) else {
                continue;
            };
            for s in points.windows(2) {
                index.insert(wire.id, s[0], s[1]);
            }
        }
        index
    }

    fn insert(&mut self, wire: WireId, a: Point, b: Point) {
        let i = self.segments.len();
        self.segments.push((wire, a, b));
        let (x0, x1) = (cell(a.x.min(b.x)), cell(a.x.max(b.x)));
        let (y0, y1) = (cell(a.y.min(b.y)), cell(a.y.max(b.y)));
        for cx in x0..=x1 {
            for cy in y0..=y1 {
                self.buckets.entry((cx, cy)).or_default().push(i);
            }
        }
    }

    /// Stretches `route` (a polyline) shares with indexed wires other than
    /// those in `ignore`, in route order.
    pub fn overlaps(&self, route: &[Point], ignore: &[WireId]) -> Vec<Overlap> {
        let mut out: Vec<Overlap> = Vec::new();
        let mut seen: Vec<usize> = Vec::new();
        for s in route.windows(2) {
            let (x0, x1) = (cell(s[0].x.min(s[1].x)), cell(s[0].x.max(s[1].x)));
            let (y0, y1) = (cell(s[0].y.min(s[1].y)), cell(s[0].y.max(s[1].y)));
            seen.clear();
            for cx in x0..=x1 {
                for cy in y0..=y1 {
                    let Some(ids) = self.buckets.get(&(cx, cy)) else {
                        continue;
                    };
                    for &i in ids {
                        if seen.contains(&i) {
                            continue;
                        }
                        seen.push(i);
                        let (wire, a, b) = self.segments[i];
                        if ignore.contains(&wire) {
                            continue;
                        }
                        if let Some((from, to)) = segment_overlap(s[0], s[1], a, b) {
                            out.push(Overlap { wire, from, to });
                        }
                    }
                }
            }
        }
        out
    }
}

/// Stretches `route` would share with the wires already on `page`, ignoring
/// `ignore` (the wire being edited).
pub fn route_overlaps(
    project: &Project,
    page: PageId,
    route: &[Point],
    ignore: &[WireId],
) -> Vec<Overlap> {
    SegmentIndex::new(project, page).overlaps(route, ignore)
}

/// Every pair of wires on a page that share a stretch, each pair once, for
/// the `wire.overlap` check.
pub fn page_overlaps(project: &Project, page: PageId) -> Vec<(WireId, Overlap)> {
    let index = SegmentIndex::new(project, page);
    let mut out = Vec::new();
    for wire in project.wires().filter(|w| w.page == page) {
        let Some(points) = wire_points(project, wire.id) else {
            continue;
        };
        for o in index.overlaps(&points, &[wire.id]) {
            // Report a pair once: from the wire with the smaller id.
            if wire.id < o.wire {
                out.push((wire.id, o));
            }
        }
    }
    out
}

/// Why a route was refused: the first stretch it would share with another
/// wire. Returned as data so the UI can draw it and tests can assert on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Refusal {
    pub other: WireId,
    pub from: Point,
    pub to: Point,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "overlaps {}", self.other)
    }
}

/// The bends for a wire from `a` to `b` through the user's `corners` that
/// share no stretch with the wires on `page`, ignoring `exempt`.
///
/// The default route is kept when it is clear. Otherwise, with no user
/// corners, the router tries moving a Z route's cross-over up to
/// [`MAX_NUDGE_STEPS`] steps either way, nearest first. Corners the user
/// clicked are never moved, so a route through them that overlaps is refused.
#[allow(clippy::too_many_arguments)]
pub fn plan_route(
    project: &Project,
    page: PageId,
    a: Point,
    from: Option<Direction>,
    corners: &[Point],
    b: Point,
    to: Option<Direction>,
    exempt: &[WireId],
) -> Result<Vec<Point>, Refusal> {
    let index = SegmentIndex::new(project, page);
    let check = |bends: &[Point]| -> Vec<Overlap> {
        let mut pts = vec![a];
        pts.extend_from_slice(bends);
        pts.push(b);
        index.overlaps(&pts, exempt)
    };
    let default = route_through(a, from, corners, b, to);
    let first = check(&default);
    let Some(refusal) = first.first() else {
        return Ok(default);
    };
    let refusal = Refusal {
        other: refusal.wire,
        from: refusal.from,
        to: refusal.to,
    };
    if !corners.is_empty() {
        return Err(refusal);
    }
    let (lo, hi) = (
        Point::new(a.x.min(b.x), a.y.min(b.y)),
        Point::new(a.x.max(b.x), a.y.max(b.y)),
    );
    for step in 1..=MAX_NUDGE_STEPS {
        for sign in [1.0, -1.0] {
            let bends = simplify(
                a,
                route_offset(a, from, b, to, sign * f64::from(step) * NUDGE_STEP),
                b,
            );
            let inside = bends.iter().all(|p| {
                p.x >= lo.x - EPS && p.x <= hi.x + EPS && p.y >= lo.y - EPS && p.y <= hi.y + EPS
            });
            if inside && bends != default && check(&bends).is_empty() {
                return Ok(bends);
            }
        }
    }
    Err(refusal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn collinear_runs_overlap_by_their_common_length() {
        let o = segment_overlap(p(0.0, 0.0), p(10.0, 0.0), p(4.0, 0.0), p(20.0, 0.0)).unwrap();
        assert_eq!(o, (p(4.0, 0.0), p(10.0, 0.0)));
        let v = segment_overlap(p(5.0, 9.0), p(5.0, 0.0), p(5.0, 3.0), p(5.0, 1.0)).unwrap();
        assert_eq!(v, (p(5.0, 1.0), p(5.0, 3.0)));
    }

    #[test]
    fn touching_crossing_and_parallel_apart_are_not_overlaps() {
        // End to end at a point.
        assert!(segment_overlap(p(0.0, 0.0), p(5.0, 0.0), p(5.0, 0.0), p(9.0, 0.0)).is_none());
        // A crossing.
        assert!(segment_overlap(p(0.0, 0.0), p(10.0, 0.0), p(5.0, -3.0), p(5.0, 3.0)).is_none());
        // A T: one ends on the side of the other.
        assert!(segment_overlap(p(0.0, 0.0), p(10.0, 0.0), p(5.0, 0.0), p(5.0, 3.0)).is_none());
        // Parallel, one step apart.
        assert!(segment_overlap(p(0.0, 0.0), p(10.0, 0.0), p(0.0, 2.5), p(10.0, 2.5)).is_none());
    }

    #[test]
    fn a_wire_laid_along_another_overlaps_even_inside_it() {
        let o = segment_overlap(p(2.0, 0.0), p(4.0, 0.0), p(0.0, 0.0), p(10.0, 0.0)).unwrap();
        assert_eq!(o, (p(2.0, 0.0), p(4.0, 0.0)));
    }
}
