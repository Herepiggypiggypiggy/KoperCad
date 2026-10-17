//! Implicit scalar transport on the cell grid: first-order upwind
//! convection with the face fluxes of the flow core (optionally second
//! order by a limited deferred correction), diffusion, sources.

use super::mesh::TurbMesh;

/// One scalar equation `d(phi)/dt + div(u phi) = div(gamma grad phi) + su - sp phi`.
pub struct Equation<'a> {
    pub mesh: &'a TurbMesh,
    /// MAC face velocities (see [`TurbMesh::vel_idx`]).
    pub vel: &'a [Vec<f64>; 3],
    pub gamma: &'a [f64],
    pub su: &'a [f64],
    /// Implicit sink coefficient, 1/s, non-negative.
    pub sp: &'a [f64],
    /// Cells held at their current value (wall-function cells, solids).
    pub fixed: &'a [bool],
    /// Second-order convection: van Leer limited linear-upwind face values
    /// as a deferred correction on top of the implicit upwind step.
    pub tvd: bool,
}

impl Equation<'_> {
    /// Gauss-Seidel sweeps of the implicit step of length `dt`; returns the
    /// largest relative change.
    pub fn solve(&self, phi: &mut [f64], phi_old: &[f64], dt: f64, sweeps: usize) -> f64 {
        let m = self.mesh;
        let mut change: f64 = 0.0;
        for sweep in 0..sweeps {
            let mut max_dphi: f64 = 0.0;
            let mut max_phi: f64 = 1e-30;
            let forward = sweep % 2 == 0;
            let total = m.cells();
            for t in 0..total {
                let c = if forward { t } else { total - 1 - t };
                if !m.fluid[c] || self.fixed[c] {
                    continue;
                }
                let ix = m.ijk(c);
                let vol = m.volume(ix);
                let mut a_p = vol / dt + self.sp[c] * vol;
                let mut b = vol / dt * phi_old[c] + self.su[c] * vol;
                for axis in 0..3 {
                    for high in [false, true] {
                        let Some(nb) = m.neighbour(ix, axis, high) else {
                            continue;
                        };
                        let cn = m.cell(nb);
                        if !m.fluid[cn] {
                            continue;
                        }
                        let area = m.area(axis, ix);
                        let dist = (m.centre[axis][nb[axis]] - m.centre[axis][ix[axis]]).abs();
                        let (g1, g2) = (self.gamma[c], self.gamma[cn]);
                        let gf = if g1 + g2 > 0.0 {
                            2.0 * g1 * g2 / (g1 + g2)
                        } else {
                            0.0
                        };
                        let d = gf * area / dist;
                        // outward volumetric flux through this face
                        let face = if high { ix } else { nb };
                        let mut hi_face = face;
                        hi_face[axis] += 1;
                        // face between `face` and `face + 1` along axis: index of the
                        // shared edge is the high edge of the lower cell
                        let u = self.vel[axis][m.vel_idx(axis, hi_face)];
                        let f_out = if high { u * area } else { -u * area };
                        let a_nb = d + (-f_out).max(0.0);
                        a_p += d + f_out.max(0.0);
                        b += a_nb * phi[cn];
                        if self.tvd && f_out != 0.0 {
                            // upwind (u), downwind (d) and far upwind (uu) cell
                            let (cu, cd, uu) = if f_out > 0.0 {
                                (c, cn, m.neighbour(ix, axis, !high))
                            } else {
                                (cn, c, m.neighbour(nb, axis, high))
                            };
                            if let Some(uu) = uu {
                                let cuu = m.cell(uu);
                                if m.fluid[cuu] {
                                    let face_value = limited_face(phi[cuu], phi[cu], phi[cd]);
                                    b -= f_out * (face_value - phi[cu]);
                                }
                            }
                        }
                    }
                }
                let new = (b / a_p).max(0.0);
                max_dphi = max_dphi.max((new - phi[c]).abs());
                max_phi = max_phi.max(new.abs());
                phi[c] = new;
            }
            change = max_dphi / max_phi;
        }
        change
    }
}

/// Face value between the upwind cell `u` and the downwind cell `d` with
/// far-upwind value `uu`: linear upwind with the van Leer limiter, so the
/// result stays between `u` and `d` (bounded) and falls back to upwind at
/// extrema.
fn limited_face(uu: f64, u: f64, d: f64) -> f64 {
    let dd = d - u;
    if dd.abs() < 1e-300 {
        return u;
    }
    let r = (u - uu) / dd;
    let psi = (r + r.abs()) / (1.0 + r.abs());
    u + 0.5 * psi * dd
}
