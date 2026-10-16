//! Pressure Poisson equation: conjugate gradients preconditioned by one
//! aggregation multigrid V-cycle (symmetric Gauss-Seidel smoothing).
//!
//! The operator is the symmetric finite-volume Laplacian with zero normal
//! gradient on every wall: `(A p)_c = sum_faces (area / distance) (p_c - p_nb)`.
//! It is singular (constants); right-hand sides must sum to zero and the
//! solution is returned with zero mean.

use super::mesh::Mesh;
use super::par::{MIN_VALUES, for_planes};
use rayon::prelude::*;

/// Ordering of the Gauss-Seidel smoother of the multigrid preconditioner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Smoother {
    /// Cell by cell in index order: serial, the reference.
    #[default]
    Lexicographic,
    /// Red cells, then black cells (chequerboard). The cells of one colour
    /// do not touch each other, so each half sweep runs on all threads.
    /// Forward is red then black, backward is black then red, which keeps
    /// the V-cycle symmetric for the conjugate gradients.
    RedBlack,
}

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

    fn sweep(&self, x: &mut [f64], b: &[f64], forward: bool, smoother: Smoother) {
        match smoother {
            Smoother::Lexicographic => self.sweep_lex(x, b, forward),
            Smoother::RedBlack => {
                let order = if forward { [0, 1] } else { [1, 0] };
                for colour in order {
                    self.half_sweep(x, b, colour);
                }
            }
        }
    }

    /// Updates the cells with `(i + j + k) % 2 == colour` from the other
    /// colour (untouched meanwhile): computed per plane into a scratch
    /// copy, then written back, so no cell is read and written at once.
    fn half_sweep(&self, x: &mut [f64], b: &[f64], colour: usize) {
        let (nx, ny) = (self.n[0], self.n[1]);
        let mut next = vec![0.0; x.len()];
        {
            let xr: &[f64] = x;
            for_planes(&mut next, nx * ny, |k, plane| {
                for j in 0..ny {
                    for i in 0..nx {
                        if (i + j + k) % 2 != colour {
                            continue;
                        }
                        let c = self.idx([i, j, k]);
                        plane[j * nx + i] = if self.diag[c] > 0.0 {
                            (b[c] + self.off(xr, [i, j, k])) / self.diag[c]
                        } else {
                            xr[c]
                        };
                    }
                }
            });
        }
        let copy = |k: usize, plane: &mut [f64]| {
            for j in 0..ny {
                for i in 0..nx {
                    if (i + j + k) % 2 == colour {
                        plane[j * nx + i] = next[(k * ny + j) * nx + i];
                    }
                }
            }
        };
        if x.len() < MIN_VALUES {
            x.chunks_mut(nx * ny)
                .enumerate()
                .for_each(|(k, p)| copy(k, p));
        } else {
            x.par_chunks_mut(nx * ny)
                .enumerate()
                .for_each(|(k, p)| copy(k, p));
        }
    }

    fn sweep_lex(&self, x: &mut [f64], b: &[f64], forward: bool) {
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

    /// Sums the fine cells of every aggregate (gathered per coarse cell in
    /// the same order as a scatter would add them: bit-identical).
    fn restrict(&self, coarse: &Level, fine: &[f64], out: &mut [f64]) {
        let f: [usize; 3] = std::array::from_fn(|a| self.n[a] / coarse.n[a]);
        let cn = coarse.n;
        for_planes(out, cn[0] * cn[1], |ck, plane| {
            for cj in 0..cn[1] {
                for ci in 0..cn[0] {
                    let mut sum = 0.0;
                    for dk in 0..f[2] {
                        for dj in 0..f[1] {
                            for di in 0..f[0] {
                                sum += fine
                                    [self.idx([ci * f[0] + di, cj * f[1] + dj, ck * f[2] + dk])];
                            }
                        }
                    }
                    plane[cj * cn[0] + ci] = sum;
                }
            }
        });
    }

    fn prolong_add(&self, coarse: &Level, cx: &[f64], fine: &mut [f64]) {
        let f: [usize; 3] = std::array::from_fn(|a| self.n[a] / coarse.n[a]);
        let (nx, ny) = (self.n[0], self.n[1]);
        for_planes(fine, nx * ny, |k, plane| {
            for j in 0..ny {
                for i in 0..nx {
                    plane[j * nx + i] += cx[coarse.idx([i / f[0], j / f[1], k / f[2]])];
                }
            }
        });
    }
}

/// The Poisson operator of one mesh with its multigrid hierarchy.
pub struct PressureSolver {
    levels: Vec<Level>,
    /// Cells with at least one open face (solid cells are left out).
    active: Vec<bool>,
    /// No open boundary: the operator is singular (constants).
    singular: bool,
    smoother: Smoother,
    /// Iterations and relative residual of the last solve.
    pub last_iterations: usize,
    pub last_residual: f64,
}

impl PressureSolver {
    pub fn new(mesh: &Mesh) -> PressureSolver {
        PressureSolver::with_openness(mesh, None)
    }

