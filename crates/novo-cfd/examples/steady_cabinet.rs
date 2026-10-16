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
        vents: if std::env::var("VENTS").is_ok() || std::env::var("FAN").is_ok() {
            let opening = |placement| Opening {
                placement,
                width_mm: 200.0,
                height_mm: 100.0,
            };
            let mut v = vec![Vent {
                opening: opening(Placement::Top { x_mm: 300.0 }),
                free_area_fraction: 0.6,
            }];
            if std::env::var("FAN").is_err() {
                v.push(Vent {
                    opening: opening(Placement::Bottom { x_mm: 300.0 }),
                    free_area_fraction: 0.6,
                });
            }
            v
        } else {
            vec![]
        },
        fans: if std::env::var("FAN").is_ok() {
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
    let levels: Vec<usize> = a
        .get(1)
        .map(|v| v.split(',').filter_map(|c| c.parse().ok()).collect())
        .unwrap_or(vec![6000]);
    let mode = a.get(2).cloned().unwrap_or("steady".into());
    let cfl: f64 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(10.0);
    let threads: usize = a.get(4).and_then(|v| v.parse().ok()).unwrap_or(4);
    let scheme = match std::env::var("SCHEME").as_deref() {
        Ok("upwind") => novo_cfd::flow::Scheme::Upwind,
        Ok("central") => novo_cfd::flow::Scheme::Central,
        Ok("quick") => novo_cfd::flow::Scheme::Quick,
        _ => novo_cfd::flow::Scheme::VanLeer,
    };
    let window: usize = std::env::var("WINDOW")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);
    install(Threads::Fixed(threads), || {
        let t_all = std::time::Instant::now();
        let mut prev: Option<FlowSolver> = None;
        for &cells in &levels {
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
                continue;
            }
            if let Some(p) = &prev {
                s.initialise_from(p);
            }
            let tweak = |mut c: SteadyConfig| {
                if let Some(v) = std::env::var("EDROP").ok().and_then(|v| v.parse().ok()) {
                    c.energy_drop = v;
                }
                if let Some(v) = std::env::var("ESW").ok().and_then(|v| v.parse().ok()) {
                    c.energy_sweeps = v;
                }
                if let Some(v) = std::env::var("EF").ok().and_then(|v| v.parse().ok()) {
                    c.energy_factor = v;
                }
                if let Some(v) = std::env::var("CTOL").ok().and_then(|v| v.parse().ok()) {
                    c.continuity_tol = v;
                }
                if let Some(v) = std::env::var("PTOL").ok().and_then(|v| v.parse().ok()) {
                    c.pressure_tol = v;
                }
                c
            };
            let stages = if std::env::var("FIRST").is_ok() {
                vec![SteadyConfig::first_order()]
            } else if std::env::var("SECOND").is_ok() {
                vec![SteadyConfig {
                    cfl,
                    ..SteadyConfig::second_order()
                }]
            } else if prev.is_some() && std::env::var("SEQ_SECOND").is_ok() {
                vec![SteadyConfig::second_order()]
            } else {
                vec![SteadyConfig::first_order(), SteadyConfig::second_order()]
            };
            let stages: Vec<SteadyConfig> = stages.into_iter().map(tweak).collect();
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
            let mut run = SteadyRun::staged(&s, stages, limits, setup.ambient_c, probes);
            loop {
                let status = run.advance(&mut s, window);
                let r = run.report();
                if std::env::var("QUIET").is_err() {
                    let tmax = s.t.iter().cloned().fold(f64::MIN, f64::max);
                    println!(
                        "it {} stage {} imbalance {:+.2}% drift {:.3} K flow {:.3} wobble {:.2}% Tmax {tmax:.2} {:.1}s",
                        r.iterations,
                        r.stage,
                        100.0 * r.imbalance,
                        r.drift_k,
                        r.flow_drift,
                        100.0 * r.wobble,
                        t0.elapsed().as_secs_f64()
                    );
                }
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
            let flows: Vec<f64> = (0..s.cfg.openings.len())
                .map(|i| s.patch_flow_m3s(i))
                .collect();
            println!(
                "  patch flows m3/s {flows:?}, net {:.2e}; series imbalance {:+.2}%",
                flows.iter().sum::<f64>(),
                100.0 * run.report().imbalance
            );
            let p = run.state.phase_s;
            println!(
                "  phases s: energy asm {:.2} sweeps {:.2} | momentum asm {:.2} sweeps {:.2} | pressure {:.2}",
                p[0], p[1], p[2], p[3], p[4]
            );
            prev = Some(s);
        }
        println!("total {:.1} s", t_all.elapsed().as_secs_f64());
    });
}
