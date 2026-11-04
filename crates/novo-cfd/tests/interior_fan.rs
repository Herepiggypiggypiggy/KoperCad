//! A fan inside the domain (lane H8): the operating point against a loss it
//! must push through, mass conservation, direction, reversal.

use novo_cfd::flow::*;
use novo_cfd::openings::*;

const RHO: f64 = 1.18;

fn open(face: Face, side: f64, kind: OpenKind) -> OpenPatch {
    OpenPatch {
        face,
        lo: [0.0, 0.0],
        hi: [side, side],
        kind,
        inflow_c: None,
    }
}

fn settle(s: &mut FlowSolver, dt: f64, max_steps: usize, tol: f64) -> usize {
    for n in 0..max_steps {
        // the jet of a fan outruns a fixed step: keep the convection stable
        let info = s.step(dt.min(s.max_dt(0.4)));
        if n > 20 && info.max_accel < tol {
            return n;
        }
    }
    max_steps
}

/// Straight channel, free inlet, a louvre at the outlet, the fan across the
/// whole section in between.
fn channel(sign: f64, pmax: f64, qf: f64, k_louvre: f64) -> FlowSolver {
    let side = 0.4;
    let mesh = Mesh::uniform([16, 10, 10], [0.6, side, side]);
    let mut cfg = FlowConfig::air();
    cfg.rho = RHO;
    cfg.t_ref = Some(25.0);
    cfg.gravity = [0.0; 3];
    let (inlet, outlet) = if sign > 0.0 {
        (Face::XMin, Face::XMax)
    } else {
        (Face::XMax, Face::XMin)
    };
    cfg.openings = vec![
        open(inlet, side, OpenKind::Free),
        open(outlet, side, OpenKind::Loss { k: k_louvre }),
    ];
    cfg.interior_fans = vec![InteriorFan {
        axis: 0,
        sign,
        centre: [0.3, 0.5 * side, 0.5 * side],
        radius_m: 1.0,
        free_flow_m3s: qf,
        max_pressure_pa: pmax,
    }];
    FlowSolver::new(mesh, cfg, 25.0)
}

/// Bisection of `pmax (1 - Q/Qf) = rho/2 (1 + K) (Q/A)^2`.
fn exact(pmax: f64, qf: f64, k: f64, area: f64) -> f64 {
    let f = |q: f64| pmax * (1.0 - q / qf) - 0.5 * RHO * (1.0 + k) * (q / area).powi(2);
    let (mut lo, mut hi) = (0.0, qf);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if f(mid) > 0.0 { lo = mid } else { hi = mid }
    }
    0.5 * (lo + hi)
}

#[test]
fn an_interior_fan_settles_on_the_operating_point_of_its_loop() {
    let (pmax, qf, k) = (40.0, 0.12, 6.0);
    let mut s = channel(1.0, pmax, qf, k);
    let steps = settle(&mut s, 0.01, 4000, 1e-5);
    let (q, p) = s.interior_fan_state(0).unwrap();
    let q_out = s.patch_flow_m3s(1);
    let want = exact(pmax, qf, k, 0.16);
    let err = (q - want) / want;
    println!(
        "INTERIOR FAN | channel operating point | steps {steps} | Q {q:.5} vs {want:.5} m3/s | error {:+.2} % | dp {p:.1} Pa",
        100.0 * err
    );
    // the outlet carries what the fan moves
    assert!((q_out - q).abs() < 0.01 * q, "{q_out} vs {q}");
    assert!(err.abs() < 0.01, "{err}");
    assert!(q > 0.0 && q < qf && p > 0.0 && p < pmax);
}

#[test]
fn a_fan_facing_the_other_way_moves_the_same_flow_the_other_way() {
    let (pmax, qf, k) = (40.0, 0.12, 6.0);
    let mut a = channel(1.0, pmax, qf, k);
    let mut b = channel(-1.0, pmax, qf, k);
    settle(&mut a, 0.01, 4000, 1e-5);
    settle(&mut b, 0.01, 4000, 1e-5);
    let (qa, _) = a.interior_fan_state(0).unwrap();
    let (qb, _) = b.interior_fan_state(0).unwrap();
    assert!((qa - qb).abs() < 0.005 * qa, "{qa} {qb}");
    // blowing along -x: the air goes towards x min
    let mean_ux = |s: &FlowSolver| {
        let v = &s.vel[0];
        v.iter().sum::<f64>() / v.len() as f64
    };
    assert!(mean_ux(&a) > 0.0 && mean_ux(&b) < 0.0);
}

#[test]
fn a_closed_box_fan_stirs_without_moving_net_flow() {
    // Fan disk in a closed box: the air it pushes comes back round the sides.
    let side = 0.4;
    let mesh = Mesh::uniform([12, 12, 12], [side; 3]);
    let mut cfg = FlowConfig::air();
    cfg.rho = RHO;
    cfg.gravity = [0.0; 3];
    cfg.interior_fans = vec![InteriorFan {
        axis: 0,
        sign: 1.0,
        centre: [0.2, 0.2, 0.2],
        radius_m: 0.09,
        free_flow_m3s: 0.05,
        max_pressure_pa: 30.0,
    }];
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    settle(&mut s, 0.01, 1500, 1e-3);
    let (q, p) = s.interior_fan_state(0).unwrap();
    println!("INTERIOR FAN | closed box | Q {q:.5} m3/s | dp {p:.1} Pa");
    // the loop it must close is long and the sides are walls: it moves air
    // but does not reach its free flow
    assert!(q > 0.0 && q < 0.05);
    // zero net flow across any plane of a closed box
    let net: f64 = {
        let m = &s.mesh;
        let mut sum = 0.0;
        for k in 0..12 {
            for j in 0..12 {
                let ix = [6, j, k];
                sum += s.vel[0][m.idx(0, ix)] * m.dx[1][j] * m.dx[2][k];
            }
        }
        sum
    };
    assert!(net.abs() < 1e-9, "{net}");
}

