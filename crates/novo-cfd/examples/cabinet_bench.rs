//! Times the cabinet solver on a reference cabinet. `cargo run --release -p novo-cfd --example cabinet_bench -- coarse`
use novo_cfd::cabinet::CabinetSolver;
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

fn main() {
    let level = match std::env::args().nth(1).as_deref() {
        Some("medium") => Fidelity::Medium,
        Some("fine") => Fidelity::Fine,
        _ => Fidelity::Coarse,
    };
    let dev = |id, rect, loss| Device {
        id,
        footprint: rect,
        height_mm: Some(100.0),
        loss_w: loss,
        rated_max_ambient_c: Some(55.0),
        rated_max_surface_c: Some(70.0),
        heat_capacity_j_per_k: None,
    };
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: r(50.0, 50.0, 690.0, 1090.0),
        },
        ambient_c: 25.0,
        devices: vec![
            dev(1, r(200.0, 300.0, 150.0, 200.0), 60.0),
            dev(2, r(400.0, 700.0, 120.0, 150.0), 40.0),
        ],
        obstacles: vec![Obstacle {
            kind: ObstacleKind::Duct,
            rect: r(100.0, 200.0, 40.0, 800.0),
            blockage: None,
        }],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    };
    let t0 = std::time::Instant::now();
    let mut last = std::time::Instant::now();
    let mut tuning = novo_cfd::cabinet::Tuning::default();
    if let Some(n) = std::env::args().nth(2).and_then(|s| s.parse().ok()) {
        tuning = tuning.with_cells(n);
    }
    let res = CabinetSolver::new(tuning).solve(&scn, level, &Control::new(), &mut |s| {
        if last.elapsed().as_secs_f64() > 5.0 {
            last = std::time::Instant::now();
            let p = &s.progress;
            println!(
                "{:6.1}s it {} T {:.2} res e {:.2e} m {:.2e} bal {:.3}",
                t0.elapsed().as_secs_f64(),
                p.iteration,
                s.max_temperature_c(),
                p.residuals.energy,
                p.residuals.momentum,
                p.energy_balance_rel
            );
        }
    });
    let s = res.unwrap();
    println!(
        "cells {} time {:.1}s it {} conv {} bal {:.4}",
        s.grid.cells(),
        t0.elapsed().as_secs_f64(),
        s.progress.iteration,
        s.progress.converged,
        s.progress.energy_balance_rel
    );
    println!("max T {:.2}", s.max_temperature_c());
    for d in &s.devices {
        println!(
            "dev {} surf {:.2} air {:.2} h {:.2} plate {:.2}",
            d.id, d.surface_c, d.air_c, d.h_w_m2k, d.plate_share
        );
    }
}
