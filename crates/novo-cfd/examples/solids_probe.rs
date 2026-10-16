//! Probe: march a small enclosure with one device and print the state.
use novo_cfd::mesh::MeshSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn main() {
    let loss: f64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(50.0);
    let cells: usize = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(6000);
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
            loss_w: loss,
            rated_max_ambient_c: None,
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    };
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: cells,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let setup = FlowSetup::new(&scn, &st).unwrap();
    let m = &setup.mesh;
    let min_dx: Vec<f64> = (0..3)
        .map(|a| m.dx[a].iter().cloned().fold(f64::MAX, f64::min))
        .collect();
    println!(
        "cells {:?} = {}, min dx {:?} m, heat {:.2} W",
        m.n,
        m.cells(),
        min_dx,
        setup.heat_w
    );
    let mut s = setup.solver();
    let t0 = std::time::Instant::now();
    let steps: usize = std::env::args()
        .nth(3)
        .and_then(|a| a.parse().ok())
        .unwrap_or(3000);
    for n in 1..=steps {
        let dt = s.max_dt(0.4).clamp(1e-4, 5.0);
        let info = s.step(dt);
        if n % 250 == 0 {
            let tmax = s.t.iter().cloned().fold(f64::MIN, f64::max);
            let vmax = s.vel.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
            let walls: f64 = (0..6).map(|f| s.wall_heat_w(f)).sum();
            println!(
                "step {n} t {:.1}s dt {:.4} dTdt {:.2e} Tmax {tmax:.1} vmax {vmax:.3} walls {walls:.2} W pit {} wall {:.1}s",
                s.time,
                dt,
                info.max_dtdt,
                info.pressure_iterations,
                t0.elapsed().as_secs_f64()
            );
        }
    }
}
