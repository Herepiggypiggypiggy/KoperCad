//! Device and mounting plate temperatures of the 3D engine against the 2D
//! engine of `novo-thermal`, on several meshes (lane CFD-C2). Both use the
//! same contact resistance to the plate (`plate_contact_h`), the same device
//! film correlation and the same IEC rear coefficient. Run `-- --nocapture`
//! to see the `VALIDATION` rows.

mod validation;
use novo_cfd::Control;
use novo_cfd::mesh::MeshSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;
use validation::report;

fn scenario(loss_w: f64) -> Scenario {
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
            loss_w,
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

/// Same device, same contact, same film: the housing temperature and the
/// share of the heat that goes into the plate agree with the 2D engine to a
/// stated band and do not move with the mesh. Not a measurement: the 2D engine
/// is the reference, and the 3D air next to the device is a few kelvin hotter
/// (the plume rises over it), so the housing sits a little above.
#[test]
fn device_and_plate_follow_the_2d_engine_on_every_mesh() {
    let scn = scenario(50.0);
    let two_d = simulate_steady(&scn).unwrap();
    let d2 = &two_d.devices[0];
    let mut surfaces = Vec::new();
    for cells in [700, 1000, 1400] {
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
            0.001,
            (3000.0, 40000),
            &Control::new(),
            |_, _| {},
            0,
        );
        assert!(rep.converged && rep.imbalance.abs() < 0.005, "{rep:?}");
        let d3 = &setup.device_report(&s)[0];
        let share = d3.to_plate_w / 50.0;
        let case = format!("device 50 W, {cells} cells");
        let e_t = report(
            &case,
            "housing rise K vs 2D",
            d3.surface_c - 25.0,
            d2.surface_c - 25.0,
        );
        println!(
            "VALIDATION | {case} | plate share | {share:.3} | {:.3} | {:+.3}",
            d2.plate_share,
            share - d2.plate_share
        );
        assert!(e_t.abs() < 0.12, "housing {e_t} at {cells} cells");
        assert!(
            (share - d2.plate_share).abs() < 0.04,
            "plate share {share} at {cells}"
        );
        surfaces.push(d3.surface_c);
    }
    let (lo, hi) = surfaces
        .iter()
        .fold((f64::MAX, f64::MIN), |a, v| (a.0.min(*v), a.1.max(*v)));
    assert!(hi - lo < 2.0, "housing depends on the mesh: {surfaces:?}");
}