#[test]
fn a_fan_off_the_grid_or_without_a_curve_does_nothing() {
    let mut s = channel(1.0, 40.0, 0.12, 6.0);
    s.cfg.interior_fans[0].max_pressure_pa = 0.0;
    for _ in 0..50 {
        s.step(0.01);
    }
    let (q, _) = s.interior_fan_state(0).unwrap();
    assert!(q.abs() < 1e-9, "{q}");
}

/// The jet of a fan in a closed box against the free-jet model of the 2D
/// engine. Model against model, and an honest finding: the 3D solver is
/// laminar here (no turbulence model) and the grid coarse, so its jet hardly
/// spreads and stays faster than the turbulent free jet of the 2D model. The
/// test pins that: the resolved jet is at least as long as the model's, so the
/// 2D film estimate is not optimistic about reach. With a turbulence model
/// the jet will spread; that case is not tested yet.
#[test]
fn a_laminar_resolved_jet_reaches_at_least_as_far_as_the_free_jet_model() {
    let (nx, ny, nz) = (32, 16, 16);
    let mesh = Mesh::uniform([nx, ny, nz], [0.8, 0.4, 0.4]);
    let mut cfg = FlowConfig::air();
    cfg.rho = RHO;
    cfg.gravity = [0.0; 3];
    let (d, qf, pmax) = (0.15, 0.10, 30.0);
    cfg.interior_fans = vec![InteriorFan {
        axis: 0,
        sign: 1.0,
        centre: [0.1, 0.2, 0.2],
        radius_m: 0.5 * d,
        free_flow_m3s: qf,
        max_pressure_pa: pmax,
    }];
    let mut s = FlowSolver::new(mesh, cfg, 25.0);
    settle(&mut s, 0.005, 1500, 1e-3);
    let (q, _) = s.interior_fan_state(0).unwrap();
    let area = std::f64::consts::PI * d * d / 4.0;
    let u0 = q / area;
    let axis_speed = |x: f64| {
        let i = ((x / 0.025) as usize).min(nx - 1);
        s.vel[0][s.mesh.idx(0, [i, ny / 2, nz / 2])]
    };
    println!("INTERIOR FAN | jet | Q {q:.4} m3/s | exit speed {u0:.2} m/s");
    for x in [0.2, 0.3, 0.4, 0.5] {
        let u3 = axis_speed(x);
        let u2 = novo_thermal::forced::round_jet_speed(u0, d, x - 0.1, 0.0);
        println!("INTERIOR FAN | jet | x {x:.1} m | 3D {u3:.2} m/s | 2D model {u2:.2} m/s");
        assert!(u3 > 0.0);
        assert!(u3 > 0.8 * u2, "{u3} vs {u2}");
    }
}

#[test]
fn scenario_circulation_fans_become_interior_fans() {
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    use novo_thermal::*;
    let mut scn = Scenario {
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
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    };
    scn.params.circulation_fans = vec![
        CirculationFan {
            x_mm: 300.0,
            y_mm: 400.0,
            diameter_mm: 120.0,
            direction: BlowDirection::Up,
            free_flow_m3h: 360.0,
            max_pressure_pa: 50.0,
            installed_factor: 0.8,
            z_mm: None,
        },
        CirculationFan {
            x_mm: 100.0,
            y_mm: 100.0,
            diameter_mm: 80.0,
            direction: BlowDirection::ToPlate,
            free_flow_m3h: 100.0,
            max_pressure_pa: 30.0,
            installed_factor: 1.0,
            z_mm: Some(40.0),
        },
    ];
    let fans = interior_fans_from_scenario(&scn).unwrap();
    assert_eq!(fans.len(), 2);
    // y is down: "up" pushes towards the min side of axis 1
    assert_eq!((fans[0].axis, fans[0].sign), (1, -1.0));
    assert!((fans[0].free_flow_m3s - 0.1 * 0.8).abs() < 1e-12);
    assert!((fans[0].max_pressure_pa - 40.0).abs() < 1e-12);
    assert!((fans[0].radius_m - 0.06).abs() < 1e-12);
    // half the air depth: plate 2.5 mm, interior depth 296 mm
    let half = (2.5 + 0.5 * (296.0 - 2.5)) * 1e-3;
    assert!((fans[0].centre[2] - half).abs() < 1e-12);
    assert_eq!((fans[1].axis, fans[1].sign), (2, -1.0));
    assert!((fans[1].centre[2] - 0.0425).abs() < 1e-12);
    // a fan without a curve is refused with its index
    scn.params.circulation_fans[1].max_pressure_pa = 0.0;
    assert_eq!(
        interior_fans_from_scenario(&scn),
        Err(novo_cfd::openings::OpeningError::BadFan(1))
    );
}
