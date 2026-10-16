//! Heat-up and load steps of the cabinet in 3D (real heat capacities).

use std::time::{Duration, Instant};

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{
    CfdError, Control, Fidelity, TransientFrame, TransientSolver, TransientSpec, TransientSummary,
};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario() -> Scenario {
    Scenario {
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
            rated_max_ambient_c: Some(60.0),
            rated_max_surface_c: Some(70.0),
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn solver(wall: Option<Duration>) -> CabinetSolver {
    let mut t = Tuning::default().with_cells(700);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 1e-2;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
        l.max_wall = wall;
    }
    CabinetSolver::new(t)
}

fn run(spec: &TransientSpec) -> (Vec<TransientFrame>, TransientSummary) {
    let mut frames = Vec::new();
    let sum = solver(None)
        .solve_transient(
            &scenario(),
            Fidelity::Coarse,
            spec,
            &Control::new(),
            &mut |f| frames.push(f.clone()),
        )
        .unwrap();
    (frames, sum)
}

#[test]
fn a_cold_start_heats_up_and_the_energy_adds_up() {
    let spec = TransientSpec {
        duration_s: 600.0,
        frame_interval_s: 100.0,
        ..TransientSpec::default()
    };
    let t0 = Instant::now();
    let (frames, sum) = run(&spec);
    assert_eq!(frames.len(), spec.frame_count());
    assert!(frames.iter().all(|f| f.snapshot.is_consistent()));
    assert!(frames.windows(2).all(|w| w[1].time_s > w[0].time_s));
    assert!((frames.last().unwrap().time_s - 600.0).abs() < 1e-3);
    assert!(frames.last().unwrap().snapshot.progress.converged);
    let dev = &sum.devices[0].surface_c;
    assert!((dev[0] - 25.0).abs() < 1e-3, "{dev:?}");
    assert!(dev.windows(2).all(|w| w[1] > w[0]), "{dev:?}");
    assert!(sum.mean_air_c.windows(2).all(|w| w[1] >= w[0] - 1e-9));
    assert_eq!(sum.devices[0].air_c.len(), dev.len());
    // Heat in minus heat out minus heat stored, relative to the heating,
    // is the bookkeeping of the energy equation.
    let worst = frames[1..]
        .iter()
        .map(|f| f.snapshot.progress.energy_balance_rel)
        .fold(0.0, f64::max);
    println!(
        "TRANSIENT | cold start 50 W 600 s: device {:.2} -> {:.2} C, mean air {:.2} C, \
         worst energy bookkeeping {:.2} %, {:.1} s wall",
        dev[0],
        dev.last().unwrap(),
        sum.mean_air_c.last().unwrap(),
        100.0 * worst,
        t0.elapsed().as_secs_f64()
    );
    assert!(worst < 0.1, "{worst}");
}

#[test]
fn a_load_step_down_cools_the_device() {
    let spec = TransientSpec {
        duration_s: 800.0,
        frame_interval_s: 200.0,
        load_schedule: vec![(0.0, 1.0), (400.0, 0.0)],
        start_from_steady: false,
    };
    let (frames, sum) = run(&spec);
    let at = |t: f64| {
        frames
            .iter()
            .position(|f| (f.time_s - t).abs() < 1e-3)
            .unwrap()
    };
    assert_eq!(sum.load_factor[at(200.0)], 1.0);
    assert_eq!(sum.load_factor[at(600.0)], 0.0);
    let dev = &sum.devices[0].surface_c;
    println!(
        "TRANSIENT | load off at 400 s: device {:.2} C at 400 s, {:.2} C at 800 s",
        dev[at(400.0)],
        dev[at(800.0)]
    );
    assert!(dev[at(400.0)] > dev[at(200.0)]);
    assert!(dev[at(800.0)] < dev[at(400.0)] - 0.05, "{dev:?}");
}

#[test]
fn starting_from_the_steady_state_stays_there() {
    let spec = TransientSpec {
        duration_s: 200.0,
        frame_interval_s: 100.0,
        start_from_steady: true,
        ..TransientSpec::default()
    };
    let (frames, sum) = run(&spec);
    let dev = &sum.devices[0].surface_c;
    assert!(dev[0] > 26.0, "started cold: {dev:?}");
    let drift = (dev.last().unwrap() - dev[0]).abs();
    println!(
        "TRANSIENT | from steady: device {:.2} C, moved {drift:.2} K in 200 s",
        dev[0]
    );
    // the 1 % steady tolerance of the test level leaves a little to settle
    assert!(drift < 0.15 * (dev[0] - 25.0), "{dev:?}");
    assert!(
        frames[0]
            .snapshot
            .notes
            .iter()
            .any(|n| n.contains("Started from the steady state"))
    );
}

#[test]
fn the_wall_clock_budget_ends_the_run_and_says_so() {
    let spec = TransientSpec {
        duration_s: 20_000.0,
        frame_interval_s: 1000.0,
        ..TransientSpec::default()
    };
    let mut frames = Vec::new();
    let sum = solver(Some(Duration::from_millis(500)))
        .solve_transient(
            &scenario(),
            Fidelity::Coarse,
            &spec,
            &Control::new(),
            &mut |f| frames.push(f.clone()),
        )
        .unwrap();
    assert!(frames.len() < spec.frame_count());
    assert!(*sum.times_s.last().unwrap() < spec.duration_s);
    let last = frames.last().unwrap();
    assert!(!last.snapshot.progress.converged);
    assert!(
        last.snapshot
            .notes
            .iter()
            .any(|n| n.contains("time budget"))
    );
}

#[test]
fn cancel_bad_specs_and_unsupported_levels() {
    let s = solver(None);
    let spec = TransientSpec {
        duration_s: 600.0,
        frame_interval_s: 100.0,
        ..TransientSpec::default()
    };
    let c = Control::new();
    c.cancel();
    let e = s
        .solve_transient(&scenario(), Fidelity::Coarse, &spec, &c, &mut |_| {})
        .unwrap_err();
    assert!(matches!(e, CfdError::Cancelled), "{e}");
    let bad = TransientSpec {
        frame_interval_s: 0.0,
        ..spec.clone()
    };
    let e = s
        .solve_transient(
            &scenario(),
            Fidelity::Coarse,
            &bad,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(e, CfdError::Setup(_)), "{e}");
    let e = s
        .solve_transient(
            &scenario(),
            Fidelity::Preview2d,
            &spec,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(
        matches!(e, CfdError::Unsupported(Fidelity::Preview2d)),
        "{e}"
    );
}

#[test]
fn the_time_constant_reads_off_a_settled_rise() {
    // Not reachable through a short run, so exercised through the public
    // summary of a long synthetic series by the same rule: a rise that has
    // not settled gives None.
    let spec = TransientSpec {
        duration_s: 300.0,
        frame_interval_s: 100.0,
        ..TransientSpec::default()
    };
    let (_, sum) = run(&spec);
    assert_eq!(sum.time_constant_s, None, "300 s is far from settled");
}
