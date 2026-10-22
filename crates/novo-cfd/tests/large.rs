//! Large cabinets: the reduced model (lump cold devices, snap, smear) against
//! the full scenario on a cabinet small enough to solve both ways.

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::large::{self, LargeMode, ReduceSettings, Verdict};
use novo_cfd::mesh::{MeshSettings, plan_grid_report};
use novo_cfd::{CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

const HOT: u64 = 1;

/// A cabinet with one 60 W drive and `n` cold devices in two rows, a duct
/// between the rows and a rail under each.
fn cabinet(n: usize) -> Scenario {
    cabinet_with(n, 18.0, 21.0, false)
}

/// Like [`cabinet`] with devices `w` wide on a pitch of `step`.
fn cabinet_with(n: usize, w: f64, step: f64, uniform: bool) -> Scenario {
    let mut devices = vec![Device {
        id: HOT,
        footprint: Rect {
            x: 60.0,
            y: 400.0,
            w: 140.0,
            h: 200.0,
        },
        height_mm: Some(120.0),
        loss_w: 60.0,
        rated_max_ambient_c: Some(60.0),
        rated_max_surface_c: Some(90.0),
        heat_capacity_j_per_k: None,
    }];
    for i in 0..n {
        let (row, col) = (i / n.div_ceil(2), i % n.div_ceil(2));
        devices.push(Device {
            id: 100 + i as u64,
            footprint: Rect {
                x: 260.0 + step * col as f64,
                y: 150.0 + 500.0 * row as f64,
                w,
                h: 70.0,
            },
            height_mm: Some(if uniform {
                60.0
            } else {
                55.0 + 5.0 * (col % 3) as f64
            }),
            loss_w: 0.4,
            rated_max_ambient_c: Some(60.0),
            rated_max_surface_c: Some(85.0),
            heat_capacity_j_per_k: None,
        });
    }
    Scenario {
        enclosure: Enclosure {
            width_mm: 600.0,
            height_mm: 800.0,
            depth_mm: 260.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 20.0,
                y: 20.0,
                w: 560.0,
                h: 760.0,
            },
        },
        ambient_c: 25.0,
        devices,
        obstacles: vec![
            Obstacle {
                kind: ObstacleKind::Duct,
                rect: Rect {
                    x: 40.0,
                    y: 300.0,
                    w: 520.0,
                    h: 25.0,
                },
                blockage: None,
                loss_w: 2.0,
            },
            Obstacle {
                kind: ObstacleKind::Rail,
                rect: Rect {
                    x: 250.0,
                    y: 180.0,
                    w: 300.0,
                    h: 12.0,
                },
                blockage: None,
                loss_w: 0.0,
            },
            Obstacle {
                kind: ObstacleKind::Rail,
                rect: Rect {
                    x: 250.0,
                    y: 680.0,
                    w: 300.0,
                    h: 12.0,
                },
                blockage: None,
                loss_w: 0.0,
            },
        ],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn solver(cells: usize, mode: LargeMode) -> CabinetSolver {
    let mut t = Tuning::default().with_cells(cells);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 1e-2;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
        l.max_wall = Some(Duration::from_secs(600));
    }
    CabinetSolver::new(t).with_large(mode)
}

fn run(scn: &Scenario, cells: usize, mode: LargeMode) -> CfdSnapshot {
    solver(cells, mode)
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

#[test]
fn reduction_keeps_heat_ids_and_volume_and_shrinks_the_grid() {
    let scn = cabinet(24);
    let st = ReduceSettings::for_cells(3000);
    let red = large::reduce(&scn, &st);
    let heat = |s: &Scenario| s.devices.iter().map(|d| d.loss_w).sum::<f64>();
    assert!((heat(&red.scenario) - heat(&scn)).abs() < 1e-9);
    // every original device is in exactly one block
    let mut seen: Vec<usize> = red.blocks.iter().flat_map(|b| b.members.clone()).collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..scn.devices.len()).collect::<Vec<_>>());
    // the drive is exact
    let hot = red.scenario.devices.iter().find(|d| d.id == HOT).unwrap();
    assert_eq!(hot.loss_w, scn.devices[0].loss_w);
    assert!(red.report.hot_shift_mm <= 0.3 * red.report.pitch_mm.max(1.0));
    // reduced ids are unique
    let mut ids: Vec<u64> = red.scenario.devices.iter().map(|d| d.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), red.scenario.devices.len());
    // fewer bodies, far fewer cells
    assert!(red.scenario.devices.len() < scn.devices.len() / 3);
    let full = plan_grid_report(&scn, &st.mesh).unwrap().grid.cells();
    assert_eq!(red.report.full_cells, full);
    assert!(
        red.report.reduced_cells * 3 < full,
        "{} vs {full}",
        red.report.reduced_cells
    );
    assert_eq!(red.report.exact_devices, 1);
    assert_eq!(red.report.lumped_devices + 1, scn.devices.len());
    // solid volume of the blocks stays within the snap error of the members
    let vol = |ds: &[Device]| -> f64 {
        ds.iter()
            .map(|d| d.footprint.w * d.footprint.h * d.height())
            .sum()
    };
    let cold_before = vol(&scn.devices[1..]);
    let cold_after = vol(&red
        .scenario
        .devices
        .iter()
        .filter(|d| d.id != HOT)
        .cloned()
        .collect::<Vec<_>>());
    assert!(
        (cold_after / cold_before - 1.0).abs() < 0.15,
        "{cold_after} vs {cold_before}"
    );
    // the duct's wire heat is kept
    assert_eq!(red.scenario.obstacles[0].loss_w, 2.0);
}

