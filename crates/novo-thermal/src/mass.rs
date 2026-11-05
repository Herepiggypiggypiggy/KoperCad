//! Thermal mass: a lumped network of heat capacities and conductances solved
//! exactly in time (design `handoff/heat-mass/`).
//!
//! The equation is `C dT/dt = -G T + g_amb Ta + P` for a handful to a few
//! hundred nodes (air, devices, plate, wall cells). `G` is symmetric positive
//! definite as soon as every node has a path to the ambient, so the matrix
//! `C^-1/2 G C^-1/2` has real positive eigenvalues; those are the inverse
//! **time constants** of the network. With them, a step of constant power and
//! ambient is advanced *exactly* (no time-stepping error), so a duty cycle or
//! a day/night curve is a list of such steps and its error comes only from
//! how coarsely the ambient is cut into steps.
//!
//! What is exact: the network arithmetic, checked against the lumped
//! exponential, the 2-node closed form, the first Fourier mode of a slab and
//! the sinusoidal-ambient amplitude and phase. What is a model: the node
//! values the cabinet builder ([`crate::heatup`]) feeds in.

use crate::Error;

/// A network of heat capacities (J/K) joined by conductances (W/K), some of
/// them to the ambient.
#[derive(Debug, Clone, Default)]
pub struct Network {
    cap: Vec<f64>,
    to_amb: Vec<f64>,
    links: Vec<(usize, usize, f64)>,
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a node of heat capacity `c` J/K and returns its index.
    pub fn add_node(&mut self, c: f64) -> usize {
        self.cap.push(c);
        self.to_amb.push(0.0);
        self.cap.len() - 1
    }

    pub fn len(&self) -> usize {
        self.cap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cap.is_empty()
    }

    pub fn capacity(&self, node: usize) -> f64 {
        self.cap[node]
    }

    /// Total heat capacity of all nodes, J/K.
    pub fn total_capacity(&self) -> f64 {
        self.cap.iter().sum()
    }

    /// Joins two nodes with conductance `g` W/K (adds up with earlier links).
    pub fn link(&mut self, a: usize, b: usize, g: f64) {
        if g > 0.0 && a != b {
            self.links.push((a, b, g));
        }
    }

    /// Joins a node to the ambient with conductance `g` W/K.
    pub fn to_ambient(&mut self, node: usize, g: f64) {
        if g > 0.0 {
            self.to_amb[node] += g;
        }
    }

    /// Joins `node` to `other` or, when `None`, to the ambient.
    pub fn link_or_ambient(&mut self, node: usize, other: Option<usize>, g: f64) {
        match other {
            Some(o) => self.link(node, o, g),
            None => self.to_ambient(node, g),
        }
    }

    /// Sum of the conductances to the ambient, W/K.
    pub fn ambient_conductance(&self) -> f64 {
        self.to_amb.iter().sum()
    }

    /// Diagonalises the network. Fails when a node has no capacity or no
    /// path to the ambient (the steady state would not exist).
    pub fn solve(&self) -> Result<Solved, Error> {
        let n = self.cap.len();
        if n == 0 {
            return Err(Error::Invalid("the thermal network has no nodes".into()));
        }
        if let Some(i) = self.cap.iter().position(|&c| !(c > 0.0 && c.is_finite())) {
            return Err(Error::Invalid(format!(
                "node {i} of the thermal network has no heat capacity"
            )));
        }
        let mut g = vec![0.0; n * n];
        for (i, &ga) in self.to_amb.iter().enumerate() {
            g[i * n + i] += ga;
        }
        for &(a, b, c) in &self.links {
            g[a * n + a] += c;
            g[b * n + b] += c;
            g[a * n + b] -= c;
            g[b * n + a] -= c;
        }
        let sqrt_c: Vec<f64> = self.cap.iter().map(|c| c.sqrt()).collect();
        let mut s = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                s[i * n + j] = g[i * n + j] / (sqrt_c[i] * sqrt_c[j]);
            }
        }
        let (rates, vecs) = jacobi_eigen(s, n);
        let smallest = rates.iter().cloned().fold(f64::INFINITY, f64::min);
        let largest = rates.iter().cloned().fold(0.0, f64::max);
        if smallest <= largest * 1e-13 {
            return Err(Error::Invalid(
                "part of the thermal network has no path to the ambient".into(),
            ));
        }
        Ok(Solved {
            n,
            sqrt_c,
            rates,
            vecs,
            to_amb: self.to_amb.clone(),
        })
    }
}

