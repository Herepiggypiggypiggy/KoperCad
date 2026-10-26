//! What a fan fix wins at the housing, 2D model against the 3D cabinet solver.
//!
//! The 3D gains below were measured by `crates/novo-cfd/tests/validation_fan_gain.rs`
//! (sweep, ignored test, 3000 cells; model against model, not a measurement)
//! and are frozen here so the 2D model can be checked in milliseconds. The
//! boxes of the middle group were the ones the fan-jet speed was fitted on;
//! the other two boxes were held out (the fitted box used both losses, 60 and
//! 120 W, so the loss is not held out).

use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

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

/// (box, loss W, fan m3/h, 3D housing gain K) for a filter fan low and a vent on top.
const GAINS_3D: [(&str, f64, f64, f64); 18] = [
    ("600x800x300", 60.0, 60.0, 9.51),
    ("600x800x300", 60.0, 120.0, 15.38),
    ("600x800x300", 60.0, 250.0, 21.44),
    ("600x800x300", 120.0, 60.0, 14.79),
    ("600x800x300", 120.0, 120.0, 23.12),
    ("600x800x300", 120.0, 250.0, 32.31),
    ("800x1200x300", 60.0, 60.0, 7.81),
    ("800x1200x300", 60.0, 120.0, 11.97),
    ("800x1200x300", 60.0, 250.0, 17.84),
    ("800x1200x300", 120.0, 60.0, 10.94),
    ("800x1200x300", 120.0, 120.0, 16.12),
    ("800x1200x300", 120.0, 250.0, 26.11),
    ("600x600x400", 60.0, 60.0, 5.34),
    ("600x600x400", 60.0, 120.0, 11.25),
    ("600x600x400", 60.0, 250.0, 15.29),
    ("600x600x400", 120.0, 60.0, 9.07),
    ("600x600x400", 120.0, 120.0, 15.78),
    ("600x600x400", 120.0, 250.0, 22.24),
];

/// The same cases with the fan on top and the vent low (exhaust), from the same sweep.
const GAINS_3D_TOP: [(&str, f64, f64, f64); 18] = [
    ("600x800x300", 60.0, 60.0, 9.80),
    ("600x800x300", 60.0, 120.0, 15.32),
    ("600x800x300", 60.0, 250.0, 21.44),
    ("600x800x300", 120.0, 60.0, 14.76),
    ("600x800x300", 120.0, 120.0, 23.11),
    ("600x800x300", 120.0, 250.0, 32.42),
    ("800x1200x300", 60.0, 60.0, 7.75),
    ("800x1200x300", 60.0, 120.0, 12.26),
    ("800x1200x300", 60.0, 250.0, 17.95),
    ("800x1200x300", 120.0, 60.0, 10.98),
    ("800x1200x300", 120.0, 120.0, 16.22),
    ("800x1200x300", 120.0, 250.0, 25.74),
    ("600x600x400", 60.0, 60.0, 5.60),
    ("600x600x400", 60.0, 120.0, 9.58),
    ("600x600x400", 60.0, 250.0, 15.67),
    ("600x600x400", 120.0, 60.0, 9.40),
    ("600x600x400", 120.0, 120.0, 15.94),
    ("600x600x400", 120.0, 250.0, 22.51),
];

