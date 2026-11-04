//! The coupled temperature system: air cells, plate cells and device nodes,
//! assembled as one sparse matrix and solved with BiCGSTAB. Steady and
//! implicit-Euler transient share the assembly.

use crate::flow::{self, Flow};
use crate::forced;
use crate::linalg::{self, Builder};
use crate::setup::{DeviceCells, Setup};

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
    /// The run stopped because the change stopped falling a little above the
    /// tolerance (see [`stalled`]). The answer may still move; not converged.
    pub stalled: bool,
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
            stalled: false,
        }
    }
}

/// Iterations over which a change that stays within three times the tolerance
/// and does not fall by a fifth counts as settled.
const STALL_WINDOW: usize = 8;

const SIGMA: f64 = 5.670_374e-8;
const PR: f64 = forced::PR;

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
    device_h_turbulent(length_m, t_surf, t_air, speed, 0.0, emissivity, view)
}

/// As [`device_h`], with turbulence intensity `tu` (0 to 1) in the stream
/// that lifts the forced part toward the turbulent plate value (see
/// [`crate::forced::plate_nusselt_with_turbulence`]).
pub fn device_h_turbulent(
    length_m: f64,
    t_surf: f64,
    t_air: f64,
    speed: f64,
    tu: f64,
    emissivity: f64,
    view: f64,
) -> f64 {
    let (h_c, tf) = convective_h(length_m, t_surf, t_air, speed, tu);
    let h_r = 4.0 * emissivity * SIGMA * tf.powi(3) * view;
    h_c + h_r
}

/// Convective part of the film (natural and forced) and the film temperature, K.
fn convective_h(length_m: f64, t_surf: f64, t_air: f64, speed: f64, tu: f64) -> (f64, f64) {
    let dt = (t_surf - t_air).abs().max(0.5);
    let tf = 0.5 * (t_surf + t_air) + 273.15;
    let (nu, alpha) = forced::air_nu_alpha(tf);
    let kf = forced::K_AIR;
    let ra = 9.81 / tf * dt * length_m.powi(3) / (nu * alpha);
    let nu_n = (0.825
        + 0.387 * ra.powf(1.0 / 6.0) / (1.0 + (0.492 / PR).powf(9.0 / 16.0)).powf(8.0 / 27.0))
    .powi(2);
    let h_n = nu_n * kf / length_m;
    let re = speed * length_m / nu;
    let h_f = forced::plate_nusselt_with_turbulence(re, tu) * kf / length_m;
    (forced::Mixed::default().combine(h_n, h_f), tf)
}

/// Update the surface coefficients from the current state.
pub fn update_device_h(s: &Setup, st: &mut State) {
    let (vx, vy) = flow::cell_velocity(s, &st.flow);
    for (k, d) in s.devices.iter().enumerate() {
        let t_air: f64 = d.air.iter().map(|&(i, w)| w * st.t_air[i]).sum();
        let local: f64 = d.air.iter().map(|&(i, w)| w * vx[i].hypot(vy[i])).sum();
        // The depth-averaged flow has no jet: the fans' stirring adds to it.
        let ((jx, jy), across, jet_h) = circulation_jets(s, st, d);
        let (lx, ly): (f64, f64) = d
            .air
            .iter()
            .fold((0.0, 0.0), |a, &(i, w)| (a.0 + w * vx[i], a.1 + w * vy[i]));
        let _ = local;
        let speed = (lx + jx).hypot(ly + jy).hypot(s.jet_speed_ms).hypot(across);
        let tu = if s.params.wake_turbulence {
            wake_tu(s, d, (lx + jx, ly + jy))
        } else {
            0.0
        };
        let mut h = device_h_turbulent(
            d.length_m,
            st.t_dev[k],
            t_air,
            speed,
            tu,
            s.params.emissivity,
            s.params.radiation_view_factor,
        );
        if let Some(hj) = jet_h {
            // A jet straight at the device: its stagnation film replaces the
            // plate forced part where it is higher.
            let tf = 0.5 * (st.t_dev[k] + t_air) + 273.15;
            let h_r =
                4.0 * s.params.emissivity * SIGMA * tf.powi(3) * s.params.radiation_view_factor;
            h = h.max(forced::Mixed::default().combine(h - h_r, hj) + h_r);
        }
        if let Some(sink) = &s.sinks[k] {
            // The fins replace the plain film over the footprint; their
            // surfaces see each other, so no radiation is credited to them.
            let f = sink.film(speed, st.t_dev[k], t_air);
            let foot = d.footprint_m2.min(d.exposed_m2);
            h = (f.h * f.area_gain * foot + h * (d.exposed_m2 - foot)) / d.exposed_m2.max(1e-9);
        }
        st.h_dev[k] = 0.5 * st.h_dev[k] + 0.5 * h;
    }
}

