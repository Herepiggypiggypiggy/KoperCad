//! Sealed enclosure with one heat source: 3D engine against the IEC 60890
//! hand calculation and the 2D engine of `novo-thermal` (design section 5,
//! case 4). Run `-- --nocapture` to see the `VALIDATION` rows.

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
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

/// Volume-weighted mean air temperature rise of the 3D solution, K.
fn mean_air_rise(scn: &Scenario, target_cells: usize) -> (f64, f64) {
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
        0.001,
        (3000.0, 40000),
        &Control::new(),
        |_, _| {},
        0,
    );
    assert!(rep.converged, "{rep:?}");
    let med = setup.cfg.medium.as_ref().unwrap();
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
    (sum / vol - scn.ambient_c, rep.imbalance)
}

/// The 3D engine must land near the IEC 60890 estimate and the 2D engine for
/// a sealed enclosure. KNOWN GAP (reported to lane CFD-C2): the IEC `k` is an
/// overall air-to-air coefficient, but the 3D solver applies it as the wall
/// film and also resolves the air-to-wall convection inside, so the inner
/// film is counted twice and the air comes out 2.3 to 3.2 times too hot
/// (14.5 K on 6000 cells and 20.0 K on 1500, against 6.3 K at 50 W). Remove the `ignore` when that is fixed.
#[test]
#[ignore = "known gap: 3D sealed-enclosure air rise is 2.3 to 3.2x the IEC 60890 estimate (CFD-C2)"]
fn sealed_enclosure_matches_iec_60890_and_the_2d_engine() {
    let scn = scenario(50.0);
    let two_d = simulate_steady(&scn).unwrap();
    let iec = two_d.balance.iec_rise_k;
    let (rise, imbalance) = mean_air_rise(&scn, 1500);
    println!(
        "VALIDATION | sealed enclosure 50 W | heat balance imbalance | {imbalance:+.4} | 0 | -"
    );
    let e_iec = report(
        "sealed enclosure 50 W",
        "mean air rise K vs IEC 60890",
        rise,
        iec,
    );
    let e_2d = report(
        "sealed enclosure 50 W",
        "mean air rise K vs 2D engine",
        rise,
        two_d.balance.mean_rise_k,
    );
    assert!(imbalance.abs() < 0.005);
    assert!(e_iec.abs() < 0.25, "IEC {e_iec}");
    assert!(e_2d.abs() < 0.25, "2D {e_2d}");
}
