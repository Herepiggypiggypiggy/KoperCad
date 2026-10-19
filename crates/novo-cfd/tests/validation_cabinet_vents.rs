//! A vented cabinet with three devices of different loss against the 2D
//! engine of `novo-thermal`: per-device housing rise, which device is the
//! hottest, the plate share, and the heat balance. Natural convection only
//! (louvre low, louvre high). The 2D engine is the reference, not a
//! measurement; the 3D air above a source is hotter than the 2D mixed air, so
//! the housings sit above the 2D estimate (see the vented-box section of
//! `docs/thermal-validation.md`). Run `-- --nocapture` to see the `VALIDATION` rows.

mod validation;
use novo_cfd::Control;
use novo_cfd::mesh::MeshSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;
use validation::report;

fn device(id: u64, x: f64, y: f64, w: f64, h: f64, loss_w: f64) -> Device {
    Device {
        id,
        footprint: Rect { x, y, w, h },
        height_mm: Some(100.0),
        loss_w,
        rated_max_ambient_c: None,
        rated_max_surface_c: None,
        heat_capacity_j_per_k: None,
    }
}

fn scenario() -> Scenario {
    let vent = |placement| Vent {
        opening: Opening {
            placement,
            width_mm: 200.0,
            height_mm: 120.0,
        },
        free_area_fraction: 0.6,
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
        devices: vec![
            device(1, 80.0, 450.0, 150.0, 150.0, 40.0),
            device(2, 330.0, 300.0, 150.0, 150.0, 60.0),
            device(3, 120.0, 100.0, 150.0, 100.0, 20.0),
        ],
        obstacles: vec![],
        vents: vec![
            vent(Placement::Bottom { x_mm: 300.0 }),
            vent(Placement::Top { x_mm: 300.0 }),
        ],
        fans: vec![],
        params: Params::default(),
    }
}

fn check(cells_list: &[usize]) {
    let scn = scenario();
    let total: f64 = scn.devices.iter().map(|d| d.loss_w).sum();
    let two_d = simulate_steady(&scn).unwrap();
    let amb = scn.ambient_c;
    let hottest_2d = two_d
        .devices
        .iter()
        .max_by(|a, b| a.surface_c.total_cmp(&b.surface_c))
        .unwrap()
        .id;
    for &cells in cells_list {
        let st = SolidsSettings {
            mesh: MeshSettings {
                target_cells: cells,
                ..MeshSettings::default()
            },
            ..SolidsSettings::default()
        };
        let setup = FlowSetup::new(&scn, &st).unwrap();
        let mut s = setup.solver();
        let rep = setup.run_steady(
            &mut s,
            0.002,
            (3000.0, 40000),
            &Control::new(),
            |_, _| {},
            0,
        );
        assert!(rep.converged, "{rep:?}");
        let case = format!("vented cabinet {total} W, 3 devices, {cells} cells");
        println!(
            "VALIDATION | {case} | heat balance imbalance | {:+.4} | 0 | -",
            rep.imbalance
        );
        assert!(rep.imbalance.abs() < 0.02, "imbalance {}", rep.imbalance);
        let d3 = setup.device_report(&s);
        let hottest_3d = d3
            .iter()
            .max_by(|a, b| a.surface_c.total_cmp(&b.surface_c))
            .unwrap()
            .id;
        for d in &d3 {
            let d2 = two_d.devices.iter().find(|x| x.id == d.id).unwrap();
            let e = report(
                &case,
                &format!("device {} housing rise K vs 2D", d.id),
                d.surface_c - amb,
                d2.surface_c - amb,
            );
            println!(
                "VALIDATION | {case} | device {} plate share | {:.3} | {:.3} | {:+.3}",
                d.id,
                d.to_plate_w / scn.devices.iter().find(|x| x.id == d.id).unwrap().loss_w,
                d2.plate_share,
                d.to_plate_w / scn.devices.iter().find(|x| x.id == d.id).unwrap().loss_w
                    - d2.plate_share
            );
            // measured +4 to +6 %
            assert!(e.abs() < 0.12, "device {} housing {e}", d.id);
        }
        println!("VALIDATION | {case} | hottest device | {hottest_3d} | {hottest_2d} | -");
        assert_eq!(hottest_3d, hottest_2d, "different hottest device");
    }
}

#[test]
fn three_devices_in_a_vented_cabinet_follow_the_2d_engine() {
    check(&[1800]);
}

/// Finer mesh, about a minute: run with `--ignored`.
#[test]
#[ignore = "slow"]
fn three_devices_in_a_vented_cabinet_do_not_move_on_a_finer_mesh() {
    check(&[3500]);
}
