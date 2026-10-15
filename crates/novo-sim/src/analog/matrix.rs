//! Dense complex LU with partial pivoting. Panel circuits are small (a few
//! hundred unknowns at most), so dense is the simple and robust choice; the
//! factorisation is kept so a fixed-step transient solves each step with a
//! forward and a back substitution only.

use super::Complex;

/// A square matrix, row major.
#[derive(Debug, Clone)]
pub struct Matrix {
    n: usize,
    a: Vec<Complex>,
}

impl Matrix {
    pub fn new(n: usize) -> Self {
        Matrix {
            n,
            a: vec![Complex::ZERO; n * n],
        }
    }

    pub fn add(&mut self, r: usize, c: usize, v: Complex) {
        self.a[r * self.n + c] += v;
    }

    /// LU-factors the matrix. `None` when it is singular (a loop of voltage
    /// sources, or a part of the circuit nothing defines).
    pub fn factor(mut self) -> Option<Lu> {
        let n = self.n;
        let mut piv = (0..n).collect::<Vec<_>>();
        // Scale for a relative singularity test.
        let scale = self
            .a
            .iter()
            .map(|x| x.abs())
            .fold(0.0, f64::max)
            .max(1e-300);
        for k in 0..n {
            let (p, best) = (k..n)
                .map(|r| (r, self.a[r * n + k].abs()))
                .fold((k, -1.0), |m, x| if x.1 > m.1 { x } else { m });
            if best <= scale * 1e-14 {
                return None;
            }
            if p != k {
                for c in 0..n {
                    self.a.swap(k * n + c, p * n + c);
                }
                piv.swap(k, p);
            }
            let d = self.a[k * n + k];
            for r in k + 1..n {
                let f = self.a[r * n + k] / d;
                self.a[r * n + k] = f;
                if f == Complex::ZERO {
                    continue;
                }
                for c in k + 1..n {
                    let t = f * self.a[k * n + c];
                    self.a[r * n + c] = self.a[r * n + c] - t;
                }
            }
        }
        Some(Lu { n, lu: self.a, piv })
    }
}

#[derive(Debug, Clone)]
pub struct Lu {
    n: usize,
    lu: Vec<Complex>,
    piv: Vec<usize>,
}

impl Lu {
    pub fn solve(&self, b: &[Complex]) -> Vec<Complex> {
        let n = self.n;
        let mut x: Vec<Complex> = self.piv.iter().map(|&p| b[p]).collect();
        for r in 1..n {
            let mut s = x[r];
            for c in 0..r {
                s = s - self.lu[r * n + c] * x[c];
            }
            x[r] = s;
        }
        for r in (0..n).rev() {
            let mut s = x[r];
            for c in r + 1..n {
                s = s - self.lu[r * n + c] * x[c];
            }
            x[r] = s / self.lu[r * n + r];
        }
        x
    }
}
