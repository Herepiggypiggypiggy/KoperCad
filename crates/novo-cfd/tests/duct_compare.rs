//! What a wire duct does to the flow and the temperatures: no duct, a slotted
//! duct (the model), a closed duct, at the real height and at the height the
//! solver used before (a share of the whole air depth). Prints the table the
//! duct study quotes; asserts the physics, not the numbers.

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::solids::DuctModel;
use novo_cfd::{CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

const DUCT_H_MM: f64 = 60.0;

fn scenario(ducts: &[(Rect, Option<f64>)]) -> Scenario {
    let opening = |placement| Opening {
        placement,
        width_mm: 200.0,
        height_mm: 100.0,
    };
    let params = Params::default();
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
            rated_max_ambient_c: Some(60.0),
            rated_max_surface_c: Some(70.0),
            heat_capacity_j_per_k: None,
        }],
        obstacles: ducts
            .iter()
            .map(|(rect, blockage)| Obstacle {
                kind: ObstacleKind::Duct,
                rect: *rect,
                blockage: *blockage,
            })
            .collect(),
        vents: vec![
            Vent {
                opening: opening(Placement::Bottom { x_mm: 300.0 }),
                free_area_fraction: 0.6,
            },
            Vent {
                opening: opening(Placement::Top { x_mm: 300.0 }),
                free_area_fraction: 0.6,
            },
        ],
        fans: vec![],
        params,
    }
}

/// Two horizontal ducts across the chimney path, one above and one below
/// the device.
fn ducts(blockage: Option<f64>) -> Vec<(Rect, Option<f64>)> {
    [250.0, 700.0]
        .iter()
        .map(|y| {
            (
                Rect {
                    x: 80.0,
                    y: *y,
                    w: 450.0,
                    h: 60.0,
                },
                blockage,
            )
        })
        .collect()
}

fn run(scn: &Scenario, duct: DuctModel) -> CfdSnapshot {
    let mut t = Tuning::default().with_cells(4000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 5e-3;
        l.report_every = 50;
        l.full_snapshot_every = Duration::ZERO;
    }
    CabinetSolver::new(t)
        .with_duct(duct)
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

#[test]
fn duct_study_none_slotted_closed_real_height_and_old_height() {
    let depth_free = 296.0 - Params::default().plate_thickness_mm;
    let real = Some(DUCT_H_MM / depth_free);
    let cases: Vec<(&str, Scenario, DuctModel)> = vec![
        ("no duct", scenario(&[]), DuctModel::default()),
        (
            "slotted, 60 mm",
            scenario(&ducts(real)),
            DuctModel::default(),
        ),
        ("closed, 60 mm", scenario(&ducts(real)), DuctModel::closed()),
        (
            "slotted, old 70 % depth",
            scenario(&ducts(None)),
            DuctModel::default(),
        ),
        (
            "closed, old 70 % depth",
            scenario(&ducts(None)),
            DuctModel::closed(),
        ),
    ];
    let mut rows = Vec::new();
    for (name, scn, duct) in &cases {
        let s = run(scn, *duct);
        let d = &s.devices[0];
        println!(
            "DUCTSTUDY | {name:26} | device surface {:5.1} C | air at device {:5.1} C | max {:5.1} C | balance {:+.3} | converged {}",
            d.surface_c,
            d.air_c,
            s.max_temperature_c(),
            s.progress.energy_balance_rel,
            s.progress.converged
        );
        rows.push(d.surface_c);
    }
    let (none, slotted, closed, old_slotted, old_closed) =
        (rows[0], rows[1], rows[2], rows[3], rows[4]);
    // a slotted duct at its real height changes the device surface little
    // against no duct; a closed block resists more than a slotted one, and
    // the tall block of the old model resists most
    assert!((slotted - none).abs() < 3.0, "{slotted} vs {none}");
    assert!(closed >= slotted - 0.3, "{closed} vs {slotted}");
    assert!(old_closed >= closed - 0.3, "{old_closed} vs {closed}");
    assert!(old_slotted >= slotted - 0.3, "{old_slotted} vs {slotted}");
}
