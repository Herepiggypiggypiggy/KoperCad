//! Mesh-independence study of the sealed reference cabinet (one 50 W device).
//! `cargo run --release -p novo-cfd --example mesh_study -- 2000 6000 20000`
use std::time::Instant;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

pub fn scenario() -> Scenario {
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
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn main() {
    let scn = scenario();
    let d2 = simulate_steady_with(
        &scn,
        Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap();
    let d = &d2.devices[0];
    println!(
        "2D engine: surface {:.1} C, plate share {:.2}",
        d.surface_c, d.plate_share
    );
    let sizes: Vec<usize> = std::env::args().skip(1).filter_map(|s| s.parse().ok()).collect();
    for n in sizes {
        let t0 = Instant::now();
        let mut tuning = Tuning::default().with_cells(n);
        tuning.coarse.max_wall = None;
        let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<f64>().ok());
        let mut m = novo_cfd::mesh::MeshSettings::default();
        if let Some(v) = env("NEAR") {
            m.near_ratio = v;
        }
        if let Some(v) = env("GROWTH") {
            m.growth = v;
        }
        if let Some(v) = env("NEED") {
            m.min_cells_across = v as usize;
        }
        if let Some(v) = env("THIN") {
            m.thin_mm = v;
        }
        tuning.coarse.mesh = Some(m);
        let r = CabinetSolver::new(tuning).solve(&scn, Fidelity::Coarse, &Control::new(), &mut |_| {});
        match r {
            Ok(s) => {
                let g = &s.grid;
                let dv = &s.devices[0];
                println!(
                    "{n:>7} -> {:>6} cells {}x{}x{} | surface {:.1} air {:.1} plate share {:.2} | steps {} conv {} | {:.0}s | notes: {}",
                    g.cells(), g.nx, g.ny, g.nz,
                    dv.surface_c, dv.air_c, dv.plate_share,
                    s.progress.iteration, s.progress.converged,
                    t0.elapsed().as_secs_f64(),
                    s.notes.iter().filter(|x| x.contains("resolved")).cloned().collect::<Vec<_>>().join("; ")
                );
            }
            Err(e) => println!("{n}: {e}"),
        }
    }
}
