//! The vented 600 x 800 x 300 mm box with 100 W: laminar, SST and k-epsilon
//! at a given cell count against the 2D engine (lane CFD-B).
//! `cargo run --release -p novo-cfd --example turb_vented -- sst 6000`
//! Add `fan` for a 120 m3/h intake fan in place of the bottom louvre.
use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::turb::TurbModel;
use novo_cfd::{Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario(loss_w: f64, fan: bool) -> Scenario {
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
        vents: if fan {
            vec![vent(Placement::Top { x_mm: 300.0 })]
        } else {
            vec![
                vent(Placement::Bottom { x_mm: 300.0 }),
                vent(Placement::Top { x_mm: 300.0 }),
            ]
        },
        fans: if fan {
            vec![Fan {
                opening: Opening {
                    placement: Placement::Bottom { x_mm: 300.0 },
                    width_mm: 200.0,
                    height_mm: 120.0,
                },
                mode: FanMode::Intake,
                free_flow_m3h: std::env::var("FANQ")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(120.0),
                max_pressure_pa: 60.0,
            }]
        } else {
            vec![]
        },
        params: Params::default(),
    }
}

fn main() {
    let model = match std::env::args().nth(1).as_deref() {
        Some("sst") => TurbModel::KOmegaSst,
        Some("ke") => TurbModel::KEpsilon,
        _ => TurbModel::Laminar,
    };
    let cells: usize = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(6000);
    let mut scn = scenario(100.0, std::env::args().any(|a| a == "fan"));
    if let Ok(m) = std::env::var("MIX") {
        scn.params.mixing_m2s = m.parse().unwrap();
    }
    let two_d = simulate_steady(&scn).unwrap();
    let d2 = &two_d.devices[0];
    let mut tuning = Tuning::default().with_cells(cells);
    for l in [&mut tuning.coarse, &mut tuning.medium, &mut tuning.fine] {
        l.max_wall = None;
        l.steady_tol = 1e-3;
        l.max_steps = 400_000;
        l.max_time_s = 6000.0;
    }
    if std::env::args().any(|a| a == "march") {
        tuning = tuning.marched();
    }
    let t0 = std::time::Instant::now();
    let s = CabinetSolver::new(tuning)
        .with_turbulence(model)
        .with_second_order_turbulence(std::env::args().any(|a| a == "2nd"))
        .solve(&scn, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap();
    // volume weighted mean air temperature
    let g = &s.grid;
    let (mut sum, mut vol) = (0.0, 0.0);
    for k in 0..g.nz {
        for j in 0..g.ny {
            for i in 0..g.nx {
                let c = g.index(i, j, k);
                if s.material[c] == 0 {
                    let v = g.volume_m3(i, j, k);
                    sum += v * f64::from(s.temperature_c[c]);
                    vol += v;
                }
            }
        }
    }
    // exhaust through the top louvre: first air cell of every column under the
    // top wall within the louvre's width, upward flow (y points down)
    let (mut q_out, mut qt) = (0.0, 0.0);
    for k in 0..g.nz {
        for i in 0..g.nx {
            let xc = 0.5 * (g.x_edges_mm[i] + g.x_edges_mm[i + 1]);
            if !(200.0..=400.0).contains(&xc) {
                continue;
            }
            if let Some(j) = (0..g.ny).find(|j| s.material[g.index(i, *j, k)] == 0) {
                let c = g.index(i, j, k);
                let vy = f64::from(s.velocity[c][1]);
                if vy < 0.0 {
                    let area = (g.x_edges_mm[i + 1] - g.x_edges_mm[i])
                        * (g.z_edges_mm[k + 1] - g.z_edges_mm[k])
                        * 1e-6;
                    q_out += -vy * area;
                    qt += -vy * area * f64::from(s.temperature_c[c]);
                }
            }
        }
    }
    let rho_cp = 1.2 * 1006.0;
    let q_2d = two_d
        .openings
        .iter()
        .filter(|f| !f.is_fan)
        .map(|f| f.flow_m3h.abs() / 3600.0)
        .fold(0.0_f64, f64::max);
    println!(
        "EXHAUST top louvre 3D flow {:.5} m3/s rise {:.2} K | 2D flow {:.5} m3/s rise {:.2} K",
        q_out,
        qt / q_out.max(1e-12) - 25.0,
        q_2d,
        two_d.balance.openings_w / (rho_cp * q_2d)
    );
    if std::env::var("PROFILE").is_ok() {
        let bands = 8usize;
        let mut b3 = vec![(0.0, 0.0); bands];
        for k in 0..g.nz {
            for j in 0..g.ny {
                for i in 0..g.nx {
                    let c = g.index(i, j, k);
                    if s.material[c] == 0 {
                        let yc = 0.5 * (g.y_edges_mm[j] + g.y_edges_mm[j + 1]);
                        let b = ((yc / 800.0 * bands as f64) as usize).min(bands - 1);
                        let v = g.volume_m3(i, j, k);
                        b3[b].0 += v * (f64::from(s.temperature_c[c]) - 25.0);
                        b3[b].1 += v;
                    }
                }
            }
        }
        let f = &two_d.air_temp;
        let mut b2 = vec![(0.0, 0.0); bands];
        for r in 0..f.grid.rows {
            for c in 0..f.grid.cols {
                let yc = (r as f64 + 0.5) * f.grid.cell_h_mm;
                let b = ((yc / f.grid.height_mm * bands as f64) as usize).min(bands - 1);
                b2[b].0 += f.at(c, r) - 25.0;
                b2[b].1 += 1.0;
            }
        }
        for b in 0..bands {
            println!(
                "BAND {b} (top=0) 3D {:.2} K  2D {:.2} K",
                b3[b].0 / b3[b].1,
                b2[b].0 / b2[b].1
            );
        }
        let bal = &two_d.balance;
        println!(
            "2D balance walls f{:.1} r{:.1} l{:.1} rt{:.1} t{:.1} b{:.1} openings {:.1} resid {:.2}",
            bal.front_w,
            bal.rear_w,
            bal.left_w,
            bal.right_w,
            bal.top_w,
            bal.bottom_w,
            bal.openings_w,
            bal.residual_w
        );
    }
    let d3 = &s.devices[0];
    let max_ratio = s.turb_visc_ratio.iter().fold(0.0_f32, |a, v| a.max(*v));
    println!(
        "RESULT {model:?} cells {} conv {} steps {} time {:.0}s | air rise 3D {:.2} K vs 2D {:.2} K ({:+.1} %) | device surface rise 3D {:.2} vs 2D {:.2} ({:+.1} %), air 3D {:.2} 2D {:.2} | max nu_t/nu {:.1} | balance {:.3}",
        g.cells(),
        s.progress.converged,
        s.progress.iteration,
        t0.elapsed().as_secs_f64(),
        sum / vol - 25.0,
        two_d.balance.mean_rise_k,
        100.0 * ((sum / vol - 25.0) / two_d.balance.mean_rise_k - 1.0),
        d3.surface_c - 25.0,
        d2.surface_c - 25.0,
        100.0 * ((d3.surface_c - 25.0) / (d2.surface_c - 25.0) - 1.0),
        d3.air_c - 25.0,
        d2.air_c - 25.0,
        max_ratio,
        s.progress.energy_balance_rel
    );
}
