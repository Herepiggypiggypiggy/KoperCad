//! Tensor-product B-spline (NURBS) surfaces: evaluation and the inverse
//! map from a point on the surface back to its (u, v), which face
//! boundaries need to be triangulated in parameter space.

use super::math::*;

/// A (rational) B-spline surface with the knots expanded. Control points
/// run along v first: point `(i, j)` is `ctrl[i * nv + j]`.
#[derive(Debug, Clone)]
pub struct SplineSurface {
    pub u_degree: usize,
    pub v_degree: usize,
    pub nu: usize,
    pub nv: usize,
    pub ctrl: Vec<V3>,
    pub weights: Option<Vec<f64>>,
    pub u_knots: Vec<f64>,
    pub v_knots: Vec<f64>,
}

/// Samples per direction of the coarse search for a first guess.
const SEARCH_GRID: usize = 16;

impl SplineSurface {
    /// `None` when the counts do not fit together.
    pub fn new(
        (u_degree, v_degree): (usize, usize),
        (nu, nv): (usize, usize),
        ctrl: Vec<V3>,
        weights: Option<Vec<f64>>,
        u_knots: Vec<f64>,
        v_knots: Vec<f64>,
    ) -> Option<SplineSurface> {
        let ok = u_degree >= 1
            && v_degree >= 1
            && nu > u_degree
            && nv > v_degree
            && ctrl.len() == nu * nv
            && u_knots.len() == nu + u_degree + 1
            && v_knots.len() == nv + v_degree + 1
            && weights
                .as_ref()
                .is_none_or(|w| w.len() == ctrl.len() && w.iter().all(|x| *x > 0.0))
            && u_knots.windows(2).all(|k| k[0] <= k[1])
            && v_knots.windows(2).all(|k| k[0] <= k[1])
            && u_knots[u_degree] < u_knots[nu]
            && v_knots[v_degree] < v_knots[nv];
        ok.then_some(SplineSurface {
            u_degree,
            v_degree,
            nu,
            nv,
            ctrl,
            weights,
            u_knots,
            v_knots,
        })
    }

    /// Parameter ranges `[(u0, u1), (v0, v1)]`.
    pub fn domain(&self) -> [(f64, f64); 2] {
        [
            (self.u_knots[self.u_degree], self.u_knots[self.nu]),
            (self.v_knots[self.v_degree], self.v_knots[self.nv]),
        ]
    }

    /// Point at (u, v); parameters outside the domain are clamped to it.
    pub fn eval(&self, u: f64, v: f64) -> V3 {
        let [(u0, u1), (v0, v1)] = self.domain();
        let (u, v) = (u.clamp(u0, u1), v.clamp(v0, v1));
        let su = span(&self.u_knots, self.nu, self.u_degree, u);
        let sv = span(&self.v_knots, self.nv, self.v_degree, v);
        let bu = basis(&self.u_knots, self.u_degree, su, u);
        let bv = basis(&self.v_knots, self.v_degree, sv, v);
        let mut acc = [0.0f64; 3];
        let mut wsum = 0.0;
        for (a, nu_a) in bu.iter().enumerate() {
            let i = su - self.u_degree + a;
            for (b, nv_b) in bv.iter().enumerate() {
                let j = sv - self.v_degree + b;
                let k = i * self.nv + j;
                let w = nu_a * nv_b * self.weights.as_ref().map_or(1.0, |w| w[k]);
                let c = self.ctrl[k];
                acc[0] += w * c[0];
                acc[1] += w * c[1];
                acc[2] += w * c[2];
                wsum += w;
            }
        }
        if wsum.abs() < 1e-14 {
            return acc;
        }
        scale(acc, 1.0 / wsum)
    }

