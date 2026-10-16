//! One-dimensional grading: cell edges along an axis with fine cells at the
//! hard planes (walls, device faces, opening edges) growing geometrically
//! towards a coarse size, with a hard bound on the size ratio of neighbours.

/// Size rules for one grid level, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Largest cell size, far from every plane.
    pub far: f64,
    /// Cell size at a plane.
    pub near: f64,
    /// Geometric growth from `near` to `far`.
    pub growth: f64,
    /// Hard bound on `larger / smaller` for neighbouring cells.
    pub max_ratio: f64,
}

/// Planes of an axis. `boxes` are intervals of solid regions: both faces
/// become planes unless the interval is thinner than two near cells (then
/// the faces are left to cut cells). `points` always become planes.
#[derive(Debug, Clone, Default)]
pub struct AxisFeatures {
    pub boxes: Vec<(f64, f64)>,
    pub points: Vec<f64>,
}

/// Cell edges `0 ..= len`, increasing, first `0.0`, last `len`.
pub fn axis_edges(len: f64, feats: &AxisFeatures, sp: &Spacing) -> Vec<f64> {
    let mut cand: Vec<f64> = Vec::new();
    for &(lo, hi) in &feats.boxes {
        if hi - lo >= 2.0 * sp.near {
            cand.push(lo);
            cand.push(hi);
        }
    }
    cand.extend_from_slice(&feats.points);
    cand.retain(|&p| p.is_finite() && p > 0.0 && p < len);
    cand.sort_by(|a, b| a.total_cmp(b));
    let min_gap = 0.25 * sp.near;
    let mut planes = vec![0.0];
    for p in cand {
        let last = *planes.last().unwrap();
        if p - last >= min_gap && len - p >= min_gap {
            planes.push(p);
        }
    }
    planes.push(len);

    let mut edges = vec![0.0];
    for w in planes.windows(2) {
        let (a, b) = (w[0], w[1]);
        let mut x = a;
        let steps = segment_steps(b - a, sp);
        let n = steps.len();
        for (i, s) in steps.into_iter().enumerate() {
            x += s;
            edges.push(if i + 1 == n { b } else { x });
        }
    }
    enforce_ratio(&mut edges, sp.max_ratio);
    edges
}

/// Cell sizes filling a segment of length `l`: fine at both ends.
fn segment_steps(l: f64, sp: &Spacing) -> Vec<f64> {
    let half = 0.5 * l;
    let mut left = Vec::new();
    let mut cum = 0.0;
    let mut h = sp.near.min(sp.far);
    while cum + h <= half + 1e-12 {
        left.push(h);
        cum += h;
        h = (h * sp.growth).min(sp.far);
    }
    if left.is_empty() {
        let n = ((l / h).round() as usize).max(1);
        return vec![l / n as f64; n];
    }
    // Graded cells from both ends plus uniform ones between. The slack is
    // taken up by the cells at the coarse size (and the middle ones), so
    // the graded part keeps its growth and no ratio cascade starts.
    let n_mid = ((l - 2.0 * cum) / h).round().max(0.0) as usize;
    let mut steps = left.clone();
    steps.extend(std::iter::repeat_n(h, n_mid));
    steps.extend(left.iter().rev());
    let is_flex = |v: f64| v >= sp.far * (1.0 - 1e-12) || v == h;
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
