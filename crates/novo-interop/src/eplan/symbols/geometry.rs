//! Fitting a library symbol onto the connection points an EPLAN placement
//! has, by where they sit and which way they face.
//!
//! A placement whose pins carry no designation, or designations that mean
//! nothing in KoperCAD (`48Vac`, `0V48`), cannot be mapped by name. Its
//! geometry still says which point is which: the primary of a transformer
//! is on one side, the secondary on the other, the earth point at the end.
//! [`fit`] finds the quarter turn (and mirror) of the library symbol, and the
//! assignment of its connections to the placement's points, that keeps every
//! point where its counterpart is. It also turns a single point to face the
//! way EPLAN's does, which is all a device connection point is.
//!
//! Positions are compared in proportion, not millimetres: both sets are
//! scaled to their own bounding box, because KoperCAD's symbols and EPLAN's
//! differ in size (a 4 mm tall terminal against our 8 mm one) but not in
//! arrangement. Nothing here reads EPLAN artwork; only the points the
//! project file already carries.

use novo_core::{Direction, Placement, Point, Rotation};
use novo_symbols::{Direction as SymbolDirection, Symbol};

/// Most points a placement may have; the search tries every assignment.
pub const MAX_POINTS: usize = 8;

/// The average squared distance, in the unit square, a fit may leave per
/// point and still count as the same arrangement.
pub const MAX_COST_PER_POINT: f64 = 0.12;

/// Cost of a point taking a connection other than the one at its own place in
/// the list, so equal arrangements keep the listed order.
const ORDER: f64 = 0.002;

/// Cost of one point facing the wrong way. Small next to a misplaced point
/// (positions decide for two or more points), but it is all that tells the
/// turns of a single point apart.
const FACING: f64 = 0.05;

/// One connection point of the placement being mapped, relative to anything
/// (only the arrangement counts), y pointing down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Port {
    pub at: Point,
    /// The way a wire leaves the point, as seen on the page, when known.
    pub dir: Option<Direction>,
}

/// How a library symbol sits over the placement's points.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    pub rotation: Rotation,
    pub mirrored: bool,
    /// For each port, in order, the index of the symbol connection it is.
    pub connection: Vec<usize>,
    /// Sum of squared distances in the unit square plus the facing costs;
    /// lower is closer.
    pub cost: f64,
}

fn to_direction(d: SymbolDirection) -> Direction {
    match d {
        SymbolDirection::Up => Direction::Up,
        SymbolDirection::Down => Direction::Down,
        SymbolDirection::Left => Direction::Left,
        SymbolDirection::Right => Direction::Right,
    }
}

/// Scales points into the unit square by their bounding box; an axis they do
/// not span at all sits in the middle.
fn unit(points: &[Point]) -> Vec<[f64; 2]> {
    let axis = |f: fn(&Point) -> f64| {
        let lo = points.iter().map(f).fold(f64::INFINITY, f64::min);
        let hi = points.iter().map(f).fold(f64::NEG_INFINITY, f64::max);
        (lo, hi - lo)
    };
    let (x0, xs) = axis(|p| p.x);
    let (y0, ys) = axis(|p| p.y);
    let scale = |v: f64, lo: f64, span: f64| {
        if span < 1e-9 { 0.5 } else { (v - lo) / span }
    };
    points
        .iter()
        .map(|p| [scale(p.x, x0, xs), scale(p.y, y0, ys)])
        .collect()
}

/// Every way to give each of `n` ports its own connection, lowest cost first
/// found; `cost[i][j]` is the cost of port `i` being connection `j`.
fn best_assignment(cost: &[Vec<f64>]) -> (Vec<usize>, f64) {
    fn go(
        cost: &[Vec<f64>],
        i: usize,
        used: &mut Vec<bool>,
        pick: &mut Vec<usize>,
        so_far: f64,
        best: &mut (Vec<usize>, f64),
    ) {
        if so_far >= best.1 {
            return;
        }
        if i == cost.len() {
            *best = (pick.clone(), so_far);
            return;
        }
        for j in 0..cost.len() {
            if used[j] {
                continue;
            }
            used[j] = true;
            pick.push(j);
            go(cost, i + 1, used, pick, so_far + cost[i][j], best);
            pick.pop();
            used[j] = false;
        }
    }
    let mut best = (Vec::new(), f64::INFINITY);
    go(
        cost,
        0,
        &mut vec![false; cost.len()],
        &mut Vec::new(),
        0.0,
        &mut best,
    );
    best
}

