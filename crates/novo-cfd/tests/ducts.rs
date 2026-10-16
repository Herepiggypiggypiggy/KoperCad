//! Porous drag (slotted ducts) and the per-face wall films.

use novo_cfd::flow::*;
use novo_cfd::openings::*;
use novo_cfd::solids::Medium;

const RHO: f64 = 1.18;

#[test]
fn a_porous_slab_sets_the_operating_point_of_a_fan_whatever_the_time_step() {
    // Fan over the x min face, free outlet, a slab of quadratic drag in the
    // middle: P (1 - Q/Qf) = rho F L (Q / A)^2.
    let (p_max, q_free, f, len) = (30.0, 0.03, 50.0, 0.2);
    let size = [0.5, 0.15, 0.15];
    let a = size[1] * size[2];
    let exact = {
        let g = |q: f64| p_max * (1.0 - q / q_free) - RHO * f * len * (q / a).powi(2);
        let (mut lo, mut hi) = (0.0, q_free);
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if g(mid) > 0.0 { lo = mid } else { hi = mid }
        }
        0.5 * (lo + hi)
    };
    let mut results = Vec::new();
    for dt in [0.005, 0.02] {
        let mesh = Mesh::uniform([20, 6, 6], size);
        let mut md = Medium::air(mesh.cells());
        for k in 0..6 {
            for j in 0..6 {
                for i in 6..14 {
                    md.drag[0][mesh.cell([i, j, k])] = f;
                }
            }
        }
        let mut cfg = FlowConfig::air();
        cfg.gravity = [0.0; 3];
        cfg.rho = RHO;
        cfg.t_ref = Some(25.0);
        cfg.openings = vec![
            OpenPatch {
                face: Face::XMin,
                lo: [0.0, 0.0],
                hi: [size[1], size[2]],
                kind: OpenKind::Fan {
                    intake: true,
                    free_flow_m3s: q_free,
                    max_pressure_pa: p_max,
                },
                inflow_c: None,
            },
            OpenPatch {
                face: Face::XMax,
                lo: [0.0, 0.0],
                hi: [size[1], size[2]],
                kind: OpenKind::Free,
                inflow_c: None,
            },
        ];
        cfg.medium = Some(md);
        let mut s = FlowSolver::new(mesh, cfg, 25.0);
        let steps = (6.0 / dt) as usize;
        for _ in 0..steps {
            s.step(dt);
        }
        let q = s.patch_flow_m3s(1);
        println!(
            "DUCTS | porous slab, dt {dt} | Q {q:.5} vs {exact:.5} m3/s | error {:+.2} %",
            100.0 * (q / exact - 1.0)
        );
        results.push(q);
        assert!((s.patch_flow_m3s(0) + q).abs() < 1e-6 * exact);
    }
    for q in &results {
        assert!((q / exact - 1.0).abs() < 0.03, "{q} vs {exact}");
    }
}

fn scenario_with_duct(rect: novo_layout::Rect) -> novo_thermal::Scenario {
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
        devices: vec![],
        obstacles: vec![Obstacle {
            kind: ObstacleKind::Duct,
            rect,
            blockage: None,
        }],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

#[test]
fn a_duct_is_open_to_flow_and_resists_across_the_slots_more_than_along() {
    use novo_cfd::mesh::MeshSettings;
    use novo_cfd::solids::{FlowSetup, SolidsSettings};
    // horizontal duct 400 x 40 mm: long axis x
    let rect = novo_layout::Rect {
        x: 100.0,
        y: 300.0,
        w: 400.0,
        h: 40.0,
    };
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: 3000,
            ..MeshSettings::default()
        },
        ..SolidsSettings::default()
    };
    let setup = FlowSetup::new(&scenario_with_duct(rect), &st).unwrap();
    let md = setup.cfg.medium.as_ref().unwrap();
    let sum = |a: usize| -> f64 { md.drag[a].iter().sum() };
    println!(
        "DUCTS | horizontal duct drag sums x {:.1} y {:.1} z {:.1}",
        sum(0),
        sum(1),
        sum(2)
    );
    assert!(
        sum(1) > 50.0 * sum(0),
        "cross must resist far more than along"
    );
    assert!(sum(2) > sum(1));
    // vertical duct: roles swap
    let rect = novo_layout::Rect {
        x: 100.0,
        y: 100.0,
        w: 40.0,
        h: 400.0,
    };
    let setup = FlowSetup::new(&scenario_with_duct(rect), &st).unwrap();
    let md = setup.cfg.medium.as_ref().unwrap();
    let sum = |a: usize| -> f64 { md.drag[a].iter().sum() };
    assert!(sum(0) > 50.0 * sum(1));
    // duct cells are porous, not solid: air share about the porosity
    let porous: Vec<f64> = md
        .fluid_frac
        .iter()
        .zip(&md.drag[2])
        .filter(|(_, d)| **d > 0.0)
        .map(|(f, _)| *f)
        .collect();
    assert!(!porous.is_empty());
    // (cells that also hold a sliver of the plate are the exception)
    let open = porous.iter().filter(|f| **f > 0.8).count();
    println!(
        "DUCTS | {} of {} duct cells keep an air share above 0.8",
        open,
        porous.len()
    );
    // the bottom layer of a duct stands on the plate; with five layers over
    // the duct that is a fifth of its cells, hence 0.75 and not 0.8
    assert!(open as f64 >= 0.75 * porous.len() as f64);
    assert!(porous.iter().all(|f| *f > 0.3), "duct cells must stay open");
}

#[test]
fn every_wall_face_gets_its_own_iec_60890_film() {
    use novo_cfd::solids::wall_conductances;
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    let mut scn = scenario_with_duct(Rect {
        x: 0.0,
        y: 0.0,
        w: 10.0,
        h: 10.0,
    });
    scn.enclosure.installation = Installation::Wall;
    let k = Material::Steel.k();
    let (wo, ho, d_o) = (0.6, 0.8, 0.3);
    let (wi, hi, di) = (0.596, 0.796, 0.296);
    let w = wall_conductances(&scn);
    let want = |b: f64, a_out: f64, a_in: f64| k * b * a_out / a_in;
    // wall mounted: rear covered (0.5), bottom open (0.5), top free (1.4), sides and front 0.9
    let expect = [
        (Face::XMin, want(0.9, d_o * ho, di * hi)),
        (Face::XMax, want(0.9, d_o * ho, di * hi)),
        (Face::YMin, want(1.4, wo * d_o, wi * di)),
        (Face::YMax, want(0.5, wo * d_o, wi * di)),
        (Face::ZMin, want(0.5, wo * ho, wi * hi)),
        (Face::ZMax, want(0.9, wo * ho, wi * hi)),
    ];
    for (f, h) in expect {
        println!("DUCTS | film {f:?} {:.4} vs {h:.4} W/m2K", w.h[f as usize]);
        assert!((w.h[f as usize] - h).abs() < 1e-9 * h, "{f:?}");
    }
    // free standing: nothing through the bottom
    scn.enclosure.installation = Installation::FreeStanding;
    let w = wall_conductances(&scn);
    assert_eq!(w.h[Face::YMax as usize], 0.0);
    assert!((w.h[Face::ZMin as usize] - want(0.9, wo * ho, wi * hi)).abs() < 1e-9);
}
