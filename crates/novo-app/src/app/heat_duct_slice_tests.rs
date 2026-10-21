//! The Z slice of the heat view must not be cut open by wire ducts that the
//! plane has already passed (reported by Tadjiro on older builds: the slice
//! looked pierced by the troughs even when placed past them).

use std::time::Duration;

use novo_cfd::cabinet::{CabinetSolver, Tuning};
use novo_cfd::{CfdSnapshot, Control, Fidelity, MaterialKind, Solver};
use novo_heatview::cfd::{Axis, Plane, slice, whole_window};

/// The example cabinet solved coarsely; `old_height` gives every duct the
/// height the bridge used before the duct fix (70 % of the air depth).
fn example_snapshot(old_height: bool) -> CfdSnapshot {
    let mut doc = crate::backend::new_document("Example").unwrap();
    super::example::build_example(&mut doc).unwrap();
    let project = doc.into_project();
    let cab = project
        .all::<novo_core::Structure>()
        .find(|s| s.enclosure)
        .map(|s| s.id)
        .unwrap();
    let mut b = novo_thermal::bridge::scenario_from_project(
        &project,
        cab,
        &novo_thermal::bridge::BridgeOptions::default(),
    )
    .unwrap();
    if old_height {
        for o in &mut b.scenario.obstacles {
            o.blockage = None;
        }
    }
    let mut t = Tuning::default().with_cells(3000);
    for l in [&mut t.coarse, &mut t.medium, &mut t.fine] {
        l.max_steps = 60;
        l.report_every = 60;
        l.full_snapshot_every = Duration::ZERO;
    }
    CabinetSolver::new(t)
        .solve(&b.scenario, Fidelity::Coarse, &Control::new(), &mut |_| {})
        .unwrap()
}

/// (cells the slice leaves out, cells of the plane that hold a duct,
/// cells of the plane that hold another solid)
fn plane_counts(snap: &CfdSnapshot, z_mm: f64) -> (usize, usize, usize) {
    let g = &snap.grid;
    let sl = slice(
        snap,
        Plane {
            axis: Axis::Z,
            offset_mm: z_mm,
        },
        whole_window(snap, Axis::Z),
        4.0,
    )
    .unwrap();
    let kind = |id: u16| snap.materials.iter().find(|m| m.id == id).map(|m| m.kind);
    let cell = |e: &[f64], x: f64| e[1..e.len() - 1].partition_point(|v| *v <= x);
    let (mut duct, mut other) = (0, 0);
    for r in 0..sl.temperature.grid.rows {
        for c in 0..sl.temperature.grid.cols {
            let (du, dv) = sl.temperature.grid.centre(c, r);
            let (i, j, k) = (
                cell(&g.x_edges_mm, sl.window.u0 + du),
                cell(&g.y_edges_mm, sl.window.v0 + dv),
                cell(&g.z_edges_mm, z_mm),
            );
            match snap.material[g.index(i, j, k)] {
                0 => {}
                id if kind(id) == Some(MaterialKind::Duct) => duct += 1,
                _ => other += 1,
            }
        }
    }
    (sl.solid.iter().filter(|s| **s).count(), duct, other)
}

#[test]
fn a_slice_past_the_duct_tops_is_not_pierced_and_one_through_them_shows_air() {
    let snap = example_snapshot(false);
    // 62.5 mm is just past the 60 mm ducts of the example
    let (left_out, duct, other) = plane_counts(&snap, 62.5);
    println!(
        "DUCTSLICE | real height, z 62.5: left out {left_out}, duct cells {duct}, other solids {other}"
    );
    assert_eq!(left_out, other, "the slice leaves out more than the solids");
    // through the ducts (30 mm): the ducts are in the plane and are shown
    let (left_out, duct, other) = plane_counts(&snap, 30.0);
    println!(
        "DUCTSLICE | real height, z 30: left out {left_out}, duct cells {duct}, other solids {other}"
    );
    assert!(duct > 0, "the plane at 30 mm should cross the ducts");
    assert_eq!(left_out, other, "duct cells are cut out of the slice");
}

#[test]
fn the_old_tall_ducts_reach_the_plane_and_the_slice_still_shows_air_there() {
    let snap = example_snapshot(true);
    let (left_out, duct, other) = plane_counts(&snap, 62.5);
    println!(
        "DUCTSLICE | old height, z 62.5: left out {left_out}, duct cells {duct}, other solids {other}"
    );
    assert!(
        duct > 0,
        "old height: the ducts should still be there at 62.5 mm"
    );
    assert_eq!(left_out, other);
}
