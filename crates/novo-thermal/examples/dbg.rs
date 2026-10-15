#[path = "../tests/common.rs"]
mod common;
use common::*;
use novo_thermal::*;
fn spread(total_w: f64) -> Vec<Device> {
    let mut v = Vec::new();
    let per = total_w / 12.0;
    let mut id = 0;
    for row in 0..4 {
        for col in 0..3 {
            id += 1;
            v.push(device(
                id,
                120.0 + col as f64 * 220.0,
                120.0 + row as f64 * 260.0,
                60.0,
                80.0,
                per,
            ));
        }
    }
    v
}
fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    match which.as_str() {
        "stack" => {
            let lower = device(1, 300.0, 800.0, 80.0, 120.0, 30.0);
            let a = simulate_steady(&scenario(vec![
                lower.clone(),
                device(2, 300.0, 600.0, 80.0, 120.0, 30.0),
            ]))
            .unwrap();
            let b = simulate_steady(&scenario(vec![
                lower,
                device(2, 500.0, 800.0, 80.0, 120.0, 30.0),
            ]))
            .unwrap();
            for r in [&a, &b] {
                for d in &r.devices {
                    println!(
                        "dev {} surf {:.2} air {:.2} h {:.2}",
                        d.id, d.surface_c, d.air_c, d.h_w_m2k
                    );
                }
                println!();
            }
        }
        "sym" => {
            let w = enclosure().interior_width_mm();
            let d = |id: u64, x: f64| device(id, x, 500.0, 80.0, 120.0, 25.0);
            let mut sc = scenario(vec![d(1, 150.0), d(2, w - 150.0 - 80.0)]);
            sc.enclosure.plate = novo_layout::Rect {
                x: 50.0,
                y: 50.0,
                w: w - 100.0,
                h: 1090.0,
            };
            let r = simulate_steady(&sc).unwrap();
            let g = r.grid;
            let (mut wt, mut wv) = (0.0f64, 0.0f64);
            for row in 0..g.rows {
                for col in 0..g.cols / 2 {
                    wt = wt.max(
                        (r.air_temp.at(col, row) - r.air_temp.at(g.cols - 1 - col, row)).abs(),
                    );
                    let (a, b) = (
                        r.velocity.at(col, row),
                        r.velocity.at(g.cols - 1 - col, row),
                    );
                    wv = wv.max((a.0 + b.0).abs()).max((a.1 - b.1).abs());
                }
            }
            println!(
                "temp asym {wt} vel asym {wv} conv {} it {}",
                r.converged, r.iterations
            );
        }
        "conv" => {
            let r = simulate_steady(&scenario(spread(120.0))).unwrap();
            println!(
                "conv {} it {} rise {} iec {}",
                r.converged, r.iterations, r.balance.mean_rise_k, r.balance.iec_rise_k
            );
        }
        "tau" => {
            let t = simulate_transient(
                &scenario(spread(120.0)),
                &TransientSettings {
                    duration_s: 16200.0,
                    steps: 120,
                    ..Default::default()
                },
            )
            .unwrap();
            for s in t.samples.iter().step_by(8) {
                println!(
                    "t {:7.0} mean {:.3} max {:.3}",
                    s.t_s, s.mean_air_c, s.max_air_c
                );
            }
            println!("tau {:?}", t.time_constant_s);
        }
        "fanvent" => {
            let mut closed = scenario(spread(120.0));
            closed.vents = vec![
                Vent {
                    opening: Opening {
                        placement: Placement::Bottom { x_mm: 400.0 },
                        width_mm: 200.0,
                        height_mm: 100.0,
                    },
                    free_area_fraction: 0.6,
                },
                Vent {
                    opening: Opening {
                        placement: Placement::Top { x_mm: 400.0 },
                        width_mm: 200.0,
                        height_mm: 100.0,
                    },
                    free_area_fraction: 0.6,
                },
            ];
            closed.fans = vec![Fan {
                opening: Opening {
                    placement: Placement::Front {
                        x_mm: 200.0,
                        y_mm: 1000.0,
                    },
                    width_mm: 120.0,
                    height_mm: 120.0,
                },
                mode: FanMode::Intake,
                free_flow_m3h: 150.0,
                max_pressure_pa: 60.0,
            }];
            let r = simulate_steady(&closed).unwrap();
            println!("conv {} it {}", r.converged, r.iterations);
            for o in &r.openings {
                println!("{:?}", o);
            }
            println!(
                "sum flow {}",
                r.openings.iter().map(|o| o.flow_m3h).sum::<f64>()
            );
            println!("{:?}", r.balance);
        }
        _ => {}
    }
}
