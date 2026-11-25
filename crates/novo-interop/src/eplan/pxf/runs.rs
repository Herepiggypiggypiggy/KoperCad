//! Wires from the runs EPLAN implies.
//!
//! An EPLAN export stores no wire objects: a wire is the straight run between
//! two points that face each other, through the file's nodes. The net rebuild
//! ([`crate::eplan::nets`]) lists those runs. This module turns them into
//! KoperCAD wires:
//!
//! - a pin ends a wire;
//! - a node where three or more runs meet is a junction: a connection dot that
//!   wires really end on, so the dot is a connection and not decoration;
//! - a node where two runs meet is a corner of the wire;
//! - a node that only one run reaches, or that lies on a stub with no pin at
//!   its end, carries nothing and is dropped (counted).
//!
//! Joins EPLAN makes by rule with no drawn run (both sides of a terminal,
//! paired interruption points, same-name terminals) get no wire: the project's
//! own rules join them, and the net comparison of the import checks that.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Direction, Point};

use crate::eplan::nets::PinKey;

/// Positions closer than this (mm) are one place.
const SAME: f64 = 0.05;

/// One end of a drawn point: where it is on the page (KoperCAD coordinates).
#[derive(Debug, Clone)]
pub(super) struct RunPoint {
    pub key: PinKey,
    pub page: usize,
    pub at: Point,
    /// A connection node; otherwise a pin (the end of a wire).
    pub node: bool,
    /// The way a pin leaves its symbol.
    pub leaves: Option<Direction>,
}

