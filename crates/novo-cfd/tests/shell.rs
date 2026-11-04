//! The layered shell in the 3D solver (heat lane H1).

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario(with_vent: bool) -> Scenario {
    let opening = |placement| Opening {
        placement,
        width_mm: 200.0,
        height_mm: 100.0,
    };
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
        obstacles: vec![],
        vents: if with_vent {
            vec![
                Vent {
                    opening: opening(Placement::Bottom { x_mm: 300.0 }),
                    free_area_fraction: 0.6,
                },
                Vent {
                    opening: opening(Placement::Top { x_mm: 300.0 }),
                    free_area_fraction: 0.6,
                },
            ]
        } else {
            vec![]
        },
        fans: vec![],
        params: Params::default(),
    }
}

fn small() -> CabinetSolver {
    let mut t = Tuning::default().with_cells(2000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 1e-2;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
    }
    CabinetSolver::new(t)
}

fn solve(scn: &Scenario) -> CfdSnapshot {
    small()
        .solve(scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

#[test]
fn walls_follow_the_shell_and_report_surface_temperatures() {
    use novo_cfd::shell_view::shell_report;
    use novo_thermal::shell::*;
    let plain = scenario(false);
    let snap = solve(&plain);
    assert!(snap.progress.converged);
    let r = shell_report(&plain, &snap);
    let amb = plain.ambient_c;
    let mut total = 0.0;
    for f in &r.faces {
        total += f.heat_w;
        if f.exposed_share > 0.0 {
            assert!(
                f.outer_c >= amb - 1e-6 && f.inner_c >= f.outer_c - 1e-6,
                "{f:?}"
            );
        }
    }
    // the heat the shell reports leaving is the device loss (3 % balance)
    for f in &r.faces {
        println!(
            "face {} heat {:.2} in {:.1} out {:.1}",
            f.face, f.heat_w, f.inner_air_c, f.outer_c
        );
    }
    println!(
        "plate {:?} materials {:?}",
        r.plate_rear_c,
        snap.materials.iter().map(|m| m.kind).collect::<Vec<_>>()
    );
    assert!((total / 50.0 - 1.0).abs() < 0.06, "walls {total} W of 50 W");
    // top of the box is warmer than the bottom
    assert!(r.faces[TOP].inner_air_c > r.faces[BOTTOM].inner_air_c);

    // 20 mm of mineral wool all round: the box gets much hotter.
    let mut wool = scenario(false);
    let mut shell = ShellModel::from_enclosure(&wool.enclosure);
    shell.add_insulation(LayerMaterial::MineralWool, 20.0, true);
    wool.params.shell = Some(shell);
    let hot = solve(&wool);
    assert!(hot.devices[0].air_c > snap.devices[0].air_c + 15.0);
    let rw = shell_report(&wool, &hot);
    let side = &rw.faces[LEFT];
    assert!(
        side.outer_c - amb < 0.25 * (side.inner_air_c - amb),
        "{side:?}"
    );
}

#[test]
fn default_shell_gives_the_iec_wall_films() {
    use novo_cfd::solids::wall_conductances;
    use novo_thermal::shell::ShellModel;
    let a = scenario(false);
    let mut b = scenario(false);
    b.params.shell = Some(ShellModel::from_enclosure(&b.enclosure));
    assert_eq!(wall_conductances(&a).h, wall_conductances(&b).h);
}