    /// The (u, v) whose surface point is nearest `p`, and that distance.
    /// `seed` is a good first guess (the previous point of a boundary
    /// walk); without one, or when the seed does not lead to the surface,
    /// a coarse grid search finds one.
    pub fn project(&self, p: V3, seed: Option<[f64; 2]>) -> ([f64; 2], f64) {
        let mut best = None;
        if let Some(s) = seed {
            best = Some(self.refine_from(p, s));
        }
        // A seed that landed away from the surface may be on the wrong
        // sheet: only trust it when it is close.
        let near = best.is_some_and(|(_, d)| d < 1e-6 * (1.0 + self.extent()));
        if !near {
            let g = self.grid_guess(p);
            let r = self.refine_from(p, g);
            if best.is_none_or(|(_, d)| r.1 < d) {
                best = Some(r);
            }
        }
        best.unwrap_or(([0.0; 2], f64::INFINITY))
    }

    fn extent(&self) -> f64 {
        let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
        for c in &self.ctrl {
            for k in 0..3 {
                lo[k] = lo[k].min(c[k]);
                hi[k] = hi[k].max(c[k]);
            }
        }
        dist(lo, hi)
    }

    fn grid_guess(&self, p: V3) -> [f64; 2] {
        let [(u0, u1), (v0, v1)] = self.domain();
        let mut best = ([u0, v0], f64::INFINITY);
        for i in 0..=SEARCH_GRID {
            let u = u0 + (u1 - u0) * i as f64 / SEARCH_GRID as f64;
            for j in 0..=SEARCH_GRID {
                let v = v0 + (v1 - v0) * j as f64 / SEARCH_GRID as f64;
                let d = dist(self.eval(u, v), p);
                if d < best.1 {
                    best = ([u, v], d);
                }
            }
        }
        best.0
    }

    /// Gauss-Newton on the squared distance with finite-difference
    /// tangents; stays inside the domain.
    fn refine_from(&self, p: V3, start: [f64; 2]) -> ([f64; 2], f64) {
        let [(u0, u1), (v0, v1)] = self.domain();
        let (hu, hv) = ((u1 - u0) * 1e-6, (v1 - v0) * 1e-6);
        let mut q = [start[0].clamp(u0, u1), start[1].clamp(v0, v1)];
        let mut d = dist(self.eval(q[0], q[1]), p);
        for _ in 0..40 {
            let s = self.eval(q[0], q[1]);
            let r = sub(s, p);
            let su = scale(
                sub(
                    self.eval((q[0] + hu).min(u1), q[1]),
                    self.eval((q[0] - hu).max(u0), q[1]),
                ),
                1.0 / ((q[0] + hu).min(u1) - (q[0] - hu).max(u0)),
            );
            let sv = scale(
                sub(
                    self.eval(q[0], (q[1] + hv).min(v1)),
                    self.eval(q[0], (q[1] - hv).max(v0)),
                ),
                1.0 / ((q[1] + hv).min(v1) - (q[1] - hv).max(v0)),
            );
            let (a, b, c) = (dot(su, su), dot(su, sv), dot(sv, sv));
            let (gu, gv) = (dot(su, r), dot(sv, r));
            let det = a * c - b * b;
            let (du, dv) = if det.abs() > 1e-18 * (a * c).abs().max(1e-300) {
                (-(c * gu - b * gv) / det, -(a * gv - b * gu) / det)
            } else {
                // Degenerate tangents (a pole): step along the steepest
                // descent, scaled to the parameter range.
                (-gu * (u1 - u0) * 1e-3, -gv * (v1 - v0) * 1e-3)
            };
            // Halve the step until the point gets no farther.
            let mut f = 1.0;
            let mut moved = false;
            for _ in 0..12 {
                let t = [(q[0] + du * f).clamp(u0, u1), (q[1] + dv * f).clamp(v0, v1)];
                let nd = dist(self.eval(t[0], t[1]), p);
                if nd <= d {
                    let step = (t[0] - q[0]).abs() / (u1 - u0) + (t[1] - q[1]).abs() / (v1 - v0);
                    q = t;
                    d = nd;
                    moved = step > 1e-12;
                    break;
                }
                f *= 0.5;
            }
            if !moved {
                break;
            }
        }
        (q, d)
    }
}

