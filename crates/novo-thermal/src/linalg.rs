//! Small sparse linear algebra: a compressed row matrix, conjugate gradients
//! for the symmetric pressure system and BiCGSTAB for the temperature system.
//! No dependencies; sizes are a few thousand to tens of thousands of unknowns.

/// Collects entries row by row; duplicates are summed.
pub struct Builder {
    rows: Vec<Vec<(usize, f64)>>,
}

impl Builder {
    pub fn new(n: usize) -> Self {
        Builder {
            rows: vec![Vec::new(); n],
        }
    }

    pub fn add(&mut self, row: usize, col: usize, value: f64) {
        self.rows[row].push((col, value));
    }

    pub fn finish(self) -> Csr {
        let n = self.rows.len();
        let mut row_ptr = Vec::with_capacity(n + 1);
        let mut col = Vec::new();
        let mut val = Vec::new();
        row_ptr.push(0);
        for mut row in self.rows {
            row.sort_by_key(|e| e.0);
            let mut last: Option<usize> = None;
            for (c, v) in row {
                if last == Some(c) {
                    *val.last_mut().expect("entry exists") += v;
                } else {
                    col.push(c);
                    val.push(v);
                    last = Some(c);
                }
            }
            row_ptr.push(col.len());
        }
        Csr {
            n,
            row_ptr,
            col,
            val,
        }
    }
}

pub struct Csr {
    pub n: usize,
    row_ptr: Vec<usize>,
    col: Vec<usize>,
    val: Vec<f64>,
}

impl Csr {
    pub fn mul(&self, x: &[f64], out: &mut [f64]) {
        for i in 0..self.n {
            let mut s = 0.0;
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                s += self.val[k] * x[self.col[k]];
            }
            out[i] = s;
        }
    }

    pub fn diagonal(&self) -> Vec<f64> {
        let mut d = vec![0.0; self.n];
        for (i, di) in d.iter_mut().enumerate() {
            for k in self.row_ptr[i]..self.row_ptr[i + 1] {
                if self.col[k] == i {
                    *di += self.val[k];
                }
            }
        }
        d
    }
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm(a: &[f64]) -> f64 {
    dot(a, a).sqrt()
}

/// Outcome of an iterative solve.
#[derive(Debug, Clone, Copy)]
pub struct SolveInfo {
    pub iterations: usize,
    /// Residual norm over the right-hand side norm at the end.
    pub residual: f64,
}

fn inverse_diagonal(a: &Csr) -> Vec<f64> {
    a.diagonal()
        .into_iter()
        .map(|d| if d.abs() > 1e-300 { 1.0 / d } else { 1.0 })
        .collect()
}

/// Jacobi preconditioned conjugate gradients for a symmetric positive
/// (semi)definite system. `x` holds the start and receives the answer.
pub fn cg(a: &Csr, b: &[f64], x: &mut [f64], tol: f64, max_iter: usize) -> SolveInfo {
    let n = a.n;
    let minv = inverse_diagonal(a);
    let mut r = vec![0.0; n];
    let mut ax = vec![0.0; n];
    a.mul(x, &mut ax);
    for i in 0..n {
        r[i] = b[i] - ax[i];
    }
    let bnorm = norm(b).max(1e-300);
    let mut z: Vec<f64> = r.iter().zip(&minv).map(|(r, m)| r * m).collect();
    let mut p = z.clone();
    let mut rz = dot(&r, &z);
    let mut ap = vec![0.0; n];
    let mut iterations = 0;
    while iterations < max_iter && norm(&r) / bnorm > tol {
        a.mul(&p, &mut ap);
        let pap = dot(&p, &ap);
        if pap.abs() < 1e-300 {
            break;
        }
        let alpha = rz / pap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        for i in 0..n {
            z[i] = r[i] * minv[i];
        }
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz;
        rz = rz_new;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
        iterations += 1;
    }
    SolveInfo {
        iterations,
        residual: norm(&r) / bnorm,
    }
}

/// Jacobi preconditioned BiCGSTAB for the general (non symmetric) system.
pub fn bicgstab(a: &Csr, b: &[f64], x: &mut [f64], tol: f64, max_iter: usize) -> SolveInfo {
    let n = a.n;
    let minv = inverse_diagonal(a);
    let bnorm = norm(b).max(1e-300);
    let mut r = vec![0.0; n];
    let mut tmp = vec![0.0; n];
    a.mul(x, &mut tmp);
    for i in 0..n {
        r[i] = b[i] - tmp[i];
    }
    let r0 = r.clone();
    let (mut rho, mut alpha, mut omega) = (1.0, 1.0, 1.0);
    let mut v = vec![0.0; n];
    let mut p = vec![0.0; n];
    let mut phat = vec![0.0; n];
    let mut shat = vec![0.0; n];
    let mut s = vec![0.0; n];
    let mut t = vec![0.0; n];
    let mut iterations = 0;
    while iterations < max_iter && norm(&r) / bnorm > tol {
        let rho_new = dot(&r0, &r);
        if rho_new.abs() < 1e-300 {
            break;
        }
        let beta = (rho_new / rho) * (alpha / omega);
        rho = rho_new;
        for i in 0..n {
            p[i] = r[i] + beta * (p[i] - omega * v[i]);
            phat[i] = p[i] * minv[i];
        }
        a.mul(&phat, &mut v);
        let r0v = dot(&r0, &v);
        if r0v.abs() < 1e-300 {
            break;
        }
        alpha = rho / r0v;
        for i in 0..n {
            s[i] = r[i] - alpha * v[i];
        }
        if norm(&s) / bnorm <= tol {
            for i in 0..n {
                x[i] += alpha * phat[i];
            }
            r.copy_from_slice(&s);
            iterations += 1;
            break;
        }
        for i in 0..n {
            shat[i] = s[i] * minv[i];
        }
        a.mul(&shat, &mut t);
        let tt = dot(&t, &t);
        if tt.abs() < 1e-300 {
            break;
        }
        omega = dot(&t, &s) / tt;
        for i in 0..n {
            x[i] += alpha * phat[i] + omega * shat[i];
            r[i] = s[i] - omega * t[i];
        }
        iterations += 1;
        if omega.abs() < 1e-300 {
            break;
        }
    }
    SolveInfo {
        iterations,
        residual: norm(&r) / bnorm,
    }
}