/// Fits `symbol` over `ports`, or `None` when the counts differ, there are
/// more than [`MAX_POINTS`] points, or no turn of the symbol puts its
/// connections close enough to the points ([`MAX_COST_PER_POINT`]).
/// Ties go to the listed order, then the unturned, unmirrored symbol.
pub fn fit(ports: &[Port], symbol: &Symbol) -> Option<Fit> {
    let n = ports.len();
    if n == 0 || n > MAX_POINTS || symbol.connections.len() != n {
        return None;
    }
    let want = unit(&ports.iter().map(|p| p.at).collect::<Vec<_>>());
    let mut best: Option<Fit> = None;
    for mirrored in [false, true] {
        for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            let placement = Placement {
                origin: Point::new(0.0, 0.0),
                rotation,
                mirrored,
            };
            let at: Vec<Point> = symbol
                .connections
                .iter()
                .map(|c| placement.apply(Point::new(c.at[0], c.at[1])))
                .collect();
            let have = unit(&at);
            let cost: Vec<Vec<f64>> = (0..n)
                .map(|i| {
                    (0..n)
                        .map(|j| {
                            let dx = want[i][0] - have[j][0];
                            let dy = want[i][1] - have[j][1];
                            let facing = match ports[i].dir {
                                Some(d)
                                    if placement.apply_direction(to_direction(
                                        symbol.connections[j].dir,
                                    )) != d =>
                                {
                                    FACING
                                }
                                _ => 0.0,
                            };
                            dx * dx + dy * dy + facing + if i == j { 0.0 } else { ORDER }
                        })
                        .collect()
                })
                .collect();
            let (connection, total) = best_assignment(&cost);
            if best.as_ref().is_none_or(|b| total < b.cost - 1e-9) {
                best = Some(Fit {
                    rotation,
                    mirrored,
                    connection,
                    cost: total,
                });
            }
        }
    }
    best.filter(|b| b.cost / n as f64 <= MAX_COST_PER_POINT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_symbols::Library;

    fn lib(id: &str) -> &'static Symbol {
        Library::shared(novo_symbols::Pack::Iec).get(id).unwrap()
    }

    fn port(x: f64, y: f64, dir: Option<Direction>) -> Port {
        Port {
            at: Point::new(x, y),
            dir,
        }
    }

    #[test]
    fn a_single_point_turns_to_face_the_way_the_port_does() {
        let pin = lib("iec/connectivity/device_pin"); // wire leaves upward
        for (dir, rotation) in [
            (Direction::Up, Rotation::R0),
            (Direction::Left, Rotation::R90),
            (Direction::Down, Rotation::R180),
            (Direction::Right, Rotation::R270),
        ] {
            let f = fit(&[port(0.0, 0.0, Some(dir))], pin).unwrap();
            assert_eq!(f.rotation, rotation, "{dir:?}");
            assert!(!f.mirrored);
            assert_eq!(f.connection, vec![0]);
        }
    }

    #[test]
    fn a_single_point_with_no_direction_stays_unturned() {
        let pin = lib("iec/connectivity/device_pin");
        let f = fit(&[port(3.0, 3.0, None)], pin).unwrap();
        assert_eq!((f.rotation, f.mirrored), (Rotation::R0, false));
    }

    #[test]
    fn the_transformer_secondary_is_found_by_where_it_sits() {
        // Listed secondary, primary, secondary, primary, earth: the order a
        // naive positional match would get wrong. y points down.
        let ports = [
            port(0.0, 8.0, None),   // secondary, left
            port(0.0, -8.0, None),  // primary, left
            port(16.0, 8.0, None),  // secondary, right
            port(16.0, -8.0, None), // primary, right
            port(24.0, 8.0, None),  // earth
        ];
        let t = lib("iec/transformers/transformer_1ph_pe");
        let f = fit(&ports, t).unwrap();
        assert_eq!((f.rotation, f.mirrored), (Rotation::R0, false));
        let ids: Vec<&str> = f
            .connection
            .iter()
            .map(|&j| t.connections[j].id.as_str())
            .collect();
        assert_eq!(ids, ["2.1", "1.1", "2.2", "1.2", "PE"]);
    }

    #[test]
    fn a_terminal_with_a_tap_keeps_its_arrangement() {
        let ports = [
            port(0.0, -2.0, None),
            port(0.0, 2.0, None),
            port(4.0, 2.0, None),
        ];
        let f = fit(&ports, lib("iec/terminals/terminal_tap")).unwrap();
        assert_eq!((f.rotation, f.mirrored), (Rotation::R0, false));
        assert_eq!(f.connection, vec![0, 1, 2]);
    }

    #[test]
    fn a_terminal_drawn_sideways_turns_the_symbol() {
        let ports = [port(0.0, 0.0, None), port(8.0, 0.0, None)];
        let f = fit(&ports, lib("iec/terminals/terminal")).unwrap();
        assert_eq!(f.rotation, Rotation::R90);
        assert_eq!(f.connection, vec![0, 1]);
    }

    #[test]
    fn a_mirrored_tap_mirrors_the_symbol() {
        // The tap on the left instead of the right.
        let ports = [
            port(0.0, -2.0, None),
            port(0.0, 2.0, None),
            port(-4.0, 2.0, None),
        ];
        let f = fit(&ports, lib("iec/terminals/terminal_tap")).unwrap();
        assert!(f.mirrored);
        assert_eq!(f.connection, vec![0, 1, 2]);
    }

    #[test]
    fn the_motor_with_two_extra_terminals_matches_by_position() {
        // 1 2 3 across the top, 4 and 5 below the outer two, 6 (earth) at the
        // right end of the top row: the order the placement lists them in.
        let ports = [
            port(0.0, -8.0, None),
            port(8.0, -8.0, None),
            port(16.0, -8.0, None),
            port(0.0, 8.0, None),
            port(16.0, 8.0, None),
            port(24.0, -8.0, None),
        ];
        let m = lib("iec/motors/motor_3ph_pe_t2");
        let f = fit(&ports, m).unwrap();
        assert_eq!((f.rotation, f.mirrored), (Rotation::R0, false));
        assert_eq!(f.connection, vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn nothing_fits_when_counts_differ_or_arrangements_do_not() {
        let tap = lib("iec/terminals/terminal_tap");
        assert!(fit(&[port(0.0, 0.0, None), port(0.0, 1.0, None)], tap).is_none());
        // Three points in a row are not a terminal with a tap.
        let row = [
            port(0.0, 0.0, None),
            port(5.0, 0.0, None),
            port(10.0, 0.0, None),
        ];
        assert!(fit(&row, tap).is_none());
        assert!(fit(&[], tap).is_none());
    }
}
