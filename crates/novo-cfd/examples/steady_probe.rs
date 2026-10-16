//! Steady path against the time march on the benchmark cavities.
//! `cargo run --release -p novo-cfd --example steady_probe -- lid|heated`
use novo_cfd::flow::*;
use std::time::Instant;

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

fn lid(re: f64, n: usize, stretch: f64) -> FlowSolver {
    let mesh = Mesh::from_edges_m([stretched(n, stretch), stretched(n, stretch), vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = 1.0 / re;
    cfg.gravity = [0.0; 3];
    cfg.faces[Face::YMin as usize].wall_velocity = [1.0, 0.0, 0.0];
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    FlowSolver::new(mesh, cfg, 0.0)
}

fn heated(ra: f64, n: usize, stretch: f64) -> FlowSolver {
    let pr = 0.71;
    let mesh = Mesh::from_edges_m([stretched(n, stretch), stretched(n, stretch), vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = (pr / ra).sqrt();
    cfg.alpha = 1.0 / (ra * pr).sqrt();
    cfg.beta = 1.0;
    cfg.gravity = [0.0, 1.0, 0.0];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(0.5);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(-0.5);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    FlowSolver::new(mesh, cfg, 0.0)
}

fn centre_u(s: &FlowSolver, n: usize) -> Vec<f64> {
    (0..n)
        .map(|j| s.vel[0][s.mesh.idx(0, [n / 2, j, 0])])
        .collect()
}

fn peak(values: &[f64]) -> f64 {
    let (i, &m) = values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();
    if i == 0 || i + 1 == values.len() {
        return m;
    }
    let (a, b, c) = (values[i - 1], values[i], values[i + 1]);
    let den = a - 2.0 * b + c;
    if den.abs() < 1e-30 {
        b
    } else {
        b - 0.125 * (c - a).powi(2) / den
    }
}

/// (Nu hot, Nu cold, u_max, v_max) of the differentially heated cavity.
fn heated_metrics(s: &FlowSolver, ra: f64, n: usize) -> (f64, f64, f64, f64) {
    let m = &s.mesh;
    let scale = (ra * 0.71).sqrt();
    let wall = |x_side: usize| -> f64 {
        let (i0, i1, sign, wall_x) = if x_side == 0 {
            (0, 1, 1.0, 0.0)
        } else {
            (n - 1, n - 2, -1.0, 1.0)
        };
        let tw = if x_side == 0 { 0.5 } else { -0.5 };
        let (h0, h1) = (
            (m.centre[0][i0] - wall_x).abs(),
            (m.centre[0][i1] - wall_x).abs(),
        );
        let mut q = 0.0;
        for j in 0..n {
            let (d0, d1) = (s.t[m.cell([i0, j, 0])] - tw, s.t[m.cell([i1, j, 0])] - tw);
            let a = (d0 * h1 * h1 - d1 * h0 * h0) / (h0 * h1 * (h1 - h0));
            q += -sign * a * m.dx[1][j];
        }
        q.abs()
    };
    let ci = n / 2;
    let u_line: Vec<f64> = (0..n).map(|j| s.vel[0][m.idx(0, [ci, j, 0])]).collect();
    let v_line: Vec<f64> = (0..n).map(|i| -s.vel[1][m.idx(1, [i, ci, 0])]).collect();
    (
        wall(0),
        wall(1),
        peak(&u_line) * scale,
        peak(&v_line) * scale,
    )
}

fn main() {
    let case = std::env::args().nth(1).unwrap_or("lid".into());
    let n: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(48);
    let cfl: f64 = std::env::args()
        .nth(3)
        .and_then(|v| v.parse().ok())
        .unwrap_or(10.0);
    let make = || match case.as_str() {
        "lid" => lid(100.0, n, 1.0),
        "lid1000" => lid(1000.0, n, 1.3),
        "heated4" => heated(1e4, n, 1.3),
        _ => heated(1e5, n, 1.5),
    };
    // reference: the time march
    let mut a = make();
    let t0 = Instant::now();
    let mut steps = 0;
    loop {
        let dt = a
            .max_dt(0.4)
            .min(if case.starts_with("lid") { 0.02 } else { 2.0 });
        let info = a.step(dt);
        steps += 1;
        let scale = 1.0;
        if steps > 100 && info.max_accel * scale < 2e-5 && info.max_dtdt < 1e-8 || steps > 40000 {
            break;
        }
    }
    let tm = t0.elapsed().as_secs_f64();
    println!("march: {steps} steps, {tm:.2} s");

    let mut b = make();
    let cfg = SteadyConfig {
        cfl,
        ..SteadyConfig::default()
    };
    let mut st = SteadyState::new(&b, cfg);
    let t0 = Instant::now();
    let mut it = 0;
    loop {
        let info = b.steady_iterate(&mut st);
        it += 1;
        if it % 20 == 0 || it < 5 {
            println!(
                "it {it} dt {:.3} du {:.2e} dT {:.2e} cont {:.2e} pit {} esw {}",
                info.dt,
                info.max_du,
                info.max_dt_k,
                info.continuity,
                info.pressure_iterations,
                info.energy_sweeps
            );
        }
        if (info.max_du < 1e-7 && info.max_dt_k < 1e-7) || it >= 3000 {
            break;
        }
    }
    let ts = t0.elapsed().as_secs_f64();
    let (ua, ub) = (centre_u(&a, n), centre_u(&b, n));
    let diff = ua
        .iter()
        .zip(&ub)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max);
    let umax = ua.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let tdiff =
        a.t.iter()
            .zip(&b.t)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0, f64::max);
    if case.starts_with("heated") {
        let ra = if case == "heated4" { 1e4 } else { 1e5 };
        println!("march  Nu hot/cold/u/v: {:?}", heated_metrics(&a, ra, n));
        println!("steady Nu hot/cold/u/v: {:?}", heated_metrics(&b, ra, n));
    }
    println!(
        "steady: {it} iterations, {ts:.2} s; centreline u max diff {diff:.2e} (umax {umax:.3}); T max diff {tdiff:.2e}; div {:.1e}",
        b.divergence_max()
    );
}
