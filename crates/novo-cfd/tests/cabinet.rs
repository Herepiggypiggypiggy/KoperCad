//! The cabinet solver: a small cabinet marched to a steady state.

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{CfdError, CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario(with_vent: bool) -> Scenario {
    let opening = |placement| Opening {
        placement,
        width_mm: 200.0,
        height_mm: 100.0,
    };
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
        vents: if with_vent {
            vec![
                Vent {
                    opening: opening(Placement::Bottom { x_mm: 300.0 }),
                    free_area_fraction: 0.6,
                },
                Vent {
                    opening: opening(Placement::Top { x_mm: 300.0 }),
                    free_area_fraction: 0.6,
                },
            ]
        } else {
            vec![]
        },
        fans: vec![],
        params: Params::default(),
    }
}

fn small() -> CabinetSolver {
    let mut t = Tuning::default().with_cells(2000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 1e-2;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
    }
    CabinetSolver::new(t)
}

fn solve(scn: &Scenario) -> (CfdSnapshot, Vec<CfdSnapshot>) {
    let mut seen = Vec::new();
    let snap = small()
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |s| {
            if seen.len() < 400 {
                seen.push(s.clone());
            }
        })
        .unwrap();
    (snap, seen)
}

#[test]
fn a_sealed_cabinet_reaches_a_steady_state_that_balances() {
    let (s, _) = solve(&scenario(false));
    assert!(s.is_consistent());
    let p = &s.progress;
    assert!(p.converged, "{p:?}");
    println!(
        "CABINET | sealed | steps {} | balance {:.4} | max T {:.1}",
        p.iteration,
        p.energy_balance_rel,
        s.max_temperature_c()
    );
    assert!(p.energy_balance_rel < 0.03, "{}", p.energy_balance_rel);
    let d = &s.devices[0];
    assert!(d.surface_c > d.air_c && d.air_c > 25.0);
    assert!((0.0..=1.0).contains(&d.plate_share));
    // nothing is colder than the outside air, nothing is hotter than the device core
    let min = s.temperature_c.iter().copied().fold(f32::MAX, f32::min);
    assert!(min > 24.9, "{min}");
    assert!(s.max_temperature_c() < 200.0);
    // the heat leaves through the wall: some solid cell has a positive flux
    assert!(s.wall_flux_w_m2.iter().any(|f| *f > 1.0));
}

#[test]
fn progress_is_published_and_the_energy_residual_falls() {
    let (_, seen) = solve(&scenario(false));
    assert!(seen.len() > 5);
    assert!(
        seen.windows(2)
            .all(|w| w[1].progress.iteration >= w[0].progress.iteration)
    );
    let first = seen.first().unwrap().progress.residuals.energy;
    let last = seen.last().unwrap().progress.residuals.energy;
    assert!(last < first, "{first} -> {last}");
    assert!(seen.iter().all(|s| s.is_consistent()));
}

#[test]
fn vents_carry_heat_out_and_cool_the_device() {
    let (sealed, _) = solve(&scenario(false));
    let (vented, _) = solve(&scenario(true));
    println!(
        "CABINET | vented | surface {:.1} vs sealed {:.1} | converged {} {} | steps {} {} | balance {:.3} {:.3} | air {:.1} {:.1}",
        vented.devices[0].surface_c,
        sealed.devices[0].surface_c,
        vented.progress.converged,
        sealed.progress.converged,
        vented.progress.iteration,
        sealed.progress.iteration,
        vented.progress.energy_balance_rel,
        sealed.progress.energy_balance_rel,
        vented.devices[0].air_c,
        sealed.devices[0].air_c
    );
    assert!(vented.progress.converged);
    assert!(vented.progress.energy_balance_rel < 0.05);
    // The air round the device is the robust measure: the housing temperature
    // of a device a few cells wide jumps with how the grid cuts it (47 to 78
    // degrees between 1200 and 2000 cells in the same cabinet).
    assert!(
        vented.devices[0].air_c < sealed.devices[0].air_c - 1.0,
        "{} vs {}",
        vented.devices[0].air_c,
        sealed.devices[0].air_c
    );
}

