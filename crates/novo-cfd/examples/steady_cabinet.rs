//! One 50 W device in a 600x800x300 wall enclosure: the steady path against
//! the time march. `steady_cabinet <cells> steady|march [cfl]`
use novo_cfd::flow::{FlowSolver, RunStatus, SteadyConfig, SteadyLimits, SteadyRun};
use novo_cfd::mesh::MeshSettings;
use novo_cfd::sched::{Threads, install};
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario(loss: f64) -> Scenario {
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
            loss_w: loss,
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn report(tag: &str, setup: &FlowSetup, s: &FlowSolver, secs: f64, iters: usize) {
    let walls: f64 = (0..6).map(|f| s.wall_heat_w(f)).sum();
    let open = s.open_heat_out_w(setup.ambient_c);
    let tmax = s.t.iter().cloned().fold(f64::MIN, f64::max);
    let vmax = s.vel.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
    // volume mean of the air cells
    let mean = s.mean_temperature();
    println!(
        "{tag}: {iters} iterations, {secs:.1} s | Tmax {tmax:.2} C, mean {mean:.2} C, vmax {vmax:.3} m/s | walls {walls:.2} W + openings {open:.2} W of {:.1} W (imbalance {:.2}%), div {:.1e}",
        setup.heat_w,
        100.0 * (setup.heat_w - walls - open) / setup.heat_w,
        s.divergence_max()
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let cells: usize = a.get(1).and_then(|v| v.parse().ok()).unwrap_or(6000);
    let mode = a.get(2).cloned().unwrap_or("steady".into());
    let cfl: f64 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(10.0);
    let threads: usize = a.get(4).and_then(|v| v.parse().ok()).unwrap_or(4);
    let scheme = match std::env::var("SCHEME").as_deref() {
        Ok("upwind") => novo_cfd::flow::Scheme::Upwind,
        Ok("central") => novo_cfd::flow::Scheme::Central,
        Ok("quick") => novo_cfd::flow::Scheme::Quick,
        _ => novo_cfd::flow::Scheme::VanLeer,
    };
    let st = SolidsSettings {
        scheme,
        mesh: MeshSettings {
            target_cells: cells,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let setup = FlowSetup::new(&scenario(50.0), &st).unwrap();
    println!(
        "cells {:?} = {}, heat {:.1} W, {mode}, {threads} threads",
        setup.mesh.n,
        setup.mesh.cells(),
        setup.heat_w
    );
    install(Threads::Fixed(threads), || {
        let mut s = setup.solver();
        let t0 = std::time::Instant::now();
        if mode == "march" {
            let r = setup.run_steady(
                &mut s,
                1e-3,
                (3000.0, 200_000),
                &novo_cfd::Control::default(),
                |_, _| {},
                0,
            );
            report("march", &setup, &s, t0.elapsed().as_secs_f64(), r.steps);
            println!(
                "  converged {} storage_rate {:.1e} sim time {:.0} s",
                r.converged, r.storage_rate, r.time_s
            );
        } else {
            let relax: f64 = std::env::var("DEFER")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1.0);
            let ef: f64 = std::env::var("EF")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10.0);
            let msw: usize = std::env::var("MSW")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3);
            let window: usize = std::env::var("WINDOW")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(100);
            let cfg = SteadyConfig {
                cfl,
                defer_relax: relax,
                energy_factor: ef,
                momentum_sweeps: msw,
                buoyancy_cfl: std::env::var("BCFL")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1.0),
                ..SteadyConfig::default()
            };
            let probes: Vec<usize> = (0..setup.classified.grid.cells())
                .filter(|&c| {
                    setup.classified.material[c] != 0
                        && setup
                            .cfg
                            .medium
                            .as_ref()
                            .is_some_and(|m| m.heat_w_m3[c] > 0.0)
                })
                .collect();
            let limits = SteadyLimits {
                window,
                ..SteadyLimits::default()
            };
            let stages = if std::env::var("STAGED").is_ok() {
                vec![SteadyConfig::first_order(), SteadyConfig::second_order()]
            } else {
                vec![cfg]
            };
            let mut run = SteadyRun::staged(&s, stages, limits, setup.ambient_c, probes);
            loop {
                let status = run.advance(&mut s, window);
                let r = run.report();
                if let (Ok(_), Some(i)) = (std::env::var("LOG"), run.last) {
                    println!(
                        "   dt {:.3} du {:.2e} dT {:.2e} cont {:.2e} pit {} esw {}",
                        i.dt,
                        i.max_du,
                        i.max_dt_k,
                        i.continuity,
                        i.pressure_iterations,
                        i.energy_sweeps
                    );
                }
                let tmax = s.t.iter().cloned().fold(f64::MIN, f64::max);
                println!(
                    "it {} stage {} imbalance {:+.2}% drift {:.3} K wobble {:.2}% Tmax {tmax:.2} {:.1}s",
                    r.iterations,
                    r.stage,
                    100.0 * r.imbalance,
                    r.drift_k,
                    100.0 * r.wobble,
                    t0.elapsed().as_secs_f64()
                );
                if status != RunStatus::Running {
                    break;
                }
            }
            report(
                "steady",
                &setup,
                &s,
                t0.elapsed().as_secs_f64(),
                run.report().iterations,
            );
            println!("  converged {}", run.report().converged);
            let p = run.state.phase_s;
            println!(
                "  phases s: energy asm {:.2} sweeps {:.2} | momentum asm {:.2} sweeps {:.2} | pressure {:.2}",
                p[0], p[1], p[2], p[3], p[4]
            );
        }
    });
}
