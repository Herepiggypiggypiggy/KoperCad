//! Lines mode and the Corners elbow (docs/DESIGN.md §4, "Wiring modes").
//!
//! In Lines the user draws the lines: every click commits one straight
//! segment locked to an axis and nothing is routed. These are the pure pieces
//! (axis lock, exact bends, the overlap pre-flight for one leg) and the one
//! engine entry that stores a wire exactly as drawn, [`draw_wire_exact`].
//! Overlaps are refused here, never nudged, because the line is what the user
//! drew; the check itself is [`crate::overlap`]'s.

use novo_core::{Endpoint, ModelError, PageId, Point, Project, Result, Tx, Wire, WireId};

use crate::edit::resolve_target;
use crate::geometry::{EPS, Target, endpoint_direction, simplify};
use crate::overlap::{Refusal, plan_route, segment_overlap};

/// The point `to` moves to when the segment from `from` is locked to the
/// axis the pointer is nearer to (a diagonal pointer takes the horizontal).
pub fn lock_axis(from: Point, to: Point) -> Point {
    if (to.x - from.x).abs() >= (to.y - from.y).abs() {
        Point::new(to.x, from.y)
    } else {
        Point::new(from.x, to.y)
    }
}

/// The elbow of a two-leg route from `prev` to `to`: first along the axis
/// the pointer has moved furthest, or along the other one when `flip` (R).
/// `None` when the points are aligned and no elbow is needed.
pub fn elbow(prev: Point, to: Point, flip: bool) -> Option<Point> {
    let (dx, dy) = ((to.x - prev.x).abs(), (to.y - prev.y).abs());
    if dx < EPS || dy < EPS {
        return None;
    }
    let horizontal_first = (dx >= dy) != flip;
    Some(if horizontal_first {
        Point::new(to.x, prev.y)
    } else {
        Point::new(prev.x, to.y)
    })
}

/// The bends of a drawn wire from `a` through `vertices` to `b`: the
/// vertices themselves, plus one right-angle bend in front of any point the
/// previous one is not aligned with (a pin the last click did not line up
/// with, or a vertex placed in Corners before the mode changed), taken on the
/// axis the later point is nearer to.
pub fn exact_bends(a: Point, vertices: &[Point], b: Point) -> Vec<Point> {
    let mut out = Vec::new();
    let mut prev = a;
    for &p in vertices.iter().chain(std::iter::once(&b)) {
        let aligned = (p.x - prev.x).abs() < EPS || (p.y - prev.y).abs() < EPS;
        if !aligned {
            out.push(lock_axis(prev, p));
        }
        out.push(p);
        prev = p;
    }
    // `b` is the wire's end, not a bend.
    out.pop();
    simplify(a, out, b)
}

/// The first stretch where a segment of `points` runs back over an earlier
/// segment of the same polyline (a wire drawn out, up, back and along its
/// own first leg). Segments that only meet at a bend do not overlap.
pub fn route_overlaps_itself(points: &[Point]) -> Option<(Point, Point)> {
    let segments: Vec<&[Point]> = points.windows(2).collect();
    (1..segments.len()).find_map(|i| {
        (0..i).find_map(|j| {
            segment_overlap(
                segments[i][0],
                segments[i][1],
                segments[j][0],
                segments[j][1],
            )
        })
    })
}

/// The wires already at a pin end (none for a junction or a point on a wire).
fn wires_at_pin(project: &Project, end: Option<Endpoint>) -> Vec<WireId> {
    match end {
        Some(e @ Endpoint::Pin { .. }) => project.wires_at(e).collect(),
        _ => Vec::new(),
    }
}

/// Whether one leg of a drawn wire, `from` to `to`, would lie over another
/// wire on `page`. `start_pin` and `end_pin` are the endpoints the leg begins
/// and ends on, when it does: wires that leave the same pin may run together
/// from it (they become a derived junction, [`crate::overlap`]).
pub fn check_leg(
    project: &Project,
    page: PageId,
    from: Point,
    to: Point,
    start_pin: Option<Endpoint>,
    end_pin: Option<Endpoint>,
) -> std::result::Result<(), Refusal> {
    let bends = exact_bends(from, &[], to);
    plan_route(
        project,
        page,
        from,
        None,
        &bends,
        to,
        None,
        &wires_at_pin(project, start_pin),
        &wires_at_pin(project, end_pin),
    )
    .map(|_| ())
}