/// Cyclic Jacobi eigen decomposition of a symmetric `n` by `n` matrix.
/// Returns the eigenvalues and the eigenvectors (`vecs[k * n + i]` is
/// component `i` of vector `k`).
fn jacobi_eigen(mut a: Vec<f64>, n: usize) -> (Vec<f64>, Vec<f64>) {
    let mut v = vec![0.0; n * n];
    for i in 0..n {
        v[i * n + i] = 1.0;
    }
    for _sweep in 0..100 {
        let mut off = 0.0;
        let mut diag = 0.0;
        for i in 0..n {
            diag += a[i * n + i] * a[i * n + i];
            for j in (i + 1)..n {
                off += a[i * n + j] * a[i * n + j];
            }
        }
        if off <= 1e-28 * diag.max(1e-300) {
            break;
        }
        for p in 0..n {
            for q in (p + 1)..n {
                let apq = a[p * n + q];
                if apq.abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q * n + q] - a[p * n + p]) / (2.0 * apq);
                let t = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let t = if theta == 0.0 { 1.0 } else { t };
                let c = 1.0 / (t * t + 1.0).sqrt();
                let sn = t * c;
                for k in 0..n {
                    let akp = a[k * n + p];
                    let akq = a[k * n + q];
                    a[k * n + p] = c * akp - sn * akq;
                    a[k * n + q] = sn * akp + c * akq;
                }
                for k in 0..n {
                    let apk = a[p * n + k];
                    let aqk = a[q * n + k];
                    a[p * n + k] = c * apk - sn * aqk;
                    a[q * n + k] = sn * apk + c * aqk;
                }
                for k in 0..n {
                    let vkp = v[k * n + p];
                    let vkq = v[k * n + q];
                    v[k * n + p] = c * vkp - sn * vkq;
                    v[k * n + q] = sn * vkp + c * vkq;
                }
            }
        }
    }
    // Columns of `v` are the eigenvectors; store them as rows, ascending.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&x, &y| a[x * n + x].total_cmp(&a[y * n + y]));
    let rates: Vec<f64> = order.iter().map(|&k| a[k * n + k]).collect();
    let mut vecs = vec![0.0; n * n];
    for (row, &k) in order.iter().enumerate() {
        for i in 0..n {
            vecs[row * n + i] = v[i * n + k];
        }
    }
    (rates, vecs)
}

/// A diagonalised network: exact time evolution.
#[derive(Debug, Clone)]
pub struct Solved {
    n: usize,
    sqrt_c: Vec<f64>,
    /// Eigenvalues of `C^-1/2 G C^-1/2`, 1/s, ascending.
    rates: Vec<f64>,
    vecs: Vec<f64>,
    to_amb: Vec<f64>,
}

impl Solved {
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    /// Time constants, s, longest first.
    pub fn time_constants_s(&self) -> Vec<f64> {
        self.rates.iter().map(|r| 1.0 / r).collect()
    }

    /// The longest time constant, s.
    pub fn slowest_s(&self) -> f64 {
        1.0 / self.rates[0]
    }

