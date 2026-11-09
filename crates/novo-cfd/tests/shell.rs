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
    // the plate sheet is published per column, and its covered mean is the reported mean
    if let Some(mean) = snap.plate_rear_c {
        let f = snap.plate_field_c.as_ref().expect("a plate field");
        assert_eq!(f.len(), snap.grid.nx * snap.grid.ny);
        let hot = f
            .iter()
            .copied()
            .filter(|t| t.is_finite())
            .fold(f32::MIN, f32::max);
        assert!(
            f64::from(hot) >= mean - 1e-3,
            "hottest column {hot} under mean {mean}"
        );
    } else {
        assert!(snap.plate_field_c.is_none());
    }
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

#[test]
fn an_extra_heat_source_heats_the_3d_cabinet_and_its_walls() {
    use novo_cfd::shell_view::shell_report;
    let plain = scenario(false);
    let base = solve(&plain);
    let mut sc = scenario(false);
    sc.params.sources.push(HeatSource {
        id: 9,
        rect: Rect {
            x: 60.0,
            y: 100.0,
            w: 300.0,
            h: 30.0,
        },
        power_w: 40.0,
        blockage: None,
    });
    let snap = solve(&sc);
    assert!(snap.progress.converged);
    let r = shell_report(&sc, &snap);
    let walls: f64 = r.faces.iter().map(|f| f.heat_w).sum();
    // 50 W device + 40 W source leave through the walls (3D balance within a few %)
    assert!((walls / 90.0 - 1.0).abs() < 0.08, "walls {walls} W of 90 W");
    assert!(snap.devices[0].air_c > base.devices[0].air_c);
}

#[test]
fn the_3d_solver_refuses_a_bad_heat_source() {
    let mut sc = scenario(false);
    sc.params.sources.push(HeatSource {
        id: 9,
        rect: Rect {
            x: 60.0,
            y: 100.0,
            w: 300.0,
            h: 30.0,
        },
        power_w: f64::NAN,
        blockage: None,
    });
    let r = small().solve(&sc, Fidelity::Coarse, &Control::new(), &mut |_| {});
    assert!(r.is_err());
}

#[test]
fn the_door_bridge_raises_the_3d_door_film() {
    use novo_cfd::solids::wall_conductances;
    use novo_thermal::shell::{DoorEdge, ShellModel};
    let a = scenario(false);
    let mut b = scenario(false);
    let mut shell = ShellModel::from_enclosure(&b.enclosure);
    shell.door_edge = Some(DoorEdge { psi_w_per_mk: 0.2 });
    b.params.shell = Some(shell);
    let (wa, wb) = (wall_conductances(&a), wall_conductances(&b));
    // z max is the door; every other face is unchanged
    assert!(wb.h[5] > wa.h[5]);
    for f in 0..5 {
        assert_eq!(wa.h[f], wb.h[f]);
    }
}
