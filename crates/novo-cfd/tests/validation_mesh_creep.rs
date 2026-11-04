//! Mesh creep of the housing temperature (lane H6). The convective film of a
//! device reads the air at a fixed distance from its surface (`FILM_REF_M`),
//! not in the first air cell, so refining the grid no longer samples a
//! hotter boundary layer. Before the fix the sealed 50 W reference cabinet
//! gained about 3 K per doubling of the cell count (85 C at 1000 cells, 90 C at
//! 20000, 99 C at 60000); now it stays within about 1 K up to 20000 cells.
//! Not a measurement: the 2D engine reads 82.3 C.

mod validation;
use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;
use validation::report;

fn scenario() -> Scenario {
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
            loss_w: 50.0,
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

fn housing_rises(cells: &[usize]) -> Vec<f64> {
    let scn = scenario();
    cells
        .iter()
        .map(|&n| {
            let mut t = Tuning::default().with_cells(n);
            for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
                l.max_wall = None;
            }
            let s = CabinetSolver::new(t)
                .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
                .unwrap();
            let rise = s.devices[0].surface_c - 25.0;
            report(
                &format!("mesh creep, {n} cells"),
                "housing rise K",
                rise,
                57.3,
            );
            rise
        })
        .collect()
}

fn spread(v: &[f64]) -> f64 {
    let (lo, hi) = v
        .iter()
        .fold((f64::MAX, f64::MIN), |a, x| (a.0.min(*x), a.1.max(*x)));
    hi - lo
}

#[test]
fn housing_rise_is_flat_from_1000_to_6000_cells() {
    let r = housing_rises(&[1000, 3000, 6000]);
    assert!(spread(&r) < 2.0, "{r:?}");
}

#[test]
#[ignore = "about 90 s in release"]
fn housing_rise_is_flat_up_to_20000_cells() {
    let r = housing_rises(&[1000, 6000, 20000]);
    assert!(spread(&r) < 2.5, "{r:?}");
}
