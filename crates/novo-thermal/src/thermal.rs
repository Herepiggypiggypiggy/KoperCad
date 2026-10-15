//! The coupled temperature system: air cells, plate cells and device nodes,
//! assembled as one sparse matrix and solved with BiCGSTAB. Steady and
//! implicit-Euler transient share the assembly.

use crate::flow::{self, Flow};
use crate::linalg::{self, Builder};
use crate::setup::Setup;

#[derive(Clone)]
pub struct State {
    pub t_air: Vec<f64>,
    /// Plate temperature per cell (meaningful where the cell is on the plate).
    pub t_plate: Vec<f64>,
    pub t_dev: Vec<f64>,
    /// Temperature the flow sees (relaxed against oscillation).
    pub t_buoy: Vec<f64>,
    /// Surface coefficient per device, W/m2K.
    pub h_dev: Vec<f64>,
    pub flow: Flow,
    pub iterations: usize,
    pub converged: bool,
}

impl State {
    pub fn uniform(s: &Setup, t: f64) -> State {
        let n = s.n();
        State {
            t_air: vec![t; n],
            t_plate: vec![t; n],
            t_dev: vec![t; s.devices.len()],
            t_buoy: vec![t; n],
            h_dev: vec![8.0; s.devices.len()],
            flow: Flow::new(s),
            iterations: 0,
            converged: false,
        }
    }
}

const SIGMA: f64 = 5.670_374e-8;
const PR: f64 = 0.71;

/// Surface coefficient of a device: natural convection on a vertical plate
/// (Churchill and Chu), forced convection on a flat plate, combined as
/// `(hn^3 + hf^3)^(1/3)`, plus linearised radiation.
pub fn device_h(
    length_m: f64,
    t_surf: f64,
    t_air: f64,
    speed: f64,
    emissivity: f64,
    view: f64,
) -> f64 {
    let dt = (t_surf - t_air).abs().max(0.5);
    let tf = 0.5 * (t_surf + t_air) + 273.15;
    let scale = (tf / 293.15).powf(1.7);
    let nu = 1.5e-5 * scale;
    let alpha = 2.1e-5 * scale;
    let kf = 0.026;
    let ra = 9.81 / tf * dt * length_m.powi(3) / (nu * alpha);
    let nu_n = (0.825
        + 0.387 * ra.powf(1.0 / 6.0) / (1.0 + (0.492 / PR).powf(9.0 / 16.0)).powf(8.0 / 27.0))
    .powi(2);
    let h_n = nu_n * kf / length_m;
    let re = speed * length_m / nu;
    let nu_f = if re < 5.0e5 {
        0.664 * re.sqrt() * PR.powf(1.0 / 3.0)
    } else {
        0.037 * re.powf(0.8) * PR.powf(1.0 / 3.0)
    };
    let h_f = nu_f * kf / length_m;
    let h_c = (h_n.powi(3) + h_f.powi(3)).cbrt();
    let h_r = 4.0 * emissivity * SIGMA * tf.powi(3) * view;
    h_c + h_r
}

/// Update the surface coefficients from the current state.
pub fn update_device_h(s: &Setup, st: &mut State) {
    let (vx, vy) = flow::cell_velocity(s, &st.flow);
    for (k, d) in s.devices.iter().enumerate() {
        let t_air: f64 = d.air.iter().map(|&(i, w)| w * st.t_air[i]).sum();
        let speed: f64 = d.air.iter().map(|&(i, w)| w * vx[i].hypot(vy[i])).sum();
        let h = device_h(
            d.length_m,
            st.t_dev[k],
            t_air,
            speed,
            s.params.emissivity,
            s.params.radiation_view_factor,
        );
        st.h_dev[k] = 0.5 * st.h_dev[k] + 0.5 * h;
    }
}

/// Transient part of the assembly: step length and the previous state.
pub struct Step<'a> {
    pub dt: f64,
    pub prev: &'a State,
}

