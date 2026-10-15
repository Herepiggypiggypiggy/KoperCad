//! Contour lines by marching squares on the cell centres, chained into
//! polylines so they can be drawn as paths and labelled along their length.

use crate::field::Scalar;
use std::collections::HashMap;

/// A line of one level, in mm. Closed lines repeat their first point last.
#[derive(Debug, Clone, PartialEq)]
pub struct Contour {
    pub level: f32,
    pub points: Vec<(f64, f64)>,
}

impl Contour {
    pub fn closed(&self) -> bool {
        self.points.len() > 3 && self.points.first() == self.points.last()
    }

    /// Length in mm.
    pub fn length(&self) -> f64 {
        self.points
            .windows(2)
            .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
            .sum()
    }

    /// Where a label of `half_len` mm either side fits on a straight enough
    /// stretch: the middle of the longest such stretch, with the line angle
    /// (radians, `y` down) there. `None` when the line is too short.
    pub fn label_spot(&self, half_len: f64) -> Option<((f64, f64), f64)> {
        let n = self.points.len();
        if n < 2 || self.length() < half_len * 4.0 {
            return None;
        }
        let mut best: Option<(f64, usize)> = None;
        for i in 1..n - 1 {
            let (a, b, c) = (self.points[i - 1], self.points[i], self.points[i + 1]);
            let (v1, v2) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
            let cross = (v1.0 * v2.1 - v1.1 * v2.0).abs();
            let norm = v1.0.hypot(v1.1) * v2.0.hypot(v2.1);
            let straight = if norm > 0.0 { 1.0 - cross / norm } else { 0.0 };
            if best.is_none_or(|(s, _)| straight > s) {
                best = Some((straight, i));
            }
        }
        let (_, i) = best?;
        let (a, c) = (self.points[i - 1], self.points[i + 1]);
        Some((self.points[i], (c.1 - a.1).atan2(c.0 - a.0)))
    }
}

/// Edge of the grid a crossing sits on: horizontal `(0, col, row)` joins
/// centres `(col, row)` and `(col + 1, row)`, vertical `(1, col, row)` joins
/// `(col, row)` and `(col, row + 1)`.
type EdgeId = (u8, usize, usize);

/// Contour lines of `field` at `level`. Cells touching a solid (`NaN`) are
/// skipped, so lines stop at device bodies instead of running through them.
pub fn contours_at(field: &Scalar, level: f32) -> Vec<Contour> {
    let g = field.grid;
    if g.cols < 2 || g.rows < 2 {
        return Vec::new();
    }
    let value = |c: usize, r: usize| field.at(c, r);
    let crossing = |e: EdgeId| -> (f64, f64) {
        let ((c0, r0), (c1, r1)) = match e.0 {
            0 => ((e.1, e.2), (e.1 + 1, e.2)),
            _ => ((e.1, e.2), (e.1, e.2 + 1)),
        };
        let (a, b) = (value(c0, r0), value(c1, r1));
        let f = if b != a {
            ((level - a) / (b - a)).clamp(0.0, 1.0) as f64
        } else {
            0.5
        };
        let (x0, y0) = g.centre(c0, r0);
        let (x1, y1) = g.centre(c1, r1);
        (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f)
    };

    let mut links: HashMap<EdgeId, Vec<EdgeId>> = HashMap::new();
    let mut link = |a: EdgeId, b: EdgeId| {
        links.entry(a).or_default().push(b);
        links.entry(b).or_default().push(a);
    };
    for r in 0..g.rows - 1 {
        for c in 0..g.cols - 1 {
            let v = [
                value(c, r),
                value(c + 1, r),
                value(c + 1, r + 1),
                value(c, r + 1),
            ];
            if v.iter().any(|x| x.is_nan()) {
                continue;
            }
            // Corners: 0 top left, 1 top right, 2 bottom right, 3 bottom left.
            let edges = [(0, c, r), (1, c + 1, r), (0, c, r + 1), (1, c, r)];
            let above = [v[0] >= level, v[1] >= level, v[2] >= level, v[3] >= level];
            // Edge k joins corner k and k + 1: top, right, bottom, left.
            let hit: Vec<usize> = (0..4).filter(|&k| above[k] != above[(k + 1) % 4]).collect();
            match hit.len() {
                2 => link(edges[hit[0]], edges[hit[1]]),
                4 => {
                    let centre = (v[0] + v[1] + v[2] + v[3]) / 4.0 >= level;
                    // Saddle: the centre value decides which corners join.
                    if centre == above[0] {
                        link(edges[0], edges[3]);
                        link(edges[1], edges[2]);
                    } else {
                        link(edges[0], edges[1]);
                        link(edges[2], edges[3]);
                    }
                }
                _ => {}
            }
        }
    }

    // Chain: start at open ends first, then take what is left as loops.
    let mut seen: HashMap<EdgeId, bool> = HashMap::new();
    let mut out = Vec::new();
    let mut starts: Vec<EdgeId> = links
        .iter()
        .filter(|(_, n)| n.len() == 1)
        .map(|(e, _)| *e)
        .collect();
    starts.sort();
    let mut rest: Vec<EdgeId> = links.keys().copied().collect();
    rest.sort();
    starts.extend(rest);
    for start in starts {
        if seen.contains_key(&start) {
            continue;
        }
        let mut chain = vec![start];
        seen.insert(start, true);
        let mut cur = start;
        loop {
            let next = links[&cur].iter().copied().find(|n| !seen.contains_key(n));
            match next {
                Some(n) => {
                    seen.insert(n, true);
                    chain.push(n);
                    cur = n;
                }
                None => {
                    if chain.len() > 2 && links[&cur].contains(&start) {
                        chain.push(start);
                    }
                    break;
                }
            }
        }
        if chain.len() >= 2 {
            out.push(Contour {
                level,
                points: chain.into_iter().map(crossing).collect(),
            });
        }
    }
    out
}

