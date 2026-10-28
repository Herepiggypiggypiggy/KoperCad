//! One-dimensional grading: cell edges along an axis with fine cells at the
//! planes that matter (walls, faces of solid bodies, opening edges) growing
//! geometrically towards a coarse size, a guaranteed number of cells across
//! every body, and a hard bound on the size ratio of neighbours.
//!
//! The grid is a tensor product, so a plane placed for one body runs through
//! the whole cabinet. That is why the coarse size is the free variable of
//! the cell budget (see `plan_grid`): bodies take what they need and empty
//! air takes the rest.

/// Size rules for one grid level, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Largest cell size in empty air.
    pub far: f64,
    /// Cell size at a plane, before bodies tighten it.
    pub near: f64,
    /// Geometric growth away from a plane.
    pub growth: f64,
    /// Hard bound on `larger / smaller` for neighbouring cells.
    pub max_ratio: f64,
}

/// An interval of an axis that is wanted resolved: a solid body or an
/// opening. Its faces become planes and it gets at least `need` cells.
/// A `sheet` is a thin body that keeps its own cell layer: its planes are
/// never merged away and it is one cell thick, whatever the neighbours do.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    pub lo: f64,
    pub hi: f64,
    pub need: usize,
    pub sheet: bool,
}

#[derive(Clone, Copy)]
struct Plane {
    pos: f64,
    keep: bool,
}

/// Cell edges `0 ..= len`, increasing, first `0.0`, last `len`.
pub fn axis_edges(len: f64, spans: &[Span], sp: &Spacing) -> Vec<f64> {
    let planes = merge_planes(len, spans, sp);
    let n_seg = planes.len() - 1;
    // Size limit of each segment: far in air, extent / need inside a body.
    let cap: Vec<f64> = (0..n_seg)
        .map(|i| {
            let mid = 0.5 * (planes[i].pos + planes[i + 1].pos);
            spans
                .iter()
                .filter(|s| s.need > 0 && s.lo <= mid && mid <= s.hi)
                .map(|s| (s.hi - s.lo) / s.need as f64)
                .fold(sp.far, f64::min)
        })
        .collect();
    // Inside a body the cells may grow faster: conduction in a solid needs
    // no boundary layer, and the cells saved go to the air.
    let growth: Vec<f64> = (0..n_seg)
        .map(|i| {
            let mid = 0.5 * (planes[i].pos + planes[i + 1].pos);
            if spans
                .iter()
                .any(|s| s.need > 1 && s.lo <= mid && mid <= s.hi)
            {
                sp.growth.max(0.97 * sp.max_ratio)
            } else {
                sp.growth
            }
        })
        .collect();
    // Size at a plane: the finer of what the segments on both sides allow.
    let q: Vec<f64> = (0..planes.len())
        .map(|i| {
            let left = if i > 0 { cap[i - 1] } else { f64::MAX };
            let right = if i < n_seg { cap[i] } else { f64::MAX };
            sp.near.min(left).min(right)
        })
        .collect();

    let mut edges = vec![0.0];
    for i in 0..n_seg {
        let (a, b) = (planes[i].pos, planes[i + 1].pos);
        let steps = segment_steps(b - a, q[i], q[i + 1], cap[i], growth[i]);
        let n = steps.len();
        let mut x = a;
        for (k, s) in steps.into_iter().enumerate() {
            x += s;
            edges.push(if k + 1 == n { b } else { x });
        }
    }
    enforce_ratio(&mut edges, sp.max_ratio);
    edges
}

/// Largest gap between two planes that counts as a sliver: the faces stay
/// planes of their own above this, however coarse the air is.
const MAX_SLIVER_MM: f64 = 5.0;