/// What a drawn wire ends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RunEnd {
    /// The point with this key, resolved by the caller.
    Pin(usize),
    /// An entry of [`RunWires::junctions`].
    Junction(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct RunWire {
    pub page: usize,
    pub from: RunEnd,
    pub to: RunEnd,
    /// Corners between the ends, in page coordinates.
    pub route: Vec<Point>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct RunWires {
    pub wires: Vec<RunWire>,
    /// Page and position of each connection dot.
    pub junctions: Vec<(usize, Point)>,
    /// Nodes that carry no wire: stubs, and T or cross dots with fewer than
    /// three runs behind them.
    pub dropped_nodes: usize,
    /// Runs that came back to where they began, cut open.
    pub loops_cut: usize,
    /// Nodes where three or more runs meet.
    pub junction_nodes: usize,
}

struct Union(Vec<usize>);

impl Union {
    fn find(&mut self, mut i: usize) -> usize {
        while self.0[i] != i {
            self.0[i] = self.0[self.0[i]];
            i = self.0[i];
        }
        i
    }
}

fn same_place(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < SAME && (a.y - b.y).abs() < SAME
}

fn straight(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() < SAME || (a.y - b.y).abs() < SAME
}

/// The corner between `a` and `b` when they are not in line: leave `a` along
/// `leaves`, or horizontally when nothing says.
fn corner(a: Point, b: Point, leaves: Option<Direction>) -> Option<Point> {
    if straight(a, b) {
        return None;
    }
    Some(match leaves {
        Some(Direction::Up | Direction::Down) => Point::new(a.x, b.y),
        _ => Point::new(b.x, a.y),
    })
}

/// Builds the wires and junctions of `runs` over `points`.
pub(super) fn wires_from_runs(points: &[RunPoint], runs: &[(PinKey, PinKey)]) -> RunWires {
    let index: BTreeMap<&PinKey, usize> = points
        .iter()
        .enumerate()
        .map(|(i, p)| (&p.key, i))
        .collect();
    let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for (a, b) in runs {
        if let (Some(&i), Some(&j)) = (index.get(a), index.get(b))
            && i != j
            && points[i].page == points[j].page
        {
            edges.insert((i.min(j), i.max(j)));
        }
    }

    // A node on the very spot of a pin (or of another node) is that pin.
    let mut uf = Union((0..points.len()).collect());
    for &(i, j) in &edges {
        let both_pins = !points[i].node && !points[j].node;
        if !both_pins && same_place(points[i].at, points[j].at) {
            let (a, b) = (uf.find(i), uf.find(j));
            if a != b {
                // The pin stays the root; of two nodes the lower index does.
                let (keep, lose) = match (points[a].node, points[b].node) {
                    (true, false) => (b, a),
                    (false, true) => (a, b),
                    _ => (a.min(b), a.max(b)),
                };
                uf.0[lose] = keep;
            }
        }
    }
    let mut adj: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for &(i, j) in &edges {
        let (a, b) = (uf.find(i), uf.find(j));
        if a != b {
            adj.entry(a).or_default().insert(b);
            adj.entry(b).or_default().insert(a);
        }
    }

    let mut dropped: BTreeSet<usize> = BTreeSet::new();
    let mut loops_cut = 0;
    let (chains, junction_of) = loop {
        prune_stubs(points, &mut adj, &mut dropped);
        // Pins end wires; nodes with three or more runs are junctions.
        let vertex = |v: usize, adj: &BTreeMap<usize, BTreeSet<usize>>| {
            !points[v].node || adj.get(&v).is_some_and(|n| n.len() >= 3)
        };
        let mut chains: Vec<Vec<usize>> = Vec::new();
        let mut loops: Vec<(usize, usize)> = Vec::new();
        let mut walked: BTreeSet<(usize, usize)> = BTreeSet::new();
        for (&start, neighbours) in &adj {
            if !vertex(start, &adj) {
                continue;
            }
            for &first in neighbours {
                if walked.contains(&(start, first)) {
                    continue;
                }
                // Follow the corners until the next pin or junction.
                let mut path = vec![start];
                let (mut prev, mut cur) = (start, first);
                while !vertex(cur, &adj) {
                    path.push(cur);
                    let Some(&next) = adj[&cur].iter().find(|&&n| n != prev) else {
                        break;
                    };
                    prev = cur;
                    cur = next;
                    if path.len() > points.len() {
                        break;
                    }
                }
                if !vertex(cur, &adj) {
                    continue;
                }
                walked.insert((start, first));
                walked.insert((cur, prev));
                path.push(cur);
                if start == cur {
                    // A run that comes back to where it began is no wire:
                    // cut it open at its first run and look again.
                    loops.push((start, first));
                } else {
                    chains.push(path);
                }
            }
        }
        if loops.is_empty() {
            let junction_of: BTreeMap<usize, usize> = adj
                .keys()
                .filter(|&&v| points[v].node && vertex(v, &adj))
                .copied()
                .enumerate()
                .map(|(j, v)| (v, j))
                .collect();
            break (chains, junction_of);
        }
        for (a, b) in loops {
            if let Some(set) = adj.get_mut(&a) {
                set.remove(&b);
            }
            if let Some(set) = adj.get_mut(&b) {
                set.remove(&a);
            }
            loops_cut += 1;
        }
    };

    let mut out = RunWires {
        dropped_nodes: dropped.len(),
        loops_cut,
        junction_nodes: junction_of.len(),
        ..RunWires::default()
    };
    let mut by_junction: Vec<(usize, usize)> = junction_of.iter().map(|(&v, &j)| (j, v)).collect();
    by_junction.sort();
    for (_, v) in by_junction {
        out.junctions.push((points[v].page, points[v].at));
    }
    let end_of = |v: usize| match junction_of.get(&v) {
        Some(&j) => RunEnd::Junction(j),
        None => RunEnd::Pin(v),
    };
    for path in chains {
        let (start, cur) = (path[0], path[path.len() - 1]);
        out.wires.push(RunWire {
            page: points[start].page,
            from: end_of(start),
            to: end_of(cur),
            route: corners(points, &path),
        });
    }
    out
}

/// Removes nodes reached by one run only, and so on inwards.
fn prune_stubs(
    points: &[RunPoint],
    adj: &mut BTreeMap<usize, BTreeSet<usize>>,
    dropped: &mut BTreeSet<usize>,
) {
    let mut stack: Vec<usize> = adj
        .iter()
        .filter(|(v, n)| points[**v].node && n.len() < 2)
        .map(|(&v, _)| v)
        .collect();
    while let Some(v) = stack.pop() {
        if !dropped.insert(v) {
            continue;
        }
        for n in adj.remove(&v).unwrap_or_default() {
            if let Some(set) = adj.get_mut(&n) {
                set.remove(&v);
                if points[n].node && set.len() < 2 {
                    stack.push(n);
                }
            }
        }
    }
}

/// The corners of a wire along `path`: the nodes where it turns, plus a
/// right-angle leg where a pin is not in line with its neighbour.
fn corners(points: &[RunPoint], path: &[usize]) -> Vec<Point> {
    let last = path.len() - 1;
    let mut full: Vec<Point> = Vec::new();
    for w in 0..path.len() {
        let here = points[path[w]].at;
        if w > 0 {
            let before = points[path[w - 1]].at;
            let leaves = if w == 1 { points[path[0]].leaves } else { None };
            if w == last && !points[path[w]].node && points[path[w]].leaves.is_some() {
                // Arrive along the pin's own direction.
                if let Some(c) = corner(here, before, points[path[w]].leaves) {
                    full.push(c);
                }
            } else if let Some(c) = corner(before, here, leaves) {
                full.push(c);
            }
        }
        full.push(here);
    }
    full.dedup_by(|a, b| same_place(*a, *b));
    let mut out = Vec::new();
    for i in 1..full.len().saturating_sub(1) {
        let (a, b, c) = (full[i - 1], full[i], full[i + 1]);
        let line = ((a.x - b.x).abs() < SAME && (b.x - c.x).abs() < SAME)
            || ((a.y - b.y).abs() < SAME && (b.y - c.y).abs() < SAME);
        if !line {
            out.push(b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pin(key: &str, x: f64, y: f64, leaves: Direction) -> RunPoint {
        RunPoint {
            key: PinKey(key.into()),
            page: 0,
            at: Point::new(x, y),
            node: false,
            leaves: Some(leaves),
        }
    }

    fn node(key: &str, x: f64, y: f64) -> RunPoint {
        RunPoint {
            key: PinKey(key.into()),
            page: 0,
            at: Point::new(x, y),
            node: true,
            leaves: None,
        }
    }

    fn run(a: &str, b: &str) -> (PinKey, PinKey) {
        (PinKey(a.into()), PinKey(b.into()))
    }

    #[test]
    fn a_corner_node_is_a_bend_of_one_wire() {
        let pts = [
            pin("a", 0.0, 0.0, Direction::Right),
            node("n", 10.0, 0.0),
            pin("b", 10.0, 20.0, Direction::Down),
        ];
        let r = wires_from_runs(&pts, &[run("a", "n"), run("n", "b")]);
        assert!(r.junctions.is_empty());
        assert_eq!(r.wires.len(), 1);
        assert_eq!(r.wires[0].route, vec![Point::new(10.0, 0.0)]);
    }

    #[test]
    fn a_tee_node_is_a_junction_the_three_wires_end_on() {
        // A trunk from `a` to `b` with a branch down to `c`.
        let pts = [
            pin("a", 0.0, 0.0, Direction::Right),
            node("t", 10.0, 0.0),
            pin("b", 20.0, 0.0, Direction::Left),
            pin("c", 10.0, 15.0, Direction::Up),
        ];
        let r = wires_from_runs(&pts, &[run("a", "t"), run("t", "b"), run("t", "c")]);
        assert_eq!(r.junctions, vec![(0, Point::new(10.0, 0.0))]);
        assert_eq!(r.wires.len(), 3);
        assert!(r.wires.iter().all(|w| w.route.is_empty()));
        for w in &r.wires {
            assert!(
                w.from == RunEnd::Junction(0) || w.to == RunEnd::Junction(0),
                "{w:?}"
            );
        }
    }

    #[test]
    fn a_stub_without_a_pin_carries_nothing_and_its_dot_goes() {
        // The tee's third run goes to a node and stops: the tee is a bend.
        let pts = [
            pin("a", 0.0, 0.0, Direction::Right),
            node("t", 10.0, 0.0),
            pin("b", 20.0, 0.0, Direction::Left),
            node("stub", 10.0, 9.0),
        ];
        let r = wires_from_runs(&pts, &[run("a", "t"), run("t", "b"), run("t", "stub")]);
        assert!(r.junctions.is_empty());
        assert_eq!(r.wires.len(), 1);
        assert_eq!(r.dropped_nodes, 1);
    }

    #[test]
    fn a_node_on_a_pin_is_that_pin() {
        let pts = [
            pin("a", 0.0, 0.0, Direction::Right),
            node("n", 0.0, 0.0),
            pin("b", 10.0, 0.0, Direction::Left),
            pin("c", 0.0, 10.0, Direction::Up),
        ];
        let r = wires_from_runs(&pts, &[run("a", "n"), run("n", "b"), run("n", "c")]);
        assert!(r.junctions.is_empty());
        assert_eq!(r.wires.len(), 2);
        assert!(
            r.wires
                .iter()
                .all(|w| w.from == RunEnd::Pin(0) || w.to == RunEnd::Pin(0))
        );
    }

    #[test]
    fn a_pin_off_the_line_of_its_node_gets_a_right_angle_leg() {
        let pts = [
            pin("a", 0.0, 3.0, Direction::Right),
            node("n", 10.0, 0.0),
            pin("b", 20.0, 0.0, Direction::Left),
        ];
        let r = wires_from_runs(&pts, &[run("a", "n"), run("n", "b")]);
        assert_eq!(r.wires.len(), 1);
        assert_eq!(
            r.wires[0].route,
            vec![Point::new(10.0, 3.0), Point::new(10.0, 0.0)]
        );
    }

    #[test]
    fn two_pins_facing_each_other_get_a_plain_wire() {
        let pts = [
            pin("a", 0.0, 0.0, Direction::Right),
            pin("b", 10.0, 0.0, Direction::Left),
        ];
        let r = wires_from_runs(&pts, &[run("a", "b"), run("b", "a")]);
        assert_eq!(r.wires.len(), 1);
        assert!(r.wires[0].route.is_empty());
    }

    #[test]
    fn a_run_that_loops_back_is_cut_and_leaves_no_dangling_dot() {
        // `t` joins `a`, `b` and a little loop through two corners back to
        // itself: the loop is no wire, and without it `t` is a plain bend.
        let pts = [
            pin("a", 0.0, 0.0, Direction::Right),
            node("t", 10.0, 0.0),
            pin("b", 20.0, 0.0, Direction::Left),
            node("l1", 10.0, 5.0),
            node("l2", 15.0, 5.0),
            node("l3", 15.0, 0.0),
        ];
        let runs = [
            run("a", "t"),
            run("t", "b"),
            run("t", "l1"),
            run("l1", "l2"),
            run("l2", "l3"),
            run("l3", "t"),
        ];
        let r = wires_from_runs(&pts, &runs);
        assert!(r.loops_cut >= 1);
        assert!(r.junctions.iter().all(|_| false), "{:?}", r.junctions);
        assert_eq!(r.wires.len(), 1);
    }
}