fn sealed(shape: &Shape, loss_w: f64) -> Scenario {
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

fn housing(scn: &Scenario) -> (f64, f64, f64) {
    let r = simulate_steady(scn).unwrap();
    let d = &r.devices[0];
    (d.surface_c, d.air_c, d.h_w_m2k)
}

/// (box, loss, flow, 2D gain, 3D gain, 2D h with the fan) for every frozen case.
fn cases() -> Vec<(&'static str, f64, f64, f64, f64, f64)> {
    GAINS_3D
        .iter()
        .map(|&(name, loss, flow, g3)| {
            let shape = SHAPES.iter().find(|s| s.name == name).unwrap();
            let base = sealed(shape, loss);
            let fix = with_fan(base.clone(), flow, true);
            let (b, _, _) = housing(&base);
            let (f, _, h) = housing(&fix);
            (name, loss, flow, b - f, g3, h)
        })
        .collect()
}

#[test]
fn a_fan_fix_wins_about_what_the_3d_solver_says_it_wins() {
    let cases = cases();
    for (name, loss, flow, g2, g3, h) in &cases {
        println!(
            "FANGAIN {name} {loss} W {flow} m3/h: 2D {g2:.1} K, 3D {g3:.1} K, ratio 3D/2D {:.2}, 2D h {h:.2}",
            g3 / g2
        );
    }
    // Before the jet speed term the ratio ran from 0.94 to 3.09 (median 1.89)
    // and grew with the fan flow. Every case, the held-out boxes included,
    // is now between 22 % below and 45 % above the 3D gain (3D / 2D from 0.69 to 1.28).
    for (name, loss, flow, g2, g3, _) in &cases {
        let ratio = g3 / g2;
        assert!(
            (0.65..=1.35).contains(&ratio),
            "{name}, {loss} W, {flow} m3/h: 2D {g2:.1} K against 3D {g3:.1} K"
        );
    }
    // The box it was fitted on is closer.
    for (name, loss, flow, g2, g3, _) in cases.iter().filter(|c| c.0 == "600x800x300") {
        assert!(
            (g3 / g2 - 1.0).abs() < 0.1,
            "fitted box, {loss} W, {flow} m3/h: 2D {g2:.1} K against 3D {g3:.1} K ({name})"
        );
    }
    // No longer flat with the flow: the gain keeps growing like the 3D gain.
    let fitted: Vec<f64> = cases
        .iter()
        .filter(|c| c.0 == "600x800x300" && c.1 == 60.0)
        .map(|c| c.3)
        .collect();
    assert!(fitted[2] > 2.0 * fitted[0], "{fitted:?}");
}

#[test]
fn without_a_fan_nothing_changes_and_the_two_fan_positions_agree() {
    let shape = &SHAPES[0];
    let base = sealed(shape, 60.0);
    assert_eq!(
        simulate_steady(&base).unwrap().devices[0].h_w_m2k.round(),
        10.0
    );
    let a = housing(&with_fan(base.clone(), 120.0, true)).0;
    let b = housing(&with_fan(base, 120.0, false)).0;
    assert!(
        (a - b).abs() < 1.0,
        "fan low {a:.1} against fan on top {b:.1}"
    );
}

/// All 36 cases of the sweep (both fan positions): the 2D fan gain is neither
/// systematically low nor high against the 3D solver. Before the jet speed term
/// it read low in 33 to 34 of 36 cases (median 3D/2D 1.89); `validation_fan_gain.rs`
/// (ignored sweep) measured, with the term in, 0.69 to 1.28, mean 0.97, median 1.01,
/// 2D low in 19 of 36 (18 with the gains frozen to two decimals). Model against model, not a measurement.
#[test]
fn over_all_36_sweep_cases_the_2d_gain_is_not_biased_against_the_3d_one() {
    let mut ratios = Vec::new();
    for (table, intake_low) in [(&GAINS_3D, true), (&GAINS_3D_TOP, false)] {
        for &(name, loss, flow, g3) in table {
            let shape = SHAPES.iter().find(|s| s.name == name).unwrap();
            let base = sealed(shape, loss);
            let (b, _, _) = housing(&base);
            let (f, _, _) = housing(&with_fan(base, flow, intake_low));
            ratios.push(g3 / (b - f));
        }
    }
    assert_eq!(ratios.len(), 36);
    let mean = ratios.iter().sum::<f64>() / 36.0;
    let mut sorted = ratios.clone();
    sorted.sort_by(f64::total_cmp);
    let median = (sorted[17] + sorted[18]) / 2.0;
    let low = ratios.iter().filter(|r| **r > 1.0).count();
    println!(
        "FANGAIN36 mean {mean:.2} median {median:.2} min {:.2} max {:.2} 2D low in {low}",
        sorted[0], sorted[35]
    );
    assert!((0.85..=1.1).contains(&mean), "mean {mean:.2}");
    assert!((0.9..=1.1).contains(&median), "median {median:.2}");
    assert!(sorted[0] > 0.65 && sorted[35] < 1.35, "{sorted:?}");
    // Neither side wins: 2D reads low in a minority-to-half of the cases, not nearly all.
    assert!((10..=26).contains(&low), "2D low in {low} of 36");
}