/// Planes from the span faces: dedup, drop slivers (a plane within 15
/// percent of the coarse size, at most [`MAX_SLIVER_MM`], of another one is
/// left to cut cells) except the faces of thin sheets.
fn merge_planes(len: f64, spans: &[Span], sp: &Spacing) -> Vec<Plane> {
    let mut cand: Vec<Plane> = Vec::new();
    for s in spans.iter().filter(|s| s.need > 0) {
        for p in [s.lo, s.hi] {
            if p.is_finite() && p > 0.0 && p < len {
                cand.push(Plane {
                    pos: p,
                    keep: s.sheet,
                });
            }
        }
    }
    cand.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    let min_gap = (0.15 * sp.far).clamp(0.2, MAX_SLIVER_MM);
    let mut out = vec![Plane {
        pos: 0.0,
        keep: true,
    }];
    for p in cand {
        let last = *out.last().unwrap();
        let gap = p.pos - last.pos;
        if gap >= min_gap || (p.keep && last.keep && gap >= 0.2) {
            out.push(p);
        } else if p.keep && !last.keep && out.len() > 1 {
            // a sheet face wins over a nearby ordinary plane
            *out.last_mut().unwrap() = p;
            let n = out.len();
            if n > 2 && out[n - 1].pos - out[n - 2].pos < 0.2 {
                out.remove(n - 2);
            }
        }
    }
    // the far end: ordinary planes too close to it go
    while out.len() > 1 {
        let l = *out.last().unwrap();
        if len - l.pos < if l.keep { 0.2 } else { min_gap } {
            out.pop();
        } else {
            break;
        }
    }
    out.push(Plane {
        pos: len,
        keep: true,
    });
    out
}

/// Cell sizes filling a segment of length `l`: `qa` and `qb` at its ends,
/// growing by `growth` up to `cap` in between.
fn segment_steps(l: f64, qa: f64, qb: f64, cap: f64, growth: f64) -> Vec<f64> {
    let (mut ha, mut hb) = (qa.min(cap), qb.min(cap));
    let (mut left, mut right) = (Vec::new(), Vec::new());
    let (mut cl, mut cr) = (0.0, 0.0);
    loop {
        let from_left = cl <= cr;
        let h = if from_left { ha } else { hb };
        if cl + cr + h > l + 1e-12 {
            break;
        }
        if from_left {
            left.push(ha);
            cl += ha;
            ha = (ha * growth).min(cap);
        } else {
            right.push(hb);
            cr += hb;
            hb = (hb * growth).min(cap);
        }
    }
    let h_mid = 0.5 * (ha + hb);
    let rem = l - cl - cr;
    let n_mid = (rem / h_mid).round().max(0.0) as usize;
    let mut steps = left;
    steps.extend(std::iter::repeat_n(h_mid, n_mid));
    steps.extend(right.iter().rev());
    if steps.is_empty() {
        let n = ((l / h_mid).round() as usize).max(1);
        return vec![l / n as f64; n];
    }
    // The slack goes to the cells at the coarse size and the middle ones, so
    // the graded part keeps its growth and no ratio cascade starts.
    let is_flex = |v: f64| v >= cap * (1.0 - 1e-12) || v == h_mid;
    let flex_sum: f64 = steps.iter().filter(|v| is_flex(**v)).sum();
    let fixed_sum: f64 = steps.iter().filter(|v| !is_flex(**v)).sum();
    if flex_sum == 0.0 {
        let scale = l / steps.iter().sum::<f64>();
        steps.iter_mut().for_each(|v| *v *= scale);
    } else {
        let f = (l - fixed_sum) / flex_sum;
        for v in steps.iter_mut() {
            if is_flex(*v) {
                *v *= f;
            }
        }
    }
    steps
}

/// Cells the halving cascade of [`enforce_ratio`] may add, per cell it starts
/// with (plus [`CASCADE_SLACK`]), before the optimal split takes over.
const CASCADE_PER_CELL: usize = 1;
const CASCADE_SLACK: usize = 64;
/// Most parts one cell is cut into by the optimal split.
const MAX_PARTS: usize = 512;