    /// Steady temperatures for node powers `power` (W) and ambient `ta`.
    pub fn steady(&self, power: &[f64], ta: f64) -> Vec<f64> {
        let n = self.n;
        // b scaled: C^-1/2 (P + g_a Ta)
        let b: Vec<f64> = (0..n)
            .map(|i| (power[i] + self.to_amb[i] * ta) / self.sqrt_c[i])
            .collect();
        let mut out = vec![0.0; n];
        for k in 0..n {
            let proj: f64 = (0..n).map(|i| self.vecs[k * n + i] * b[i]).sum();
            let w = proj / self.rates[k];
            for (i, o) in out.iter_mut().enumerate() {
                *o += self.vecs[k * n + i] * w;
            }
        }
        for (o, c) in out.iter_mut().zip(&self.sqrt_c) {
            *o /= c;
        }
        out
    }

    /// Temperatures after `dt` seconds of constant `power` and ambient `ta`,
    /// starting from `t0`. Exact.
    pub fn advance(&self, t0: &[f64], power: &[f64], ta: f64, dt: f64) -> Vec<f64> {
        let n = self.n;
        let tss = self.steady(power, ta);
        let z: Vec<f64> = (0..n).map(|i| (t0[i] - tss[i]) * self.sqrt_c[i]).collect();
        let mut out = vec![0.0; n];
        for k in 0..n {
            let proj: f64 = (0..n).map(|i| self.vecs[k * n + i] * z[i]).sum();
            let w = proj * (-self.rates[k] * dt).exp();
            for (i, o) in out.iter_mut().enumerate() {
                *o += self.vecs[k * n + i] * w;
            }
        }
        for (i, o) in out.iter_mut().enumerate() {
            *o = tss[i] + *o / self.sqrt_c[i];
        }
        out
    }

    /// Runs `phases` from `start`; `power` is the full-load node powers and
    /// each phase scales it. Samples every `sample_s` and at each phase end.
    pub fn run(&self, start: &[f64], power: &[f64], phases: &[Phase], sample_s: f64) -> Trajectory {
        let mut t = 0.0;
        let mut cur = start.to_vec();
        let mut traj = Trajectory {
            times_s: vec![0.0],
            temps: vec![cur.clone()],
        };
        for ph in phases {
            let p: Vec<f64> = power.iter().map(|w| w * ph.power_scale).collect();
            let base = cur.clone();
            let mut done = 0.0;
            while done < ph.duration_s - 1e-9 {
                let step = sample_s.min(ph.duration_s - done);
                done += step;
                let state = self.advance(&base, &p, ph.ambient_c, done);
                traj.times_s.push(t + done);
                traj.temps.push(state);
            }
            cur = traj.temps.last().cloned().unwrap_or(cur);
            t += ph.duration_s;
        }
        traj
    }

    /// The state at the start of a cycle that repeats forever: running the
    /// phases once from it returns to it. Exact (solves the cycle map).
    pub fn periodic_start(&self, power: &[f64], cycle: &[Phase]) -> Vec<f64> {
        let n = self.n;
        let apply = |t0: &[f64]| -> Vec<f64> {
            let mut cur = t0.to_vec();
            for ph in cycle {
                let p: Vec<f64> = power.iter().map(|w| w * ph.power_scale).collect();
                cur = self.advance(&cur, &p, ph.ambient_c, ph.duration_s);
            }
            cur
        };
        let c = apply(&vec![0.0; n]);
        // (I - M) x = c, with M's columns found from unit vectors. The ambient
        // is inside `c`, M is the pure response (zero power, zero ambient).
        let mut m = vec![0.0; n * n];
        for j in 0..n {
            let mut e = vec![0.0; n];
            e[j] = 1.0;
            let mut cur = e;
            for ph in cycle {
                cur = self.advance(&cur, &vec![0.0; n], 0.0, ph.duration_s);
            }
            for i in 0..n {
                m[i * n + j] = cur[i];
            }
        }
        let mut a = vec![0.0; n * n];
        for i in 0..n {
            for j in 0..n {
                a[i * n + j] = if i == j { 1.0 } else { 0.0 } - m[i * n + j];
            }
        }
        solve_dense(a, c, n)
    }
}

