//! Stencil storage and Gauss-Seidel sweeps for the steady path.

use rayon::prelude::*;

/// One linear equation of a field point:
/// `diag x = rhs + sum_b lo[b] x[id - stride_b] + hi[b] x[id + stride_b]`.
/// `diag == 0` marks a point that is not solved (wall face, solid).
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Coef {
    pub diag: f64,
    pub rhs: f64,
    /// Deferred correction of the scheme, added to the right side after blending
    /// with the previous iteration's.
    pub defer: f64,
    pub lo: [f64; 3],
    pub hi: [f64; 3],
}

/// Below this many points a loop runs on the calling thread.
const MIN_POINTS: usize = 4096;

/// Calls `f(k, plane)` for every k-plane of `out`.
pub(super) fn par_planes<T, F>(out: &mut [T], plane_len: usize, f: F)
where
    T: Send,
    F: Fn(usize, &mut [T]) + Sync + Send,
{
    if plane_len == 0 {
        return;
    }
    if out.len() < MIN_POINTS {
        for (k, p) in out.chunks_mut(plane_len).enumerate() {
            f(k, p);
        }
    } else {
        out.par_chunks_mut(plane_len)
            .enumerate()
            .for_each(|(k, p)| f(k, p));
    }
}

/// Strides of a field with `d` points per axis.
pub(super) fn strides(d: [usize; 3]) -> [usize; 3] {
    [1, d[0], d[0] * d[1]]
}

/// One Gauss-Seidel sweep over `x`, lexicographic when `forward`, reversed
/// otherwise. Returns the largest change.
pub(super) fn sweep(coef: &[Coef], x: &mut [f64], d: [usize; 3], forward: bool) -> f64 {
    let st = strides(d);
    let mut change: f64 = 0.0;
    let mut visit = |i: usize, j: usize, k: usize| {
        let id = k * st[2] + j * st[1] + i;
        let c = &coef[id];
        if c.diag <= 0.0 {
            return;
        }
        let ix = [i, j, k];
        let mut s = c.rhs;
        for b in 0..3 {
            if c.lo[b] != 0.0 {
                s += c.lo[b] * x[id - st[b]];
            }
            if c.hi[b] != 0.0 {
                debug_assert!(ix[b] + 1 < d[b]);
                s += c.hi[b] * x[id + st[b]];
            }
        }
        let new = s / c.diag;
        change = change.max((new - x[id]).abs());
        x[id] = new;
    };
    if forward {
        for k in 0..d[2] {
            for j in 0..d[1] {
                for i in 0..d[0] {
                    visit(i, j, k);
                }
            }
        }
    } else {
        for k in (0..d[2]).rev() {
            for j in (0..d[1]).rev() {
                for i in (0..d[0]).rev() {
                    visit(i, j, k);
                }
            }
        }
    }
    change
}

/// Largest `|rhs + sum nb - diag x|` and the largest `diag |x|`: the equation
/// residual and its natural scale.
pub(super) fn residual(coef: &[Coef], x: &[f64], d: [usize; 3]) -> (f64, f64) {
    let st = strides(d);
    let (mut r, mut s): (f64, f64) = (0.0, 0.0);
    for k in 0..d[2] {
        for j in 0..d[1] {
            for i in 0..d[0] {
                let id = k * st[2] + j * st[1] + i;
                let c = &coef[id];
                if c.diag <= 0.0 {
                    continue;
                }
                let mut v = c.rhs - c.diag * x[id];
                for b in 0..3 {
                    if c.lo[b] != 0.0 {
                        v += c.lo[b] * x[id - st[b]];
                    }
                    if c.hi[b] != 0.0 {
                        v += c.hi[b] * x[id + st[b]];
                    }
                }
                r = r.max(v.abs());
                s = s.max(c.diag * x[id].abs());
            }
        }
    }
    (r, s)
}

/// Folds the deferred correction into `rhs`: `lambda` of the new one and the
/// rest of the previous iteration's (damps the chatter of limited schemes).
pub(super) fn apply_defer(coef: &mut [Coef], prev: &mut [f64], lambda: f64) {
    coef.par_iter_mut()
        .zip(prev.par_iter_mut())
        .for_each(|(c, p)| {
            let d = lambda * c.defer + (1.0 - lambda) * *p;
            *p = d;
            c.rhs -= d;
        });
}