/// Split the larger cell of any pair whose ratio exceeds `max_ratio` until
/// none does. Only adds edges, so planes stay edges.
///
/// Halving the larger cell is the usual cheap fix and adds a few edges. On
/// some edge patterns (a row of cells that alternate in size, as the planes of
/// a cabinet with rails and ducts give) the halves keep forcing their
/// neighbours to halve and the cells shrink without end: a 1 m axis ended with
/// thousands of 0.06 mm cells and the run never got past meshing. When the
/// cascade has added more than it should, the cheapest valid split takes over
/// (see [`graded_split`]).
pub fn enforce_ratio(edges: &mut Vec<f64>, max_ratio: f64) {
    let original = edges.clone();
    let budget = original.len() * CASCADE_PER_CELL + CASCADE_SLACK;
    if cascade(edges, max_ratio, original.len() + budget) {
        return;
    }
    *edges = graded_split(&original, max_ratio).unwrap_or(original);
}

/// The halving cascade; `false` (edges left half done) when it would take
/// the list past `limit` edges.
fn cascade(edges: &mut Vec<f64>, max_ratio: f64, limit: usize) -> bool {
    loop {
        let mut changed = false;
        let mut i = 0;
        while i + 2 < edges.len() {
            let a = edges[i + 1] - edges[i];
            let b = edges[i + 2] - edges[i + 1];
            if a > max_ratio * b {
                let mid = 0.5 * (edges[i] + edges[i + 1]);
                edges.insert(i + 1, mid);
                changed = true;
            } else if b > max_ratio * a {
                let mid = 0.5 * (edges[i + 1] + edges[i + 2]);
                edges.insert(i + 2, mid);
                changed = true;
            }
            if edges.len() > limit {
                return false;
            }
            i += 1;
        }
        if !changed {
            return true;
        }
    }
}

