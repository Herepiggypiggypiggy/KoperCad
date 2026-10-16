//! Open boundaries: patches, fan and vent operating point, stack effect.

use novo_cfd::flow::*;
use novo_cfd::openings::*;

const RHO: f64 = 1.18;
const CP_RHO: f64 = 0.026 / 2.2e-5; // J/m3K from the air constants of FlowConfig::air

fn patch(face: Face, lo: [f64; 2], hi: [f64; 2], kind: OpenKind) -> OpenPatch {
    OpenPatch {
        face,
        lo,
        hi,
        kind,
        inflow_c: None,
    }
}

#[test]
fn a_patch_gets_its_exact_area_also_across_partial_cells() {
    let mesh = Mesh::uniform([10, 10, 10], [1.0, 1.0, 1.0]);
    // 0.25 x 0.35 m on the x min face: cells are 0.1 m, so partial overlaps.
    let p = patch(Face::XMin, [0.12, 0.3], [0.37, 0.65], OpenKind::Free);
    let set = OpenSet::resolve(&mesh, &[p]);
    let want = 0.25 * 0.35;
    assert!(
        (set.patch_area[0] - want).abs() < 1e-12,
        "{}",
        set.patch_area[0]
    );
    let sum: f64 = set.faces.iter().map(|f| f.area).sum();
    assert!((sum - want).abs() < 1e-12);
    // all of them on the min side of axis 0
    assert!(set.faces.iter().all(|f| f.axis == 0 && !f.upper));
    assert!(set.afrac[0].iter().all(|v| (0.0..=1.0).contains(v)));
}

#[test]
fn two_patches_do_not_claim_the_same_face() {
    let mesh = Mesh::uniform([4, 4, 4], [1.0, 1.0, 1.0]);
    let a = patch(Face::YMax, [0.0, 0.0], [1.0, 1.0], OpenKind::Free);
    let b = patch(Face::YMax, [0.0, 0.0], [0.5, 0.5], OpenKind::vent(0.5, 0.6));
    let set = OpenSet::resolve(&mesh, &[a, b]);
    assert!((set.patch_area[0] - 1.0).abs() < 1e-12);
    assert_eq!(set.patch_area[1], 0.0);
}

#[test]
fn scenario_vents_and_fans_become_patches() {
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    use novo_thermal::*;
    let scn = Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::FreeStanding,
            plate: Rect {
                x: 50.0,
                y: 50.0,
                w: 600.0,
                h: 800.0,
            },
        },
        ambient_c: 35.0,
        devices: vec![],
        obstacles: vec![],
        vents: vec![Vent {
            opening: Opening {
                placement: Placement::Bottom { x_mm: 200.0 },
                width_mm: 100.0,
                height_mm: 150.0,
            },
            free_area_fraction: 0.5,
        }],
        fans: vec![Fan {
            opening: Opening {
                placement: Placement::Top { x_mm: 600.0 },
                width_mm: 120.0,
                height_mm: 120.0,
            },
            mode: FanMode::Exhaust,
            free_flow_m3h: 200.0,
            max_pressure_pa: 60.0,
        }],
        params: Params::default(),
    };
    let ps = patches_from_scenario(&scn).unwrap();
    assert_eq!(ps.len(), 2);
    assert_eq!(ps[0].face, Face::YMax);
    assert_eq!(ps[1].face, Face::YMin);
    // vent: x 150..250 mm, depth centred 150 mm: z 0.0375..0.1875 m wait interior depth 296
    assert!((ps[0].lo[0] - 0.150).abs() < 1e-12 && (ps[0].hi[0] - 0.250).abs() < 1e-12);
    assert!(((ps[0].hi[1] - ps[0].lo[1]) - 0.150).abs() < 1e-12);
    let OpenKind::Loss { k } = ps[0].kind else {
        panic!()
    };
    assert!((k - 1.0 / (0.3f64 * 0.3)).abs() < 1e-9, "{k}");
    match ps[1].kind {
        OpenKind::Fan {
            intake,
            free_flow_m3s,
            max_pressure_pa,
        } => {
            assert!(!intake);
            assert!((free_flow_m3s - 200.0 / 3600.0).abs() < 1e-12);
            assert_eq!(max_pressure_pa, 60.0);
        }
        _ => panic!(),
    }
}

fn cfg_open(patches: Vec<OpenPatch>, t_ref: f64) -> FlowConfig {
    let mut cfg = FlowConfig::air();
    cfg.openings = patches;
    cfg.t_ref = Some(t_ref);
    cfg.rho = RHO;
    cfg
}

