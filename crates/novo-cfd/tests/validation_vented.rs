//! Vented enclosure (louvre low, louvre high, no fan) with one heat source:
//! buoyancy-driven flow of the 3D engine against the 2D engine of
//! `novo-thermal` and the stack law (design section 5). Run `-- --nocapture` to see the `VALIDATION` rows.

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
        vents: vec![
            Vent {
                opening: Opening {
                    placement: Placement::Bottom { x_mm: 300.0 },
                    width_mm: 200.0,
                    height_mm: 120.0,
                },
                free_area_fraction: 0.6,
            },
            Vent {
                opening: Opening {
                    placement: Placement::Top { x_mm: 300.0 },
                    width_mm: 200.0,
                    height_mm: 120.0,
                },
                free_area_fraction: 0.6,
            },
        ],
        fans: vec![],
        params: Params::default(),
    }
}

/// 3D steady state: mean air rise, heat through the openings and the walls,
/// and the flow through the low and the high vent, averaged over a window
/// because a plume wobbles.
struct Vented {
    rise: f64,
    open_w: f64,
    wall_w: f64,
    q_in: f64,
    q_out: f64,
    imbalance: f64,
}

fn run(scn: &Scenario, target_cells: usize) -> Vented {
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let setup = FlowSetup::new(scn, &st).unwrap();
    let mut s = setup.solver();
    let rep = setup.run_steady(
        &mut s,
        0.002,
        (3000.0, 40000),
        &Control::new(),
        |_, _| {},
        0,
    );
    assert!(rep.converged, "{rep:?}");
    let amb = scn.ambient_c;
    let med = setup.cfg.medium.as_ref().unwrap();
    let (mut t_sum, mut open, mut wall, mut qi, mut qo, mut n) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    let mut t = 0.0;
    while t < 30.0 {
        let dt = s.max_dt(0.4).min(0.5);
        s.step(dt);
        t += dt;
        let m = &s.mesh;
        let (mut sum, mut vol) = (0.0, 0.0);
        for k in 0..m.n[2] {
            for j in 0..m.n[1] {
                for i in 0..m.n[0] {
                    let id = m.cell([i, j, k]);
                    if med.fluid_frac[id] > 0.99 {
                        let v = m.volume([i, j, k]);
                        sum += v * s.t[id];
                        vol += v;
                    }
                }
            }
        }
        t_sum += dt * (sum / vol - amb);
        open += dt * s.open_heat_out_w(amb);
        wall += dt * (0..6).map(|f| s.wall_heat_w(f)).sum::<f64>();
        qi += dt * -s.patch_flow_m3s(0);
        qo += dt * s.patch_flow_m3s(1);
        n += dt;
    }
    Vented {
        rise: t_sum / n,
        open_w: open / n,
        wall_w: wall / n,
        q_in: qi / n,
        q_out: qo / n,
        imbalance: rep.imbalance,
    }
}

/// The two engines must agree on the heat that leaves through the vents, the
/// air rise and the flow through the louvres within the stated bands. The 2D engine
/// carries the stack law (and the 3D chimney case in `openings.rs` checks it
/// directly), so this is the second, independent route to the same flow.
#[test]
fn vented_enclosure_matches_the_2d_engine() {
    let power = 100.0;
    let scn = scenario(power);
    let two_d = simulate_steady(&scn).unwrap();
    let b = &two_d.balance;
    let q_2d = two_d
        .openings
        .iter()
        .filter(|f| !f.is_fan)
        .map(|f| f.flow_m3h.abs() / 3600.0)
        .fold(0.0_f64, f64::max);
    for cells in [1000, 1800] {
        let v = run(&scn, cells);
        let case = format!("vented enclosure {power} W, {cells} cells");
        println!(
            "VALIDATION | {case} | heat balance imbalance | {:+.4} | 0 | -",
            v.imbalance
        );
        let heat = v.open_w + v.wall_w;
        println!(
            "{case}: open {:.1} W wall {:.1} W Q in {:.5} out {:.5} (2D: open {:.1} W, Q {:.5})",
            v.open_w, v.wall_w, v.q_in, v.q_out, b.openings_w, q_2d
        );
        let e_rise = report(&case, "mean air rise K vs 2D engine", v.rise, b.mean_rise_k);
        let e_flow = report(
            &case,
            "vent flow m3/s vs 2D engine",
            0.5 * (v.q_in + v.q_out),
            q_2d,
        );
        let e_open = report(
            &case,
            "heat through vents W vs 2D engine",
            v.open_w,
            b.openings_w,
        );
        // the engine carries the whole loss away at steady state
        assert!((heat - power).abs() / power < 0.02, "heat {heat}");
        // air in and air out agree (no net mass source)
        assert!(
            (v.q_in - v.q_out).abs() < 0.05 * v.q_in.abs(),
            "in {} out {}",
            v.q_in,
            v.q_out
        );
        assert!(e_rise.abs() < 0.3, "rise {e_rise}");
        assert!(e_flow.abs() < 0.2, "flow {e_flow}");
        assert!(e_open.abs() < 0.25, "open {e_open}");
    }
}