/// Gaussian elimination with partial pivoting.
fn solve_dense(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Vec<f64> {
    for col in 0..n {
        let piv = (col..n)
            .max_by(|&x, &y| a[x * n + col].abs().total_cmp(&a[y * n + col].abs()))
            .unwrap_or(col);
        if piv != col {
            for j in 0..n {
                a.swap(col * n + j, piv * n + j);
            }
            b.swap(col, piv);
        }
        let d = a[col * n + col];
        if d.abs() < 1e-300 {
            continue;
        }
        for r in (col + 1)..n {
            let f = a[r * n + col] / d;
            if f != 0.0 {
                for j in col..n {
                    a[r * n + j] -= f * a[col * n + j];
                }
                b[r] -= f * b[col];
            }
        }
    }
    let mut x = vec![0.0; n];
    for i in (0..n).rev() {
        let mut s = b[i];
        for j in (i + 1)..n {
            s -= a[i * n + j] * x[j];
        }
        let d = a[i * n + i];
        x[i] = if d.abs() < 1e-300 { 0.0 } else { s / d };
    }
    x
}

/// A stretch of constant load and ambient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Phase {
    pub duration_s: f64,
    /// Multiplies every node power (0 = off, 1 = full load).
    pub power_scale: f64,
    pub ambient_c: f64,
}

impl Phase {
    /// `cycles` of `on_s` at `on_scale` then `off_s` at `off_scale`, at a
    /// constant ambient.
    pub fn duty_cycle(
        on_s: f64,
        on_scale: f64,
        off_s: f64,
        off_scale: f64,
        ambient_c: f64,
        cycles: usize,
    ) -> Vec<Phase> {
        let mut v = Vec::with_capacity(2 * cycles);
        for _ in 0..cycles {
            v.push(Phase {
                duration_s: on_s,
                power_scale: on_scale,
                ambient_c,
            });
            v.push(Phase {
                duration_s: off_s,
                power_scale: off_scale,
                ambient_c,
            });
        }
        v
    }

    /// A sinusoidal day: ambient `mean_c + swing_c cos(2 pi (t - peak_h) / 24 h)`
    /// cut into steps of `step_s`, each at the ambient of its midpoint, at
    /// constant `power_scale`, for `days` days starting at midnight.
    pub fn daily_ambient(
        mean_c: f64,
        swing_c: f64,
        peak_hour: f64,
        step_s: f64,
        days: f64,
        power_scale: f64,
    ) -> Vec<Phase> {
        let total = days * 86_400.0;
        let mut v = Vec::new();
        let mut t = 0.0;
        while t < total - 1e-9 {
            let d = step_s.min(total - t);
            let mid = t + 0.5 * d;
            let ang = 2.0 * std::f64::consts::PI * (mid / 3600.0 - peak_hour) / 24.0;
            v.push(Phase {
                duration_s: d,
                power_scale,
                ambient_c: mean_c + swing_c * ang.cos(),
            });
            t += d;
        }
        v
    }
}

/// Temperatures of every node at sample times.
#[derive(Debug, Clone, PartialEq)]
pub struct Trajectory {
    pub times_s: Vec<f64>,
    /// `temps[sample][node]`.
    pub temps: Vec<Vec<f64>>,
}

impl Trajectory {
    /// First time `node` reaches `level` (interpolated between samples), s.
    pub fn first_reach(&self, node: usize, level: f64) -> Option<f64> {
        for w in 1..self.times_s.len() {
            let (a, b) = (self.temps[w - 1][node], self.temps[w][node]);
            if (a - level) * (b - level) <= 0.0 && a != b {
                let f = (level - a) / (b - a);
                return Some(self.times_s[w - 1] + f * (self.times_s[w] - self.times_s[w - 1]));
            }
            if a == level {
                return Some(self.times_s[w - 1]);
            }
        }
        None
    }