/// Assembles and solves the temperature system once for the flow in `st`.
/// `load` multiplies every device loss. Returns the largest temperature change.
pub fn solve_temperatures(s: &Setup, st: &mut State, load: f64, step: Option<&Step>) -> f64 {
    let (nx, ny, n) = (s.nx, s.ny, s.n());
    let np = s.plate_cells.len();
    let nd = s.devices.len();
    let size = n + np + nd;
    let rho_cp = s.air.rho_cp();
    let amb = s.ambient;
    let mut bld = Builder::new(size);
    let mut rhs = vec![0.0; size];
    let plate_row = |i: usize| s.plate_idx[i].map(|k| n + k);

    // Advection and diffusion between air cells.
    let kp = s.params.plate_k * s.params.plate_thickness_mm / 1000.0;
    for r in 0..ny {
        for c in 0..nx {
            let i = r * nx + c;
            for (east, j) in [(true, i + 1), (false, i + nx)] {
                if (east && c + 1 >= nx) || (!east && r + 1 >= ny) {
                    continue;
                }
                let (len, ds, q, speed) = if east {
                    (s.hy, s.hx, st.flow.qx[i], st.flow.sx[i])
                } else {
                    (s.hx, s.hy, st.flow.qy[i], st.flow.sy[i])
                };
                let phi = flow::face_open_fraction(s.block[i], s.block[j]);
                if phi >= 0.02 {
                    // Upwind advection.
                    let m = rho_cp * q.abs();
                    if q >= 0.0 {
                        bld.add(i, i, m);
                        bld.add(j, i, -m);
                    } else {
                        bld.add(j, j, m);
                        bld.add(i, j, -m);
                    }
                    // Conduction plus unresolved mixing.
                    let d = s.air.k + rho_cp * (s.params.mixing_m2s + 0.05 * speed * 0.05);
                    let g = d * len * s.depth * phi / ds;
                    bld.add(i, i, g);
                    bld.add(j, j, g);
                    bld.add(i, j, -g);
                    bld.add(j, i, -g);
                }
                // Plate conduction.
                if let (Some(pi), Some(pj)) = (plate_row(i), plate_row(j)) {
                    let f = 0.5 * (s.plate_frac[i] + s.plate_frac[j]);
                    let g = kp * len * f / ds;
                    bld.add(pi, pi, g);
                    bld.add(pj, pj, g);
                    bld.add(pi, pj, -g);
                    bld.add(pj, pi, -g);
                }
            }
        }
    }

    // Ports: out-flow carries the cell temperature, in-flow brings ambient.
    for (k, port) in s.ports.iter().enumerate() {
        let q = st.flow.port_q[k];
        if q > 0.0 {
            bld.add(port.cell, port.cell, rho_cp * q);
        } else {
            rhs[port.cell] += rho_cp * (-q) * amb;
        }
    }

    for i in 0..n {
        // Walls.
        let gw: f64 = s.g_wall[i].iter().sum();
        bld.add(i, i, gw);
        rhs[i] += gw * amb;
        // Air and plate.
        if let Some(pi) = plate_row(i) {
            let g = s.g_ap[i];
            bld.add(i, i, g);
            bld.add(i, pi, -g);
            bld.add(pi, pi, g);
            bld.add(pi, i, -g);
            let gp = s.g_pa[i];
            bld.add(pi, pi, gp);
            rhs[pi] += gp * amb;
        }
    }

    // Devices.
    for (k, d) in s.devices.iter().enumerate() {
        let row = n + np + k;
        let g_air = st.h_dev[k] * d.exposed_m2;
        let g_plate = s.params.plate_contact_h * d.footprint_m2 * d.plate_overlap;
        for &(i, w) in &d.air {
            bld.add(i, i, w * g_air);
            bld.add(i, row, -w * g_air);
            bld.add(row, row, w * g_air);
            bld.add(row, i, -w * g_air);
        }
        for &(i, w) in &d.plate {
            if let Some(pi) = plate_row(i) {
                bld.add(pi, pi, w * g_plate);
                bld.add(pi, row, -w * g_plate);
                bld.add(row, row, w * g_plate);
                bld.add(row, pi, -w * g_plate);
            }
        }
        rhs[row] += d.loss_w * load;
    }

    // Transient storage.
    if let Some(step) = step {
        let inv = 1.0 / step.dt;
        for i in 0..n {
            let c = s.c_air[i] * inv;
            bld.add(i, i, c);
            rhs[i] += c * step.prev.t_air[i];
            if let Some(pi) = plate_row(i) {
                let c = s.c_plate[i] * inv;
                bld.add(pi, pi, c);
                rhs[pi] += c * step.prev.t_plate[i];
            }
        }
        for (k, d) in s.devices.iter().enumerate() {
            let c = d.capacity * inv;
            bld.add(n + np + k, n + np + k, c);
            rhs[n + np + k] += c * step.prev.t_dev[k];
        }
    }

    let a = bld.finish();
    let mut x = vec![0.0; size];
    x[..n].copy_from_slice(&st.t_air);
    for (k, &i) in s.plate_cells.iter().enumerate() {
        x[n + k] = st.t_plate[i];
    }
    x[n + np..].copy_from_slice(&st.t_dev);
    linalg::bicgstab(&a, &rhs, &mut x, 1e-10, 3000);

    let mut change = 0.0_f64;
    for (t, new) in st.t_air.iter_mut().zip(&x[..n]) {
        change = change.max((new - *t).abs());
        *t = *new;
    }
    for (k, &i) in s.plate_cells.iter().enumerate() {
        change = change.max((x[n + k] - st.t_plate[i]).abs());
        st.t_plate[i] = x[n + k];
    }
    for k in 0..nd {
        change = change.max((x[n + np + k] - st.t_dev[k]).abs());
        st.t_dev[k] = x[n + np + k];
    }
    change
}

