//! `heat_compare::diff`: sign, plates of different size, results that
//! cannot be compared.

use std::sync::Arc;

use novo_cfd::{Control, Fidelity, Solver, StubSolver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::{Device, Enclosure, Params, Scenario};

use super::heat_compare::{colour, diff};
use super::heat_run::Solution;

fn scenario(plate_w: f64, plate_h: f64, loss_w: f64) -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: plate_w + 104.0,
            height_mm: plate_h + 104.0,
            depth_mm: 304.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 50.0,
                y: 50.0,
                w: plate_w,
                h: plate_h,
            },
        },
        ambient_c: 35.0,
        devices: vec![Device {
            id: 1,
            footprint: Rect {
                x: 100.0,
                y: 100.0,
                w: 120.0,
                h: 160.0,
            },
            height_mm: Some(80.0),
            loss_w,
            rated_max_ambient_c: None,
            rated_max_surface_c: Some(70.0),
            heat_capacity_j_per_k: None,
        }],
        obstacles: Vec::new(),
        vents: Vec::new(),
        fans: Vec::new(),
        params: Params::default(),
    }
}

fn solve(scn: &Scenario, id: u64) -> Solution {
    let snap = StubSolver
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap();
    let p = scn.enclosure.plate;
    Solution {
        snapshot: Arc::new(snap),
        id,
        scenario_key: 0,
        layout_key: 0,
        ambient: scn.ambient_c,
        plate: [p.x, p.y, p.w, p.h],
        messages: Vec::new(),
        wire_notes: Vec::new(),
    }
}

fn b_plate(s: &Solution) -> novo_heatview::Scalar {
    let [x, y, w, h] = s.plate;
    novo_heatview::cfd::as_2d(
        &s.snapshot,
        novo_heatview::cfd::Window { u0: x, v0: y, w, h },
        10.0,
    )
    .unwrap()
    .temperature
}

#[test]
fn a_cooler_b_gives_negative_changes_and_a_falling_peak() {
    let a = solve(&scenario(600.0, 800.0, 80.0), 1);
    let b = solve(&scenario(600.0, 800.0, 10.0), 2);
    let d = diff(&a, &b, &b_plate(&b), 10.0).unwrap();
    assert!(d.field.extent().unwrap().0 < -0.1);
    assert!(d.rows[0].delta() < 0.0);
    assert!(d.rows[0].a > d.rows[0].b);
    assert_eq!(d.rows[0].id, 1);
    assert_eq!(
        d.a_surface.len(),
        1,
        "A's device readings for the side view"
    );
    assert_eq!(d.a_surface[0].0, 1);
    assert!((d.a_surface[0].1 - d.rows[0].a).abs() < 1e-9);
    assert!(d.extent >= 1.0);
    let after = d.verdict.split('→').nth(1).unwrap();
    let peak_b: f64 = after.trim().split(' ').next().unwrap().parse().unwrap();
    assert!(peak_b < d.rows[0].a);
}

#[test]
fn swapping_a_and_b_flips_the_sign() {
    let a = solve(&scenario(600.0, 800.0, 80.0), 1);
    let b = solve(&scenario(600.0, 800.0, 10.0), 2);
    let ab = diff(&a, &b, &b_plate(&b), 10.0).unwrap();
    let ba = diff(&b, &a, &b_plate(&a), 10.0).unwrap();
    assert!(ab.rows[0].delta() < 0.0 && ba.rows[0].delta() > 0.0);
    assert!((ab.rows[0].delta() + ba.rows[0].delta()).abs() < 1e-9);
}

#[test]
fn plates_of_different_size_are_compared_on_a_s_grid() {
    let a = solve(&scenario(600.0, 800.0, 80.0), 1);
    let b = solve(&scenario(400.0, 500.0, 80.0), 2);
    let d = diff(&a, &b, &b_plate(&b), 10.0).unwrap();
    assert_eq!(d.field.grid, d.a_plate.grid);
    assert_eq!((d.field.grid.cols, d.field.grid.rows), (60, 80));
}

#[test]
fn a_result_that_cannot_be_sliced_gives_no_difference() {
    let mut a = solve(&scenario(600.0, 800.0, 80.0), 1);
    let b = solve(&scenario(600.0, 800.0, 10.0), 2);
    let mut broken = (*a.snapshot).clone();
    broken.temperature_c.pop();
    a.snapshot = Arc::new(broken);
    assert!(diff(&a, &b, &b_plate(&b), 10.0).is_none());
}

#[test]
fn results_without_a_common_device_say_so() {
    let a = solve(&scenario(600.0, 800.0, 80.0), 1);
    let mut other = scenario(600.0, 800.0, 80.0);
    other.devices[0].id = 9;
    let b = solve(&other, 2);
    let d = diff(&a, &b, &b_plate(&b), 10.0).unwrap();
    assert!(d.rows.is_empty());
    assert_eq!(d.verdict, "No device is in both results.");
}

#[test]
fn colours_are_symmetric_around_no_change() {
    let mid = colour(0.0, 4.0);
    assert_eq!(colour(f32::NAN, 4.0), mid);
    assert_eq!(
        colour(100.0, 4.0),
        colour(4.0, 4.0),
        "clamped to the extent"
    );
}
