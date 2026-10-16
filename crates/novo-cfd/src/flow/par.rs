//! Thread helpers for the flow kernels (lane CFD-E).
//!
//! Only loops whose cells are independent are run in parallel, and only
//! with per-cell writes or max reductions, so results are bit-identical to
//! the serial loop whatever the thread count. Sums (dot products) and the
//! Gauss-Seidel sweeps stay serial.

use rayon::prelude::*;

/// Below this many values a loop runs on the calling thread.
pub(super) const MIN_VALUES: usize = 4096;

/// Calls `f(k, plane)` for every k-plane of `out` (`plane_len` values each).
pub(super) fn for_planes<F>(out: &mut [f64], plane_len: usize, f: F)
where
    F: Fn(usize, &mut [f64]) + Sync + Send,
{
    if plane_len == 0 {
        return;
    }
    if out.len() < MIN_VALUES {
        for (k, p) in out.chunks_mut(plane_len).enumerate() {
            f(k, p);
        }
    } else {
        out.par_chunks_mut(plane_len)
            .enumerate()
            .for_each(|(k, p)| f(k, p));
    }
}

/// Maximum of `f(k)` over the k-planes `0..nz` (`values` = work size hint).
/// Exact: a maximum does not depend on the order.
pub(super) fn max_over_planes<F>(nz: usize, values: usize, f: F) -> f64
where
    F: Fn(usize) -> f64 + Sync + Send,
{
    if values < MIN_VALUES {
        (0..nz).map(f).fold(f64::NEG_INFINITY, f64::max)
    } else {
        (0..nz)
            .into_par_iter()
            .map(f)
            .reduce(|| f64::NEG_INFINITY, f64::max)
    }
}