/// Air speed (m/s) that the jets of the circulation fans bring to a device,
/// and, for a fan blowing straight at the plate over the device, the film
/// coefficient of that impinging jet. Free-jet model: parts in the way do not
/// shade the device (see [`forced::round_jet_speed`], [`forced::jet_h`]).
fn circulation_jets(s: &Setup, st: &State, d: &DeviceCells) -> ((f64, f64), f64, Option<f64>) {
    use crate::model::BlowDirection as B;
    let depth_mm = s.depth * 1000.0;
    let (cx, cy) = (d.rect.x + 0.5 * d.rect.w, d.rect.y + 0.5 * d.rect.h);
    let mut jet = (0.0_f64, 0.0_f64);
    let mut across = 0.0_f64;
    let mut h_jet: Option<f64> = None;
    for (k, f) in s.params.circulation_fans.iter().enumerate() {
        let q = match f.direction {
            B::ToDoor | B::ToPlate => f.q_max_m3s() * 0.5,
            _ => flow::circulation_flow(s, &st.flow, k).max(0.0),
        };
        // Clear disc: about 80 % of the frame circle (hub and guard).
        let d_m = f.diameter_mm / 1000.0;
        let u0 = q / (0.8 * std::f64::consts::FRAC_PI_4 * d_m * d_m);
        if u0 <= 0.0 {
            continue;
        }
        let z_fan = f.z_mm.unwrap_or(0.5 * depth_mm);
        let dz = (z_fan - d.height_mm).abs();
        match f.direction {
            B::Right | B::Left | B::Up | B::Down => {
                let (ax, ay) = match f.direction {
                    B::Right => (1.0, 0.0),
                    B::Left => (-1.0, 0.0),
                    B::Down => (0.0, 1.0),
                    _ => (0.0, -1.0),
                };
                let (rx, ry) = (cx - f.x_mm, cy - f.y_mm);
                let along = rx * ax + ry * ay;
                let lateral = (rx * ay - ry * ax).abs();
                let half_along = 0.5 * (d.rect.w * ax.abs() + d.rect.h * ay.abs());
                let half_lat = 0.5 * (d.rect.w * ay.abs() + d.rect.h * ax.abs());
                let x = (along - half_along).max(0.5 * f.diameter_mm);
                if along <= 0.0 {
                    continue;
                }
                let r = (lateral - half_lat).max(0.0).hypot(dz);
                let u = forced::round_jet_speed(u0, f.diameter_mm, x, r);
                jet = (jet.0 + u * ax, jet.1 + u * ay);
            }
            B::ToPlate => {
                let h_mm = (z_fan - d.height_mm).max(0.0);
                let lateral = (cx - f.x_mm).hypot(cy - f.y_mm);
                let r_dev = (d.footprint_m2 / std::f64::consts::PI).sqrt();
                let tf = 0.5
                    * (st.t_air.iter().copied().sum::<f64>() / st.t_air.len() as f64 + s.ambient)
                    + 273.15;
                if lateral <= 0.5 * d.rect.w.min(d.rect.h)
                    && let Some(hj) = forced::jet_h(u0, d_m, r_dev, h_mm / 1000.0, tf)
                {
                    h_jet = Some(h_jet.map_or(hj, |v: f64| v.max(hj)));
                }
                let x = h_mm.max(0.5 * f.diameter_mm);
                let r = (lateral - 0.5 * d.rect.w.min(d.rect.h)).max(0.0);
                let u = forced::round_jet_speed(u0, f.diameter_mm, x, r);
                // Spreads over the plate away from the axis; straight under it
                // there is no direction, only speed.
                across = across.hypot(u);
                let (rx, ry) = (cx - f.x_mm, cy - f.y_mm);
                let n = rx.hypot(ry);
                if n > 1.0 {
                    jet = (jet.0 + 0.5 * u * rx / n, jet.1 + 0.5 * u * ry / n);
                }
            }
            B::ToDoor => {}
        }
    }
    (jet, across, h_jet)
}

