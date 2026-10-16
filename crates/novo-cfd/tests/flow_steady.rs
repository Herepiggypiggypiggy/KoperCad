//! The steady path (`steady_iterate`, `SteadyRun`) against the benchmarks the
//! time march is held to, and against the time march itself.
//!
//! Prints numbers; run with `-- --nocapture` to see them.

use novo_cfd::flow::*;
use novo_cfd::mesh::MeshSettings;
use novo_cfd::sched::{Threads, install};
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn stretched(n: usize, s: f64) -> Vec<f64> {
    (0..=n)
        .map(|i| 0.5 * (1.0 + (s * (2.0 * i as f64 / n as f64 - 1.0)).tanh() / s.tanh()))
        .collect()
}

fn run_to_end(flow: &mut FlowSolver, mut run: SteadyRun) -> SteadyReport {
    loop {
        if run.advance(flow, 50) != RunStatus::Running {
            return run.report();
        }
    }
}

// ---- lid driven cavity, Ghia, Ghia and Shin (1982) ------------------------

/// (y from the bottom, u) on the vertical centreline x = 0.5, Re 100.
const GHIA_U_100: [(f64, f64); 15] = [
    (0.0547, -0.03717),
    (0.0625, -0.04192),
    (0.0703, -0.04775),
    (0.1016, -0.06434),
    (0.1719, -0.10150),
    (0.2813, -0.15662),
    (0.4531, -0.21090),
    (0.5000, -0.20581),
    (0.6172, -0.13641),
    (0.7344, 0.00332),
    (0.8516, 0.23151),
    (0.9531, 0.68717),
    (0.9609, 0.73722),
    (0.9688, 0.78871),
    (0.9766, 0.84123),
];
/// (x, v) on the horizontal centreline y = 0.5 (v positive up), Re 100.
const GHIA_V_100: [(f64, f64); 15] = [
    (0.0625, 0.09233),
    (0.0703, 0.10091),
    (0.0781, 0.10890),
    (0.0938, 0.12317),
    (0.1563, 0.16077),
    (0.2266, 0.17507),
    (0.2344, 0.17527),
    (0.5000, 0.05454),
    (0.8047, -0.24533),
    (0.8594, -0.22445),
    (0.9063, -0.16914),
    (0.9453, -0.10313),
    (0.9531, -0.08864),
    (0.9609, -0.07391),
    (0.9688, -0.05906),
];

fn interp(pos: &[f64], val: &[f64], x: f64) -> f64 {
    let i = pos.partition_point(|p| *p < x).clamp(1, pos.len() - 1);
    let t = (x - pos[i - 1]) / (pos[i] - pos[i - 1]);
    val[i - 1] * (1.0 - t) + val[i] * t
}

fn lid_cavity(re: f64, n: usize, stretch: f64) -> FlowSolver {
    let mesh = Mesh::from_edges_m([stretched(n, stretch), stretched(n, stretch), vec![0.0, 0.1]]);
    let mut cfg = FlowConfig::air();
    cfg.nu = 1.0 / re;
    cfg.gravity = [0.0; 3];
    cfg.faces[Face::YMin as usize].wall_velocity = [1.0, 0.0, 0.0];
    cfg.faces[Face::ZMin as usize] = FaceBc::SYMMETRY;
    cfg.faces[Face::ZMax as usize] = FaceBc::SYMMETRY;
    FlowSolver::new(mesh, cfg, 0.0)
}

#[test]
fn lid_cavity_re100_steady_matches_ghia() {
    let n = 48;
    let mut s = lid_cavity(100.0, n, 1.0);
    let run = SteadyRun::new(
        &s,
        SteadyConfig::default(),
        SteadyLimits::default(),
        0.0,
        vec![],
    );
    let rep = run_to_end(&mut s, run);
    let m = &s.mesh;
    let ci = n / 2;
    let (mut ys, mut us) = (vec![0.0], vec![0.0]);
    for j in (0..n).rev() {
        ys.push(1.0 - m.centre[1][j]);
        us.push(s.vel[0][m.idx(0, [ci, j, 0])]);
    }
    ys.push(1.0);
    us.push(1.0);
    let (mut xs, mut vs) = (vec![0.0], vec![0.0]);
    for i in 0..n {
        xs.push(m.centre[0][i]);
        vs.push(-s.vel[1][m.idx(1, [i, ci, 0])]);
    }
    xs.push(1.0);
    vs.push(0.0);
    let worst = |table: &[(f64, f64)], pos: &[f64], val: &[f64]| {
        table
            .iter()
            .map(|(x, r)| (interp(pos, val, *x) - r).abs())
            .fold(0.0, f64::max)
    };
    let (eu, ev) = (worst(&GHIA_U_100, &ys, &us), worst(&GHIA_V_100, &xs, &vs));
    println!(
        "Re 100, 48x48 steady: u max err {eu:.4}, v max err {ev:.4}; {} iterations, converged {}, div {:.1e}",
        rep.iterations,
        rep.converged,
        s.divergence_max()
    );
    assert!(rep.converged && rep.iterations < 3000, "{rep:?}");
    assert!(s.divergence_max() < 1e-4);
    // the march is held to 0.02 here
    assert!(eu < 0.02 && ev < 0.02, "u {eu} v {ev}");
}

