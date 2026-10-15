//! Reduced-order airflow: a depth-averaged porous-medium flow field.
//!
//! Velocity follows from a pressure field, `u = -(grad p - b) / r(|u|)`, with
//! `b` the buoyancy force of the warm air and `r` a laminar gap friction plus
//! a quadratic drag for clutter and obstacles. Mass is conserved cell by cell
//! including vents and fans, which are linear (fans) or linearised orifice
//! (vents) relations between the cell pressure and the flow through the port.
//! Pressure is measured against the outside air at the same height, so the
//! stack effect needs no special treatment.

use crate::linalg::{self, Builder};
use crate::setup::{Port, PortKind, Setup};

#[derive(Clone)]
pub struct Flow {
    /// Pressure against the outside, Pa.
    pub p: Vec<f64>,
    /// Volume flow from cell `i` to `i + 1` (east) and to `i + nx` (south, down
    /// the screen), m3/s.
    pub qx: Vec<f64>,
    pub qy: Vec<f64>,
    /// Lagged open-path speeds on the same faces, m/s.
    pub sx: Vec<f64>,
    pub sy: Vec<f64>,
    /// Flow out of the enclosure per port, m3/s (negative = in).
    pub port_q: Vec<f64>,
    /// Port conductance used last, m3/s per Pa.
    port_g: Vec<f64>,
}

impl Flow {
    pub fn new(s: &Setup) -> Flow {
        let n = s.n();
        Flow {
            p: vec![0.0; n],
            qx: vec![0.0; n],
            qy: vec![0.0; n],
            sx: vec![0.05; n],
            sy: vec![0.05; n],
            port_q: vec![0.0; s.ports.len()],
            port_g: vec![0.0; s.ports.len()],
        }
    }
}

/// Smallest open share of a face; below it the face is a wall.
const SOLID: f64 = 0.02;
/// Speed floor in the drag law, m/s, so that still air has a finite
/// resistance.
const SPEED_FLOOR: f64 = 0.01;

/// Form drag coefficient of a partly blocked cell.
fn form_drag(block: f64) -> f64 {
    let b = block.min(0.98);
    1.5 * (b / (1.0 - b)).powi(2)
}

/// Open fraction of a face between two cells, harmonic mean.
fn open_fraction(a: f64, b: f64) -> f64 {
    let (oa, ob) = ((1.0 - a).max(0.0), (1.0 - b).max(0.0));
    if oa + ob <= 0.0 {
        0.0
    } else {
        2.0 * oa * ob / (oa + ob)
    }
}

struct Face {
    i: usize,
    j: usize,
    sigma: f64,
    force: f64,
    /// Open-path conversion: speed = |q| / (len * depth * phi).
    area: f64,
}

/// One Picard sweep: rebuild the pressure system from the current lagged
/// speeds, ports and temperatures, solve it and update fluxes.
pub fn sweep(s: &Setup, t_air: &[f64], flow: &mut Flow) {
    let (nx, ny, n) = (s.nx, s.ny, s.n());
    let a_lam = 12.0 * s.air.mu / (s.depth * s.depth);
    let clutter = s.params.clutter_drag_per_m;
    let buoy = s.air.rho * s.air.g * s.air.beta;
    let mut faces: Vec<Face> = Vec::with_capacity(2 * n);
    for r in 0..ny {
        for c in 0..nx {
            let i = r * nx + c;
            for (dir_east, j) in [(true, i + 1), (false, i + nx)] {
                if (dir_east && c + 1 >= nx) || (!dir_east && r + 1 >= ny) {
                    continue;
                }
                let phi = open_fraction(s.block[i], s.block[j]);
                if phi < SOLID {
                    continue;
                }
                let (len, ds) = if dir_east { (s.hy, s.hx) } else { (s.hx, s.hy) };
                let speed = if dir_east { flow.sx[i] } else { flow.sy[i] };
                let zeta = form_drag(0.5 * (s.block[i] + s.block[j]));
                let c_f = clutter + zeta / ds;
                let resist = a_lam + s.air.rho * c_f * speed.max(SPEED_FLOOR);
                let area = len * s.depth * phi;
                let sigma = area / (resist * ds);
                // Buoyancy only acts along the vertical (screen y, down).
                let force = if dir_east {
                    0.0
                } else {
                    -buoy * (0.5 * (t_air[i] + t_air[j]) - s.ambient) * ds
                };
                faces.push(Face {
                    i,
                    j,
                    sigma,
                    force,
                    area,
                });
            }
        }
    }

    // Port laws: out-flow = g * p - q0.
    let mut g = vec![0.0; n];
    let mut q0 = vec![0.0; n];
    let mut port_law = vec![(0.0, 0.0); s.ports.len()];
    for (k, port) in s.ports.iter().enumerate() {
        let (gk, q0k) = port_law_of(s, port, flow.p[port.cell], flow.port_g[k]);
        port_law[k] = (gk, q0k);
        g[port.cell] += gk;
        q0[port.cell] += q0k;
    }

    let mut bld = Builder::new(n);
    let mut rhs = vec![0.0; n];
    for f in &faces {
        bld.add(f.i, f.i, f.sigma);
        bld.add(f.j, f.j, f.sigma);
        bld.add(f.i, f.j, -f.sigma);
        bld.add(f.j, f.i, -f.sigma);
        rhs[f.i] -= f.sigma * f.force;
        rhs[f.j] += f.sigma * f.force;
    }
    for i in 0..n {
        bld.add(i, i, g[i]);
        rhs[i] += q0[i];
    }
    // A closed box fixes the pressure only up to a constant; the system is
    // singular but consistent (the right-hand side sums to zero), which
    // conjugate gradients handles. No anchor: it would leak mass.
    let a = bld.finish();
    let closed = g.iter().all(|v| *v == 0.0);
    let info = linalg::cg(&a, &rhs, &mut flow.p, 1e-10, 4000);
    if closed {
        let mean = flow.p.iter().sum::<f64>() / n as f64;
        for p in &mut flow.p {
            *p -= mean;
        }
    }
    if std::env::var("THERMAL_DEBUG").is_ok() {
        eprintln!(
            "    cg {} its residual {:.2e}",
            info.iterations, info.residual
        );
    }

    // Fluxes and speeds (relaxed).
    let relax = 0.5;
    for q in flow.qx.iter_mut().chain(flow.qy.iter_mut()) {
        *q = 0.0;
    }
    for f in &faces {
        let q = f.sigma * (flow.p[f.i] - flow.p[f.j] + f.force);
        let east = f.j == f.i + 1 && !(f.j == f.i + nx);
        let speed = q.abs() / f.area;
        if east {
            flow.qx[f.i] = q;
            flow.sx[f.i] = (1.0 - relax) * flow.sx[f.i] + relax * speed;
        } else {
            flow.qy[f.i] = q;
            flow.sy[f.i] = (1.0 - relax) * flow.sy[f.i] + relax * speed;
        }
    }
    for (k, port) in s.ports.iter().enumerate() {
        let (gk, q0k) = port_law[k];
        let q = gk * flow.p[port.cell] - q0k;
        // Fans do not run backwards in this model.
        flow.port_q[k] = match port.kind {
            PortKind::Fan { intake: true, .. } => q.min(0.0),
            PortKind::Fan { intake: false, .. } => q.max(0.0),
            PortKind::Vent { .. } => q,
        };
        flow.port_g[k] = gk;
    }
}