#[test]
fn cancelling_returns_promptly() {
    let control = Control::new();
    let c2 = control.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        c2.cancel();
    });
    let mut tuning = Tuning::default().with_cells(2000);
    tuning.coarse.steady_tol = 1e-12;
    let t0 = std::time::Instant::now();
    let r =
        CabinetSolver::new(tuning).solve(&scenario(false), Fidelity::Coarse, &control, &mut |_| {});
    assert!(matches!(r, Err(CfdError::Cancelled)));
    assert!(t0.elapsed() < Duration::from_secs(5));
}

#[test]
fn the_time_budget_ends_an_unfinished_level_honestly() {
    let mut tuning = Tuning::default().with_cells(2000);
    tuning.coarse.steady_tol = 1e-12;
    tuning.coarse.max_wall = Some(Duration::from_millis(300));
    let s = CabinetSolver::new(tuning)
        .solve(
            &scenario(false),
            Fidelity::Coarse,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap();
    assert!(!s.progress.converged);
    assert!(
        s.notes
            .iter()
            .any(|n| n.contains("Stopped before a steady state"))
    );
}

#[test]
fn preview_is_not_a_cabinet_level_and_the_validated_flag_follows_the_statement() {
    let r = small().solve(
        &scenario(false),
        Fidelity::Preview2d,
        &Control::new(),
        &mut |_| {},
    );
    assert!(matches!(r, Err(CfdError::Unsupported(Fidelity::Preview2d))));
    let (s, _) = solve(&scenario(false));
    assert_eq!(s.accuracy.validated, novo_cfd::cabinet::ENCLOSURE_VALIDATED);
    assert!(!s.accuracy.headline.is_empty() && !s.accuracy.limits.is_empty());
}

#[test]
fn the_steady_path_agrees_with_the_march_and_labels_its_preview() {
    for vent in [false, true] {
        let scn = scenario(vent);
        let (steady, seen) = {
            let mut tu = Tuning::default().with_cells(800);
            for l in [&mut tu.coarse, &mut tu.medium, &mut tu.fine] {
                l.report_every = 20;
                l.full_snapshot_every = Duration::ZERO;
            }
            let mut seen = Vec::new();
            let snap = CabinetSolver::new(tu)
                .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |s| {
                    seen.push(s.clone())
                })
                .unwrap();
            (snap, seen)
        };
        let mut t = Tuning::default().with_cells(800).marched();
        for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
            l.steady_tol = 1e-2;
            l.report_every = 20;
        }
        let march = CabinetSolver::new(t)
            .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
            .unwrap();
        let (a, b) = (&steady.devices[0], &march.devices[0]);
        println!(
            "CABINET | steady vs march | vent {vent} | surface {:.2} vs {:.2} | air {:.2} vs {:.2} | iterations {} vs steps {}",
            a.surface_c,
            b.surface_c,
            a.air_c,
            b.air_c,
            steady.progress.iteration,
            march.progress.iteration
        );
        let rise = b.surface_c - 25.0;
        assert!(
            (a.surface_c - b.surface_c).abs() < 0.05 * rise,
            "{} vs {}",
            a.surface_c,
            b.surface_c
        );
        assert!(steady.progress.converged);
        assert!(steady.progress.iteration < march.progress.iteration);
        // the first-order preview is announced while it runs
        assert!(
            seen.iter()
                .any(|s| s.notes.iter().any(|n| n.contains("First-order preview")))
        );
        assert!(
            !steady
                .notes
                .iter()
                .any(|n| n.contains("First-order preview"))
        );
    }
}