/// Turbulence a device meets from a blocking part upstream of it: walk back
/// against the local flow to the first mostly blocked cell that is not the
/// device's own, take the width of that block across the flow as the wake
/// width and the walked distance as the distance behind it. Model (see
/// [`crate::forced::wake_turbulence`]).
fn wake_tu(s: &Setup, d: &DeviceCells, (ux, uy): (f64, f64)) -> f64 {
    let own: Vec<usize> = d.air.iter().map(|&(i, _)| i).collect();
    let speed = ux.hypot(uy);
    if speed < 0.02 {
        return 0.0;
    }
    let (dx, dy) = (-ux / speed, -uy / speed); // upstream, screen axes
    let (c0, r0) = {
        let i = d
            .air
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|a| a.0)
            .unwrap_or(0);
        ((i % s.nx) as f64, (i / s.nx) as f64)
    };
    let step = s.hx.min(s.hy);
    let reach = (0.4 / step).ceil() as usize;
    for k in 1..=reach {
        let (cx, cy) = (
            c0 + 0.5 + dx * k as f64 * step / s.hx,
            r0 + 0.5 + dy * k as f64 * step / s.hy,
        );
        if cx < 0.0 || cy < 0.0 || cx >= s.nx as f64 || cy >= s.ny as f64 {
            return 0.0;
        }
        let i = cy as usize * s.nx + cx as usize;
        if s.block[i] >= 0.5 && !own.contains(&i) {
            // Width of the block across the flow (perpendicular walk both ways).
            let (px, py) = (-dy, dx);
            let mut count = 1;
            for sign in [1.0, -1.0] {
                for m in 1..=40 {
                    let (qx, qy) = (
                        cx + sign * px * m as f64 * step / s.hx,
                        cy + sign * py * m as f64 * step / s.hy,
                    );
                    if qx < 0.0 || qy < 0.0 || qx >= s.nx as f64 || qy >= s.ny as f64 {
                        break;
                    }
                    if s.block[qy as usize * s.nx + qx as usize] >= 0.5 {
                        count += 1;
                    } else {
                        break;
                    }
                }
            }
            let width = count as f64 * step;
            return forced::wake_turbulence(0.0, k as f64 * step, width);
        }
    }
    0.0
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
                    let d = s.air.k
                        + rho_cp * (s.params.mixing_m2s + s.jet_mixing_m2s + 0.05 * speed * 0.05);
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
    for &(i, w) in &s.sources {
        rhs[i] += w * load;
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

/// Whether the change history has stalled: the latest change is within three
/// times the tolerance (0.03 K by default) and has not fallen by a fifth
/// over the last `STALL_WINDOW` iterations. More iterations then only repeat
/// the same wobble. Such a run is *not* converged: its answer can still move.
fn stalled(history: &[f64], tolerance_k: f64) -> bool {
    let Some(&change) = history.last() else {
        return false;
    };
    history.len() > STALL_WINDOW
        && change >= tolerance_k
        && change < 3.0 * tolerance_k
        && change > 0.8 * history[history.len() - 1 - STALL_WINDOW]
}

/// Steady state, starting from a similar solved state when one is given (the
/// same grid, e.g. the central run for the uncertainty band runs).
pub fn steady_from(s: &Setup, load: f64, init: Option<&State>) -> State {
    let total: f64 = s.total_loss_w() * load;
    let rise = if s.k_ae > 0.0 { total / s.k_ae } else { 0.0 };
    let n = s.n();
    let mut st = match init {
        Some(i) if i.t_air.len() == n && i.t_dev.len() == s.devices.len() => {
            let mut st = i.clone();
            st.t_buoy = st.t_air.clone();
            st.flow.reset_ports(s.ports.len());
            st.iterations = 0;
            st.converged = false;
            st.stalled = false;
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
    // The change can stop falling a little above the tolerance (the plume
    // and the relaxed flow trade places by a few hundredths of a kelvin). A
    // run whose change has stalled (see [`stalled`]) stops with `stalled`
    // set and `converged` false: 60 iterations took 25 s on a 60-device
    // cabinet and did not converge either.
    let mut history: Vec<f64> = Vec::new();
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
        history.push(change);
        if stalled(&history, s.params.tolerance_k) {
            st.stalled = true;
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
    let loss = s.total_loss_w() * load;
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

#[cfg(test)]
mod tests {
    use super::{STALL_WINDOW, stalled};

    #[test]
    fn a_change_that_stops_falling_just_above_tolerance_is_stalled() {
        let flat = vec![0.02; STALL_WINDOW + 1];
        assert!(stalled(&flat, 0.01));
    }

    #[test]
    fn a_falling_tiny_or_large_change_is_not_stalled() {
        // Still falling by more than a fifth.
        let falling: Vec<f64> = (0..=STALL_WINDOW)
            .map(|i| 0.03 * 0.8_f64.powi(i as i32))
            .collect();
        assert!(!stalled(&falling, 0.01));
        // Met the tolerance: converged, not stalled.
        assert!(!stalled(&[0.005; STALL_WINDOW + 1], 0.01));
        // Far from the tolerance.
        assert!(!stalled(&[0.5; STALL_WINDOW + 1], 0.01));
        // Too early.
        assert!(!stalled(&[0.02; 3], 0.01));
    }
}