/// Contours for several levels, longest first within each level.
pub fn contours(field: &Scalar, levels: &[f64]) -> Vec<Contour> {
    let mut all = Vec::new();
    for &l in levels {
        let mut at = contours_at(field, l as f32);
        at.sort_by(|a, b| b.length().total_cmp(&a.length()));
        all.extend(at);
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::Grid;

    fn cone(n: usize) -> Scalar {
        let grid = Grid {
            cols: n,
            rows: n,
            cell_w: 10.0,
            cell_h: 10.0,
        };
        let mid = (n as f32 - 1.0) / 2.0;
        let values = (0..n * n)
            .map(|i| {
                let (c, r) = ((i % n) as f32, (i / n) as f32);
                100.0 - (c - mid).hypot(r - mid) * 10.0
            })
            .collect();
        Scalar::new(grid, values)
    }

    #[test]
    fn a_cone_gives_one_closed_ring_near_the_right_radius() {
        let f = cone(21);
        let lines = contours_at(&f, 70.0); // radius 3 cells = 30 mm
        assert_eq!(lines.len(), 1);
        let ring = &lines[0];
        assert!(ring.closed());
        let (cx, cy) = f.grid.centre(10, 10);
        for p in &ring.points {
            let d = (p.0 - cx).hypot(p.1 - cy);
            assert!((d - 30.0).abs() < 1.5, "{d}");
        }
    }

    #[test]
    fn a_ramp_gives_one_straight_open_line() {
        let grid = Grid {
            cols: 6,
            rows: 4,
            cell_w: 10.0,
            cell_h: 10.0,
        };
        let values = (0..24).map(|i| (i % 6) as f32 * 10.0).collect();
        let lines = contours_at(&Scalar::new(grid, values), 25.0);
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].closed());
        assert!(lines[0].points.iter().all(|p| (p.0 - 30.0).abs() < 1e-6));
        assert!((lines[0].length() - 30.0).abs() < 1e-6);
    }

    #[test]
    fn nothing_below_or_above_the_data() {
        let f = cone(9);
        assert!(contours_at(&f, 500.0).is_empty());
        assert!(contours_at(&f, -500.0).is_empty());
    }

    #[test]
    fn lines_stop_at_solids() {
        let mut f = cone(21);
        for r in 8..13 {
            for c in 8..13 {
                f.values[r * 21 + c] = f32::NAN;
            }
        }
        let lines = contours_at(&f, 70.0);
        let (cx, cy) = f.grid.centre(10, 10);
        assert!(
            lines
                .iter()
                .flat_map(|l| &l.points)
                .all(|p| (p.0 - cx).hypot(p.1 - cy) > 25.0)
        );
    }

    #[test]
    fn label_spot_needs_room() {
        let f = cone(21);
        let ring = &contours_at(&f, 70.0)[0];
        assert!(ring.label_spot(5.0).is_some());
        assert!(ring.label_spot(500.0).is_none());
    }
}