/// Cuts every cell of `edges` into equal parts so that neighbouring parts
/// differ by at most `max_ratio` and the total number of cells is the least
/// that allows it. Dynamic programming over the part count of each cell: the
/// parts of the next cell that fit one choice are a contiguous range of
/// counts, so a sliding minimum keeps it linear. `None` when no cut within
/// [`MAX_PARTS`] works or an edge is not increasing.
fn graded_split(edges: &[f64], max_ratio: f64) -> Option<Vec<f64>> {
    use std::collections::VecDeque;
    let cells = edges.len().checked_sub(1).filter(|n| *n > 0)?;
    let h: Vec<f64> = edges.windows(2).map(|w| w[1] - w[0]).collect();
    if h.iter().any(|v| !(*v > 0.0 && v.is_finite())) {
        return None;
    }
    let min_h = h.iter().copied().fold(f64::MAX, f64::min);
    // No part is made much smaller than the smallest cell there already is.
    let kmax: Vec<usize> = h
        .iter()
        .map(|v| ((v / (0.5 * min_h)).ceil() as usize).clamp(1, MAX_PARTS))
        .collect();
    let r = max_ratio * (1.0 - 1e-9);
    const INF: u32 = u32::MAX / 2;
    // cost[i][k - 1]: fewest cells for cells 0..=i when cell i has k parts;
    // from[i][k - 1]: the part count of cell i - 1 behind it.
    let mut cost: Vec<Vec<u32>> = Vec::with_capacity(cells);
    let mut from: Vec<Vec<u32>> = Vec::with_capacity(cells);
    cost.push((1..=kmax[0] as u32).collect());
    from.push(vec![0; kmax[0]]);
    for i in 1..cells {
        let prev = &cost[i - 1];
        let mut c = vec![INF; kmax[i]];
        let mut f = vec![0u32; kmax[i]];
        let mut window: VecDeque<usize> = VecDeque::new();
        let mut next = 1usize; // the next part count of cell i - 1 to add
        let mut lo_prev = 1usize;
        for k in 1..=kmax[i] {
            let p = h[i] / k as f64;
            // part counts k' of the previous cell with h' / k' in [p / r, p r]
            let lo = ((h[i - 1] / (p * r)).ceil() as usize).clamp(1, kmax[i - 1] + 1);
            let hi = ((h[i - 1] * r / p).floor() as usize).min(kmax[i - 1]);
            let lo = lo.max(lo_prev);
            lo_prev = lo;
            while next <= hi {
                while window
                    .back()
                    .is_some_and(|&b| prev[b - 1] >= prev[next - 1])
                {
                    window.pop_back();
                }
                window.push_back(next);
                next += 1;
            }
            while window.front().is_some_and(|&w| w < lo) {
                window.pop_front();
            }
            if let Some(&best) = window.front()
                && prev[best - 1] < INF
            {
                c[k - 1] = prev[best - 1] + k as u32;
                f[k - 1] = best as u32;
            }
        }
        cost.push(c);
        from.push(f);
    }
    let last = cost.last()?;
    let (mut k, best) = last
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .map(|(i, v)| (i + 1, *v))?;
    if best >= INF {
        return None;
    }
    let mut parts = vec![0usize; cells];
    for i in (0..cells).rev() {
        parts[i] = k;
        k = from[i][k - 1] as usize;
    }
    let mut out = vec![edges[0]];
    for (i, n) in parts.iter().enumerate() {
        for j in 1..=*n {
            out.push(if j == *n {
                edges[i + 1]
            } else {
                edges[i] + h[i] * j as f64 / *n as f64
            });
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cells of the given sizes, repeated, as edges from 0.
    fn edges_of(pattern: &[f64], times: usize) -> Vec<f64> {
        let mut e = vec![0.0];
        for _ in 0..times {
            for c in pattern {
                e.push(e.last().unwrap() + c);
            }
        }
        e
    }

    fn worst_ratio(e: &[f64]) -> f64 {
        let h: Vec<f64> = e.windows(2).map(|w| w[1] - w[0]).collect();
        h.windows(2)
            .map(|w| (w[0] / w[1]).max(w[1] / w[0]))
            .fold(1.0, f64::max)
    }

    #[test]
    fn alternating_cells_no_longer_refine_without_end() {
        // Cells that alternate 10.15 / 6.22 beside 7.8: halving the larger of
        // a pair forced its neighbours to halve, over and over (3000 edges
        // from 100 and still going; a 1 m axis took minutes).
        for pattern in [
            &[7.8, 7.8, 10.15, 6.22][..],
            &[7.8, 7.8, 7.8, 10.15, 6.22][..],
            &[8.0, 8.0, 10.15, 6.2, 8.0, 8.0][..],
        ] {
            let original = edges_of(pattern, 25);
            let mut e = original.clone();
            enforce_ratio(&mut e, 1.5);
            assert!(worst_ratio(&e) <= 1.5, "{pattern:?}: {}", worst_ratio(&e));
            assert!(
                e.len() <= 3 * original.len(),
                "{pattern:?}: {} edges from {}",
                e.len(),
                original.len()
            );
            // Every original edge (the planes of the bodies) is still an edge.
            assert!(
                original
                    .iter()
                    .all(|o| e.iter().any(|x| (x - o).abs() < 1e-9)),
                "{pattern:?}: an edge moved"
            );
        }
    }

    #[test]
    fn a_mild_violation_still_takes_the_cheap_cascade() {
        // One pair too far apart: a few halvings, the usual result.
        let mut e = edges_of(&[10.0, 10.0, 4.0, 4.0, 4.0], 1);
        let before = e.len();
        enforce_ratio(&mut e, 1.5);
        assert!(worst_ratio(&e) <= 1.5);
        assert!(e.len() <= before + 6, "{} from {before}", e.len());
    }

    #[test]
    fn a_big_jump_ends_with_the_ratio_held() {
        let mut e = vec![0.0, 0.5, 100.0];
        enforce_ratio(&mut e, 1.5);
        assert!(worst_ratio(&e) <= 1.5);
        // Equal parts cannot grade: the big cell is cut to the small one's size.
        assert!(e.len() < 200, "{}", e.len());
    }
}
