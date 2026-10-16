//! Solids in the flow grid: layered conduction, blocked flow, energy balance.

use novo_cfd::flow::*;
use novo_cfd::solids::Medium;

/// A medium of horizontal layers along x: `(thickness fractions, k, rho_cp)`.
fn layers(mesh: &Mesh, spec: &[(f64, f64, f64)]) -> Medium {
    let n = mesh.cells();
    let mut md = Medium::air(n);
    let len = mesh.edges[0][mesh.n[0]];
    for k in 0..mesh.n[2] {
        for j in 0..mesh.n[1] {
            for i in 0..mesh.n[0] {
                let x = mesh.centre[0][i] / len;
                let mut acc = 0.0;
                for &(frac, kk, c) in spec {
                    acc += frac;
                    if x < acc {
                        let id = mesh.cell([i, j, k]);
                        md.fluid_frac[id] = 0.0;
                        md.k_solid[id] = kk;
                        md.c_solid[id] = c;
                        break;
                    }
                }
            }
        }
    }
    md
}

#[test]
fn conduction_through_layers_matches_the_series_resistance() {
    // Three layers between fixed walls at 0 and 100 C, no flow. Layer
    // boundaries lie on cell faces (thickness fractions 0.25, 0.5, 0.25 of 24
    // cells): the harmonic face conductivity is then exact and the steady
    // profile is piecewise linear.
    let mesh = Mesh::uniform([24, 3, 3], [0.24, 0.03, 0.03]);
    let spec = [(0.25, 1.0, 1.0e6), (0.5, 50.0, 3.0e6), (0.25, 0.2, 2.0e6)];
    let mut cfg = FlowConfig::air();
    cfg.gravity = [0.0; 3];
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(0.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(100.0);
    cfg.medium = Some(layers(&mesh, &spec));
    let mut s = FlowSolver::new(mesh, cfg, 50.0);
    for _ in 0..4000 {
        let info = s.step(2000.0);
        if info.max_dtdt < 1e-12 {
            break;
        }
    }
    // The fixed wall sits half a cell from the first centre: resistances in
    // series per unit area.
    let len = 0.24;
    let r: Vec<f64> = spec.iter().map(|&(f, k, _)| f * len / k).collect();
    let r_total: f64 = r.iter().sum();
    let flux = 100.0 / r_total;
    let mut worst: f64 = 0.0;
    let mut acc = 0.0;
    let mut bounds = vec![0.0];
    for &(f, _, _) in &spec {
        acc += f * len;
        bounds.push(acc);
    }
    let temp_at = |x: f64| -> f64 {
        let mut t = 0.0;
        for (li, &(f, k, _)) in spec.iter().enumerate() {
            let lo = bounds[li];
            let hi = lo + f * len;
            if x <= hi {
                return t + flux * (x - lo) / k;
            }
            t += flux * f * len / k;
        }
        t
    };
    for i in 0..24 {
        let x = s.mesh.centre[0][i];
        let got = s.t[s.mesh.cell([i, 1, 1])];
        worst = worst.max((got - temp_at(x)).abs());
    }
    println!("SOLIDS | three layers, series resistance | max error {worst:.2e} K");
    assert!(worst < 1e-5, "{worst}");
    // heat through the wall equals flux * area
    let q = s.wall_heat_w(Face::XMax as usize);
    let area = 0.03 * 0.03;
    // XMax is the hot wall: heat flows in (negative out)
    assert!((q + flux * area).abs() / (flux * area) < 1e-5, "{q}");
}

#[test]
fn a_solid_block_stops_the_flow_and_the_pressure_solve_still_converges() {
    // Lid-free box with a heated side: buoyant flow around a solid block.
    let mesh = Mesh::uniform([16, 16, 4], [0.4, 0.4, 0.1]);
    let n = mesh.cells();
    let mut md = Medium::air(n);
    for k in 0..4 {
        for j in 5..11 {
            for i in 5..11 {
                let id = mesh.cell([i, j, k]);
                md.fluid_frac[id] = 0.0;
                md.k_solid[id] = 1.0;
                md.c_solid[id] = 1.0e6;
                md.heat_w_m3[id] = 2000.0;
            }
        }
    }
    let mut cfg = FlowConfig::air();
    cfg.faces[Face::XMin as usize].thermal = ThermalBc::Fixed(20.0);
    cfg.faces[Face::XMax as usize].thermal = ThermalBc::Fixed(20.0);
    cfg.medium = Some(md);
    let mut s = FlowSolver::new(mesh, cfg, 20.0);
    for _ in 0..200 {
        let dt = s.max_dt(0.4).clamp(1e-3, 0.5);
        s.step(dt);
    }
    // no velocity on any face of the block
    let m = &s.mesh;
    let mut worst: f64 = 0.0;
    for j in 5..11 {
        for k in 0..4 {
            for i in 5..12 {
                worst = worst.max(s.vel[0][m.idx(0, [i, j, k])].abs());
            }
        }
    }
    for k in 0..4 {
        for i in 5..11 {
            for j in 5..12 {
                worst = worst.max(s.vel[1][m.idx(1, [i, j, k])].abs());
            }
        }
    }
    println!(
        "SOLIDS | velocity on block faces {worst:.2e} m/s | divergence {:.2e}",
        s.divergence_max()
    );
    assert_eq!(worst, 0.0);
    assert!(s.divergence_max() < 1e-6);
    // the heated block is hotter than the walls and the air moves
    let tmax = s.t.iter().cloned().fold(f64::MIN, f64::max);
    let vmax = s.vel.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
    assert!(tmax > 20.05 && vmax > 1e-3, "tmax {tmax} vmax {vmax}");
}

fn scenario(loss_w: f64, with_vent: bool) -> novo_thermal::Scenario {
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    use novo_thermal::*;
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
        vents: if with_vent {
            vec![
                Vent {
                    opening: Opening {
                        placement: Placement::Bottom { x_mm: 300.0 },
                        width_mm: 200.0,
                        height_mm: 100.0,
                    },
                    free_area_fraction: 0.6,
                },
                Vent {
                    opening: Opening {
                        placement: Placement::Top { x_mm: 300.0 },
                        width_mm: 200.0,
                        height_mm: 100.0,
                    },
                    free_area_fraction: 0.6,
                },
            ]
        } else {
            vec![]
        },
        fans: vec![],
        params: Params::default(),
    }
}