#[test]
fn turbulence_is_an_option_that_shows_in_the_snapshot_and_the_statement() {
    use novo_cfd::turb::TurbModel;
    let laminar = solve(&scenario(true)).0;
    assert!(laminar.turb_visc_ratio.iter().all(|v| *v == 0.0));
    for model in [TurbModel::KOmegaSst, TurbModel::KEpsilon] {
        let snap = small()
            .with_turbulence(model)
            .solve(
                &scenario(true),
                Fidelity::Coarse,
                &Control::new(),
                &mut |_| {},
            )
            .unwrap();
        let ratio = &snap.turb_visc_ratio;
        assert!(ratio.iter().all(|v| v.is_finite() && *v >= 0.0));
        assert!(ratio.iter().cloned().fold(0.0_f32, f32::max) > 0.0);
        assert!(snap.temperature_c.iter().all(|t| t.is_finite()));
        assert!(
            snap.accuracy
                .limits
                .iter()
                .all(|l| !l.contains("Turbulence and radiation are not modelled")),
            "the statement must not claim the model is laminar"
        );
        assert!(snap.accuracy.headline.contains("Turbulence"));
        // the eddy viscosity must not scale the device film conductance: the
        // device runs within 15 % of its laminar rise (it fell by 35 % when it did)
        let rise = |s: &CfdSnapshot| s.devices[0].surface_c - 25.0;
        let ratio = rise(&snap) / rise(&laminar);
        assert!((0.85..1.15).contains(&ratio), "{model:?}: {ratio}");
    }
}