/// Knot span `k` with `knots[k] <= t < knots[k + 1]` (the last span for
/// the end of the domain). `n` control points, degree `p`.
fn span(knots: &[f64], n: usize, p: usize, t: f64) -> usize {
    if t >= knots[n] {
        // The last non-empty span.
        let mut k = n - 1;
        while k > p && knots[k] >= knots[n] {
            k -= 1;
        }
        return k;
    }
    let (mut lo, mut hi) = (p, n);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if t < knots[mid] {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    lo
}

/// The `p + 1` basis functions that are not zero in span `k`.
fn basis(knots: &[f64], p: usize, k: usize, t: f64) -> Vec<f64> {
    let mut n = vec![0.0; p + 1];
    let mut left = vec![0.0; p + 1];
    let mut right = vec![0.0; p + 1];
    n[0] = 1.0;
    for j in 1..=p {
        left[j] = t - knots[k + 1 - j];
        right[j] = knots[k + j] - t;
        let mut saved = 0.0;
        for r in 0..j {
            let den = right[r + 1] + left[j - r];
            let tmp = if den.abs() < 1e-300 { 0.0 } else { n[r] / den };
            n[r] = saved + right[r + 1] * tmp;
            saved = left[j - r] * tmp;
        }
        n[j] = saved;
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bicubic Bezier patch over [0, 3] x [0, 3] with a bump in the
    /// middle: z is 0 on the border control points and 9 inside.
    pub(crate) fn bump() -> SplineSurface {
        let mut ctrl = Vec::new();
        for i in 0..4 {
            for j in 0..4 {
                let inner = (1..=2).contains(&i) && (1..=2).contains(&j);
                ctrl.push([
                    i as f64 * 10.0,
                    j as f64 * 10.0,
                    if inner { 9.0 } else { 0.0 },
                ]);
            }
        }
        let k = vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
        SplineSurface::new((3, 3), (4, 4), ctrl, None, k.clone(), k).unwrap()
    }

    #[test]
    fn corners_are_interpolated_and_the_middle_rises() {
        let s = bump();
        assert_eq!(s.eval(0.0, 0.0), [0.0, 0.0, 0.0]);
        assert_eq!(s.eval(1.0, 1.0), [30.0, 30.0, 0.0]);
        // Bezier midpoint: weights 1/8, 3/8, 3/8, 1/8 per direction.
        let m = s.eval(0.5, 0.5);
        assert!((m[2] - 9.0 * 0.75 * 0.75).abs() < 1e-12, "{m:?}");
        assert!((m[0] - 15.0).abs() < 1e-12);
    }

    #[test]
    fn a_point_on_the_surface_projects_back_to_its_parameters() {
        let s = bump();
        for (u, v) in [(0.3, 0.8), (0.5, 0.5), (0.9, 0.1), (0.0, 0.4)] {
            let p = s.eval(u, v);
            let (q, d) = s.project(p, None);
            assert!(d < 1e-6, "distance {d}");
            assert!((q[0] - u).abs() < 1e-5 && (q[1] - v).abs() < 1e-5, "{q:?}");
        }
    }

    #[test]
    fn rational_quarter_circle_stays_on_the_circle() {
        // Degree 2 x 1 patch: a quarter cylinder of radius 5, length 10.
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let mut ctrl = Vec::new();
        let mut weights = Vec::new();
        for (x, y, wt) in [(5.0, 0.0, 1.0), (5.0, 5.0, w), (0.0, 5.0, 1.0)] {
            for z in [0.0, 10.0] {
                ctrl.push([x, y, z]);
                weights.push(wt);
            }
        }
        let s = SplineSurface::new(
            (2, 1),
            (3, 2),
            ctrl,
            Some(weights),
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        for i in 0..=10 {
            let p = s.eval(i as f64 / 10.0, 0.5);
            assert!((p[0].hypot(p[1]) - 5.0).abs() < 1e-12);
            assert!((p[2] - 5.0).abs() < 1e-12);
        }
    }
}
