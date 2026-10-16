//! Wall time and result of `CabinetSolver` per level on the 600x800x300 mm
//! test cabinet with one 50 W device: `cabinet_levels [march]`.
use std::time::Instant;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn main() {
    let scn = Scenario {
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
    };
    let tuning = if std::env::args().any(|a| a == "march") {
        Tuning::default().marched()
    } else {
        Tuning::default()
    };
    let solver = CabinetSolver::new(tuning);
    for f in [Fidelity::Coarse, Fidelity::Medium, Fidelity::Fine] {
        let t0 = Instant::now();
        let s = solver.solve(&scn, f, &Control::new(), &mut |_| {}).unwrap();
        println!(
            "{f:?}: {:.1} s, {} iterations, converged {}, balance {:.2}%, device surface {:.2} C, max T {:.2} C",
            t0.elapsed().as_secs_f64(),
            s.progress.iteration,
            s.progress.converged,
            100.0 * s.progress.energy_balance_rel,
            s.devices[0].surface_c,
            s.max_temperature_c()
        );
    }
}
