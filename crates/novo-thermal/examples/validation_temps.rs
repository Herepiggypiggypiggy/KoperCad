//! Prints the temperatures of the validation scenarios (the physics tests'
//! cases and a dense 60-device cabinet) so two versions of the 2D solver can
//! be compared in kelvin:
//!
//! ```text
//! cargo run --release -p novo-thermal --example validation_temps
//! ```
//!
//! Every case is one central run (no uncertainty bands) on the coarse grid
//! the tests use. Uses only the public API, so it also builds on older
//! commits for before and after tables.

use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn enclosure() -> Enclosure {
    Enclosure {
        width_mm: 800.0,
        height_mm: 1200.0,
        depth_mm: 300.0,
        wall_mm: 2.0,
        material: Material::Steel,
        installation: Installation::Wall,
        plate: Rect {
            x: 50.0,
            y: 50.0,
            w: 690.0,
            h: 1090.0,
        },
    }
}

fn device(id: u64, x: f64, y: f64, w: f64, h: f64, loss: f64) -> Device {
    Device {
        id,
        footprint: Rect { x, y, w, h },
        height_mm: Some(60.0),
        loss_w: loss,
        rated_max_ambient_c: Some(55.0),
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    }
}

fn scenario(devices: Vec<Device>) -> Scenario {
    Scenario {
        enclosure: enclosure(),
        ambient_c: 25.0,
        devices,
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn spread(total_w: f64) -> Vec<Device> {
    let mut v = Vec::new();
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
                total_w / 12.0,
            ));
        }
    }
    v
}

fn opening(placement: Placement, w: f64, h: f64) -> Opening {
    Opening {
        placement,
        width_mm: w,
        height_mm: h,
    }
}

fn fan_box(fan: bool) -> Scenario {
    let mut sc = scenario(vec![Device {
        footprint: Rect {
            x: 200.0,
            y: 450.0,
            w: 150.0,
            h: 150.0,
        },
        height_mm: Some(100.0),
        ..device(7, 0.0, 0.0, 0.0, 0.0, 100.0)
    }]);
    sc.enclosure.width_mm = 600.0;
    sc.enclosure.height_mm = 800.0;
    sc.enclosure.plate = Rect {
        x: 40.0,
        y: 40.0,
        w: 510.0,
        h: 700.0,
    };
    let o = |p| opening(p, 200.0, 120.0);
    sc.vents = vec![Vent {
        opening: o(Placement::Top { x_mm: 300.0 }),
        free_area_fraction: 0.6,
    }];
    if fan {
        sc.fans = vec![Fan {
            opening: o(Placement::Bottom { x_mm: 300.0 }),
            mode: FanMode::Intake,
            free_flow_m3h: 120.0,
            max_pressure_pa: 60.0,
        }];
    } else {
        sc.vents.push(Vent {
            opening: o(Placement::Bottom { x_mm: 300.0 }),
            free_area_fraction: 0.6,
        });
    }
    sc
}

/// 60 devices on a 1.6 x 1.9 m plate, 100 W in all: the dense cabinet of the
/// scale benchmark.
fn dense() -> Scenario {
    let mut sc = scenario(Vec::new());
    sc.enclosure.width_mm = 1700.0;
    sc.enclosure.height_mm = 2000.0;
    sc.enclosure.depth_mm = 400.0;
    sc.enclosure.plate = Rect {
        x: 50.0,
        y: 50.0,
        w: 1600.0,
        h: 1900.0,
    };
    let mut id = 0;
    for row in 0..6 {
        for col in 0..10 {
            id += 1;
            sc.devices.push(device(
                id,
                60.0 + col as f64 * 150.0,
                150.0 + row as f64 * 230.0,
                54.0,
                80.0,
                100.0 / 60.0,
            ));
        }
    }
    sc
}

fn main() {
    let mut vented = scenario(spread(120.0));
    vented.vents = vec![
        Vent {
            opening: opening(Placement::Bottom { x_mm: 400.0 }, 200.0, 100.0),
            free_area_fraction: 0.6,
        },
        Vent {
            opening: opening(Placement::Top { x_mm: 400.0 }, 200.0, 100.0),
            free_area_fraction: 0.6,
        },
    ];
    let mut fans = scenario(spread(200.0));
    let flow = 3.1 * 200.0 / 10.0 * 1.2;
    fans.fans = vec![
        Fan {
            opening: opening(Placement::Bottom { x_mm: 200.0 }, 150.0, 150.0),
            mode: FanMode::Intake,
            free_flow_m3h: flow,
            max_pressure_pa: 80.0,
        },
        Fan {
            opening: opening(Placement::Top { x_mm: 600.0 }, 150.0, 150.0),
            mode: FanMode::Exhaust,
            free_flow_m3h: flow,
            max_pressure_pa: 80.0,
        },
    ];
    let lower = device(1, 300.0, 800.0, 80.0, 120.0, 30.0);
    let cases: Vec<(&str, Scenario)> = vec![
        ("closed box, 120 W spread", scenario(spread(120.0))),
        (
            "closed box, 40 W hot device",
            scenario(vec![device(1, 350.0, 700.0, 80.0, 120.0, 40.0)]),
        ),
        ("two vents, 120 W spread", vented),
        ("fan pair, 200 W spread", fans),
        (
            "stacked pair, 2 x 30 W",
            scenario(vec![
                lower.clone(),
                device(2, 300.0, 600.0, 80.0, 120.0, 30.0),
            ]),
        ),
        (
            "side by side pair, 2 x 30 W",
            scenario(vec![lower, device(2, 500.0, 800.0, 80.0, 120.0, 30.0)]),
        ),
        ("fan box, 100 W, one fan", fan_box(true)),
        ("fan box, 100 W, two vents", fan_box(false)),
        ("dense cabinet, 60 devices, 100 W", dense()),
    ];
    println!(
        "| Case | Mean air rise (K) | Air at top (C) | Hottest surface (C) | Mean device air (C) | Iterations | Converged | Stalled |"
    );
    println!("| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |");
    for (name, mut sc) in cases {
        // The tests' coarse grid, except the dense cabinet: default grid.
        if !name.starts_with("dense") {
            sc.params.cell_mm = sc.params.cell_mm.max(40.0);
        }
        let start = std::time::Instant::now();
        let r = simulate_steady_with(
            &sc,
            Options {
                bands: false,
                advice: false,
            },
        )
        .expect("scenario solves");
        let hottest = r
            .devices
            .iter()
            .map(|d| d.surface_c)
            .fold(f64::MIN, f64::max);
        let mean_air = r.devices.iter().map(|d| d.air_c).sum::<f64>() / r.devices.len() as f64;
        let stalled = r.notes.iter().any(|n| n.contains("stalled"));
        eprintln!("{name}: {:.1} s", start.elapsed().as_secs_f64());
        println!(
            "| {name} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} | {} |",
            r.balance.mean_rise_k,
            r.air_at_top_c,
            hottest,
            mean_air,
            r.iterations,
            r.converged,
            stalled
        );
    }
}