/// Linearised law of one port at the last known cell pressure.
fn port_law_of(s: &Setup, port: &Port, p_last: f64, g_last: f64) -> (f64, f64) {
    match port.kind {
        PortKind::Vent { a_eff_m2 } => {
            let g_new = a_eff_m2 * (2.0 / (s.air.rho * p_last.abs().max(0.05))).sqrt();
            let g = if g_last > 0.0 {
                0.5 * g_last + 0.5 * g_new
            } else {
                g_new
            };
            (g, 0.0)
        }
        PortKind::Fan {
            q_max,
            p_max,
            intake,
        } => {
            let gk = q_max / p_max;
            // The straight-line curve runs from (0 flow, p_max) to (free flow,
            // 0 Pa); beyond free flow it is not defined, so the flow stays at
            // free flow when the other side sucks harder.
            if intake {
                // Q_in = q_max (1 - p / p_max) for 0 <= p < p_max.
                if p_last < 0.0 {
                    (0.0, q_max)
                } else if p_last < p_max {
                    (gk, q_max)
                } else {
                    (0.0, 0.0)
                }
            } else if p_last > 0.0 {
                (0.0, -q_max)
            } else if p_last > -p_max {
                // Q_out = q_max (1 + p / p_max) for -p_max < p <= 0.
                (gk, -q_max)
            } else {
                (0.0, 0.0)
            }
        }
    }
}

/// Iterates [`sweep`] until the speeds settle or `sweeps` is reached.
pub fn solve(s: &Setup, t_air: &[f64], flow: &mut Flow, sweeps: usize) {
    for _ in 0..sweeps {
        let before: Vec<f64> = flow.qy.iter().chain(flow.qx.iter()).copied().collect();
        sweep(s, t_air, flow);
        let scale = before.iter().fold(1e-9_f64, |m, v| m.max(v.abs()));
        let change = before
            .iter()
            .zip(flow.qy.iter().chain(flow.qx.iter()))
            .fold(0.0_f64, |m, (a, b)| m.max((a - b).abs()));
        if change / scale < 1e-4 {
            break;
        }
    }
}

/// Open-path speed of every cell as screen axes (right, down), m/s.
pub fn cell_velocity(s: &Setup, flow: &Flow) -> (Vec<f64>, Vec<f64>) {
    let (nx, ny, n) = (s.nx, s.ny, s.n());
    let mut vx = vec![0.0; n];
    let mut vy = vec![0.0; n];
    let ux = |i: usize, j: usize, q: f64| {
        let phi = open_fraction(s.block[i], s.block[j]).max(0.2);
        q / (s.hy * s.depth * phi)
    };
    let uy = |i: usize, j: usize, q: f64| {
        let phi = open_fraction(s.block[i], s.block[j]).max(0.2);
        q / (s.hx * s.depth * phi)
    };
    for r in 0..ny {
        for c in 0..nx {
            let i = r * nx + c;
            let east = if c + 1 < nx {
                Some(ux(i, i + 1, flow.qx[i]))
            } else {
                None
            };
            let west = if c > 0 {
                Some(ux(i - 1, i, flow.qx[i - 1]))
            } else {
                None
            };
            vx[i] = match (east, west) {
                (Some(a), Some(b)) => 0.5 * (a + b),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => 0.0,
            };
            let south = if r + 1 < ny {
                Some(uy(i, i + nx, flow.qy[i]))
            } else {
                None
            };
            let north = if r > 0 {
                Some(uy(i - nx, i, flow.qy[i - nx]))
            } else {
                None
            };
            vy[i] = match (south, north) {
                (Some(a), Some(b)) => 0.5 * (a + b),
                (Some(a), None) | (None, Some(a)) => a,
                (None, None) => 0.0,
            };
            if s.block[i] >= 1.0 - SOLID {
                vx[i] = 0.0;
                vy[i] = 0.0;
            }
        }
    }
    (vx, vy)
}

pub(crate) fn face_open_fraction(a: f64, b: f64) -> f64 {
    open_fraction(a, b)
}
