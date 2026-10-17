//! Linear systems of the steady path: implicit first order upwind
//! convection, the full diffusion, and a deferred correction that restores
//! the configured scheme (explicit `F (phi_scheme - phi_upwind)` from the
//! previous iterate, which vanishes from the answer at convergence).

use super::super::{FlowSolver, ThermalBc, harmonic, wall_value, with};
use super::sweep::{Coef, par_planes};
use crate::flow::scheme::{Sample, Scheme, face_value};

/// Convection of one direction as implicit upwind coefficients plus the
/// deferred correction. `f_lo`, `f_hi`: advecting velocities (flux per unit
/// area) of the two faces; `phi_*` the scheme values, `up_*` the upwind
/// values; `w` the width. Adds the implicit part to `c` (scaled by `scale`)
/// and returns the explicit correction to subtract from the right side.
struct Dir {
    f_lo: f64,
    f_hi: f64,
    ho_lo: f64,
    ho_hi: f64,
    up_lo: f64,
    up_hi: f64,
    w: f64,
}

impl Dir {
    /// Adds the upwind coefficients for direction `b` and returns the
    /// (diag, neighbour sum) of the conservative form and the correction.
    fn add(&self, c: &mut Coef, b: usize, scale: f64) -> (f64, f64, f64) {
        let k = scale / self.w;
        let lo = self.f_lo.max(0.0) * k;
        let hi = (-self.f_hi).max(0.0) * k;
        let diag = (self.f_hi.max(0.0) + (-self.f_lo).max(0.0)) * k;
        c.lo[b] += lo;
        c.hi[b] += hi;
        let defer =
            (self.f_hi * (self.ho_hi - self.up_hi) - self.f_lo * (self.ho_lo - self.up_lo)) * k;
        (diag, lo + hi, defer)
    }
}

impl FlowSolver {
    /// Advecting velocity of the edge `e` along `b` for velocity component
    /// `c` at `ix` (the average of the two `b` velocities beside it).
    fn advecting(&self, c: usize, b: usize, e: usize, ix: [usize; 3]) -> f64 {
        let m = &self.mesh;
        let (lo, hi) = (ix[c] - 1, ix[c]);
        let w_lo = m.dx[c][hi] / (m.dx[c][lo] + m.dx[c][hi]);
        let mut ia = with(ix, b, e);
        ia[c] = lo;
        let mut ib = ia;
        ib[c] = hi;
        let (va, vb) = (m.idx(b, ia), m.idx(b, ib));
        w_lo * self.vel[b][va] * self.open.afrac[b][va]
            + (1.0 - w_lo) * self.vel[b][vb] * self.open.afrac[b][vb]
    }

