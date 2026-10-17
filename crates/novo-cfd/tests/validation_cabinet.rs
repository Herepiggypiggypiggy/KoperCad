//! The cabinet solver as the app runs it (`CabinetSolver`, steady path, plate
//! sheet and lumped devices) against the 2D engine and IEC 60890, for the
//! sealed and the vented reference cabinet. Run with `--release -- --nocapture`
//! to see the `VALIDATION` rows. The result of this test is what
//! `ENCLOSURE_VALIDATED` rests on; if it fails, set that flag to false.

mod validation;

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;
use validation::report;

fn scenario(vented: bool) -> Scenario {
    let vent = |placement| Vent {
        opening: Opening {
            placement,
            width_mm: 200.0,
            height_mm: 120.0,
        },
        free_area_fraction: 0.6,
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
            loss_w: if vented { 100.0 } else { 50.0 },
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: if vented {
            vec![
                vent(Placement::Bottom { x_mm: 300.0 }),
                vent(Placement::Top { x_mm: 300.0 }),
            ]
        } else {
            vec![]
        },
        fans: vec![],
        params: Params::default(),
    }
}

fn solve(scn: &Scenario, cells: usize) -> CfdSnapshot {
    let mut t = Tuning::default().with_cells(cells);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.report_every = 50;
        l.full_snapshot_every = Duration::from_secs(3600);
    }
    CabinetSolver::new(t)
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

/// Volume-weighted mean air rise over ambient, K.
fn mean_air_rise(s: &CfdSnapshot, amb: f64) -> f64 {
    let g = &s.grid;
    let (mut sum, mut vol) = (0.0, 0.0);
    for k in 0..g.nz {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let c = i + g.nx * (j + g.ny * k);
                if s.material[c] != 0 {
                    continue;
                }
                let v = (g.x_edges_mm[i + 1] - g.x_edges_mm[i])
                    * (g.y_edges_mm[j + 1] - g.y_edges_mm[j])
                    * (g.z_edges_mm[k + 1] - g.z_edges_mm[k]);
                sum += v * (f64::from(s.temperature_c[c]) - amb);
                vol += v;
            }
        }
    }
    sum / vol
}

#[test]
fn the_cabinet_solver_follows_the_2d_engine_and_iec_60890() {
    for (vented, cells_list) in [(false, [1500usize, 3000]), (true, [1500, 3000])] {
        let scn = scenario(vented);
        let two_d = simulate_steady(&scn).unwrap();
        let d2 = &two_d.devices[0];
        let amb = scn.ambient_c;
        for cells in cells_list {
            let case = format!(
                "cabinet {} {} W, {cells} cells",
                if vented { "vented" } else { "sealed" },
                scn.devices[0].loss_w
            );
            let s = solve(&scn, cells);
            let d = &s.devices[0];
            let e_air = report(
                &case,
                "mean air rise K vs 2D engine",
                mean_air_rise(&s, amb),
                two_d.balance.mean_rise_k,
            );
            if !vented {
                report(
                    &case,
                    "mean air rise K vs IEC 60890",
                    mean_air_rise(&s, amb),
                    two_d.balance.iec_rise_k,
                );
            }
            let e_hous = report(
                &case,
                "housing rise K vs 2D engine",
                d.surface_c - amb,
                d2.surface_c - amb,
            );
            let e_plate = d.plate_share - d2.plate_share;
            println!(
                "VALIDATION | {case} | plate share vs 2D engine | {:.3} | {:.3} | abs {e_plate:+.3}",
                d.plate_share, d2.plate_share
            );
            println!(
                "{case}: converged {} balance {:.4}",
                s.progress.converged, s.progress.energy_balance_rel
            );
            assert!(e_air.abs() < 0.20, "{case}: air rise {e_air}");
            assert!(e_hous.abs() < 0.12, "{case}: housing {e_hous}");
            assert!(e_plate.abs() < 0.03, "{case}: plate share {e_plate}");
        }
    }
}
