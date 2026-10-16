//! Device surface and air temperature against cell count, sealed 600x800x300 cabinet, one 50 W device.
//! `cargo run --release -p novo-cfd --example surface_study -- 1000 3000 6000`
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
    let r2 = simulate_steady_with(
        &scn,
        Options {
            bands: false,
            advice: false,
        },
    )
    .unwrap();
    let d2 = &r2.devices[0];
    println!(
        "2D engine | surface {:.1} air {:.1} h {:.1} plate {:.2} | mean air rise {:.1}",
        d2.surface_c, d2.air_c, d2.h_w_m2k, d2.plate_share, r2.balance.mean_rise_k
    );
    for n in std::env::args()
        .skip(1)
        .filter_map(|a| a.parse::<usize>().ok())
    {
        let mut t = Tuning::default().with_cells(n);
        t.coarse.steady_tol = 3e-3;
        t.coarse.max_wall = None;
        let t0 = std::time::Instant::now();
        let s = CabinetSolver::new(t)
            .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |p| {
                if std::env::var("VERBOSE").is_ok() && p.progress.iteration % 500 == 0 {
                    println!(
                        "  it {} energy {:.2e} t {:.0}s",
                        p.progress.iteration,
                        p.progress.residuals.energy,
                        t0.elapsed().as_secs_f64()
                    );
                }
            })
            .unwrap();
        let d = &s.devices[0];
        println!(
            "cells {} (asked {n}) | surface {:.1} (band {:.0}..{:.0}) air {:.1} h {:.1} plate {:.2} | conv {} bal {:.3} | {:.0}s | unresolved {}",
            s.grid.cells(),
            d.surface_c,
            d.surface_band_c.0,
            d.surface_band_c.1,
            d.air_c,
            d.h_w_m2k,
            d.plate_share,
            s.progress.converged,
            s.progress.energy_balance_rel,
            t0.elapsed().as_secs_f64(),
            s.notes.iter().any(|n| n.contains("not resolved"))
        );
    }
}
