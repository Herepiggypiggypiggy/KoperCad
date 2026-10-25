//! How well does the 2D model predict what a fan wins, against the 3D cabinet
//! solver? #1483 compared one box and one fan and found the 2D gain about a
//! third low. This sweeps the fan flow, the cabinet shape, the loss and where
//! the fan and vent sit, and prints the 3D gain over the 2D gain for each.
//! Whether to calibrate the 2D model is decided from this table
//! (`docs/thermal-validation.md`). Model against model, not a measurement.
//!
//! Run with `--release -- --ignored --nocapture` for the table; the default
//! test is one small slice of it that must keep the sign and the order.

mod validation;

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

#[derive(Clone, Copy)]
struct Shape {
    name: &'static str,
    w: f64,
    h: f64,
    d: f64,
}

const SHAPES: [Shape; 3] = [
    Shape {
        name: "600x800x300",
        w: 600.0,
        h: 800.0,
        d: 300.0,
    },
    Shape {
        name: "800x1200x300",
        w: 800.0,
        h: 1200.0,
        d: 300.0,
    },
    Shape {
        name: "600x600x400",
        w: 600.0,
        h: 600.0,
        d: 400.0,
    },
];

/// Where the fan and the vent go: `true` is a fan low blowing in with the vent
/// on top, `false` an exhaust fan on top with the vent low.
type Layout = bool;

fn sealed(shape: Shape, loss_w: f64) -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: shape.w,
            height_mm: shape.h,
            depth_mm: shape.d,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 40.0,
                y: 40.0,
                w: shape.w - 90.0,
                h: shape.h - 100.0,
            },
        },
        ambient_c: 35.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: shape.w / 3.0,
                y: shape.h * 0.55,
                w: 150.0,
                h: 150.0,
            },
            height_mm: Some(100.0),
            loss_w,
            rated_max_ambient_c: Some(40.0),
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn with_fan(mut scn: Scenario, flow_m3h: f64, intake_low: Layout) -> Scenario {
    let x = scn.enclosure.width_mm / 2.0;
    let opening = |placement| Opening {
        placement,
        width_mm: 200.0,
        height_mm: 120.0,
    };
    let (fan_at, vent_at, mode) = if intake_low {
        (
            Placement::Bottom { x_mm: x },
            Placement::Top { x_mm: x },
            FanMode::Intake,
        )
    } else {
        (
            Placement::Top { x_mm: x },
            Placement::Bottom { x_mm: x },
            FanMode::Exhaust,
        )
    };
    scn.fans = vec![Fan {
        opening: opening(fan_at),
        mode,
        free_flow_m3h: flow_m3h,
        max_pressure_pa: 60.0,
    }];
    scn.vents = vec![Vent {
        opening: opening(vent_at),
        free_area_fraction: 0.6,
    }];
    scn
}

fn housing_3d(scn: &Scenario) -> f64 {
    let mut t = Tuning::default().with_cells(3000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.report_every = 50;
        l.full_snapshot_every = Duration::from_secs(3600);
    }
    CabinetSolver::new(t)
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
        .devices[0]
        .surface_c
}

fn housing_2d(scn: &Scenario) -> f64 {
    simulate_steady(scn).unwrap().devices[0].surface_c
}

/// (2D gain, 3D gain) of the fan fix, K at the device housing.
fn gains(shape: Shape, loss_w: f64, flow: f64, layout: Layout) -> (f64, f64, String) {
    let base = sealed(shape, loss_w);
    let fix = with_fan(base.clone(), flow, layout);
    let case = format!(
        "fan {flow:.0} m3/h {}, {} W, {}",
        if layout {
            "in low, vent top"
        } else {
            "out top, vent low"
        },
        loss_w,
        shape.name
    );
    let g2 = housing_2d(&base) - housing_2d(&fix);
    let g3 = housing_3d(&base) - housing_3d(&fix);
    validation::report(&case, "housing K gained, 3D vs 2D", g3, g2);
    (g2, g3, case)
}

#[test]
fn a_fan_wins_in_both_models_and_a_bigger_fan_wins_more() {
    let shape = SHAPES[0];
    let mut last = (0.0, 0.0);
    for flow in [60.0, 250.0] {
        let (g2, g3, case) = gains(shape, 120.0, flow, true);
        assert!(g2 > 0.0 && g3 > 0.0, "{case}: gains {g2:.1} 2D, {g3:.1} 3D");
        assert!(
            g2 > last.0 && g3 > last.1,
            "{case}: a bigger fan wins no more"
        );
        last = (g2, g3);
    }
}

/// The whole table. Slow in debug builds.
#[test]
#[ignore = "slow: 72 3D solves, about 18 minutes in a debug build"]
fn the_fan_gain_sweep() {
    struct Case {
        shape: &'static str,
        loss: f64,
        flow: f64,
        ratio: f64,
    }
    let mut cases = Vec::new();
    for shape in SHAPES {
        for loss in [60.0, 120.0] {
            for flow in [60.0, 120.0, 250.0] {
                for layout in [true, false] {
                    let (g2, g3, _) = gains(shape, loss, flow, layout);
                    cases.push(Case {
                        shape: shape.name,
                        loss,
                        flow,
                        ratio: g3 / g2,
                    });
                }
            }
        }
    }
    let ratios: Vec<f64> = cases.iter().map(|c| c.ratio).collect();
    let n = ratios.len();
    let mean = ratios.iter().sum::<f64>() / n as f64;
    let mut sorted = ratios.clone();
    sorted.sort_by(f64::total_cmp);
    let (lo, hi, median) = (sorted[0], sorted[n - 1], sorted[n / 2]);
    let low_in_2d = ratios.iter().filter(|r| **r > 1.0).count();
    println!(
        "VALIDATION | fan gain sweep | 3D gain / 2D gain over {n} cases | mean {mean:.2}, median {median:.2}, min {lo:.2}, max {hi:.2}; 2D reads low in {low_in_2d} of {n} | 1 | -"
    );
    // The means of the groups the documentation quotes.
    let group = |name: &str, pick: &dyn Fn(&Case) -> bool| {
        let r: Vec<f64> = cases.iter().filter(|c| pick(c)).map(|c| c.ratio).collect();
        let m = r.iter().sum::<f64>() / r.len() as f64;
        let lo = r.iter().copied().fold(f64::MAX, f64::min);
        let hi = r.iter().copied().fold(f64::MIN, f64::max);
        println!(
            "VALIDATION | fan gain sweep | {name} | mean {m:.2}, range {lo:.2} to {hi:.2} over {} | - | -",
            r.len()
        );
    };
    for flow in [60.0, 120.0, 250.0] {
        group(&format!("fan {flow:.0} m3/h"), &|c| c.flow == flow);
    }
    for shape in SHAPES {
        group(&format!("box {}", shape.name), &|c| c.shape == shape.name);
    }
    for loss in [60.0, 120.0] {
        group(&format!("{loss:.0} W"), &|c| c.loss == loss);
    }
    let worst_pair = cases
        .chunks(2)
        .map(|p| (p[0].ratio - p[1].ratio).abs())
        .fold(0.0, f64::max);
    println!(
        "VALIDATION | fan gain sweep | the two fan positions differ by at most | {worst_pair:.2} | - | -"
    );
}