// ---- differentially heated cavity, de Vahl Davis (1983) ---------------------

/// Ra, mean Nusselt number, u_max on x = 0.5, v_max on y = 0.5 (units alpha / H).
const DE_VAHL_DAVIS: [(f64, f64, f64, f64); 2] =
    [(1e4, 2.243, 16.178, 19.617), (1e5, 4.519, 34.73, 68.59)];

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

fn heated_cavity(ra: f64, n: usize, stretch: f64) -> FlowSolver {
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

fn check_heated(ra: f64, n: usize, stretch: f64, tol_nu: f64, tol_vel: f64) {
    let pr = 0.71;
    let (_, nu_ref, u_ref, v_ref) = *DE_VAHL_DAVIS.iter().find(|r| r.0 == ra).unwrap();
    let mut s = heated_cavity(ra, n, stretch);
    let run = SteadyRun::new(
        &s,
        SteadyConfig::default(),
        SteadyLimits::default(),
        0.0,
        vec![],
    );
    let rep = run_to_end(&mut s, run);
    let m = &s.mesh;
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
    let (hot, cold) = (wall(0), wall(1));
    let scale = (ra * pr).sqrt();
    let ci = n / 2;
    let u_line: Vec<f64> = (0..n).map(|j| s.vel[0][m.idx(0, [ci, j, 0])]).collect();
    let v_line: Vec<f64> = (0..n).map(|i| -s.vel[1][m.idx(1, [i, ci, 0])]).collect();
    let (nu, u_max, v_max) = (
        0.5 * (hot + cold),
        peak(&u_line) * scale,
        peak(&v_line) * scale,
    );
    let (e_nu, e_u, e_v) = (
        (nu - nu_ref) / nu_ref,
        (u_max - u_ref) / u_ref,
        (v_max - v_ref) / v_ref,
    );
    println!(
        "Ra {ra:.0e}, {n}x{n} steady: Nu {nu:.3} ({:+.2}%), u_max {u_max:.2} ({:+.2}%), v_max {v_max:.2} ({:+.2}%), hot/cold mismatch {:.1e}, {} iterations",
        100.0 * e_nu,
        100.0 * e_u,
        100.0 * e_v,
        (hot - cold).abs() / hot,
        rep.iterations
    );
    assert!(rep.converged, "{rep:?}");
    assert!((hot - cold).abs() / hot < 0.01, "heat in and out differ");
    assert!(e_nu.abs() < tol_nu, "Nu error {e_nu}");
    assert!(e_u.abs() < tol_vel && e_v.abs() < tol_vel, "{e_u} {e_v}");
}

#[test]
fn heated_cavity_ra_1e4_steady() {
    // same bands as the time march test
    check_heated(1e4, 32, 1.3, 0.01, 0.03);
}

#[test]
fn heated_cavity_ra_1e5_steady() {
    check_heated(1e5, 40, 1.5, 0.02, 0.05);
}

// ---- a cabinet: steady against the time march --------------------------------

fn cabinet(cells: usize, scheme: Scheme) -> FlowSetup {
    cabinet_with(cells, scheme, false, false)
}

fn cabinet_with(cells: usize, scheme: Scheme, vents: bool, fan: bool) -> FlowSetup {
    let opening = |placement| Opening {
        placement,
        width_mm: 200.0,
        height_mm: 100.0,
    };
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 600.0,
            height_mm: 800.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 40.0,
                y: 40.0,
                w: 510.0,
                h: 700.0,
            },
        },
        ambient_c: 25.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: 200.0,
                y: 450.0,
                w: 150.0,
                h: 150.0,
            },
            height_mm: Some(100.0),
            loss_w: 50.0,
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: if vents || fan {
            let mut v = vec![Vent {
                opening: opening(Placement::Top { x_mm: 300.0 }),
                free_area_fraction: 0.6,
            }];
            if !fan {
                v.push(Vent {
                    opening: opening(Placement::Bottom { x_mm: 300.0 }),
                    free_area_fraction: 0.6,
                });
            }
            v
        } else {
            vec![]
        },
        fans: if fan {
            vec![Fan {
                opening: Opening {
                    placement: Placement::Bottom { x_mm: 300.0 },
                    width_mm: 120.0,
                    height_mm: 120.0,
                },
                mode: FanMode::Intake,
                free_flow_m3h: 100.0,
                max_pressure_pa: 40.0,
            }]
        } else {
            vec![]
        },
        params: Params::default(),
    };
    let st = SolidsSettings {
        scheme,
        mesh: MeshSettings {
            target_cells: cells,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    FlowSetup::new(&scn, &st).unwrap()
}

fn device_cells(setup: &FlowSetup) -> Vec<usize> {
    (0..setup.classified.grid.cells())
        .filter(|&c| {
            setup.classified.material[c] != 0
                && setup
                    .cfg
                    .medium
                    .as_ref()
                    .is_some_and(|m| m.heat_w_m3[c] > 0.0)
        })
        .collect()
}

fn mean_over(s: &FlowSolver, cells: &[usize]) -> f64 {
    cells.iter().map(|&c| s.t[c]).sum::<f64>() / cells.len() as f64
}

fn heat_out(setup: &FlowSetup, s: &FlowSolver) -> f64 {
    (0..6).map(|f| s.wall_heat_w(f)).sum::<f64>() + s.open_heat_out_w(setup.ambient_c)
}

/// Steady, first order then second order, against the time march at the same
/// mesh and scheme.
#[test]
fn small_cabinet_steady_agrees_with_the_march() {
    let setup = cabinet(700, Scheme::VanLeer);
    let dev = device_cells(&setup);
    let mut march = setup.solver();
    let t0 = std::time::Instant::now();
    let m = setup.run_steady(
        &mut march,
        1e-3,
        (2000.0, 200_000),
        &novo_cfd::Control::default(),
        |_, _| {},
        0,
    );
    let t_march = t0.elapsed().as_secs_f64();
    assert!(m.converged, "{m:?}");

    let mut s = setup.solver();
    let t0 = std::time::Instant::now();
    let run = SteadyRun::staged(
        &s,
        vec![SteadyConfig::first_order(), SteadyConfig::second_order()],
        SteadyLimits::default(),
        setup.ambient_c,
        dev.clone(),
    );
    let rep = run_to_end(&mut s, run);
    let t_steady = t0.elapsed().as_secs_f64();
    let (dm, ds) = (mean_over(&march, &dev), mean_over(&s, &dev));
    let (am, as_) = (march.mean_temperature(), s.mean_temperature());
    let rise = dm - setup.ambient_c;
    println!(
        "{} cells: march {} steps {t_march:.1} s, device {dm:.2} C, air {am:.2} C | steady {} iterations {t_steady:.1} s, device {ds:.2} C ({:+.2}%), air {as_:.2} C, imbalance {:+.2}%",
        setup.mesh.cells(),
        m.steps,
        rep.iterations,
        100.0 * (ds - dm) / rise,
        100.0 * rep.imbalance
    );
    assert!(rep.converged, "{rep:?}");
    assert!(rep.imbalance.abs() < 0.01);
    assert!((heat_out(&setup, &s) - setup.heat_w).abs() < 0.015 * setup.heat_w);
    // same equations and scheme: the answers agree within the wobble and the
    // march's own stopping tolerance
    assert!((ds - dm).abs() < 0.02 * rise, "device {ds} against {dm}");
    assert!(
        (as_ - am).abs() < 0.02 * (am - setup.ambient_c),
        "air {as_} against {am}"
    );
}

/// Steady against the march on a cabinet with open faces: two vents (the
/// stack effect) and a fan with a top vent (a forced jet).
fn open_faces_case(vents: bool, fan: bool) {
    let setup = cabinet_with(900, Scheme::VanLeer, vents, fan);
    let dev = device_cells(&setup);
    let mut march = setup.solver();
    let m = setup.run_steady(
        &mut march,
        1e-3,
        (2000.0, 200_000),
        &novo_cfd::Control::default(),
        |_, _| {},
        0,
    );
    assert!(m.converged, "{m:?}");
    let mut s = setup.solver();
    let run = SteadyRun::staged(
        &s,
        vec![SteadyConfig::first_order(), SteadyConfig::second_order()],
        SteadyLimits::default(),
        setup.ambient_c,
        dev.clone(),
    );
    let rep = run_to_end(&mut s, run);
    let (dm, ds) = (mean_over(&march, &dev), mean_over(&s, &dev));
    let rise = dm - setup.ambient_c;
    let (om, os) = (
        march.open_heat_out_w(setup.ambient_c),
        s.open_heat_out_w(setup.ambient_c),
    );
    println!(
        "vents {vents} fan {fan}: march {} steps, device {dm:.2} C, openings {om:.2} W | steady {} iterations, device {ds:.2} C ({:+.2}% of the rise), openings {os:.2} W, imbalance {:+.2}%",
        m.steps,
        rep.iterations,
        100.0 * (ds - dm) / rise,
        100.0 * rep.imbalance
    );
    assert!(rep.converged, "{rep:?}");
    assert!((ds - dm).abs() < 0.03 * rise, "device {ds} against {dm}");
    assert!(
        (os - om).abs() < 0.05 * setup.heat_w,
        "openings {os} against {om}"
    );
    assert!((heat_out(&setup, &s) - setup.heat_w).abs() < 0.03 * setup.heat_w);
}

#[test]
fn vented_cabinet_steady_agrees_with_the_march() {
    open_faces_case(true, false);
}

#[test]
fn fan_cooled_cabinet_steady_agrees_with_the_march() {
    open_faces_case(false, true);
}

/// A coarse solution on a finer mesh: linear fields come out exactly, and the
/// fine run starts closer to its answer than a cold one.
#[test]
fn a_coarse_solution_starts_the_finer_mesh() {
    let make = |n: usize| {
        let mesh = Mesh::uniform([n, n, 2], [1.0, 1.0, 0.1]);
        let mut cfg = FlowConfig::air();
        cfg.gravity = [0.0; 3];
        FlowSolver::new(mesh, cfg, 0.0)
    };
    let mut coarse = make(6);
    for k in 0..2 {
        for j in 0..6 {
            for i in 0..6 {
                let (x, y) = (coarse.mesh.centre[0][i], coarse.mesh.centre[1][j]);
                let id = coarse.mesh.cell([i, j, k]);
                coarse.t[id] = 3.0 * x - 2.0 * y + 1.0;
                coarse.p[id] = x + y;
            }
        }
    }
    let mut fine = make(15);
    fine.initialise_from(&coarse);
    let m = &fine.mesh;
    // away from the border (constant beyond the coarse centres) a linear field is exact
    for j in 3..12 {
        for i in 3..12 {
            let (x, y) = (m.centre[0][i], m.centre[1][j]);
            let id = m.cell([i, j, 0]);
            assert!((fine.t[id] - (3.0 * x - 2.0 * y + 1.0)).abs() < 1e-12);
            assert!((fine.p[id] - (x + y)).abs() < 1e-12);
        }
    }
}

#[test]
fn first_order_cabinet_converges_to_a_fixed_point() {
    let setup = cabinet(700, Scheme::VanLeer);
    let mut s = setup.solver();
    let run = SteadyRun::new(
        &s,
        SteadyConfig::first_order(),
        SteadyLimits::default(),
        setup.ambient_c,
        device_cells(&setup),
    );
    let rep = run_to_end(&mut s, run);
    println!(
        "first order, {} cells: {} iterations, imbalance {:+.3}%, wobble {:.3}%",
        setup.mesh.cells(),
        rep.iterations,
        100.0 * rep.imbalance,
        100.0 * rep.wobble
    );
    assert!(rep.converged && rep.iterations < 1500, "{rep:?}");
    assert!(rep.imbalance.abs() < 0.005);
    assert!(rep.wobble < 0.005, "first order must not wobble");
    assert!(s.divergence_max() < 1e-4);
}

/// Assembly and the pressure solve give the same bits on any thread count.
#[test]
fn steady_iterations_do_not_depend_on_the_thread_count() {
    let checksum = |threads: usize| -> u64 {
        install(Threads::Fixed(threads), || {
            let mut s = heated_cavity(1e4, 24, 1.3);
            let mut st = SteadyState::new(&s, SteadyConfig::default());
            for _ in 0..40 {
                s.steady_iterate(&mut st);
            }
            s.vel
                .iter()
                .flatten()
                .chain(s.t.iter())
                .chain(s.p.iter())
                .fold(0u64, |h, v| h.rotate_left(5) ^ v.to_bits())
        })
    };
    assert_eq!(checksum(1), checksum(4));
}
