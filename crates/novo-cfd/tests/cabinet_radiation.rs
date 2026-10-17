//! The cabinet solver with surface radiation coupled in (the Fine level).

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, RadiationCoupling, Tuning};
use novo_cfd::mesh::MeshSettings;
use novo_cfd::rad::RadSettings;
use novo_cfd::solids::{FlowSetup, SolidsSettings};
use novo_cfd::{CfdSnapshot, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

/// A sealed steel cabinet with a hot device (100 W) and a cold one (0 W)
/// side by side, so the cold one can only warm by radiation and by air.
fn scenario() -> Scenario {
    let device = |id, x, loss_w| Device {
        id,
        footprint: Rect {
            x,
            y: 300.0,
            w: 120.0,
            h: 200.0,
        },
        height_mm: Some(100.0),
        loss_w,
        rated_max_ambient_c: Some(60.0),
        rated_max_surface_c: Some(90.0),
        heat_capacity_j_per_k: None,
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
        devices: vec![device(1, 150.0, 100.0), device(2, 300.0, 0.0)],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn run(radiation: bool) -> CfdSnapshot {
    let mut t = Tuning::default().with_cells(3000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.steady_tol = 1e-2;
        l.report_every = 20;
        l.full_snapshot_every = Duration::ZERO;
        l.radiation = radiation;
    }
    CabinetSolver::new(t)
        .solve(&scenario(), Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

#[test]
fn coupled_radiation_balances_and_moves_heat_to_the_cold_device() {
    let lumped = run(false);
    let coupled = run(true);
    assert!(coupled.is_consistent());
    println!(
        "RADIATION | hot {:.1} -> {:.1} | cold {:.1} -> {:.1} | balance {:.4}",
        lumped.devices[0].surface_c,
        coupled.devices[0].surface_c,
        lumped.devices[1].surface_c,
        coupled.devices[1].surface_c,
        coupled.progress.energy_balance_rel
    );
    assert!(coupled.progress.converged, "{:?}", coupled.progress);
    assert!(coupled.progress.energy_balance_rel < 0.03);
    // the hot device gives heat to the surfaces it sees, the idle one takes some
    assert!(coupled.devices[0].surface_c < lumped.devices[0].surface_c - 1.0);
    assert!(coupled.devices[1].surface_c > lumped.devices[1].surface_c + 0.3);
    assert!(coupled.devices[0].surface_c > coupled.devices[1].surface_c);
    let min = coupled
        .temperature_c
        .iter()
        .copied()
        .fold(f32::MAX, f32::min);
    assert!(min > 24.9, "{min}");
}

#[test]
fn the_result_says_radiation_is_coupled_only_when_it_is() {
    let lumped = run(false);
    let coupled = run(true);
    let text = |s: &CfdSnapshot| s.notes.join(" ") + &s.accuracy.limits.join(" ");
    assert!(!text(&lumped).contains("surface-to-surface"));
    assert!(text(&lumped).contains("radiation are not modelled"));
    let t = text(&coupled);
    assert!(t.contains("surface-to-surface"), "{t}");
    assert!(!t.contains("radiation are not modelled"), "{t}");
    assert!(coupled.accuracy.headline.contains("Radiation"));
}

#[test]
fn only_the_fine_level_couples_radiation_by_default() {
    let t = Tuning::default();
    assert!(!t.coarse.radiation && !t.medium.radiation && t.fine.radiation);
}

#[test]
fn the_source_moves_heat_around_and_creates_none() {
    let st = SolidsSettings {
        mesh: MeshSettings {
            target_cells: 3000,
            ..MeshSettings::for_fidelity(Fidelity::Coarse).unwrap()
        },
        ..SolidsSettings::default()
    };
    let setup = FlowSetup::new(&scenario(), &st).unwrap();
    let mut flow = setup.solver();
    // a temperature field with a hot device: radiation has something to move
    let md = setup.cfg.medium.as_ref().unwrap();
    for (id, t) in flow.t.iter_mut().enumerate() {
        if md.dev_frac[id] > 0.5 && md.heat_w_m3[id] > 0.0 {
            *t = 90.0;
        }
    }
    let mut rad =
        RadiationCoupling::new(&setup.classified, &mut flow, &RadSettings::default()).unwrap();
    rad.refresh(&mut flow);
    let extra = flow.extra_heat_w_m3.as_ref().unwrap();
    let m = &flow.mesh;
    let (mut net, mut gross) = (0.0, 0.0);
    for (id, q) in extra.iter().enumerate() {
        let w = q * m.volume(m.unindex(id));
        net += w;
        gross += w.abs();
    }
    println!("RADIATION | gross {gross:.1} W | net {net:.4} W");
    assert!(gross > 10.0, "{gross}");
    assert!(net.abs() < 0.01 * gross, "{net} of {gross}");
    let r = rad.report();
    assert!(r.imbalance_w.abs() < 0.01 * r.exchanged_w.max(1.0));
    // the hot device loses heat
    let hot: f64 = (0..extra.len())
        .filter(|&id| md.dev_frac[id] > 0.5 && md.heat_w_m3[id] > 0.0)
        .map(|id| extra[id])
        .sum();
    assert!(hot < 0.0);
}

#[test]
fn switched_off_in_the_settings_it_changes_nothing() {
    let st = SolidsSettings::default();
    let setup = FlowSetup::new(&scenario(), &st).unwrap();
    let mut flow = setup.solver();
    let off = RadSettings {
        enabled: false,
        ..RadSettings::default()
    };
    assert!(RadiationCoupling::new(&setup.classified, &mut flow, &off).is_none());
    assert!(flow.extra_heat_w_m3.is_none());
    assert!(flow.cfg.device_view > 0.0);
}
