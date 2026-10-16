//! Pressure Poisson equation: conjugate gradients preconditioned by one
//! aggregation multigrid V-cycle (symmetric Gauss-Seidel smoothing).
//!
//! The operator is the symmetric finite-volume Laplacian with zero normal
//! gradient on every wall: `(A p)_c = sum_faces (area / distance) (p_c - p_nb)`.
//! It is singular (constants); right-hand sides must sum to zero and the
//! solution is returned with zero mean.

use super::mesh::Mesh;
use super::par::for_planes;

/// Face coupling coefficients and the diagonal of one level.
struct Level {
    n: [usize; 3],
    /// `cf[a][cell]`: coupling between the cell and its lower neighbour along
    /// axis `a` (0 on the domain boundary).
    cf: [Vec<f64>; 3],
    /// Operator diagonal: the face couplings plus any open-boundary term.
    diag: Vec<f64>,
    /// Diagonal of the face couplings alone.
    base: Vec<f64>,
}

impl Level {
    fn idx(&self, i: [usize; 3]) -> usize {
        (i[2] * self.n[1] + i[1]) * self.n[0] + i[0]
    }

    fn cells(&self) -> usize {
        self.n[0] * self.n[1] * self.n[2]
    }

    fn finish(n: [usize; 3], cf: [Vec<f64>; 3]) -> Level {
        let mut l = Level {
            n,
            cf,
            diag: vec![0.0; n[0] * n[1] * n[2]],
            base: Vec::new(),
        };
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let c = l.idx([i, j, k]);
                    let mut d = l.cf[0][c] + l.cf[1][c] + l.cf[2][c];
                    if i + 1 < n[0] {
                        d += l.cf[0][l.idx([i + 1, j, k])];
                    }
                    if j + 1 < n[1] {
                        d += l.cf[1][l.idx([i, j + 1, k])];
                    }
                    if k + 1 < n[2] {
                        d += l.cf[2][l.idx([i, j, k + 1])];
                    }
                    l.diag[c] = d;
                }
            }
        }
        l.base = l.diag.clone();
        l
    }

    /// Sum of neighbour values weighted by couplings.
    fn off(&self, x: &[f64], i: [usize; 3]) -> f64 {
        let c = self.idx(i);
        let mut s = 0.0;
        for a in 0..3 {
            if i[a] > 0 {
                let mut m = i;
                m[a] -= 1;
                s += self.cf[a][c] * x[self.idx(m)];
            }
            if i[a] + 1 < self.n[a] {
                let mut m = i;
                m[a] += 1;
                s += self.cf[a][self.idx(m)] * x[self.idx(m)];
            }
        }
        s
    }

    fn apply(&self, x: &[f64], y: &mut [f64]) {
        let (nx, ny) = (self.n[0], self.n[1]);
        for_planes(y, nx * ny, |k, plane| {
            for j in 0..ny {
                for i in 0..nx {
                    let c = self.idx([i, j, k]);
                    plane[j * nx + i] = self.diag[c] * x[c] - self.off(x, [i, j, k]);
                }
            }
        });
    }

    fn sweep(&self, x: &mut [f64], b: &[f64], forward: bool) {
        let (nx, ny, nz) = (self.n[0], self.n[1], self.n[2]);
        let order = |n: usize, f: bool, v: usize| if f { v } else { n - 1 - v };
        for kk in 0..nz {
            let k = order(nz, forward, kk);
            for jj in 0..ny {
                let j = order(ny, forward, jj);
                for ii in 0..nx {
                    let i = order(nx, forward, ii);
                    let c = self.idx([i, j, k]);
                    if self.diag[c] > 0.0 {
                        x[c] = (b[c] + self.off(x, [i, j, k])) / self.diag[c];
                    }
                }
            }
        }
    }

    /// Aggregates pairs of cells along every even axis.
    fn coarsen(&self) -> Level {
        let f: [usize; 3] = std::array::from_fn(|a| {
            if self.n[a].is_multiple_of(2) && self.n[a] >= 2 {
                2
            } else {
                1
            }
        });
        let cn: [usize; 3] = std::array::from_fn(|a| self.n[a] / f[a]);
        let len = cn[0] * cn[1] * cn[2];
        let mut cf = [vec![0.0; len], vec![0.0; len], vec![0.0; len]];
        for a in 0..3 {
            for k in 0..cn[2] {
                for j in 0..cn[1] {
                    for i in 0..cn[0] {
                        let ci = [i, j, k];
                        if ci[a] == 0 {
                            continue;
                        }
                        let mut s = 0.0;
                        // fine cells of the aggregate on the upper side of the interface
                        let mut lo = [ci[0] * f[0], ci[1] * f[1], ci[2] * f[2]];
                        lo[a] = ci[a] * f[a];
                        for dk in 0..(if a == 2 { 1 } else { f[2] }) {
                            for dj in 0..(if a == 1 { 1 } else { f[1] }) {
                                for di in 0..(if a == 0 { 1 } else { f[0] }) {
                                    let fi = [lo[0] + di, lo[1] + dj, lo[2] + dk];
                                    s += self.cf[a][self.idx(fi)];
                                }
                            }
                        }
                        cf[a][(k * cn[1] + j) * cn[0] + i] = s;
                    }
                }
            }
        }
        Level::finish(cn, cf)
    }

    fn restrict(&self, coarse: &Level, fine: &[f64], out: &mut [f64]) {
        out.iter_mut().for_each(|v| *v = 0.0);
        let f: [usize; 3] = std::array::from_fn(|a| self.n[a] / coarse.n[a]);
        for k in 0..self.n[2] {
            for j in 0..self.n[1] {
                for i in 0..self.n[0] {
                    out[coarse.idx([i / f[0], j / f[1], k / f[2]])] += fine[self.idx([i, j, k])];
                }
            }
        }
    }

    fn prolong_add(&self, coarse: &Level, cx: &[f64], fine: &mut [f64]) {
        let f: [usize; 3] = std::array::from_fn(|a| self.n[a] / coarse.n[a]);
        for k in 0..self.n[2] {
            for j in 0..self.n[1] {
                for i in 0..self.n[0] {
                    fine[self.idx([i, j, k])] += cx[coarse.idx([i / f[0], j / f[1], k / f[2]])];
                }
            }
        }
    }
}