/// Marches until the largest velocity change per second is small.
fn settle(s: &mut FlowSolver, dt: f64, max_steps: usize, tol: f64) -> usize {
    for n in 0..max_steps {
        let info = s.step(dt);
        if n > 20 && info.max_accel < tol {
            return n;
        }
    }
    max_steps
}

#[test]
fn an_intake_fan_into_a_vent_settles_on_the_operating_point() {
    // Fan over the whole x min face (0.4 x 0.4 m), small vent opposite.
    // Fan: dp = P (1 - Q/Qf). Vent: Q = A_eff sqrt(2 dp / rho).
    let (side, pmax, qf) = (0.4, 50.0, 0.1);
    let (vent_w, free, cd) = (0.1, 0.5, 0.6);
    let mesh = Mesh::uniform([12, 12, 12], [side; 3]);
    let c = 0.5 * side;
    let patches = vec![
        patch(
            Face::XMin,
            [0.0, 0.0],
            [side, side],
            OpenKind::Fan {
                intake: true,
                free_flow_m3s: qf,
                max_pressure_pa: pmax,
            },
        ),
        patch(
            Face::XMax,
            [c - 0.5 * vent_w, c - 0.5 * vent_w],
            [c + 0.5 * vent_w, c + 0.5 * vent_w],
            OpenKind::vent(free, cd),
        ),
    ];
    let mut cfg = cfg_open(patches, 25.0);
    cfg.gravity = [0.0; 3];
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    let steps = settle(&mut s, 0.01, 4000, 1e-5);
    let q_fan = -s.patch_flow_m3s(0);
    let q_vent = s.patch_flow_m3s(1);
    // analytic: P (1 - Q/Qf) = rho/2 (Q / A_eff)^2, bisection
    let a_eff = cd * free * vent_w * vent_w;
    let f = |q: f64| pmax * (1.0 - q / qf) - 0.5 * RHO * (q / a_eff).powi(2);
    let (mut lo, mut hi) = (0.0, qf);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 { lo = mid } else { hi = mid }
    }
    let exact = 0.5 * (lo + hi);
    let err = (q_vent - exact) / exact;
    println!(
        "OPENINGS | fan+vent operating point | steps {steps} | Q {q_vent:.5} vs {exact:.5} m3/s | error {:+.2} %",
        100.0 * err
    );
    assert!(
        (q_fan - q_vent).abs() < 1e-6 * exact,
        "mass: {q_fan} {q_vent}"
    );
    assert!(err.abs() < 0.03, "error {err}");
    assert!(s.divergence_max() < 1e-6);
}

#[test]
fn an_exhaust_fan_pulls_air_through_a_free_opening() {
    let side = 0.3;
    let mesh = Mesh::uniform([10, 10, 10], [side; 3]);
    let patches = vec![
        patch(
            Face::ZMax,
            [0.0, 0.0],
            [side, side],
            OpenKind::Fan {
                intake: false,
                free_flow_m3s: 0.05,
                max_pressure_pa: 30.0,
            },
        ),
        patch(Face::ZMin, [0.0, 0.0], [side, side], OpenKind::Free),
    ];
    let mut cfg = cfg_open(patches, 25.0);
    cfg.gravity = [0.0; 3];
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    settle(&mut s, 0.01, 4000, 1e-5);
    let q = s.patch_flow_m3s(0);
    // Free inlet costs the inflow head: P (1 - Q/Qf) = rho/2 (Q/A)^2
    let a = side * side;
    let f = |q: f64| 30.0 * (1.0 - q / 0.05) - 0.5 * RHO * (q / a).powi(2);
    let (mut lo, mut hi) = (0.0, 0.05);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 { lo = mid } else { hi = mid }
    }
    let exact = 0.5 * (lo + hi);
    println!("OPENINGS | exhaust fan, free inlet | Q {q:.5} vs {exact:.5}");
    assert!(
        q > 0.0 && (q - exact).abs() / exact < 0.03,
        "{q} vs {exact}"
    );
    // the inlet really takes it in
    assert!((s.patch_flow_m3s(1) + q).abs() < 1e-6 * q);
}

#[test]
fn still_ambient_air_stays_still_with_openings() {
    let side = 0.4;
    let mesh = Mesh::uniform([8, 8, 8], [side; 3]);
    let patches = vec![
        patch(Face::YMin, [0.1, 0.1], [0.3, 0.3], OpenKind::Free),
        patch(Face::YMax, [0.1, 0.1], [0.3, 0.3], OpenKind::vent(0.6, 0.6)),
    ];
    let mut s = FlowSolver::new(mesh, cfg_open(patches, 30.0), 30.0);
    for _ in 0..100 {
        s.step(0.05);
    }
    let vmax = s.vel.iter().flatten().fold(0.0_f64, |a, v| a.max(v.abs()));
    println!("OPENINGS | still ambient | max velocity {vmax:.2e}");
    assert!(vmax < 1e-8, "velocity {vmax}");
}

