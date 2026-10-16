//! Page geometry the editor works with: wire polylines, orthogonal routes and
//! hit testing on model objects.
//!
//! Hit testing here is on the model, not on the canvas scene, so the editing
//! logic runs (and is tested) without a window.

use novo_core::{Direction, Endpoint, JunctionId, PageId, Point, Project, WireId};

/// Tolerance for "the same point", in millimetres.
pub const EPS: f64 = 1e-6;

/// The points of a wire from its start through its bends to its end.
pub fn wire_points(project: &Project, wire: WireId) -> Option<Vec<Point>> {
    let w = project.wire(wire)?;
    let a = project.endpoint_position(w.from)?;
    let b = project.endpoint_position(w.to)?;
    let mut pts = Vec::with_capacity(w.route.len() + 2);
    pts.push(a);
    pts.extend(w.route.iter().copied());
    pts.push(b);
    Some(pts)
}

/// Closest point on segment `a`–`b` to `p`, and its parameter in 0..=1.
pub fn closest_on_segment(p: Point, a: Point, b: Point) -> (Point, f64) {
    let ab = b - a;
    let len2 = ab.x * ab.x + ab.y * ab.y;
    if len2 == 0.0 {
        return (a, 0.0);
    }
    let ap = p - a;
    let t = ((ap.x * ab.x + ap.y * ab.y) / len2).clamp(0.0, 1.0);
    (Point::new(a.x + ab.x * t, a.y + ab.y * t), t)
}

/// Where `p` lies on a polyline: segment index, the point itself and its
/// distance from `p`.
pub fn locate_on_polyline(points: &[Point], p: Point) -> Option<(usize, Point, f64)> {
    (0..points.len().saturating_sub(1))
        .map(|i| {
            let (q, _) = closest_on_segment(p, points[i], points[i + 1]);
            (i, q, q.distance(p))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
}

/// Length of a polyline up to point `q` on segment `seg`: the position of
/// `q` along it, used to sort points along a wire.
pub fn arc_position(points: &[Point], seg: usize, q: Point) -> f64 {
    let before: f64 = points
        .windows(2)
        .take(seg)
        .map(|s| s[0].distance(s[1]))
        .sum();
    before + points[seg].distance(q)
}

/// Snaps a point to the grid.
pub fn snap(p: Point, grid: f64) -> Point {
    if grid <= 0.0 {
        return p;
    }
    Point::new((p.x / grid).round() * grid, (p.y / grid).round() * grid)
}

fn is_vertical(d: Direction) -> bool {
    matches!(d, Direction::Up | Direction::Down)
}

/// Bend points of an orthogonal route from `a` to `b`, at most two.
///
/// `from` is the direction the wire leaves `a` (a pin's exit direction),
/// `to` the direction it leaves `b`. Two vertical pins get a Z route with
/// the cross-over halfway, a vertical and a horizontal pin an L route that
/// leaves each pin along its axis, and aligned points a straight wire.
pub fn route(a: Point, from: Option<Direction>, b: Point, to: Option<Direction>) -> Vec<Point> {
    route_offset(a, from, b, to, 0.0)
}

/// [`route`] with the cross-over of a Z route moved `shift` millimetres from
/// halfway (down or right when positive). L and straight routes have no
/// cross-over and ignore it.
pub fn route_offset(
    a: Point,
    from: Option<Direction>,
    b: Point,
    to: Option<Direction>,
    shift: f64,
) -> Vec<Point> {
    let dx = (b.x - a.x).abs();
    let dy = (b.y - a.y).abs();
    if dx < EPS || dy < EPS {
        return Vec::new();
    }
    let a_vert = from.map(is_vertical);
    let b_vert = to.map(is_vertical);
    match (a_vert, b_vert) {
        (Some(true), Some(true)) => {
            let mid = (a.y + b.y) / 2.0 + shift;
            vec![Point::new(a.x, mid), Point::new(b.x, mid)]
        }
        (Some(false), Some(false)) => {
            let mid = (a.x + b.x) / 2.0 + shift;
            vec![Point::new(mid, a.y), Point::new(mid, b.y)]
        }
        // Leave `a` vertically, or arrive at `b` horizontally.
        (Some(true), _) | (None, Some(false)) => vec![Point::new(a.x, b.y)],
        // Leave `a` horizontally, or arrive at `b` vertically.
        (Some(false), _) | (None, Some(true)) => vec![Point::new(b.x, a.y)],
        // Free corner to free corner: go the longer way first.
        (None, None) => {
            if dy >= dx {
                vec![Point::new(a.x, b.y)]
            } else {
                vec![Point::new(b.x, a.y)]
            }
        }
    }
}

/// A full route through user corners: bends between `a`, each corner and
/// `b`, with the corners themselves included.
pub fn route_through(
    a: Point,
    from: Option<Direction>,
    corners: &[Point],
    b: Point,
    to: Option<Direction>,
) -> Vec<Point> {
    let mut out = Vec::new();
    let mut prev = a;
    let mut prev_dir = from;
    for &c in corners {
        out.extend(route(prev, prev_dir, c, None));
        out.push(c);
        // Leave a corner at right angles to how we arrived.
        let last = out.len().checked_sub(2).map_or(prev, |i| out[i]);
        prev_dir = Some(if (c.x - last.x).abs() > (c.y - last.y).abs() {
            Direction::Down
        } else {
            Direction::Right
        });
        prev = c;
    }
    out.extend(route(prev, prev_dir, b, to));
    simplify(a, out, b)
}

/// Drops bend points that repeat a neighbour or lie on a straight line
/// between their neighbours.
pub fn simplify(a: Point, bends: Vec<Point>, b: Point) -> Vec<Point> {
    let mut pts = vec![a];
    pts.extend(bends);
    pts.push(b);
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_some_and(|l: &Point| l.approx_eq(p, EPS)) {
            continue;
        }
        while out.len() >= 2 {
            let (u, v) = (out[out.len() - 2], out[out.len() - 1]);
            let collinear = ((u.x - v.x).abs() < EPS && (v.x - p.x).abs() < EPS)
                || ((u.y - v.y).abs() < EPS && (v.y - p.y).abs() < EPS);
            if collinear {
                out.pop();
            } else {
                break;
            }
        }
        out.push(p);
    }
    if out.len() <= 2 {
        return Vec::new();
    }
    out[1..out.len() - 1].to_vec()
}

/// What sits under a point on a page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    /// A pin or junction, with its exact position.
    Endpoint(Endpoint, Point),
    /// A point inside a wire (not at its ends).
    Wire(WireId, Point),
}

