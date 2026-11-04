//! Evidence for the interior fan in the 3D cabinet solver (lane H8): the same
//! sealed 120 W box solved at the Coarse level without a fan inside, and with
//! a circulation fan beside the device. Without the fan the result must be
//! what main gives (printed, compared by hand with a run on main); with it the
//! device runs a little cooler (the film does not read the jet yet). Model, not measurement. Ignored, slow (3D solves).

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

#[derive(Clone, Copy)]
struct Shape {
    name: &'static str,
    w: f64,
    h: f64,
    d: f64,
}

fn sealed(shape: Shape, loss_w: f64, fx: f64, fy: f64) -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: shape.w,
            height_mm: shape.h,
            depth_mm: shape.d,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 40.0,
                y: 40.0,
                w: shape.w - 90.0,
                h: shape.h - 100.0,
            },
        },
        ambient_c: 35.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: shape.w * fx - 75.0,
                y: shape.h * fy - 75.0,
                w: 150.0,
                h: 150.0,
            },
            height_mm: Some(100.0),
            loss_w,
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

fn housing_3d(scn: &Scenario) -> f64 {
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

fn fan_beside(scn: &mut Scenario, dir: BlowDirection, flow_m3h: f64) {
    let f = &scn.devices[0].footprint;
    scn.params.circulation_fans = vec![CirculationFan {
        // beside the device, in the interior frame
        x_mm: f.x + f.w + 90.0,
        y_mm: f.y + 0.5 * f.h,
        diameter_mm: 120.0,
        direction: dir,
        free_flow_m3h: flow_m3h,
        max_pressure_pa: 40.0,
        installed_factor: 0.8,
        z_mm: Some(60.0),
    }];
}

#[test]
#[ignore = "slow: 3D solves"]
fn a_circulation_fan_cools_the_device_in_the_3d_solver_and_nothing_else_changes() {
    let shape = Shape {
        name: "600x800x300",
        w: 600.0,
        h: 800.0,
        d: 300.0,
    };
    let base = sealed(shape, 120.0, 0.35, 0.5);
    let off = housing_3d(&base);
    let off_again = housing_3d(&base);
    let mut fan = base.clone();
    fan_beside(&mut fan, BlowDirection::Left, 250.0);
    let on = housing_3d(&fan);
    println!(
        "INTERIOR FAN CABINET | {} | no fan {off:.2} C | no fan again {off_again:.2} C | fan beside, 250 m3/h {on:.2} C | change {:+.2} K",
        shape.name,
        on - off
    );
    assert_eq!(off, off_again, "the run without a fan is repeatable");
    // The air is stirred, but the device film still comes from the natural
    // convection correlation, not from the resolved jet (the next step), so the
    // gain is small: cooler, and by less than 5 K.
    assert!(on < off && on > off - 5.0, "{on} against {off}");
}