    pub fn node_series(&self, node: usize) -> Vec<f64> {
        self.temps.iter().map(|t| t[node]).collect()
    }
}

/// One layer of a wall: thickness, conductivity, volumetric heat capacity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layer {
    pub thickness_m: f64,
    /// W/(m K).
    pub k: f64,
    /// rho times specific heat, J/(m3 K).
    pub rho_cp: f64,
    /// Number of cells across the layer (at least 1).
    pub cells: usize,
}

/// A wall discretised across its thickness (1D finite volume).
#[derive(Debug, Clone)]
pub struct Slab {
    /// Node of each cell, inner side first.
    pub nodes: Vec<usize>,
    area_m2: f64,
    /// Conductance from the inner surface to the first cell centre, W/K.
    g_in: f64,
    /// Conductance from the last cell centre to the outer surface, W/K.
    g_out: f64,
}

impl Network {
    /// Adds a wall of `layers` (inner side first) and face area `area_m2`.
    /// Join its two faces with [`Slab::connect_inner`] and
    /// [`Slab::connect_outer`].
    pub fn add_slab(&mut self, layers: &[Layer], area_m2: f64) -> Slab {
        self.add_slab_with(layers, area_m2, area_m2)
    }

    /// Like [`Network::add_slab`] with the heat capacity over `cap_area_m2`
    /// and the conduction and films over `area_m2`: a face of which only a
    /// share gives off heat (the IEC surface factor) keeps its full mass.
    pub fn add_slab_with(&mut self, layers: &[Layer], cap_area_m2: f64, area_m2: f64) -> Slab {
        let mut nodes = Vec::new();
        // (node, half-cell resistance parameter dx/(2k A))
        let mut half_r: Vec<f64> = Vec::new();
        for l in layers {
            let cells = l.cells.max(1);
            let dx = l.thickness_m / cells as f64;
            for _ in 0..cells {
                nodes.push(self.add_node(l.rho_cp * dx * cap_area_m2));
                half_r.push(dx / (2.0 * l.k * area_m2));
            }
        }
        for i in 1..nodes.len() {
            self.link(nodes[i - 1], nodes[i], 1.0 / (half_r[i - 1] + half_r[i]));
        }
        Slab {
            g_in: 1.0 / half_r[0],
            g_out: 1.0 / half_r[half_r.len() - 1],
            nodes,
            area_m2,
        }
    }
}

impl Slab {
    /// Joins the inner face to `to` (or the ambient) through a film of
    /// coefficient `h` W/(m2 K) over the whole face.
    pub fn connect_inner(&self, net: &mut Network, to: Option<usize>, h: f64) {
        net.link_or_ambient(self.nodes[0], to, series(h * self.area_m2, self.g_in));
    }

    /// Like [`Slab::connect_inner`] over a part `area_m2` of the face.
    pub fn connect_inner_area(&self, net: &mut Network, to: Option<usize>, h: f64, area_m2: f64) {
        let share = area_m2 / self.area_m2;
        net.link_or_ambient(self.nodes[0], to, series(h * area_m2, self.g_in * share));
    }

    /// Joins the outer face to `to` (or the ambient) through a film.
    pub fn connect_outer(&self, net: &mut Network, to: Option<usize>, h: f64) {
        let last = self.nodes[self.nodes.len() - 1];
        net.link_or_ambient(last, to, series(h * self.area_m2, self.g_out));
    }

    /// Inner surface temperature given the cell temperatures `t` and the
    /// temperature `t_other` on the other side of the inner film `h`.
    pub fn inner_surface(&self, t: &[f64], t_other: f64, h: f64) -> f64 {
        let (ga, gf) = (self.g_in, h * self.area_m2);
        (ga * t[self.nodes[0]] + gf * t_other) / (ga + gf)
    }

