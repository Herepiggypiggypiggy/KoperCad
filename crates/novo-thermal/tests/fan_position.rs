//! Where the device sits against the fan, 2D model against the 3D cabinet solver.
//!
//! The 3D gains were measured by `crates/novo-cfd/tests/validation_fan_position.rs`
//! (ignored sweep, 3000 cells, 120 W, 35 C outside; model against model, not a
//! measurement) and are frozen here for the 250 m3/h fan; the 120 m3/h rows were seen
//! once and are listed in the documentation, not pinned. Columns: box, device centre as a fraction
//! of the width and of the height from the top, fan m3/h, 3D housing gain in K.
//! A fan low in the middle blows towards a vent on top in the middle.
//!
//! This pins a known limit rather than a goal: the 2D model gives every device
//! the same fan jet speed, so it is within a factor of about three of the 3D solver for a device on the
//! line between fan and vent (the fitted band holds only near the fan end) and
//! overstates the gain for one beside it. If a change
//! moves these numbers, `docs/thermal-validation.md` ("Fan gain: where the
//! device sits") and the what-if fan note need the same change.

use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

struct Shape {
    name: &'static str,
    w: f64,
    h: f64,
    d: f64,
}

const SHAPES: [Shape; 4] = [
    Shape {
        name: "1000x600x250",
        w: 1000.0,
        h: 600.0,
        d: 250.0,
    },
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

fn sealed(shape: &Shape, loss_w: f64, centre: (f64, f64)) -> Scenario {
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
                x: centre.0 - 75.0,
                y: centre.1 - 75.0,
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

fn with_fan(mut scn: Scenario, flow_m3h: f64, intake_low: bool) -> Scenario {
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

const GAINS_3D: [(&str, f64, f64, f64, f64); 20] = [
    ("600x800x300", 0.50, 0.20, 250.0, 39.90),
    ("600x800x300", 0.50, 0.55, 250.0, 31.60),
    ("600x800x300", 0.50, 0.85, 250.0, 42.50),
    ("600x800x300", 0.20, 0.55, 250.0, 23.60),
    ("600x800x300", 0.80, 0.55, 250.0, 21.00),
    ("800x1200x300", 0.50, 0.20, 250.0, 34.00),
    ("800x1200x300", 0.50, 0.55, 250.0, 7.50),
    ("800x1200x300", 0.50, 0.85, 250.0, 29.90),
    ("800x1200x300", 0.20, 0.55, 250.0, 8.70),
    ("800x1200x300", 0.80, 0.55, 250.0, 8.80),
    ("600x600x400", 0.50, 0.20, 250.0, 21.30),
    ("600x600x400", 0.50, 0.55, 250.0, 14.80),
    ("600x600x400", 0.50, 0.85, 250.0, 28.40),
    ("600x600x400", 0.20, 0.55, 250.0, 17.00),
    ("600x600x400", 0.80, 0.55, 250.0, 15.40),
    ("1000x600x250", 0.50, 0.20, 250.0, 58.00),
    ("1000x600x250", 0.50, 0.55, 250.0, 32.20),
    ("1000x600x250", 0.50, 0.85, 250.0, 36.70),
    ("1000x600x250", 0.20, 0.55, 250.0, -4.20),
    ("1000x600x250", 0.80, 0.55, 250.0, -3.80),
];

fn ratio(name: &str, fx: f64, fy: f64, flow: f64, g3: f64) -> f64 {
    let sh = SHAPES.iter().find(|s| s.name == name).unwrap();
    let base = sealed(sh, 120.0, (sh.w * fx, sh.h * fy));
    let g2 = housing(&base) - housing(&with_fan(base.clone(), flow, true));
    g3 / g2
}

fn housing(scn: &Scenario) -> f64 {
    simulate_steady(scn).unwrap().devices[0].surface_c
}

#[test]
fn the_2d_gain_follows_a_device_on_the_jet_line_and_overstates_one_beside_it() {
    let ratios: Vec<(f64, f64)> = std::thread::scope(|sc| {
        let jobs: Vec<_> = GAINS_3D
            .iter()
            .map(|&(n, fx, fy, flow, g3)| sc.spawn(move || (fx, ratio(n, fx, fy, flow, g3))))
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    });
    let (mut on, mut off) = (Vec::new(), Vec::new());
    for (fx, r) in ratios {
        if fx == 0.5 { on.push(r) } else { off.push(r) }
    }
    on.sort_by(f64::total_cmp);
    let median = on[on.len() / 2];
    assert!(
        (0.8..=1.2).contains(&median),
        "on the line the median 3D/2D is {median:.2}, was 1.00 when this was written"
    );
    // The widest misses on the line are the middle of the tall box (0.2 to 0.4)
    // and the near-vent end of the wide box (1.3).
    assert!(on[0] > 0.1 && on[on.len() - 1] < 1.6, "{on:?}");
    for r in &off {
        assert!(
            *r < 0.85,
            "beside the jet the 2D gain used to be too high; now {r:.2}: {off:?}"
        );
    }
}
