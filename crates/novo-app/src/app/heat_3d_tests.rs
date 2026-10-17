//! The 3D heat overlays on an analytic field.

use novo_cfd::{
    AccuracyStatement, CfdSnapshot, Fidelity, Grid3, MaterialInfo, MaterialKind, Progress,
    Residuals,
};
use novo_heatview::cfd::Axis;
use novo_heatview::range::Range;
use novo_view3d::{Overlay, PlateFrame};

use super::heat_3d::{
    Flow, Heat3d, Place, flow, overlays, shell_mesh, slice_mesh, stream_lines, tint_devices,
    tracers,
};

/// 100 x 80 x 60 mm; `T = 20 + x / 10` (20 to 30 C); a wind of 1 m/s along
/// +x; a 20 mm cube device at the origin.
pub(super) fn snapshot() -> CfdSnapshot {
    let grid = Grid3::uniform([20, 16, 12], [100.0, 80.0, 60.0]);
    let n = grid.cells();
    let (mut temp, mut mat) = (Vec::new(), Vec::new());
    for k in 0..grid.nz {
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let c = grid.centre_mm(i, j, k);
                temp.push((c[0] / 10.0 + 20.0) as f32);
                mat.push(u16::from(c[0] < 20.0 && c[1] < 20.0 && c[2] < 20.0));
            }
        }
    }
    CfdSnapshot {
        grid,
        temperature_c: temp,
        velocity: vec![[1.0, 0.0, 0.0]; n],
        pressure_pa: vec![0.0; n],
        turb_visc_ratio: vec![0.0; n],
        material: mat,
        materials: vec![
            MaterialInfo {
                id: 0,
                kind: MaterialKind::Air,
                name: "Air".into(),
                conductivity_w_mk: 0.026,
            },
            MaterialInfo {
                id: 1,
                kind: MaterialKind::Device(7),
                name: "-U1".into(),
                conductivity_w_mk: 20.0,
            },
        ],
        wall_flux_w_m2: vec![0.0; n],
        devices: Vec::new(),
        hot_spots: Vec::new(),
        progress: Progress {
            fidelity: Fidelity::Coarse,
            iteration: 1,
            residuals: Residuals::default(),
            converged: true,
            elapsed_s: 0.0,
            energy_balance_rel: 0.0,
            monitor_drift_k: 0.0,
        },
        notes: Vec::new(),
        accuracy: AccuracyStatement::default(),
    }
}

/// The plate corner sits 10 mm in from the solver origin on both axes.
pub(super) fn place() -> Place {
    Place {
        plate_x: 10.0,
        plate_y: 10.0,
        frame: PlateFrame {
            width: 80.0,
            height: 60.0,
        },
    }
}

const RANGE: Range = Range { lo: 20.0, hi: 30.0 };

#[test]
fn a_slice_parallel_to_the_plate_lies_at_its_depth() {
    let m = slice_mesh(&snapshot(), place(), RANGE, Axis::Z, 0.5).unwrap();
    assert!(m.is_valid() && m.translucent);
    assert!(m.positions.iter().all(|p| (p.z - 30.0).abs() < 1e-4));
}

#[test]
fn a_slice_follows_the_plate_frame() {
    let m = slice_mesh(&snapshot(), place(), RANGE, Axis::Z, 0.5).unwrap();
    // Solver x 0..100 is plate x -10..90; solver y 0..80 is world y 70..-10.
    let (lo, hi) = m
        .positions
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.x), b.max(p.x)));
    assert!(
        (lo + 10.0).abs() < 1e-3 && (hi - 90.0).abs() < 1e-3,
        "{lo} {hi}"
    );
    let top = m.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
    assert!((top - 70.0).abs() < 1e-3, "{top}");
}

#[test]
fn a_slice_across_x_has_its_x_fixed_and_leaves_out_solids() {
    let snap = snapshot();
    let m = slice_mesh(&snap, place(), RANGE, Axis::X, 0.05).unwrap();
    assert!(m.is_valid());
    // Solver x = 5 mm is plate x = -5 mm.
    assert!(m.positions.iter().all(|p| (p.x + 5.0).abs() < 1e-4));
    // The device fills y < 20, z < 20 there: no quad starts inside it.
    let full = slice_mesh(&snap, place(), RANGE, Axis::X, 0.95).unwrap();
    assert!(m.indices.len() < full.indices.len());
}