impl Target {
    pub fn position(&self) -> Point {
        match *self {
            Target::Endpoint(_, p) | Target::Wire(_, p) => p,
        }
    }
}

/// The connectable object nearest to `p` within `tolerance`: pins and
/// junctions win over wires, so a click near a pin on a wire ends on the pin.
pub fn target_at(project: &Project, page: PageId, p: Point, tolerance: f64) -> Option<Target> {
    let contents = project.page_contents(page);
    let mut best: Option<(f64, Target)> = None;
    let offer = |best: &mut Option<(f64, Target)>, d: f64, t: Target| {
        if d <= tolerance && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
            *best = Some((d, t));
        }
    };
    for &sid in &contents.symbols {
        let Some(sym) = project.symbol(sid) else {
            continue;
        };
        for pin in 0..sym.pins.len() {
            let pin = pin as u16;
            if let Some(q) = sym.pin_position(pin) {
                offer(
                    &mut best,
                    q.distance(p),
                    Target::Endpoint(Endpoint::pin(sid, pin), q),
                );
            }
        }
    }
    for &jid in &contents.junctions {
        if let Some(j) = project.junction(jid) {
            offer(
                &mut best,
                j.position.distance(p),
                Target::Endpoint(Endpoint::junction(jid), j.position),
            );
        }
    }
    let found_endpoint = best.is_some();
    for &wid in &contents.wires {
        let Some(pts) = wire_points(project, wid) else {
            continue;
        };
        if found_endpoint {
            break;
        }
        if let Some((_, q, d)) = locate_on_polyline(&pts, p) {
            let at_end = q.approx_eq(pts[0], EPS) || q.approx_eq(pts[pts.len() - 1], EPS);
            if !at_end {
                offer(&mut best, d, Target::Wire(wid, q));
            }
        }
    }
    best.map(|(_, t)| t)
}

