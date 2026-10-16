//! Grid sequencing: a solution on a coarse mesh as the starting point on a
//! finer one. The meshes need not be nested (the cabinet mesher builds each
//! level on its own); every field is interpolated linearly along each axis
//! at the sample positions of the fine mesh, constant beyond the coarse
//! samples.

use super::super::FlowSolver;
use crate::flow::mesh::Mesh;

/// `(low, high, weight of high)` of every target position among the sources.
fn axis_weights(src: &[f64], dst: &[f64]) -> Vec<(usize, usize, f64)> {
    let n = src.len();
    dst.iter()
        .map(|&x| {
            if n == 1 || x <= src[0] {
                return (0, 0, 0.0);
            }
            if x >= src[n - 1] {
                return (n - 1, n - 1, 0.0);
            }
            let hi = src.partition_point(|p| *p < x).clamp(1, n - 1);
            (hi - 1, hi, (x - src[hi - 1]) / (src[hi] - src[hi - 1]))
        })
        .collect()
}

/// Positions of the samples of velocity component `c` (`edges` along `c`,
/// centres elsewhere) or of the cells (`c = None`) along `axis`.
fn positions(m: &Mesh, c: Option<usize>, axis: usize) -> Vec<f64> {
    if c == Some(axis) {
        m.edges[axis].clone()
    } else {
        m.centre[axis].clone()
    }
}

/// Interpolates one field from `from` (dims `fd`, sampled at `fp`) onto the
/// grid with dims `td` sampled at `tp`.
fn interpolate(
    from: &[f64],
    fd: [usize; 3],
    fp: [Vec<f64>; 3],
    td: [usize; 3],
    tp: [Vec<f64>; 3],
) -> Vec<f64> {
    let w: [Vec<(usize, usize, f64)>; 3] = std::array::from_fn(|a| axis_weights(&fp[a], &tp[a]));
    let at = |i: usize, j: usize, k: usize| from[(k * fd[1] + j) * fd[0] + i];
    let mut out = Vec::with_capacity(td[0] * td[1] * td[2]);
    for k in 0..td[2] {
        let (k0, k1, wk) = w[2][k];
        for j in 0..td[1] {
            let (j0, j1, wj) = w[1][j];
            for &(i0, i1, wi) in &w[0] {
                let lerp_i = |j: usize, k: usize| at(i0, j, k) * (1.0 - wi) + at(i1, j, k) * wi;
                let lerp_j = |k: usize| lerp_i(j0, k) * (1.0 - wj) + lerp_i(j1, k) * wj;
                out.push(lerp_j(k0) * (1.0 - wk) + lerp_j(k1) * wk);
            }
        }
    }
    out
}

impl FlowSolver {
    /// Starts from `coarse`'s solution: temperature, pressure and velocity are
    /// interpolated onto this mesh (walls and blocked faces stay at rest).
    /// The result is not divergence free; the first iteration projects it.
    pub fn initialise_from(&mut self, coarse: &FlowSolver) {
        let (mc, mf) = (&coarse.mesh, &self.mesh);
        let cell_pos =
            |m: &Mesh| -> [Vec<f64>; 3] { std::array::from_fn(|a| positions(m, None, a)) };
        self.t = interpolate(&coarse.t, mc.n, cell_pos(mc), mf.n, cell_pos(mf));
        let p = interpolate(&coarse.p, mc.n, cell_pos(mc), mf.n, cell_pos(mf));
        self.p = p;
        for c in 0..3 {
            let pos =
                |m: &Mesh| -> [Vec<f64>; 3] { std::array::from_fn(|a| positions(m, Some(c), a)) };
            let v = interpolate(&coarse.vel[c], mc.dims(c), pos(mc), mf.dims(c), pos(mf));
            let d = mf.dims(c);
            for k in 0..d[2] {
                for j in 0..d[1] {
                    for i in 0..d[0] {
                        let ix = [i, j, k];
                        let id = mf.idx(c, ix);
                        let wall = ix[c] == 0 || ix[c] == mf.n[c];
                        self.vel[c][id] = if wall || self.open.afrac[c][id] == 0.0 {
                            0.0
                        } else {
                            v[id]
                        };
                    }
                }
            }
        }
    }
}
