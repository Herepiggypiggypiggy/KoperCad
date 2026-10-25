//! Where in the box the device sits, and how well the 2D fan gain follows the
//! 3D solver. `validation_fan_gain.rs` fixed the device on the line between the
//! fan and the vent (about 25 mm beside it); this moves it along that line and
//! sideways. Positions are fractions of the width and of the height counted
//! from the top, so a fan low in the box blows towards a vent on top. 120 W,
//! 35 C outside. Results and what they mean: `docs/thermal-validation.md`,
//! "Fan gain: where the device sits". Model against model, not a measurement.
//! Ignored, slow: about 35 s per 3D solve in a debug build.

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

/// Where the fan and the vent go: `true` is a fan low blowing in with the vent
/// on top, `false` an exhaust fan on top with the vent low.
type Layout = bool;

fn sealed(shape: Shape, loss_w: f64, fx: f64, fy: f64) -> Scenario {
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
                x: shape.w * fx - 75.0,
                y: shape.h * fy - 75.0,
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

#[test]
#[ignore = "slow: 3D solves"]
fn the_fan_position_sweep() {
    sweep(
        &[
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
        ],
        &[120.0, 250.0],
    );
}

/// A box that none of the fitting used: wider and shallower.
#[test]
#[ignore = "slow: 3D solves"]
fn the_fourth_box() {
    let shape = Shape {
        name: "1000x600x250",
        w: 1000.0,
        h: 600.0,
        d: 250.0,
    };
    sweep(&[shape], &[250.0]);
}

fn sweep(shapes: &[Shape], flows: &[f64]) {
    for &shape in shapes {
        for (fx, fy) in [
            (0.5, 0.2),
            (0.5, 0.55),
            (0.5, 0.85),
            (0.2, 0.55),
            (0.8, 0.55),
        ] {
            for &flow in flows {
                let base = sealed(shape, 120.0, fx, fy);
                let fix = with_fan(base.clone(), flow, true);
                let (b2, f2) = (housing_2d(&base), housing_2d(&fix));
                let (b3, f3) = (housing_3d(&base), housing_3d(&fix));
                let (g2, g3) = (b2 - f2, b3 - f3);
                let off = shape.w * (fx - 0.5).abs();
                println!(
                    "POS | {} | x {fx:.2} y {fy:.2} | {flow:.0} m3/h | off the line {off:.0} mm | 3D {g3:.1} K | 2D {g2:.1} K | ratio {:.2} | sealed 3D {b3:.1} 2D {b2:.1} C | fan 3D {f3:.1} 2D {f2:.1} C",
                    shape.name,
                    g3 / g2
                );
            }
        }
    }
}