#[test]
fn slice_colours_follow_the_temperature() {
    let snap = snapshot();
    let at = |frac: f32| slice_mesh(&snap, place(), RANGE, Axis::Z, frac).unwrap();
    let m = at(0.5);
    // Left (cold) vertices are not the same colour as right (hot) ones.
    let left = m.positions.iter().position(|p| p.x < 0.0).unwrap();
    let right = m.positions.iter().position(|p| p.x > 80.0).unwrap();
    assert_ne!(m.colors[left], m.colors[right]);
    assert!(m.colors.iter().all(|c| c[3] > 0 && c[3] < 255));
}

#[test]
fn the_shell_sits_where_the_temperature_reaches_the_level() {
    let m = shell_mesh(&snapshot(), place(), RANGE, 25.0);
    assert!(m.is_valid() && !m.indices.is_empty());
    // T = 25 at solver x = 50, plate x = 40.
    assert!(m.positions.iter().all(|p| (p.x - 40.0).abs() < 6.0));
    assert!(
        shell_mesh(&snapshot(), place(), RANGE, 90.0)
            .indices
            .is_empty()
    );
}

#[test]
fn streamlines_run_downwind_in_the_scene_frame() {
    let l = stream_lines(&flow(&snapshot(), place()));
    assert!(l.is_valid() && !l.starts.is_empty());
    for (k, &s) in l.starts.iter().enumerate() {
        let end = l
            .starts
            .get(k + 1)
            .map_or(l.positions.len(), |e| *e as usize);
        let line = &l.positions[s as usize..end];
        assert!(
            line.windows(2).all(|w| w[1].x >= w[0].x),
            "wind blows along +x"
        );
    }
}

#[test]
fn settings_choose_the_overlays() {
    let snap = snapshot();
    let fl = flow(&snap, place());
    let count = |s: Heat3d| overlays(&snap, 20.0, place(), &s, &fl).len();
    let off = Heat3d {
        slice: false,
        iso: false,
        lines: false,
        ..Heat3d::default()
    };
    assert_eq!(count(off), 0);
    assert_eq!(count(Heat3d { slice: true, ..off }), 1);
    assert_eq!(
        count(Heat3d {
            slice: true,
            iso: true,
            lines: true,
            ..off
        }),
        3
    );
    assert!(matches!(
        overlays(
            &snap,
            20.0,
            place(),
            &Heat3d {
                lines: true,
                slice: false,
                ..off
            },
            &fl
        )[0],
        Overlay::Lines(_)
    ));
}

#[test]
fn a_broken_snapshot_gives_no_overlays() {
    let mut snap = snapshot();
    snap.temperature_c.pop();
    assert!(overlays(&snap, 20.0, place(), &Heat3d::default(), &Flow::default()).is_empty());
    assert!(flow(&snap, place()).lines.is_empty());
}

/// Plate corner at solver (10, 30) and a plate 50 mm high: nothing symmetric.
fn lopsided() -> Place {
    Place {
        plate_x: 10.0,
        plate_y: 30.0,
        frame: PlateFrame {
            width: 80.0,
            height: 50.0,
        },
    }
}

#[test]
fn solver_corners_land_where_the_plate_frame_puts_them() {
    // The device stands at solver x < 20, y < 20, z < 20, so a slice at
    // z = 10 has a hole there: world x -10 to 10, y 60 to 80 on this plate.
    let m = slice_mesh(&snapshot(), lopsided(), RANGE, Axis::Z, 10.0 / 60.0).unwrap();
    let has = |x: f32, y: f32| {
        m.positions
            .iter()
            .any(|p| (p.x - x).abs() < 1e-3 && (p.y - y).abs() < 1e-3)
    };
    assert!(!has(-2.0, 72.0), "the hole is where the device is");
    assert!(has(-2.0, 8.0) && has(78.0, 72.0), "the rest is there");
    // The extent is the solver box: plate x -10 to 90, world y 0 to 80.
    let ys = m.positions.iter().map(|p| p.y);
    assert!((ys.clone().fold(f32::MIN, f32::max) - 80.0).abs() < 1e-3);
    assert!(ys.fold(f32::MAX, f32::min).abs() < 1e-3);
}

