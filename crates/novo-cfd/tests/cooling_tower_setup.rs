//! The cooling tower cell of the laptop test: 20 devices on three rails with
//! ducts, an 800 x 1000 x 300 mm enclosure and a 750 x 950 mm plate. Its setup
//! used to hang in the large-cabinet pitch search (a mesh cascade that refined
//! forever), so Simulate sat at "meshing and setting up" for minutes and Cancel
//! did nothing. These tests keep the setup short and cancellable.

use std::time::{Duration, Instant};

use novo_cfd::cabinet::CabinetSolver;
use novo_cfd::large::{ReduceSettings, reduce_with};
use novo_cfd::mesh::MeshSettings;
use novo_cfd::{CfdError, Control, Fidelity, Solver};
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

/// Devices of a real panel: breakers, contactors, a drive, supplies, a PLC.
fn cell(watts: f64) -> Scenario {
    // 20 devices in three rows on rails, sizes as a real panel: breakers,
    // contactors, a drive, supplies, a PLC.
    let sizes: [(f64, f64); 20] = [
        (45.0, 90.0),
        (45.0, 90.0),
        (45.0, 90.0),
        (45.0, 90.0),
        (36.0, 90.0),
        (36.0, 90.0),
        (54.0, 90.0),
        (54.0, 90.0),
        (90.0, 120.0),
        (72.0, 100.0),
        (70.0, 110.0),
        (90.0, 90.0),
        (36.0, 90.0),
        (36.0, 90.0),
        (45.0, 90.0),
        (45.0, 90.0),
        (100.0, 160.0),
        (120.0, 200.0),
        (60.0, 90.0),
        (60.0, 90.0),
    ];
    let rows = [(160.0, 8usize), (460.0, 6), (760.0, 6)];
    let mut devices = Vec::new();
    let mut k = 0;
    for (y, n) in rows {
        let mut x = 90.0;
        for _ in 0..n {
            let (w, h) = sizes[k];
            devices.push(Device {
                id: 1 + k as u64,
                footprint: Rect { x, y, w, h },
                height_mm: Some(70.0),
                loss_w: watts,
                rated_max_ambient_c: None,
                rated_max_surface_c: None,
                heat_capacity_j_per_k: None,
            });
            x += w + 12.0;
            k += 1;
        }
    }
    let mut obstacles = Vec::new();
    for y in [150.0, 450.0, 750.0] {
        obstacles.push(Obstacle {
            kind: ObstacleKind::Rail,
            rect: Rect {
                x: 60.0,
                y: y + 40.0,
                w: 600.0,
                h: 35.0,
            },
            blockage: None,
            loss_w: 0.0,
        });
        obstacles.push(Obstacle {
            kind: ObstacleKind::Duct,
            rect: Rect {
                x: 40.0,
                y: y - 60.0,
                w: 600.0,
                h: 40.0,
            },
            blockage: None,
            loss_w: 0.0,
        });
    }
    Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 23.0,
                y: 23.0,
                w: 750.0,
                h: 950.0,
            },
        },
        ambient_c: 35.0,
        devices,
        obstacles,
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

fn coarse_mesh() -> MeshSettings {
    // What the Coarse level of the app meshes to (about 6 000 cells).
    MeshSettings {
        target_cells: 6_000,
        ..MeshSettings::default()
    }
}

#[test]
fn the_reduction_of_the_cell_ends_in_seconds_with_and_without_loss_data() {
    for watts in [0.0, 20.0] {
        let t = Instant::now();
        let red = reduce_with(
            &cell(watts),
            &ReduceSettings {
                mesh: coarse_mesh(),
                ..ReduceSettings::for_cells(6_000)
            },
            &|| false,
        )
        .expect("not cancelled");
        let took = t.elapsed();
        assert!(
            took < Duration::from_secs(20),
            "{watts} W: the pitch search took {took:?}"
        );
        assert!(red.report.reduced_cells > 0);
    }
}

#[test]
fn a_cancelled_setup_stops_in_the_reduction_and_names_its_stage() {
    let control = Control::new();
    control.cancel();
    let t = Instant::now();
    let r = CabinetSolver::default().solve(&cell(0.0), Fidelity::Coarse, &control, &mut |_| {});
    assert!(matches!(r, Err(CfdError::Cancelled)), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    assert!(
        ["Planning the grid", "Reducing the cabinet", "Meshing"].contains(&control.phase()),
        "{}",
        control.phase()
    );
}

#[test]
fn cancel_while_the_pitch_search_runs_ends_it() {
    let cancelled = std::sync::atomic::AtomicUsize::new(0);
    let r = reduce_with(
        &cell(0.0),
        &ReduceSettings {
            mesh: coarse_mesh(),
            ..ReduceSettings::for_cells(6_000)
        },
        // Yes from the third pitch on.
        &|| cancelled.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= 2,
    );
    assert!(r.is_none());
}
