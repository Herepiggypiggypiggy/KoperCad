//! Heat of the wires in a duct (`Obstacle::loss_w`) in the 3D solver: the
//! duct box releases it into its cells, the grid receives exactly that, and
//! a sealed cabinet with only wire heat balances and warms in proportion.

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::mesh::MeshSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_cfd::{CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn cabinet(duct_w: f64) -> Scenario {
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
        devices: vec![],
        obstacles: vec![Obstacle {
            kind: ObstacleKind::Duct,
            rect: Rect {
                x: 100.0,
                y: 400.0,
                w: 400.0,
                h: 60.0,
            },
            blockage: Some(0.2),
            loss_w: duct_w,
        }],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn solve(scn: &Scenario) -> CfdSnapshot {
    let mut t = Tuning::default().with_cells(2000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 1e-2;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
    }
    CabinetSolver::new(t)
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

#[test]
fn the_grid_receives_exactly_the_wire_heat_of_the_duct() {
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: 3000,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    for w in [0.0, 12.5, 80.0] {
        let setup = FlowSetup::new(&cabinet(w), &st).unwrap();
        assert!(
            (setup.heat_w - w).abs() <= 1e-6 * w.max(1.0),
            "{} vs {w}",
            setup.heat_w
        );
    }
}

#[test]
fn a_sealed_cabinet_with_only_wire_heat_balances_and_warms_with_the_heat() {
    let a = solve(&cabinet(20.0));
    let b = solve(&cabinet(40.0));
    for s in [&a, &b] {
        assert!(s.is_consistent());
        assert!(s.progress.converged, "{:?}", s.progress);
        assert!(
            s.progress.energy_balance_rel < 0.03,
            "{}",
            s.progress.energy_balance_rel
        );
    }
    let rise = |s: &CfdSnapshot| f64::from(s.max_temperature_c()) - 25.0;
    println!(
        "WIRE HEAT | max rise 20 W {:.2} K, 40 W {:.2} K",
        rise(&a),
        rise(&b)
    );
    assert!(rise(&a) > 0.5);
    // Linear conduction and convection around the same duct: twice the heat,
    // about twice the rise (natural convection makes it a little less).
    let ratio = rise(&b) / rise(&a);
    assert!((1.6..2.2).contains(&ratio), "{ratio}");
}
