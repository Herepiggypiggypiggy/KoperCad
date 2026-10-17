//! Radiation coupled to the flow solver: energy balance and heat split.

use novo_cfd::Control;
use novo_cfd::mesh::MeshSettings;
use novo_cfd::rad::RadSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

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

struct Run {
    surface_c: f64,
    radiation_w: f64,
    to_plate_w: f64,
    imbalance: f64,
    converged: bool,
    wall_out_w: f64,
}

fn run(radiation: bool) -> Run {
    let scn = scenario(50.0);
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: 1500,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let mut setup = FlowSetup::new(&scn, &st).unwrap();
    if radiation {
        let rs = RadSettings {
            view_factors: novo_cfd::rad::ViewFactorSettings {
                rays_per_patch: 800,
                ..Default::default()
            },
            ..RadSettings::default()
        };
        assert!(setup.enable_radiation(&scn, &rs));
    }
    let mut s = setup.solver();
    let rep = setup.run_steady(
        &mut s,
        0.001,
        (4000.0, 60000),
        &Control::new(),
        |_, _| {},
        0,
    );
    let d = &setup.device_report(&s)[0];
    println!(
        "RAD | radiation {radiation}: surface {:.1} C, to plate {:.1} W, radiated {:.1} W, walls {:.1} W, imbalance {:+.3} %, {} steps",
        d.surface_c,
        d.to_plate_w,
        d.radiation_w,
        rep.heat_out_walls_w,
        100.0 * rep.imbalance,
        rep.steps
    );
    Run {
        surface_c: d.surface_c,
        radiation_w: d.radiation_w,
        to_plate_w: d.to_plate_w,
        imbalance: rep.imbalance,
        converged: rep.converged,
        wall_out_w: rep.heat_out_walls_w,
    }
}

#[test]
fn coupled_radiation_balances_and_takes_a_share_of_the_device_heat() {
    let off = run(false);
    let on = run(true);
    assert!(off.converged && on.converged);
    assert!(on.imbalance.abs() < 0.005, "imbalance {}", on.imbalance);
    assert_eq!(off.radiation_w, 0.0);
    // A 50 W device in a sealed cabinet: radiation carries a clear share,
    // but not most of it (the plate and the air take the rest).
    let share = on.radiation_w / 50.0;
    assert!((0.1..0.6).contains(&share), "radiated {} W", on.radiation_w);
    // All the heat still leaves through the walls.
    assert!((on.wall_out_w - 50.0).abs() < 0.5, "{}", on.wall_out_w);
    // The explicit exchange replaces the 2D film's radiation, which counts
    // only half of the surface (view factor 0.5) and radiates to the air
    // temperature. Seeing the cooler walls and plate lets the device give
    // off more by radiation, so it runs cooler, by a few ten kelvin at most.
    let drop = off.surface_c - on.surface_c;
    assert!(
        (2.0..25.0).contains(&drop),
        "{} vs {}",
        on.surface_c,
        off.surface_c
    );
    assert!(on.to_plate_w > 0.0 && off.to_plate_w > 0.0);
}