#[test]
fn wall_films_add_up_to_the_iec_60890_conductance() {
    use novo_cfd::solids::wall_conductances;
    let scn = scenario(50.0, false);
    let w = wall_conductances(&scn);
    let e = &scn.enclosure;
    let (wi, hi, di) = (
        e.interior_width_mm() / 1e3,
        e.interior_height_mm() / 1e3,
        e.interior_depth_mm() / 1e3,
    );
    // film * interior face area summed over the faces = k * effective surface
    let area = [di * hi, di * hi, wi * di, wi * di, wi * hi, wi * hi];
    let sum: f64 = (0..6).map(|f| w.h[f] * area[f]).sum();
    let iec = e.material.k()
        * novo_layout::sizing::effective_surface(
            e.width_mm,
            e.height_mm,
            e.depth_mm,
            e.installation,
        );
    println!("SOLIDS | wall conductance {sum:.4} W/K vs IEC k A_eff {iec:.4} W/K");
    assert!((sum - iec).abs() / iec < 1e-9);
}

#[test]
fn a_heated_device_in_a_sealed_enclosure_balances_to_the_walls() {
    use novo_cfd::Control;
    use novo_cfd::mesh::MeshSettings;
    use novo_cfd::solids::{FlowSetup, SolidsSettings};
    let scn = scenario(50.0, false);
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: 1500,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let setup = FlowSetup::new(&scn, &st).unwrap();
    assert!(
        (setup.heat_w - 50.0).abs() < 1e-6 * 50.0,
        "heat {}",
        setup.heat_w
    );
    let mut s = setup.solver();
    let rep = setup.run_steady(
        &mut s,
        0.001,
        (3000.0, 40000),
        &Control::new(),
        |_, _| {},
        0,
    );
    println!(
        "SOLIDS | sealed enclosure 50 W | {} steps, {:.0} s, storage {:+.2} %, walls {:.2} W, imbalance {:+.2} %",
        rep.steps,
        rep.time_s,
        100.0 * rep.storage_rate,
        rep.heat_out_walls_w,
        100.0 * rep.imbalance
    );
    let tmax = s.t.iter().cloned().fold(f64::MIN, f64::max);
    let tmean = s.t.iter().sum::<f64>() / s.t.len() as f64;
    println!(
        "SOLIDS | hottest cell {tmax:.1} C, mean {tmean:.1} C (IEC dT = {:.1} K)",
        50.0 / (5.5
            * novo_layout::sizing::effective_surface(
                600.0,
                800.0,
                300.0,
                scn.enclosure.installation
            ))
    );
    assert!(rep.converged, "not converged: {rep:?}");
    assert!(rep.imbalance.abs() < 0.005, "{rep:?}");
}