/// `q`, a point inside `wire`, moved along its segment to the nearest grid
/// line, so a junction made there sits on the grid and the pointer does not
/// glide along the wire. The grid wins: when the nearest grid line falls
/// outside the segment's inside (a short stretch, near a corner or an end),
/// the nearest grid line that does fall inside it is used instead. Only a
/// slanted segment, or one with no grid line inside it, keeps `q` as it is.
pub fn snap_along_wire(project: &Project, wire: WireId, q: Point, grid: f64) -> Point {
    let Some(pts) = wire_points(project, wire) else {
        return q;
    };
    let Some((seg, _, _)) = locate_on_polyline(&pts, q) else {
        return q;
    };
    if grid <= 0.0 {
        return q;
    }
    let (a, b) = (pts[seg], pts[seg + 1]);
    let horizontal = (a.y - b.y).abs() < EPS;
    if !horizontal && (a.x - b.x).abs() >= EPS {
        return q;
    }
    // Position along the segment axis; ends are excluded from the lines.
    let axis = |p: Point| if horizontal { p.x } else { p.y };
    let (lo, hi) = (axis(a).min(axis(b)), axis(a).max(axis(b)));
    let first = ((lo + EPS) / grid).floor() as i64;
    let last = ((hi - EPS) / grid).ceil() as i64;
    let best = (first..=last)
        .map(|k| k as f64 * grid)
        .filter(|v| *v > lo + EPS && *v < hi - EPS)
        .min_by(|u, v| (u - axis(q)).abs().total_cmp(&(v - axis(q)).abs()));
    match best {
        Some(v) if horizontal => Point::new(v, a.y),
        Some(v) => Point::new(a.x, v),
        None => q,
    }
}

/// Wires of a page with a point inside them (not at an end), with the
/// segment index.
pub fn wires_through(project: &Project, page: PageId, p: Point) -> Vec<(WireId, usize)> {
    project
        .page_contents(page)
        .wires
        .into_iter()
        .filter_map(|w| {
            let pts = wire_points(project, w)?;
            let (seg, q, d) = locate_on_polyline(&pts, p)?;
            let at_end = q.approx_eq(pts[0], EPS) || q.approx_eq(pts[pts.len() - 1], EPS);
            (d < EPS && !at_end).then_some((w, seg))
        })
        .collect()
}

/// The exit direction of an endpoint: the pin's direction, or `None` for a
/// junction or an open end.
pub fn endpoint_direction(project: &Project, end: Endpoint) -> Option<Direction> {
    match end {
        Endpoint::Pin { symbol, pin } => project.symbol(symbol)?.pin_direction(pin),
        Endpoint::Junction { .. } | Endpoint::Free { .. } => None,
    }
}

/// Junctions of a page, by position.
pub fn junction_at(project: &Project, page: PageId, p: Point) -> Option<JunctionId> {
    project
        .page_contents(page)
        .junctions
        .into_iter()
        .find(|&j| {
            project
                .junction(j)
                .is_some_and(|j| j.position.approx_eq(p, EPS))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y)
    }

    #[test]
    fn aligned_points_route_straight() {
        assert!(
            route(
                p(0.0, 0.0),
                Some(Direction::Down),
                p(0.0, 40.0),
                Some(Direction::Up)
            )
            .is_empty()
        );
    }

    #[test]
    fn two_vertical_pins_get_a_z_route() {
        let bends = route(
            p(0.0, 0.0),
            Some(Direction::Down),
            p(20.0, 40.0),
            Some(Direction::Up),
        );
        assert_eq!(bends, vec![p(0.0, 20.0), p(20.0, 20.0)]);
    }

    #[test]
    fn vertical_to_horizontal_gets_an_l_route() {
        let bends = route(
            p(0.0, 0.0),
            Some(Direction::Down),
            p(20.0, 40.0),
            Some(Direction::Left),
        );
        assert_eq!(bends, vec![p(0.0, 40.0)]);
        let bends = route(
            p(0.0, 0.0),
            Some(Direction::Right),
            p(20.0, 40.0),
            Some(Direction::Up),
        );
        assert_eq!(bends, vec![p(20.0, 0.0)]);
    }

    #[test]
    fn corners_are_kept_and_routes_simplified() {
        let bends = route_through(
            p(0.0, 0.0),
            Some(Direction::Down),
            &[p(0.0, 20.0), p(40.0, 20.0)],
            p(40.0, 60.0),
            Some(Direction::Up),
        );
        assert_eq!(bends, vec![p(0.0, 20.0), p(40.0, 20.0)]);
    }

    #[test]
    fn simplify_drops_collinear_points() {
        let bends = simplify(p(0.0, 0.0), vec![p(0.0, 10.0), p(0.0, 20.0)], p(0.0, 30.0));
        assert!(bends.is_empty());
    }
}