    /// `open[a][mesh.idx(a, ix)]` is the open share of the face of
    /// component `a` (0 = blocked by a solid). The coupling across a face is
    /// scaled by it; a cell with no open face drops out of the system.
    pub fn with_openness(mesh: &Mesh, open: Option<&[Vec<f64>; 3]>) -> PressureSolver {
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
                        let share = open.map_or(1.0, |o| o[a][mesh.idx(a, ci)]);
                        cf[a][mesh.cell(ci)] = share * mesh.area(a, ci) / d;
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
        let active = levels[0].diag.iter().map(|d| *d > 0.0).collect();
        PressureSolver {
            levels,
            active,
            singular: true,
            smoother: Smoother::default(),
            last_iterations: 0,
            last_residual: 0.0,
        }
    }

    /// Chooses the smoother ordering (does not change the operator).
    pub fn set_smoother(&mut self, smoother: Smoother) {
        self.smoother = smoother;
    }

    pub fn smoother(&self) -> Smoother {
        self.smoother
    }

    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Replaces the face couplings (open share times any drag factor) and
    /// rebuilds the hierarchy. Open-boundary conductances are cleared: set
    /// them again afterwards.
    pub fn set_coupling(&mut self, mesh: &Mesh, coupling: &[Vec<f64>; 3]) {
        let fresh = PressureSolver::with_openness(mesh, Some(coupling));
        self.levels = fresh.levels;
        self.active = fresh.active;
        self.singular = true;
    }

    /// Adds a conductance `g[cell]` to the diagonal: the cell is tied to a
    /// fixed pressure (zero) through an open boundary face. With any
    /// positive entry the operator is no longer singular and the mean is not
    /// removed. All zeros restores the closed box.
    pub fn set_open_conductance(&mut self, g: &[f64]) {
        let mut cur = g.to_vec();
        self.singular = g.iter().all(|v| *v == 0.0);
        for ((a, d), g) in self.active.iter_mut().zip(&self.levels[0].base).zip(g) {
            *a = *d + *g > 0.0;
        }
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
                lev.sweep(x, b, true, self.smoother);
                lev.sweep(x, b, false, self.smoother);
            }
            return;
        }
        lev.sweep(x, b, true, self.smoother);
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
        lev.sweep(x, b, false, self.smoother);
    }

    /// Solves `A p = b` (the mean of `b` is removed). `p` is the starting
    /// guess and the result. Returns the iteration count.
    pub fn solve(&mut self, b: &[f64], p: &mut [f64], rel_tol: f64, max_iter: usize) -> usize {
        let n = b.len();
        let live = self.active.iter().filter(|a| **a).count().max(1) as f64;
        let mean_b = if self.singular {
            b.iter()
                .zip(&self.active)
                .filter(|(_, a)| **a)
                .map(|(v, _)| v)
                .sum::<f64>()
                / live
        } else {
            0.0
        };
        let b: Vec<f64> = b
            .iter()
            .zip(&self.active)
            .map(|(v, a)| if *a { v - mean_b } else { 0.0 })
            .collect();
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
                axpy2(alpha, &d, &q, p, &mut r);
                it += 1;
                rel = dot(&r, &r).sqrt() / bnorm;
                if rel <= rel_tol {
                    break;
                }
                self.vcycle(0, &r, &mut z);
                let rz_new = dot(&r, &z);
                let beta = rz_new / rz;
                rz = rz_new;
                update_direction(beta, &z, &mut d);
            }
        }
        if self.singular {
            let mean_p = p
                .iter()
                .zip(&self.active)
                .filter(|(_, a)| **a)
                .map(|(v, _)| v)
                .sum::<f64>()
                / live;
            for (v, a) in p.iter_mut().zip(&self.active) {
                *v = if *a { *v - mean_p } else { 0.0 };
            }
        }
        self.last_iterations = it;
        self.last_residual = rel;
        it
    }
}

/// `p += alpha d`, `r -= alpha q` (per element, so thread-count independent).
fn axpy2(alpha: f64, d: &[f64], q: &[f64], p: &mut [f64], r: &mut [f64]) {
    if p.len() < MIN_VALUES {
        for c in 0..p.len() {
            p[c] += alpha * d[c];
            r[c] -= alpha * q[c];
        }
        return;
    }
    p.par_chunks_mut(4096)
        .zip(r.par_chunks_mut(4096))
        .zip(d.par_chunks(4096).zip(q.par_chunks(4096)))
        .for_each(|((p, r), (d, q))| {
            for c in 0..p.len() {
                p[c] += alpha * d[c];
                r[c] -= alpha * q[c];
            }
        });
}

/// `d = z + beta d`.
fn update_direction(beta: f64, z: &[f64], d: &mut [f64]) {
    if d.len() < MIN_VALUES {
        for c in 0..d.len() {
            d[c] = z[c] + beta * d[c];
        }
        return;
    }
    d.par_chunks_mut(4096)
        .zip(z.par_chunks(4096))
        .for_each(|(d, z)| {
            for c in 0..d.len() {
                d[c] = z[c] + beta * d[c];
            }
        });
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