#[test]
fn the_door_side_is_the_high_z() {
    let snap = snapshot();
    let near = slice_mesh(&snap, lopsided(), RANGE, Axis::Z, 0.1).unwrap();
    let far = slice_mesh(&snap, lopsided(), RANGE, Axis::Z, 0.9).unwrap();
    assert!(near.positions[0].z < far.positions[0].z);
}

#[test]
fn tracers_move_downwind_and_stay_on_their_line() {
    let fl = flow(&snapshot(), place());
    let (a, b) = (tracers(&fl, 0.0), tracers(&fl, 0.5));
    assert!(a.is_valid() && b.is_valid() && !a.positions.is_empty());
    assert_ne!(a.positions, b.positions, "they move");
    // Wind is along +x: within one dash, x never decreases.
    for l in [&a, &b] {
        for (k, &s) in l.starts.iter().enumerate() {
            let end = l
                .starts
                .get(k + 1)
                .map_or(l.positions.len(), |e| *e as usize);
            assert!(
                l.positions[s as usize..end]
                    .windows(2)
                    .all(|w| w[1].x >= w[0].x - 1e-3)
            );
        }
    }
    // The tail is fainter than the head.
    let s = a.starts[0] as usize;
    let e = a.starts.get(1).map_or(a.positions.len(), |e| *e as usize);
    assert!(a.colors[s][3] < a.colors[e - 1][3]);
}

#[test]
fn devices_are_coloured_by_the_air_in_front_of_them() {
    use novo_view3d::{Item, ItemId, ItemKind, Pose, Scene, Shape};
    let frame = PlateFrame {
        width: 100.0,
        height: 80.0,
    };
    let place = Place {
        plate_x: 0.0,
        plate_y: 0.0,
        frame,
    };
    let mut scene = Scene::new(frame);
    // Two 10 mm devices on the plate: one near x = 10 (cool), one near x = 90 (warm).
    for (raw, x) in [(1, 10.0), (2, 90.0)] {
        scene.push(Item::new(
            ItemId::new(ItemKind::Device, raw),
            Shape::Box,
            Pose::at(frame.world(x, 40.0, 0.0)),
            glam::Vec3::new(10.0, 10.0, 20.0),
        ));
    }
    let tinted = tint_devices(&scene, &snapshot(), 20.0, place, 7);
    assert_ne!(tinted.revision, scene.revision);
    let (a, b) = (tinted.items[0].tint, tinted.items[1].tint);
    assert!(a.is_some() && b.is_some() && a != b, "{a:?} {b:?}");
    // The original is untouched.
    assert!(scene.items.iter().all(|i| i.tint.is_none()));
}

#[test]
fn the_slice_edges_are_the_result_along_its_axis() {
    use super::heat_3d::slice_edges;
    let snap = snapshot();
    assert_eq!(slice_edges(&snap, Axis::X).len(), 21);
    assert_eq!(slice_edges(&snap, Axis::Y).len(), 17);
    assert_eq!(slice_edges(&snap, Axis::Z).last(), Some(&60.0));
}

#[test]
fn stepping_goes_to_the_neighbouring_cell_centre_on_a_graded_axis() {
    use super::heat_3d::step_cell;
    // Fine cells near 0 (the wall), coarse further in.
    let edges = [0.0, 1.0, 3.0, 7.0, 15.0, 31.0];
    // From inside the first cell forward: the centre of the second.
    assert_eq!(step_cell(&edges, 0.5, 1), 2.0);
    // From the centre of the second (2.0): the third (5.0), then back to the second.
    assert_eq!(step_cell(&edges, 2.0, 1), 5.0);
    assert_eq!(step_cell(&edges, 5.0, -1), 2.0);
    // Ends hold.
    assert_eq!(step_cell(&edges, 0.5, -1), 0.5);
    assert_eq!(step_cell(&edges, 31.0, 1), 23.0);
}