    /// Momentum equations of component `c` with pseudo step `dt` into
    /// `coef` (sized like the component). The pressure gradient of the
    /// current `p` is on the right side.
    pub(super) fn assemble_momentum(&self, c: usize, dt: f64, sch: Scheme, coef: &mut [Coef]) {
        let m = &self.mesh;
        let d = m.dims(c);
        let u = &self.vel;
        let nu = self.cfg.nu;
        let (nu_t, wall_nu) = (self.nu_t.as_deref(), self.wall_nu.as_deref());
        let nu_cell = |cell: [usize; 3]| nu + nu_t.map_or(0.0, |v| v[m.cell(cell)]);
        par_planes(coef, d[0] * d[1], |k, plane| {
            for j in 0..d[1] {
                for i in 0..d[0] {
                    let ix = [i, j, k];
                    let id = m.idx(c, ix);
                    let mut cf = Coef::default();
                    if ix[c] == 0 || ix[c] == m.n[c] || self.open.afrac[c][id] == 0.0 {
                        plane[j * d[0] + i] = cf;
                        continue;
                    }
                    let mut diag = 1.0 / dt + self.drag_rate_at(c, id);
                    let mut rhs = u[c][id] / dt;
                    let (mut conv_diag, mut conv_nb) = (0.0, 0.0);
                    let mut defer_sum = 0.0;
                    for b in 0..3 {
                        let s = ix[b];
                        // ---- convection
                        let dir = if b == c {
                            let w = 0.5 * (m.dx[c][s - 1] + m.dx[c][s]);
                            let at = |q: usize| -> Sample {
                                (m.edges[c][q], u[c][m.idx(c, with(ix, c, q))])
                            };
                            let nb = |q: isize| -> Option<Sample> {
                                (q >= 0 && q <= m.n[c] as isize).then(|| at(q as usize))
                            };
                            let (um, u0, up) = (at(s - 1).1, at(s).1, at(s + 1).1);
                            let u_lo = 0.5 * (um + u0);
                            let u_hi = 0.5 * (u0 + up);
                            let si = s as isize;
                            Dir {
                                f_lo: u_lo,
                                f_hi: u_hi,
                                ho_lo: face_value(
                                    sch,
                                    m.centre[c][s - 1],
                                    nb(si - 2),
                                    at(s - 1),
                                    at(s),
                                    nb(si + 1),
                                    u_lo > 0.0,
                                ),
                                ho_hi: face_value(
                                    sch,
                                    m.centre[c][s],
                                    nb(si - 1),
                                    at(s),
                                    at(s + 1),
                                    nb(si + 2),
                                    u_hi > 0.0,
                                ),
                                up_lo: if u_lo > 0.0 { um } else { u0 },
                                up_hi: if u_hi > 0.0 { u0 } else { up },
                                w,
                            }
                        } else {
                            let w = m.dx[b][s];
                            let at = |q: usize| -> Sample {
                                (m.centre[b][q], u[c][m.idx(c, with(ix, b, q))])
                            };
                            let mut face = |e: usize| -> (f64, f64, f64) {
                                if e == 0 || e == m.n[b] {
                                    // momentum through an open face: out with
                                    // the cell's own value, in with none
                                    let ub = self.advecting(c, b, e, ix);
                                    let inflow = if e == 0 { ub > 0.0 } else { ub < 0.0 };
                                    if !inflow {
                                        let out = if e == 0 { -ub } else { ub };
                                        conv_diag += out / w;
                                    }
                                    return (0.0, 0.0, 0.0);
                                }
                                let ub = self.advecting(c, b, e, ix);
                                let ll = (e >= 2).then(|| at(e - 2));
                                let uu = (e + 1 < m.n[b]).then(|| at(e + 1));
                                let phi = face_value(
                                    sch,
                                    m.edges[b][e],
                                    ll,
                                    at(e - 1),
                                    at(e),
                                    uu,
                                    ub > 0.0,
                                );
                                let up = if ub > 0.0 { at(e - 1).1 } else { at(e).1 };
                                (ub, phi, up)
                            };
                            let lo = face(s);
                            let hi = face(s + 1);
                            Dir {
                                f_lo: lo.0,
                                f_hi: hi.0,
                                ho_lo: lo.1,
                                ho_hi: hi.1,
                                up_lo: lo.2,
                                up_hi: hi.2,
                                w,
                            }
                        };
                        let (cd, cn, defer) = dir.add(&mut cf, b, 1.0);
                        conv_diag += cd;
                        conv_nb += cn;
                        defer_sum += defer;
                        // ---- diffusion
                        if b == c {
                            let w = 0.5 * (m.dx[c][s - 1] + m.dx[c][s]);
                            let nu_lo = nu_cell(with(ix, c, s - 1));
                            let nu_hi = nu_cell(ix);
                            let cl = nu_lo / (m.dx[c][s - 1] * w);
                            let cu = nu_hi / (m.dx[c][s] * w);
                            diag += cl + cu;
                            cf.lo[c] += cl;
                            cf.hi[c] += cu;
                        } else {
                            let w = m.dx[b][s];
                            let side = |t: usize| {
                                [
                                    with(with(ix, b, t), c, ix[c] - 1),
                                    with(with(ix, b, t), c, ix[c]),
                                ]
                            };
                            let nu_here = {
                                let [a, a2] = side(s);
                                0.5 * (nu_cell(a) + nu_cell(a2))
                            };
                            for upper in [false, true] {
                                let face = 2 * b + upper as usize;
                                if (!upper && s == 0) || (upper && s + 1 == m.n[b]) {
                                    let bc = self.cfg.faces[face];
                                    if !bc.slip {
                                        let closed = 1.0 - self.open_share(b, upper, ix, c);
                                        let nu_w =
                                            wall_value(wall_nu, m, &side(s), face).unwrap_or(nu);
                                        let cw = closed * nu_w / (0.5 * w * w);
                                        diag += cw;
                                        rhs += cw * bc.wall_velocity[c];
                                    }
                                } else {
                                    let q = if upper { s + 1 } else { s - 1 };
                                    let nbr = m.idx(c, with(ix, b, q));
                                    if self.open.afrac[c][nbr] == 0.0 {
                                        let nu_w =
                                            wall_value(wall_nu, m, &side(s), face).unwrap_or(nu);
                                        diag += nu_w / (0.5 * w * w);
                                    } else {
                                        let dist = (m.centre[b][q] - m.centre[b][s]).abs();
                                        let nu_there = {
                                            let [a, a2] = side(q);
                                            0.5 * (nu_cell(a) + nu_cell(a2))
                                        };
                                        let cn = harmonic(nu_here, nu_there) / (dist * w);
                                        diag += cn;
                                        if upper {
                                            cf.hi[b] += cn;
                                        } else {
                                            cf.lo[b] += cn;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // the conservative upwind diagonal, never below the sum
                    // of its neighbours (a leftover divergence must not cost
                    // diagonal dominance)
                    diag += conv_diag.max(conv_nb);
                    // buoyancy, pressure gradient
                    let (lo, hi) = (with(ix, c, ix[c] - 1), ix);
                    let w_lo = m.dx[c][ix[c]] / (m.dx[c][ix[c] - 1] + m.dx[c][ix[c]]);
                    let tf = w_lo * self.t[m.cell(lo)] + (1.0 - w_lo) * self.t[m.cell(hi)];
                    rhs += -self.cfg.gravity[c] * self.cfg.beta * (tf - self.t_buoyancy_ref);
                    let dist = m.centre[c][ix[c]] - m.centre[c][ix[c] - 1];
                    rhs -= (self.p[m.cell(hi)] - self.p[m.cell(lo)]) / dist;
                    cf.diag = diag;
                    cf.rhs = rhs;
                    cf.defer = defer_sum;
                    plane[j * d[0] + i] = cf;
                }
            }
        });
    }

    /// Energy equation with pseudo step `dt` into `coef` (sized like the cells).
    pub(super) fn assemble_energy(&self, dt: f64, sch: Scheme, coef: &mut [Coef]) {
        let m = &self.mesh;
        let n = m.n;
        let (nu_t, wall_alpha) = (self.nu_t.as_deref(), self.wall_alpha.as_deref());
        let pr_t = self.cfg.prandtl_t;
        let k_eff =
            |id: usize| self.k_cell[id] + nu_t.map_or(0.0, |v| self.c_cell[id] * v[id] / pr_t);
        par_planes(coef, n[0] * n[1], |k, plane| {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let ix = [i, j, k];
                    let id = m.cell(ix);
                    let mut cf = Coef::default();
                    let (kc, cc) = (self.k_cell[id], self.c_cell[id]);
                    let scale = self.conv_scale[id];
                    let mut diag = 1.0 / dt;
                    let mut rhs = self.t[id] / dt + self.heat_scale * self.q_cell[id];
                    let (mut conv_diag, mut conv_nb) = (0.0, 0.0);
                    let mut defer_sum = 0.0;
                    for a in 0..3 {
                        let s = ix[a];
                        let w = m.dx[a][s];
                        // ---- convection
                        let mut face = |e: usize| -> (f64, f64, f64) {
                            let v = m.idx(a, with(ix, a, e));
                            if e == 0 || e == n[a] {
                                if self.open.is_empty() {
                                    return (0.0, 0.0, 0.0);
                                }
                                let phi = self.open.afrac[a][v];
                                if phi == 0.0 {
                                    return (0.0, 0.0, 0.0);
                                }
                                let ua = self.vel[a][v];
                                let inflow = if e == 0 { ua > 0.0 } else { ua < 0.0 };
                                let q = (phi * ua).abs() / w * scale;
                                if inflow {
                                    rhs += q * self.t_in[self.open.patch_of[a][v]];
                                } else {
                                    conv_diag += q;
                                }
                                return (0.0, 0.0, 0.0);
                            }
                            let ua = self.vel[a][v];
                            if ua == 0.0 {
                                return (0.0, 0.0, 0.0);
                            }
                            let at = |q: usize| -> Sample {
                                (m.centre[a][q], self.t[m.cell(with(ix, a, q))])
                            };
                            let ll = (e >= 2).then(|| at(e - 2));
                            let uu = (e + 1 < n[a]).then(|| at(e + 1));
                            let phi =
                                face_value(sch, m.edges[a][e], ll, at(e - 1), at(e), uu, ua > 0.0);
                            let up = if ua > 0.0 { at(e - 1).1 } else { at(e).1 };
                            (self.open.afrac[a][v] * ua, phi, up)
                        };
                        let lo = face(s);
                        let hi = face(s + 1);
                        let dir = Dir {
                            f_lo: lo.0,
                            f_hi: hi.0,
                            ho_lo: lo.1,
                            ho_hi: hi.1,
                            up_lo: lo.2,
                            up_hi: hi.2,
                            w,
                        };
                        let (cd, cn, defer) = dir.add(&mut cf, a, scale);
                        conv_diag += cd;
                        conv_nb += cn;
                        defer_sum += defer;
                        // ---- conduction
                        for upper in [false, true] {
                            let fidx = 2 * a + upper as usize;
                            if (!upper && s == 0) || (upper && s + 1 == n[a]) {
                                let a_wall = wall_value(wall_alpha, m, &[ix], fidx);
                                let kc = a_wall.map_or(kc, |aw| aw * cc);
                                let mut closed = 1.0
                                    - self.open.afrac[a]
                                        [m.idx(a, with(ix, a, if upper { n[a] } else { 0 }))];
                                if fidx == 4 {
                                    closed *= self.rear_bare(i, j);
                                }
                                match self.cfg.faces[fidx].thermal {
                                    ThermalBc::Adiabatic => {}
                                    ThermalBc::Fixed(tw) => {
                                        let cw = closed * kc / (cc * 0.5 * w * w);
                                        diag += cw;
                                        rhs += cw * tw;
                                    }
                                    ThermalBc::Convective { h, t_out } => {
                                        let r = 0.5 * w / kc + 1.0 / h;
                                        let cw = closed / (cc * r * w);
                                        diag += cw;
                                        rhs += cw * t_out;
                                    }
                                    ThermalBc::Overall { u, t_out } => {
                                        let cw = closed * u / (cc * w);
                                        diag += cw;
                                        rhs += cw * t_out;
                                    }
                                }
                            } else {
                                let q = if upper { s + 1 } else { s - 1 };
                                let dist = (m.centre[a][q] - m.centre[a][s]).abs();
                                let nb = m.cell(with(ix, a, q));
                                let (lo, hi) = if upper { (id, nb) } else { (nb, id) };
                                let (lo_ix, hi_ix) = if upper {
                                    (ix, with(ix, a, q))
                                } else {
                                    (with(ix, a, q), ix)
                                };
                                let mut kf = self.k_face[a][hi];
                                let (c_lo, c_hi) = (self.c_cell[lo], self.c_cell[hi]);
                                let (w_lo, w_hi) = (m.dx[a][lo_ix[a]], m.dx[a][hi_ix[a]]);
                                let wall_lo = wall_value(wall_alpha, m, &[lo_ix], 2 * a + 1);
                                let wall_hi = wall_value(wall_alpha, m, &[hi_ix], 2 * a);
                                if let Some(aw) = wall_lo {
                                    kf = dist
                                        / (0.5 * w_lo / (aw * c_lo) + 0.5 * w_hi / self.k_cell[hi]);
                                } else if let Some(aw) = wall_hi {
                                    kf = dist
                                        / (0.5 * w_hi / (aw * c_hi) + 0.5 * w_lo / self.k_cell[lo]);
                                } else if nu_t.is_some() {
                                    let eff = harmonic(k_eff(lo), k_eff(hi));
                                    let mol = harmonic(self.k_cell[lo], self.k_cell[hi]);
                                    kf *= eff / mol;
                                }
                                let cn = kf / (cc * dist * w);
                                diag += cn;
                                if upper {
                                    cf.hi[a] += cn;
                                } else {
                                    cf.lo[a] += cn;
                                }
                            }
                        }
                    }
                    if k == 0
                        && let Some((g, tp)) = self.sheet_coupling(i, j)
                    {
                        let cw = g / (cc * m.volume(ix));
                        diag += cw;
                        rhs += cw * tp;
                    }
                    if let Some(r) = &self.rad_load {
                        let cv = 1.0 / (cc * m.volume(ix));
                        diag += r.cell_g[id] * cv;
                        rhs += r.cell_s[id] * cv;
                    }
                    cf.diag = diag + conv_diag.max(conv_nb);
                    cf.rhs = rhs;
                    cf.defer = defer_sum;
                    plane[j * n[0] + i] = cf;
                }
            }
        });
    }
}