/// The mesh spends its cell budget on bodies first (WS CFD-C1), so a 120 cell
/// request no longer leaves a 150 mm device unresolved: the grid grows past the
/// request to the floor that gives the device cells across, the note says so
/// by not raising the "not resolved" warning, and the band stays the narrow
/// one. A device the grid really cannot resolve still gets the wide band; that
/// path is exercised by the mesh tests (`a_slim_device_is_kept_and_reported`).
#[test]
fn a_small_cell_request_still_resolves_the_device() {
    let mut t = Tuning::default().with_cells(120);
    t.coarse.steady_tol = 1e-2;
    let s = CabinetSolver::new(t)
        .solve(
            &scenario(false),
            Fidelity::Coarse,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap();
    assert!(
        !s.notes.iter().any(|n| n.contains("not resolved")),
        "{:?}",
        s.notes
    );
    assert!(s.temperature_c.len() > 120);
    let d = &s.devices[0];
    assert!(d.surface_band_c.1 - d.surface_band_c.0 < 1.0 * (d.surface_c - 25.0).max(1.0));
}

/// The default solver (plate sheet, lumped devices) lands near the 2D engine
/// for the sealed reference case, with the films updated during the march.
#[test]
fn the_default_cabinet_run_follows_the_2d_engine_for_housing_and_plate() {
    let scn = scenario(false);
    let two_d = novo_thermal::simulate_steady(&scn).unwrap();
    let d2 = &two_d.devices[0];
    let (s, _) = solve(&scn);
    let d = &s.devices[0];
    println!(
        "CABINET | housing {:.1} (2D {:.1}) | plate share {:.3} (2D {:.3}) | h {:.1}",
        d.surface_c, d2.surface_c, d.plate_share, d2.plate_share, d.h_w_m2k
    );
    assert!(s.progress.converged, "{:?}", s.progress);
    assert!(((d.surface_c - 25.0) / (d2.surface_c - 25.0) - 1.0).abs() < 0.10);
    assert!((d.plate_share - d2.plate_share).abs() < 0.06);
    assert!(
        d.h_w_m2k > 5.0 && d.h_w_m2k < 15.0,
        "film never updated: {}",
        d.h_w_m2k
    );
}

#[test]
fn the_steady_path_drives_k_omega_sst_and_agrees_with_the_marched_one() {
    use novo_cfd::turb::TurbModel;
    for model in [TurbModel::KOmegaSst] {
        for vent in [false, true] {
            let scn = scenario(vent);
            let mut tu = Tuning::default().with_cells(800);
            for l in [&mut tu.coarse, &mut tu.medium, &mut tu.fine] {
                l.report_every = 20;
                l.steady_tol = 1e-2;
            }
            let run = |t: Tuning| {
                CabinetSolver::new(t)
                    .with_turbulence(model)
                    .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
                    .unwrap()
            };
            let steady = run(tu.clone());
            let march = run(tu.marched());
            let (a, b) = (&steady.devices[0], &march.devices[0]);
            println!(
                "CABINET | {model:?} vent {vent} | surface steady {:.2} vs march {:.2} | max nu_t/nu {:.1} vs {:.1} | iterations {} vs steps {}",
                a.surface_c,
                b.surface_c,
                steady
                    .turb_visc_ratio
                    .iter()
                    .cloned()
                    .fold(0.0_f32, f32::max),
                march
                    .turb_visc_ratio
                    .iter()
                    .cloned()
                    .fold(0.0_f32, f32::max),
                steady.progress.iteration,
                march.progress.iteration
            );
            assert!(steady.progress.converged);
            assert!(steady.notes.iter().any(|n| n.contains("window")));
            assert!(
                steady
                    .turb_visc_ratio
                    .iter()
                    .cloned()
                    .fold(0.0_f32, f32::max)
                    > 0.0
            );
            let rise = b.surface_c - 25.0;
            assert!((a.surface_c - b.surface_c).abs() < 0.06 * rise);
        }
    }
}

#[test]
fn k_epsilon_still_marches() {
    use novo_cfd::turb::TurbModel;
    let snap = small()
        .with_turbulence(TurbModel::KEpsilon)
        .solve(
            &scenario(true),
            Fidelity::Coarse,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap();
    assert!(
        !snap.notes.iter().any(|n| n.contains("window")),
        "{:?}",
        snap.notes
    );
}

#[test]
fn a_steady_result_with_a_heat_balance_error_above_two_percent_says_so() {
    // the note is derived from the same number the progress carries
    let (s, _) = solve(&scenario(false));
    let noted = s.notes.iter().any(|n| n.contains("Heat balance off by"));
    assert_eq!(noted, s.progress.energy_balance_rel > 0.02);
}

#[test]
fn a_result_with_a_fan_states_how_good_its_heat_balance_is() {
    let mut scn = scenario(true);
    scn.fans = vec![Fan {
        opening: Opening {
            placement: Placement::Top { x_mm: 300.0 },
            width_mm: 100.0,
            height_mm: 100.0,
        },
        mode: FanMode::Exhaust,
        free_flow_m3h: 100.0,
        max_pressure_pa: 40.0,
    }];
    let (fan, _) = solve(&scn);
    let note = fan
        .notes
        .iter()
        .find(|n| n.starts_with("Fan case"))
        .unwrap_or_else(|| panic!("no fan note in {:?}", fan.notes));
    assert!(
        note.contains("3 %") && note.contains("Heat balance here"),
        "{note}"
    );
    // Without a fan there is no such note.
    let (plain, _) = solve(&scenario(true));
    assert!(!plain.notes.iter().any(|n| n.starts_with("Fan case")));
}

#[test]
fn the_turbulence_statement_stays_whole_when_radiation_is_marked() {
    use novo_cfd::cabinet::{accuracy_with_turbulence, mark_radiation};
    use novo_cfd::turb::TurbModel;
    let mut a = accuracy_with_turbulence(Fidelity::Fine, TurbModel::KOmegaSst);
    assert!(
        a.limits
            .iter()
            .any(|l| l.contains("Radiation is not modelled"))
    );
    mark_radiation(&mut a);
    assert!(
        a.limits
            .iter()
            .all(|l| !l.to_lowercase().contains("radiation is not modelled"))
    );
    let t = a
        .limits
        .iter()
        .find(|l| l.starts_with("Turbulence: "))
        .unwrap();
    assert!(
        t.ends_with("read air temperatures of a fan case as an estimate."),
        "{t}"
    );
}
