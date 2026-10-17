//! The 3D transient with surface radiation coupled (RadMode).

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, RadMode, Tuning};
use novo_cfd::{Control, Fidelity, TransientSolver, TransientSpec, TransientSummary};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

/// A sealed steel cabinet with a hot device (100 W) and an idle one beside it.
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

fn run(mode: RadMode) -> (TransientSummary, bool, f64) {
    let mut t = Tuning::default().with_cells(1500);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.full_snapshot_every = Duration::ZERO;
    }
    let spec = TransientSpec {
        duration_s: 300.0,
        frame_interval_s: 100.0,
        ..TransientSpec::default()
    };
    let (mut on, mut balance) = (false, 1.0);
    let sum = CabinetSolver::new(t)
        .with_radiation(mode)
        .solve_transient(
            &scenario(),
            Fidelity::Coarse,
            &spec,
            &Control::new(),
            &mut |f| {
                on = f.snapshot.radiation_on;
                balance = f.snapshot.progress.energy_balance_rel;
            },
        )
        .unwrap();
    (sum, on, balance)
}

#[test]
fn the_transient_couples_radiation_when_asked_and_keeps_its_energy_books() {
    let (lumped, on_lumped, _) = run(RadMode::Off);
    let (coupled, on, balance) = run(RadMode::On);
    assert!(!on_lumped && on);
    assert!(balance < 0.08, "{balance}");
    let hot = |s: &TransientSummary| *s.devices[0].surface_c.last().unwrap();
    let cold = |s: &TransientSummary| *s.devices[1].surface_c.last().unwrap();
    println!(
        "TRANSIENT RAD | hot {:.1} -> {:.1} | idle {:.1} -> {:.1} | balance {balance:.4}",
        hot(&lumped),
        hot(&coupled),
        cold(&lumped),
        cold(&coupled)
    );
    // the hot device gives heat to what it sees, the idle one takes some
    assert!(hot(&coupled) < hot(&lumped));
    assert!(cold(&coupled) > cold(&lumped));
    assert!(hot(&coupled) > cold(&coupled));
}