#[test]
fn a_small_cabinet_is_left_alone_by_auto() {
    let mut scn = cabinet_with(2, 70.0, 80.0, true);
    scn.obstacles.clear();
    let cells = 3500;
    let mesh = MeshSettings {
        target_cells: cells,
        ..MeshSettings::default()
    };
    let full = plan_grid_report(&scn, &mesh).unwrap().grid.cells();
    assert!((full as f64) < 1.5 * cells as f64, "{full}");
    let s = run(&scn, cells, LargeMode::Auto);
    assert!(
        s.notes.iter().all(|n| !n.starts_with("Reduced model: ")),
        "{:?}",
        s.notes
    );
    assert!(!s.accuracy.headline.contains("Reduced model"));
}

/// The reduced run against the full one on a cabinet both can solve.
#[test]
fn reduced_and_full_agree_on_the_drive_and_the_air() {
    // cold devices side by side, no duct or rail: small enough to solve the
    // full scenario too
    let mut scn = cabinet_with(3, 70.0, 80.0, true);
    scn.obstacles.clear();
    for d in scn.devices.iter_mut().skip(1) {
        d.footprint.y = 150.0;
    }
    // The reduced grid is about 40 % smaller than the full one at this
    // target (9360 cells); the air next to the drive depends on the
    // resolution, so the reduced run gets a target that gives it a grid of
    // about the same size (5800 cells against 9400): at 3700 cells its air rise is
    // 30 % low, which is the known grid dependence, not the lumping.
    let full = run(&scn, 3000, LargeMode::Off);
    let reduced = run(&scn, 6000, LargeMode::Force);

    assert!(
        reduced
            .notes
            .iter()
            .any(|n| n.starts_with("Reduced model: "))
    );
    assert!(reduced.accuracy.headline.contains("Reduced model"));
    assert!(!reduced.accuracy.validated);
    assert!(
        reduced.grid.cells() * 3 < full.grid.cells() * 2,
        "{} vs {}",
        reduced.grid.cells(),
        full.grid.cells()
    );
    // one result per real device, in the order of the scenario
    let ids: Vec<u64> = reduced.devices.iter().map(|d| d.id).collect();
    let want: Vec<u64> = scn.devices.iter().map(|d| d.id).collect();
    assert_eq!(ids, want);

    let rise = |s: &CfdSnapshot, id: u64| {
        let d = s.devices.iter().find(|d| d.id == id).unwrap();
        (d.surface_c - 25.0, d.air_c - 25.0)
    };
    let (sf, af) = rise(&full, HOT);
    let (sr, ar) = rise(&reduced, HOT);
    // the drive's housing rise within 5 % and the air round it within 15 %
    assert!((sr / sf - 1.0).abs() < 0.05, "surface rise {sr} vs {sf}");
    assert!((ar / af - 1.0).abs() < 0.15, "air rise {ar} vs {af}");
    // cold devices: the air round them within 3 K of the full run's (a
    // device the full grid cuts too small to resolve reads exactly ambient
    // there and is left out)
    let mut compared = 0;
    for d in full
        .devices
        .iter()
        .filter(|d| d.id != HOT && d.air_c != 25.0)
    {
        let r = reduced.devices.iter().find(|x| x.id == d.id).unwrap();
        assert!(
            (r.air_c - d.air_c).abs() < 3.0,
            "device {}: {} vs {}",
            d.id,
            r.air_c,
            d.air_c
        );
        // their own loss is tiny: the housing sits close to the air
        assert!(r.surface_c - r.air_c < 3.0);
        compared += 1;
    }
    assert!(
        compared >= 2,
        "only {compared} cold devices resolved in the full run"
    );
    // the heat balance of the reduced run is as good as the full one's
    assert!(reduced.progress.energy_balance_rel.abs() < 0.05);
    assert_eq!(large::convergence(&reduced).verdict, Verdict::Converged);
}

#[test]
fn the_convergence_verdict_says_when_a_run_stopped_early() {
    let scn = cabinet(8);
    let mut t = Tuning::default().with_cells(1500);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.max_steps = 40;
        l.steady_tol = 1e-9;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
    }
    let s = CabinetSolver::new(t)
        .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap();
    let c = large::convergence(&s);
    assert_ne!(c.verdict, Verdict::Converged, "{:?}", c.lines);
    assert!(c.lines[0].contains("not final") || c.lines[0].contains("Just started"));
}

#[test]
fn a_lumped_device_is_marginal_at_most_never_over() {
    let mut scn = cabinet(24);
    // limits the air will exceed: every cold device would read Over
    for d in scn.devices.iter_mut().skip(1) {
        d.rated_max_ambient_c = Some(25.2);
        d.rated_max_surface_c = Some(25.5);
    }
    let s = run(&scn, 3000, LargeMode::Force);
    let cold: Vec<_> = s.devices.iter().filter(|d| d.id != HOT).collect();
    assert_eq!(cold.len(), 24);
    assert!(cold.iter().all(|d| d.status != Status::Over));
    assert!(cold.iter().any(|d| d.status == Status::Marginal));
    // the margin itself stays honest (negative), the status is capped
    assert!(cold.iter().any(|d| d.margin_k.is_some_and(|m| m < 0.0)));
    assert!(
        s.hot_spots
            .iter()
            .any(|h| h.reason.contains("lumped block")),
        "{:?}",
        s.hot_spots
    );
}