/// A heat source in a box with a low inlet and a high outlet: buoyancy
/// pulls air through. Energy leaves with the air (walls adiabatic), and the
/// flow follows the stack law with the right scaling.
fn chimney(power_w: f64) -> (f64, f64) {
    let (w, h, d) = (0.3, 0.6, 0.3);
    let mesh = Mesh::uniform([8, 16, 8], [w, h, d]);
    // y is down: the top is y = 0. Inlet low on the left wall, outlet high on the right wall.
    let (a_in, a_out) = (0.1, 0.1); // side of the square openings
    let patches = vec![
        patch(
            Face::XMin,
            [h - 0.05 - a_in, 0.1],
            [h - 0.05, 0.1 + a_in],
            OpenKind::vent(1.0, 0.6),
        ),
        patch(
            Face::XMax,
            [0.05, 0.1],
            [0.05 + a_in, 0.1 + a_out],
            OpenKind::vent(1.0, 0.6),
        ),
    ];
    let t_amb = 25.0;
    let mut cfg = cfg_open(patches, t_amb);
    let mut src = vec![0.0; mesh.cells()];
    // a block of 2x2x2 cells at the bottom centre
    let vol = 8.0 * mesh.volume([3, 13, 3]);
    for k in 3..5 {
        for j in 13..15 {
            for i in 3..5 {
                src[mesh.cell([i, j, k])] = power_w / (CP_RHO * vol);
            }
        }
    }
    cfg.heat_source = Some(src);
    let mut s = FlowSolver::new(mesh, cfg, t_amb);
    // Run to about five turnover times, then average the last quarter: a
    // plume wobbles, its mean is what the stack law speaks about.
    let (t_end, t_avg) = (240.0, 180.0);
    let (mut t, mut n) = (0.0, 0.0);
    let (mut q_sum, mut heat_sum) = (0.0, 0.0);
    while t < t_end {
        let dt = s.max_dt(0.4).min(0.5);
        s.step(dt);
        t += dt;
        if t > t_avg {
            let (mut num, mut den) = (0.0, 0.0);
            for f in s.open.faces.iter().filter(|f| f.patch == 1) {
                let u = f.outward() * s.vel[f.axis][f.vel];
                num += u * f.area * (s.t[f.cell] - t_amb);
                den += u * f.area;
            }
            q_sum += den * dt;
            heat_sum += CP_RHO * num * dt;
            n += dt;
        }
    }
    let (q, heat) = (q_sum / n, heat_sum / n);
    println!("chimney {power_w} W: mean Q {q:.5} m3/s, heat out {heat:.3} W");
    (q, heat)
}

#[test]
fn stack_effect_follows_the_chimney_law() {
    let (q1, heat1) = chimney(20.0);
    let (q8, _) = chimney(160.0);
    // Net heat = rho cp Q (T_out - T_amb) with T_out the flux-weighted outlet value.
    println!("OPENINGS | chimney 20 W | heat carried out {heat1:.3} W");
    assert!((heat1 - 20.0).abs() / 20.0 < 0.03, "energy {heat1}");
    // Ideal stack: Q^3 = A_eq^2 * 2 g H P / (T rho cp), H between the centres
    let a_eq = {
        let a: f64 = 0.6 * 0.1 * 0.1;
        a * a / (2.0 * a * a).sqrt()
    };
    let h_centres = (0.6 - 0.05 - 0.05) - (0.05 + 0.05);
    let ideal = |p: f64| (a_eq * a_eq * 2.0 * 9.81 * h_centres * p / (298.15 * CP_RHO)).cbrt();
    println!(
        "OPENINGS | chimney Q {q1:.5} vs ideal {:.5} ({:+.1} %); 160 W: {q8:.5} vs {:.5} ({:+.1} %)",
        ideal(20.0),
        100.0 * (q1 / ideal(20.0) - 1.0),
        ideal(160.0),
        100.0 * (q8 / ideal(160.0) - 1.0)
    );
    // The scaling Q ~ P^(1/3): 8 times the power, twice the flow.
    let ratio = q8 / q1;
    assert!((ratio - 2.0).abs() < 0.2, "ratio {ratio}");
    // The ideal fully mixed law is an upper-ish estimate: within 15 percent (measured -9 percent).
    assert!((q1 / ideal(20.0) - 1.0).abs() < 0.15);
    assert!((q8 / ideal(160.0) - 1.0).abs() < 0.15);
}
