//! Red-black smoother against the lexicographic one (lane CFD-E): same
//! answers; convergence within 30 percent (it is a little slower on strongly
//! stretched meshes, a little faster on uniform ones). Prints the numbers
//! (`-- --nocapture`).

use std::time::Instant;

use novo_cfd::flow::*;
use novo_cfd::sched::{Threads, install};

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

#[test]
fn poisson_same_solution_and_no_more_iterations() {
    let pi = std::f64::consts::PI;
    for (name, mesh) in [
        ("uniform 32^3", Mesh::uniform([32, 32, 32], [1.0; 3])),
        (
            "stretched 24x40x16",
            Mesh::from_edges_m([stretched(24, 1.5), stretched(40, 2.0), stretched(16, 1.0)]),
        ),
    ] {
        let n = mesh.n;
        let mut b = vec![0.0; mesh.cells()];
        for k in 0..n[2] {
            for j in 0..n[1] {
                for i in 0..n[0] {
                    let ix = [i, j, k];
                    let (x, y, z) = (mesh.centre[0][i], mesh.centre[1][j], mesh.centre[2][k]);
                    let f = (pi * x).cos() * (pi * y).cos() * (pi * z).cos()
                        + 0.3 * (2.0 * pi * x).cos();
                    b[mesh.cell(ix)] = f * mesh.volume(ix);
                }
            }
        }
        let mean = b.iter().sum::<f64>() / b.len() as f64;
        b.iter_mut().for_each(|v| *v -= mean);
        let mut out = Vec::new();
        for sm in [Smoother::Lexicographic, Smoother::RedBlack] {
            let mut solver = PressureSolver::new(&mesh);
            solver.set_smoother(sm);
            let mut p = vec![0.0; mesh.cells()];
            let iters = solver.solve(&b, &mut p, 1e-12, 200);
            println!(
                "{name} {sm:?}: {iters} iterations, residual {:.1e}",
                solver.last_residual
            );
            out.push((iters, p));
        }
        let scale = out[0].1.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
        let diff = out[0]
            .1
            .iter()
            .zip(&out[1].1)
            .fold(0.0_f64, |a, (x, y)| a.max((x - y).abs()));
        println!(
            "{name}: max difference {:.2e} (solution scale {scale:.2e})",
            diff / scale
        );
        assert!(diff / scale < 1e-9, "solutions differ");
        assert!(
            out[1].0 as f64 <= 1.3 * out[0].0 as f64 + 3.0,
            "red-black needs {} iterations against {}",
            out[1].0,
            out[0].0
        );
    }
}

struct Run {
    nu: f64,
    steps: usize,
    pressure_iterations: usize,
    secs: f64,
}

fn cavity(ra: f64, n: usize, sm: Smoother) -> Run {
    let pr = 0.71;
    let e = stretched(n, 1.5);
    let mesh = Mesh::from_edges_m([e.clone(), e, vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = (pr / ra).sqrt();
    cfg.alpha = 1.0 / (ra * pr).sqrt();
    cfg.beta = 1.0;
    cfg.gravity = [0.0, 1.0, 0.0];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(0.5);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(-0.5);
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    let mut s = FlowSolver::new(mesh, cfg, 0.0);
    s.set_pressure_smoother(sm);
    let scale = (ra * pr).sqrt();
    let t0 = Instant::now();
    let (mut steps, mut settled, mut pit) = (0, 0, 0);
    while steps < 40_000 && settled < 20 {
        let dt = s.max_dt(0.4).min(2.0);
        let info = s.step(dt);
        steps += 1;
        pit += info.pressure_iterations;
        if steps > 50 && info.max_accel * scale < 1e-6 && info.max_dtdt < 1e-8 {
            settled += 1;
        } else {
            settled = 0;
        }
    }
    // Hot wall Nusselt number: conductive flux from the first cells.
    let m = &s.mesh;
    let mut q = 0.0;
    for j in 0..n {
        let (h0, h1) = (m.centre[0][0], m.centre[0][1]);
        let (d0, d1) = (s.t[m.cell([0, j, 0])] - 0.5, s.t[m.cell([1, j, 0])] - 0.5);
        let g = (d0 * h1 * h1 - d1 * h0 * h0) / (h0 * h1 * (h1 - h0));
        q += g.abs() * m.dx[1][j];
    }
    Run {
        nu: q,
        steps,
        pressure_iterations: pit,
        secs: t0.elapsed().as_secs_f64(),
    }
}

#[test]
fn heated_cavity_same_answer_and_convergence() {
    for ra in [1e4, 1e5] {
        let lex = install(Threads::Fixed(1), || {
            cavity(ra, 32, Smoother::Lexicographic)
        });
        let rb = install(Threads::Fixed(3), || cavity(ra, 32, Smoother::RedBlack));
        println!(
            "Ra {ra:e}: Nu {:.5} (lex, {} steps, {} p-iters, {:.1}s) vs {:.5} (red-black, {} steps, {} p-iters, {:.1}s)",
            lex.nu,
            lex.steps,
            lex.pressure_iterations,
            lex.secs,
            rb.nu,
            rb.steps,
            rb.pressure_iterations,
            rb.secs
        );
        assert!((lex.nu - rb.nu).abs() / lex.nu < 1e-4, "Nusselt differs");
        assert!(
            rb.pressure_iterations as f64 <= 1.15 * lex.pressure_iterations as f64,
            "red-black needs more pressure iterations"
        );
    }
}
