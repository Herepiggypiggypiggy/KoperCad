//! Does the 3D cabinet solver order the fixes of the cooling what-if the way
//! the 2D model does? The what-if ranks fixes with the 2D engine; this test
//! solves the cabinet as it is and with each vent or fan fix of the answer in
//! 3D, and compares the housing temperature of the hot device. Run with
//! `--release -- --nocapture` to see the `VALIDATION` rows.
//!
//! It is a comparison of two models, not a measurement: `ENCLOSURE_VALIDATED`
//! stays false. A pair of fixes the 2D model puts less than `NOISE_K` apart
//! is not ordered: the solver itself moves that much with the mesh.

mod validation;

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::whatif::{self, Conditions, Fix, Settings};
use novo_thermal::*;

/// 2D differences below this are not asked to agree.
const NOISE_K: f64 = 3.0;

/// The 600 x 800 x 300 mm sealed box, one 120 W device that is over its limit.
fn hot_box() -> Scenario {
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
        ambient_c: 35.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: 200.0,
                y: 450.0,
                w: 150.0,
                h: 150.0,
            },
            height_mm: Some(100.0),
            loss_w: 120.0,
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

fn solve_housing_c(scn: &Scenario) -> f64 {
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

#[test]
fn the_3d_solver_orders_the_what_if_fixes_like_the_2d_model() {
    let scn = hot_box();
    let wi = whatif::run(
        &scn,
        &Conditions::as_entered(&scn),
        None,
        &Settings::quick(),
        &whatif::Control::default(),
    )
    .unwrap();
    // The cabinet as it is, then every fix that adds openings.
    let mut cases: Vec<(String, f64, f64)> = vec![(
        "as it is".into(),
        wi.baseline.surface_c[0],
        solve_housing_c(&scn),
    )];
    for c in &wi.choices {
        if let Fix::Openings { candidate } = &c.fix {
            cases.push((
                c.fix.title(),
                c.reading.surface_c[0],
                solve_housing_c(&candidate.apply(&scn)),
            ));
        }
    }
    assert!(
        cases.len() >= 3,
        "the what-if offered {} cases",
        cases.len()
    );
    for (name, two_d, three_d) in &cases {
        validation::report(
            &format!("what-if: {name}"),
            "housing C, 3D vs 2D",
            *three_d,
            *two_d,
        );
    }
    // What each fix wins, 3D against 2D: the number a person acts on. The two
    // models do not agree on its size (the 2D model reads the fan fix low),
    // so this is reported, and only its sign is asserted.
    let (base_2d, base_3d) = (cases[0].1, cases[0].2);
    for (name, two_d, three_d) in &cases[1..] {
        let (g2, g3) = (base_2d - two_d, base_3d - three_d);
        validation::report(
            &format!("what-if: {name}"),
            "housing K gained, 3D vs 2D",
            g3,
            g2,
        );
        assert!(
            g2 > 0.0 && g3 > 0.0,
            "'{name}': gains {g2:.1} 2D, {g3:.1} 3D"
        );
    }
    let mut compared = 0;
    for (i, a) in cases.iter().enumerate() {
        for b in &cases[i + 1..] {
            if (a.1 - b.1).abs() < NOISE_K {
                continue;
            }
            compared += 1;
            assert_eq!(
                a.1 < b.1,
                a.2 < b.2,
                "'{}' ({:.1} 2D, {:.1} 3D) vs '{}' ({:.1} 2D, {:.1} 3D) are ordered differently",
                a.0,
                a.1,
                a.2,
                b.0,
                b.1,
                b.2
            );
        }
    }
    println!(
        "VALIDATION | what-if ordering | pairs more than {NOISE_K} K apart in 2D that 3D orders the same | {compared} of {compared} | all | 0"
    );
    assert!(compared >= 2, "too few pairs to say anything: {compared}");
}