/// The Poisson operator of one mesh with its multigrid hierarchy.
pub struct PressureSolver {
    levels: Vec<Level>,
    /// No open boundary: the operator is singular (constants).
    singular: bool,
    /// Iterations and relative residual of the last solve.
    pub last_iterations: usize,
    pub last_residual: f64,
}

impl PressureSolver {
    pub fn new(mesh: &Mesh) -> PressureSolver {
        let n = mesh.n;
        let len = mesh.cells();
        let mut cf = [vec![0.0; len], vec![0.0; len], vec![0.0; len]];
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let ci = [i, j, k];
                    for a in 0..3 {
                        if ci[a] == 0 {
                            continue;
                        }
                        let d = mesh.centre[a][ci[a]] - mesh.centre[a][ci[a] - 1];
                        cf[a][mesh.cell(ci)] = mesh.area(a, ci) / d;
                    }
                }
            }
        }
        let mut levels = vec![Level::finish(n, cf)];
        while levels.last().unwrap().cells() > 64 {
            let next = levels.last().unwrap().coarsen();
            if next.cells() == levels.last().unwrap().cells() {
                break;
            }
            levels.push(next);
        }
        PressureSolver {
            levels,
            singular: true,
            last_iterations: 0,
            last_residual: 0.0,
        }
    }

    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Adds a conductance `g[cell]` to the diagonal: the cell is tied to a
    /// fixed pressure (zero) through an open boundary face. With any
    /// positive entry the operator is no longer singular and the mean is not
    /// removed. All zeros restores the closed box.
    pub fn set_open_conductance(&mut self, g: &[f64]) {
        let mut cur = g.to_vec();
        self.singular = g.iter().all(|v| *v == 0.0);
        for l in 0..self.levels.len() {
            let next = (l + 1 < self.levels.len()).then(|| {
                let mut out = vec![0.0; self.levels[l + 1].cells()];
                self.levels[l].restrict(&self.levels[l + 1], &cur, &mut out);
                out
            });
            let lev = &mut self.levels[l];
            for ((d, b), g) in lev.diag.iter_mut().zip(&lev.base).zip(&cur) {
                *d = b + g;
            }
            match next {
                Some(n) => cur = n,
                None => break,
            }
        }
    }

    /// `y = A x` on the finest level.
    pub fn apply(&self, x: &[f64], y: &mut [f64]) {
        self.levels[0].apply(x, y);
    }

    fn vcycle(&self, l: usize, b: &[f64], x: &mut [f64]) {
        let lev = &self.levels[l];
        x.iter_mut().for_each(|v| *v = 0.0);
        if l + 1 == self.levels.len() {
            for _ in 0..30 {
                lev.sweep(x, b, true);
                lev.sweep(x, b, false);
            }
            return;
        }
        lev.sweep(x, b, true);
        let mut r = vec![0.0; b.len()];
        lev.apply(x, &mut r);
        for (ri, bi) in r.iter_mut().zip(b) {
            *ri = bi - *ri;
        }
        let coarse = &self.levels[l + 1];
        let mut cb = vec![0.0; coarse.cells()];
        lev.restrict(coarse, &r, &mut cb);
        let mut cx = vec![0.0; coarse.cells()];
        self.vcycle(l + 1, &cb, &mut cx);
        lev.prolong_add(coarse, &cx, x);
        lev.sweep(x, b, false);
    }

    /// Solves `A p = b` (the mean of `b` is removed). `p` is the starting
    /// guess and the result. Returns the iteration count.
    pub fn solve(&mut self, b: &[f64], p: &mut [f64], rel_tol: f64, max_iter: usize) -> usize {
        let n = b.len();
        let mean_b = if self.singular {
            b.iter().sum::<f64>() / n as f64
        } else {
            0.0
        };
        let b: Vec<f64> = b.iter().map(|v| v - mean_b).collect();
        let bnorm = dot(&b, &b).sqrt().max(1e-300);
        let mut r = vec![0.0; n];
        self.levels[0].apply(p, &mut r);
        for (ri, bi) in r.iter_mut().zip(&b) {
            *ri = bi - *ri;
        }
        let mut z = vec![0.0; n];
        let mut q = vec![0.0; n];
        let mut it = 0;
        let mut rel = dot(&r, &r).sqrt() / bnorm;
        if rel > rel_tol {
            self.vcycle(0, &r, &mut z);
            let mut d = z.clone();
            let mut rz = dot(&r, &z);
            while it < max_iter {
                self.levels[0].apply(&d, &mut q);
                let dq = dot(&d, &q);
                if dq <= 0.0 {
                    break;
                }
                let alpha = rz / dq;
                for c in 0..n {
                    p[c] += alpha * d[c];
                    r[c] -= alpha * q[c];
                }
                it += 1;
                rel = dot(&r, &r).sqrt() / bnorm;
                if rel <= rel_tol {
                    break;
                }
                self.vcycle(0, &r, &mut z);
                let rz_new = dot(&r, &z);
                let beta = rz_new / rz;
                rz = rz_new;
                for c in 0..n {
                    d[c] = z[c] + beta * d[c];
                }
            }
        }
        if self.singular {
            let mean_p = p.iter().sum::<f64>() / n as f64;
            p.iter_mut().for_each(|v| *v -= mean_p);
        }
        self.last_iterations = it;
        self.last_residual = rel;
        it
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