/// Steady state at the given load factor.
pub fn steady(s: &Setup, load: f64) -> State {
    steady_from(s, load, None)
}

/// Steady state, starting from a similar solved state when one is given (the
/// same grid, e.g. the central run for the uncertainty band runs).
pub fn steady_from(s: &Setup, load: f64, init: Option<&State>) -> State {
    let total: f64 = s.devices.iter().map(|d| d.loss_w).sum::<f64>() * load;
    let rise = if s.k_ae > 0.0 { total / s.k_ae } else { 0.0 };
    let n = s.n();
    let mut st = match init {
        Some(i) if i.t_air.len() == n && i.t_dev.len() == s.devices.len() => {
            let mut st = i.clone();
            st.t_buoy = st.t_air.clone();
            st.flow.reset_ports(s.ports.len());
            st.iterations = 0;
            st.converged = false;
            st
        }
        _ => {
            let mut st = State::uniform(s, s.ambient + rise.min(60.0));
            for k in 0..s.devices.len() {
                st.t_dev[k] = s.ambient + rise.min(60.0) + 10.0;
            }
            st
        }
    };
    let warm = init.is_some();
    // Fixed point between flow and temperature. The buoyancy driven flow can
    // swing between two states when a plume leans one way or the other, so the
    // temperature the flow sees is relaxed, and the relaxation is cut whenever
    // the change stops falling.
    let mut alpha = if warm { 0.3 } else { 0.5 };
    let mut previous = f64::INFINITY;
    for it in 0..s.params.max_iterations {
        let relax = if it == 0 && !warm { 1.0 } else { alpha };
        for i in 0..st.t_buoy.len() {
            st.t_buoy[i] += relax * (st.t_air[i] - st.t_buoy[i]);
        }
        let tb = st.t_buoy.clone();
        flow::solve(s, &tb, &mut st.flow, 15);
        update_device_h(s, &mut st);
        let change = solve_temperatures(s, &mut st, load, None);
        st.iterations = it + 1;
        if change < s.params.tolerance_k && it >= 2 {
            st.converged = true;
            break;
        }
        if it >= 2 && change > 0.7 * previous {
            alpha = (alpha * 0.6).max(0.03);
        }
        previous = change;
    }
    st
}

/// Energy balance quantities of a state, W.
pub struct Fluxes {
    pub loss: f64,
    pub faces: [f64; 6],
    pub openings: f64,
    pub storage: f64,
}

pub fn fluxes(s: &Setup, st: &State, load: f64, step: Option<&Step>) -> Fluxes {
    let n = s.n();
    let amb = s.ambient;
    let mut faces = [0.0; 6];
    for i in 0..n {
        for (f, face) in faces.iter_mut().enumerate() {
            *face += s.g_wall[i][f] * (st.t_air[i] - amb);
        }
        if s.plate_idx[i].is_some() {
            faces[crate::setup::REAR] += s.g_pa[i] * (st.t_plate[i] - amb);
        }
    }
    let rho_cp = s.air.rho_cp();
    let mut openings = 0.0;
    for (k, port) in s.ports.iter().enumerate() {
        let q = st.flow.port_q[k];
        if q > 0.0 {
            openings += rho_cp * q * (st.t_air[port.cell] - amb);
        }
    }
    let mut storage = 0.0;
    if let Some(step) = step {
        for i in 0..n {
            storage += s.c_air[i] * (st.t_air[i] - step.prev.t_air[i]) / step.dt;
            if s.plate_idx[i].is_some() {
                storage += s.c_plate[i] * (st.t_plate[i] - step.prev.t_plate[i]) / step.dt;
            }
        }
        for (k, d) in s.devices.iter().enumerate() {
            storage += d.capacity * (st.t_dev[k] - step.prev.t_dev[k]) / step.dt;
        }
    }
    let loss = s.devices.iter().map(|d| d.loss_w).sum::<f64>() * load;
    Fluxes {
        loss,
        faces,
        openings,
        storage,
    }
}

/// Plate share of a device's loss, from the converged state.
pub fn plate_share(s: &Setup, st: &State, k: usize) -> f64 {
    let d = &s.devices[k];
    let g_plate = s.params.plate_contact_h * d.footprint_m2 * d.plate_overlap;
    if d.loss_w <= 0.0 {
        return 0.0;
    }
    let tp: f64 = d.plate.iter().map(|&(i, w)| w * st.t_plate[i]).sum();
    let q = g_plate * (st.t_dev[k] - tp);
    (q / d.loss_w).clamp(0.0, 1.0)
}
