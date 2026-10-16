//! Fully developed turbulent channel (half height, symmetry at the
//! centreline) driven by a pressure gradient, used to test the turbulence
//! models and wall functions. Shared by the turbulence tests.
#![allow(dead_code)]

use novo_cfd::result::Grid3;
use novo_cfd::turb::{FaceKind, TurbConfig, TurbModel, TurbState};

pub struct Channel {
    pub state: TurbState,
    pub u: Vec<f64>,
    /// Wall-normal cell centres, m.
    pub y: Vec<f64>,
    pub re_tau: f64,
    pub nu: f64,
    pub n: usize,
}

/// Half channel of height 1 m, `u_tau` = 1 m/s so `nu = 1 / re_tau`.
/// `first_cell_plus` is the height of the first cell in wall units.
pub fn channel(model: TurbModel, re_tau: f64, n: usize, first_cell_plus: f64) -> Channel {
    let nu = 1.0 / re_tau;
    let h0 = first_cell_plus / re_tau;
    // geometric stretching to total height 1
    let ratio = solve_ratio(h0, n);
    let mut edges = vec![0.0];
    let mut h = h0;
    for _ in 0..n {
        edges.push(edges.last().unwrap() + h);
        h *= ratio;
    }
    let top = *edges.last().unwrap();
    let edges_mm: Vec<f64> = edges.iter().map(|e| e / top * 1e3).collect();
    let grid = Grid3 {
        nx: 1,
        ny: n,
        nz: 1,
        x_edges_mm: vec![0.0, 1.0],
        y_edges_mm: edges_mm,
        z_edges_mm: vec![0.0, 1.0],
    };
    let mut cfg = TurbConfig::air(model);
    cfg.nu = nu;
    cfg.faces = [FaceKind::Symmetry; 6];
    cfg.faces[2] = FaceKind::Wall; // y min is the wall
    cfg.intensity = 0.05;
    cfg.length_scale = 0.05;
    let state = TurbState::new(&grid, None, cfg, 15.0);
    let y: Vec<f64> = (0..n)
        .map(|j| 0.5 * (edges[j] + edges[j + 1]) / top)
        .collect();
    Channel {
        state,
        u: vec![5.0; n],
        y,
        re_tau,
        nu,
        n,
    }
}

fn solve_ratio(h0: f64, n: usize) -> f64 {
    // h0 (r^n - 1)/(r - 1) = 1
    let f = |r: f64| h0 * (r.powi(n as i32) - 1.0) / (r - 1.0) - 1.0;
    let (mut lo, mut hi) = (1.0 + 1e-9, 3.0);
    if f(lo) > 0.0 {
        return 1.0;
    }
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    0.5 * (lo + hi)
}

impl Channel {
    /// Momentum balance `d/dy((nu + nu_t) dU/dy) = -1`, wall shear from the
    /// wall function, zero shear at the centreline. Tridiagonal solve.
    pub fn solve_momentum(&mut self) {
        let m = &self.state.mesh;
        let n = self.n;
        let dy: Vec<f64> = (0..n).map(|j| m.dx[1][j]).collect();
        let yc: Vec<f64> = (0..n).map(|j| m.centre[1][j]).collect();
        let (mut a, mut b, mut c, mut rhs) =
            (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for j in 0..n {
            rhs[j] = dy[j]; // G = 1, per unit area
            if j == 0 {
                let walls = self.state.walls();
                let nu_w = walls
                    .iter()
                    .find(|w| w.cell == 0)
                    .map_or(self.nu, |w| w.nu_wall);
                b[0] += nu_w / (0.5 * dy[0]) * 1.0;
            }
            if j + 1 < n {
                let g = 2.0 * (self.nu + self.state.nu_t[j]) * (self.nu + self.state.nu_t[j + 1])
                    / (2.0 * self.nu + self.state.nu_t[j] + self.state.nu_t[j + 1]);
                let d = g / (yc[j + 1] - yc[j]);
                b[j] += d;
                c[j] = -d;
                b[j + 1] += d;
                a[j + 1] = -d;
            }
        }
        // thomas
        for j in 1..n {
            let w = a[j] / b[j - 1];
            b[j] -= w * c[j - 1];
            rhs[j] -= w * rhs[j - 1];
        }
        let mut u = vec![0.0; n];
        u[n - 1] = rhs[n - 1] / b[n - 1];
        for j in (0..n - 1).rev() {
            u[j] = (rhs[j] - c[j] * u[j + 1]) / b[j];
        }
        // under-relax: the pseudo-time coupling of nu_t and U is stiff
        for (a, b) in self.u.iter_mut().zip(u) {
            *a += 0.5 * (b - *a);
        }
    }

    pub fn faces(&self) -> [Vec<f64>; 3] {
        let m = &self.state.mesh;
        let mut vx = vec![0.0; m.vel_len(0)];
        for j in 0..self.n {
            vx[m.vel_idx(0, [0, j, 0])] = self.u[j];
            vx[m.vel_idx(0, [1, j, 0])] = self.u[j];
        }
        [vx, vec![0.0; m.vel_len(1)], vec![0.0; m.vel_len(2)]]
    }

    /// Iterate to a steady state; returns the iterations used.
    pub fn converge(&mut self, max_iter: usize) -> usize {
        for it in 0..max_iter {
            let faces = self.faces();
            let step = self.state.advance(&faces, None, 1.0, 4);
            self.solve_momentum();
            if it > 50 && step.k_change < 1e-9 && step.w_change < 1e-9 {
                return it;
            }
        }
        max_iter
    }
}

/// Reichardt's law of the wall.
pub fn reichardt(y_plus: f64) -> f64 {
    (1.0 + 0.41 * y_plus).ln() / 0.41
        + 7.8 * (1.0 - (-y_plus / 11.0).exp() - y_plus / 11.0 * (-y_plus / 3.0).exp())
}