/// Draws a wire from `start` through `vertices` to `end` exactly as drawn:
/// the vertices are the bends (plus the right-angle bend [`exact_bends`]
/// adds where a point is not aligned) and nothing is routed or nudged. A
/// route that would lie over another wire is refused. Ends on wires become
/// T-junctions.
pub fn draw_wire_exact(
    tx: &mut Tx<'_>,
    page: PageId,
    start: Target,
    vertices: &[Point],
    end: Target,
) -> Result<WireId> {
    let from = resolve_target(tx, page, start)?;
    let to = resolve_target(tx, page, end)?;
    let project = tx.project();
    let a = project
        .endpoint_position(from)
        .ok_or(ModelError::NotFound(endpoint_id(from)))?;
    let b = project
        .endpoint_position(to)
        .ok_or(ModelError::NotFound(endpoint_id(to)))?;
    // The bends are already aligned, so the router keeps every one of them
    // and, being given corners, never moves them.
    let drawn = exact_bends(a, vertices, b);
    let bends = plan_route(
        project,
        page,
        a,
        endpoint_direction(project, from),
        &drawn,
        b,
        endpoint_direction(project, to),
        &wires_at_pin(project, Some(from)),
        &wires_at_pin(project, Some(to)),
    )
    .map_err(|r| ModelError::InvalidValue {
        id: r.other.into(),
        field: "route overlaps this wire",
    })?;
    let wire = tx.connect(from, to)?;
    if !bends.is_empty() {
        tx.update::<Wire>(wire, |w| w.route = bends)?;
    }
    Ok(wire)
}

fn endpoint_id(end: Endpoint) -> novo_core::AnyId {
    match end {
        Endpoint::Pin { symbol, .. } => symbol.into(),
        Endpoint::Junction { junction } => junction.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn the_segment_locks_to_the_nearer_axis() {
        assert_eq!(lock_axis(p(0.0, 0.0), p(10.0, 3.0)), p(10.0, 0.0));
        assert_eq!(lock_axis(p(0.0, 0.0), p(3.0, -10.0)), p(0.0, -10.0));
        // A true diagonal takes the horizontal.
        assert_eq!(lock_axis(p(0.0, 0.0), p(5.0, 5.0)), p(5.0, 0.0));
    }

    #[test]
    fn the_elbow_runs_along_the_longer_move_first_and_r_flips_it() {
        assert_eq!(elbow(p(0.0, 0.0), p(10.0, 4.0), false), Some(p(10.0, 0.0)));
        assert_eq!(elbow(p(0.0, 0.0), p(10.0, 4.0), true), Some(p(0.0, 4.0)));
        assert_eq!(elbow(p(0.0, 0.0), p(4.0, 10.0), false), Some(p(0.0, 10.0)));
        assert_eq!(elbow(p(0.0, 0.0), p(10.0, 0.0), true), None);
    }

    #[test]
    fn a_line_back_over_the_wire_itself_is_found() {
        // Out, up, back along the first leg.
        let back = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0), p(10.0, 0.0)];
        assert!(route_overlaps_itself(&back).is_some());
        let over = [
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 5.0),
            p(0.0, 5.0),
            p(0.0, 0.0),
        ];
        // Back down the first column is not along the first leg: no overlap.
        assert!(route_overlaps_itself(&over).is_none());
        let along = [
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(10.0, 5.0),
            p(5.0, 5.0),
            p(5.0, 0.0),
            p(2.0, 0.0),
        ];
        assert_eq!(
            route_overlaps_itself(&along),
            Some((p(2.0, 0.0), p(5.0, 0.0)))
        );
        // A straight run on is not an overlap.
        assert!(route_overlaps_itself(&[p(0.0, 0.0), p(5.0, 0.0), p(9.0, 0.0)]).is_none());
    }

    #[test]
    fn drawn_vertices_are_kept_and_only_misaligned_points_gain_a_bend() {
        // Aligned: exactly the vertices.
        assert_eq!(
            exact_bends(p(0.0, 0.0), &[p(10.0, 0.0), p(10.0, 5.0)], p(20.0, 5.0)),
            vec![p(10.0, 0.0), p(10.0, 5.0)]
        );
        // The end is off the last axis: one bend on the nearer axis.
        assert_eq!(
            exact_bends(p(0.0, 0.0), &[p(10.0, 0.0)], p(14.0, 8.0)),
            vec![p(10.0, 0.0), p(10.0, 8.0)]
        );
        // Straight pin to pin: nothing.
        assert!(exact_bends(p(0.0, 0.0), &[], p(10.0, 0.0)).is_empty());
        // Collinear vertices collapse.
        assert!(exact_bends(p(0.0, 0.0), &[p(5.0, 0.0)], p(10.0, 0.0)).is_empty());
    }
}
