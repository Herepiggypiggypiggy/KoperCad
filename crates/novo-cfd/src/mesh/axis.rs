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

/// Split the larger cell of any pair whose ratio exceeds `max_ratio` until
/// none does. Only adds edges, so planes stay edges.
pub fn enforce_ratio(edges: &mut Vec<f64>, max_ratio: f64) {
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
            i += 1;
        }
        if !changed {
            return;
        }
    }
}