    /// Outer surface temperature.
    pub fn outer_surface(&self, t: &[f64], t_other: f64, h: f64) -> f64 {
        let (ga, gf) = (self.g_out, h * self.area_m2);
        (ga * t[self.nodes[self.nodes.len() - 1]] + gf * t_other) / (ga + gf)
    }
}

fn series(a: f64, b: f64) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        0.0
    } else {
        1.0 / (1.0 / a + 1.0 / b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * a.abs().max(b.abs()).max(1e-12)
    }

    fn lumped(c: f64, g: f64) -> (Solved, f64) {
        let mut n = Network::new();
        let a = n.add_node(c);
        n.to_ambient(a, g);
        (n.solve().unwrap(), c / g)
    }

    #[test]
    fn lumped_heating_follows_the_exponential() {
        let (s, tau) = lumped(5_000.0, 2.5);
        assert!(close(s.slowest_s(), tau, 1e-12));
        let p = [100.0];
        for &t in &[0.0, 300.0, tau, 3.0 * tau, 10.0 * tau] {
            let got = s.advance(&[20.0], &p, 20.0, t)[0];
            let want = 20.0 + 100.0 / 2.5 * (1.0 - (-t / tau).exp());
            assert!((got - want).abs() < 1e-9, "t={t}: {got} vs {want}");
        }
        assert!(close(s.steady(&p, 20.0)[0], 60.0, 1e-12));
    }

    #[test]
    fn lumped_cooling_follows_the_exponential() {
        let (s, tau) = lumped(800.0, 4.0);
        let got = s.advance(&[70.0], &[0.0], 25.0, 1.5 * tau)[0];
        let want = 25.0 + 45.0 * (-1.5f64).exp();
        assert!((got - want).abs() < 1e-9);
    }

    #[test]
    fn two_nodes_match_the_closed_form_eigenvalues() {
        // C1 dT1 = -g1 T1 - g12 (T1 - T2); C2 dT2 = -g12 (T2 - T1)
        let (c1, c2, g1, g12) = (100.0, 400.0, 2.0, 5.0);
        let mut n = Network::new();
        let a = n.add_node(c1);
        let b = n.add_node(c2);
        n.to_ambient(a, g1);
        n.link(a, b, g12);
        let s = n.solve().unwrap();
        // det(lambda C - (-G)) : (c1 l + g1 + g12)(c2 l + g12) - g12^2 = 0
        let (qa, qb, qc) = (
            c1 * c2,
            c1 * g12 + c2 * (g1 + g12),
            (g1 + g12) * g12 - g12 * g12,
        );
        let disc = (qb * qb - 4.0 * qa * qc).sqrt();
        let l_small = (qb - disc) / (2.0 * qa);
        let l_big = (qb + disc) / (2.0 * qa);
        let tc = s.time_constants_s();
        assert!(close(tc[0], 1.0 / l_small, 1e-10), "{tc:?}");
        assert!(close(tc[1], 1.0 / l_big, 1e-10));
    }

    #[test]
    fn heat_is_conserved_on_the_way_to_steady() {
        // Energy in = stored + lost to ambient (integrated by fine sampling).
        let mut n = Network::new();
        let a = n.add_node(300.0);
        let b = n.add_node(1200.0);
        n.to_ambient(b, 3.0);
        n.link(a, b, 8.0);
        let s = n.solve().unwrap();
        let p = [50.0, 0.0];
        let (dt, steps) = (0.5, 4000);
        let mut t = vec![20.0, 20.0];
        let mut lost = 0.0;
        for _ in 0..steps {
            let nt = s.advance(&t, &p, 20.0, dt);
            lost += 3.0 * (0.5 * (t[1] + nt[1]) - 20.0) * dt; // trapezoid
            t = nt;
        }
        let stored = 300.0 * (t[0] - 20.0) + 1200.0 * (t[1] - 20.0);
        let input = 50.0 * dt * steps as f64;
        assert!(
            (input - stored - lost).abs() < 1e-3 * input,
            "{input} {stored} {lost}"
        );
    }

    #[test]
    fn a_node_without_a_path_to_the_ambient_is_rejected() {
        let mut n = Network::new();
        let a = n.add_node(10.0);
        let b = n.add_node(10.0);
        n.link(a, b, 1.0);
        assert!(n.solve().is_err());
        let mut n = Network::new();
        n.add_node(0.0);
        assert!(n.solve().is_err());
    }

    #[test]
    fn sinusoidal_ambient_has_the_lumped_amplitude_and_phase() {
        // Steady oscillation of C dT = -G (T - Ta): amplitude ratio
        // 1/sqrt(1 + (w tau)^2), lag atan(w tau).
        let (c, g) = (40_000.0, 5.0);
        let (s, tau) = lumped(c, g);
        let mean = 25.0;
        let swing = 8.0;
        let phases = Phase::daily_ambient(mean, swing, 15.0, 300.0, 6.0, 0.0);
        let traj = s.run(&[mean], &[0.0], &phases, 300.0);
        let w = 2.0 * std::f64::consts::PI / 86_400.0;
        let want_amp = swing / (1.0 + (w * tau).powi(2)).sqrt();
        let want_lag_s = (w * tau).atan() / w;
        // Last full day.
        let n = traj.times_s.len();
        let day: Vec<(f64, f64)> = (0..n)
            .filter(|&i| traj.times_s[i] >= 5.0 * 86_400.0)
            .map(|i| (traj.times_s[i], traj.temps[i][0]))
            .collect();
        let hi = day.iter().cloned().fold(f64::MIN, |m, p| m.max(p.1));
        let lo = day.iter().cloned().fold(f64::MAX, |m, p| m.min(p.1));
        assert!(
            close((hi - lo) / 2.0, want_amp, 0.01),
            "{hi} {lo} {want_amp}"
        );
        let peak_t = day.iter().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
        let peak_in_day = (peak_t % 86_400.0) / 3600.0;
        let want_peak = 15.0 + want_lag_s / 3600.0;
        assert!(
            (peak_in_day - want_peak).abs() < 0.2,
            "{peak_in_day} vs {want_peak}"
        );
    }

    #[test]
    fn duty_cycle_ripple_matches_the_closed_form() {
        // Lumped, power on for t1 then off for t2, periodic steady state:
        // T_start = Ta + (P/G) e^{-t2/tau} (1 - e^{-t1/tau}) / (1 - e^{-(t1+t2)/tau}).
        let (c, g, p) = (20_000.0, 4.0, 200.0);
        let (s, tau) = lumped(c, g);
        let (t1, t2) = (900.0, 1500.0);
        let cycle = Phase::duty_cycle(t1, 1.0, t2, 0.0, 30.0, 1);
        let start = s.periodic_start(&[p], &cycle)[0];
        let ea = (-t1 / tau).exp();
        let eb = (-t2 / tau).exp();
        let want_start = 30.0 + p / g * eb * (1.0 - ea) / (1.0 - ea * eb);
        assert!(close(start, want_start, 1e-9), "{start} vs {want_start}");
        // And one more cycle returns to it.
        let traj = s.run(&[start], &[p], &cycle, 60.0);
        assert!(close(traj.temps.last().unwrap()[0], start, 1e-9));
    }

    #[test]
    fn a_slab_with_cold_faces_decays_with_the_first_fourier_mode() {
        // Faces held at ambient by very large films: first mode rate
        // alpha (pi/L)^2.
        let (k, rho_cp, l) = (2.0, 2.0e6, 0.05);
        let alpha = k / rho_cp;
        let want = l * l / (std::f64::consts::PI.powi(2) * alpha);
        let mut net = Network::new();
        let slab = net.add_slab(
            &[Layer {
                thickness_m: l,
                k,
                rho_cp,
                cells: 40,
            }],
            1.0,
        );
        slab.connect_inner(&mut net, None, 1.0e9);
        slab.connect_outer(&mut net, None, 1.0e9);
        let tau = net.solve().unwrap().slowest_s();
        assert!(close(tau, want, 2e-3), "{tau} vs {want}");
    }

    #[test]
    fn a_thin_slab_with_a_film_is_lumped() {
        // Low Biot number: one time constant rho c L / (h_total).
        let (k, rho_cp, l, h) = (50.0, 3.85e6, 0.0025, 8.0);
        let mut net = Network::new();
        let slab = net.add_slab(
            &[Layer {
                thickness_m: l,
                k,
                rho_cp,
                cells: 4,
            }],
            1.0,
        );
        slab.connect_inner(&mut net, None, h);
        slab.connect_outer(&mut net, None, h);
        let tau = net.solve().unwrap().slowest_s();
        let want = rho_cp * l / (2.0 * h);
        assert!(close(tau, want, 1e-3), "{tau} vs {want}");
    }

    #[test]
    fn a_slab_step_in_surface_temperature_matches_the_series_solution() {
        // Initially 0, faces suddenly at 1: centre temperature
        // 1 - (4/pi) sum (-1)^n/(2n+1) exp(-(2n+1)^2 pi^2 alpha t / L^2).
        let (k, rho_cp, l) = (1.5, 1.8e6, 0.04);
        let alpha = k / rho_cp;
        let mut net = Network::new();
        let slab = net.add_slab(
            &[Layer {
                thickness_m: l,
                k,
                rho_cp,
                cells: 41,
            }],
            1.0,
        );
        slab.connect_inner(&mut net, None, 1.0e9);
        slab.connect_outer(&mut net, None, 1.0e9);
        let s = net.solve().unwrap();
        let centre = slab.nodes[20];
        for &t in &[200.0, 600.0, 1200.0] {
            let got = s.advance(&vec![0.0; 41], &vec![0.0; 41], 1.0, t)[centre];
            let mut sum = 0.0;
            for n in 0..200 {
                let m = (2 * n + 1) as f64;
                let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
                sum += sign / m * (-(m * std::f64::consts::PI / l).powi(2) * alpha * t).exp();
            }
            let want = 1.0 - 4.0 / std::f64::consts::PI * sum;
            assert!((got - want).abs() < 2e-3, "t={t}: {got} vs {want}");
        }
    }

    #[test]
    fn layers_in_series_have_the_series_conductance() {
        // Steady heat flux through a two-layer wall = dT / (R1 + R2 + films).
        let mut net = Network::new();
        let slab = net.add_slab(
            &[
                Layer {
                    thickness_m: 0.002,
                    k: 50.0,
                    rho_cp: 3.85e6,
                    cells: 2,
                },
                Layer {
                    thickness_m: 0.03,
                    k: 0.04,
                    rho_cp: 5.0e4,
                    cells: 6,
                },
            ],
            2.0,
        );
        let inside = net.add_node(1000.0);
        slab.connect_inner(&mut net, Some(inside), 8.0);
        slab.connect_outer(&mut net, None, 12.0);
        let s = net.solve().unwrap();
        let mut power = vec![0.0; net.len()];
        power[inside] = 100.0;
        let t = s.steady(&power, 20.0);
        let r = 1.0 / (8.0 * 2.0) + 0.002 / (50.0 * 2.0) + 0.03 / (0.04 * 2.0) + 1.0 / (12.0 * 2.0);
        let want = 20.0 + 100.0 * r;
        assert!(close(t[inside], want, 1e-9), "{} vs {want}", t[inside]);
        // Outer surface from the film.
        let so = slab.outer_surface(&t, 20.0, 12.0);
        assert!(close(so, 20.0 + 100.0 / (12.0 * 2.0), 1e-9));
    }
}
