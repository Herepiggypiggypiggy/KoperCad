//! The 3D heat overlays on an analytic field.

use novo_cfd::{
    AccuracyStatement, CfdSnapshot, Fidelity, Grid3, MaterialInfo, MaterialKind, Progress,
    Residuals,
};
use novo_heatview::cfd::Axis;
use novo_heatview::range::Range;
use novo_view3d::{Overlay, PlateFrame};

use super::heat_3d::{Heat3d, Place, overlays, shell_mesh, slice_mesh, stream_lines};

/// 100 x 80 x 60 mm; `T = 20 + x / 10` (20 to 30 C); a wind of 1 m/s along
/// +x; a 20 mm cube device at the origin.
fn snapshot() -> CfdSnapshot {
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
fn place() -> Place {
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
    let l = stream_lines(&snapshot(), place());
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
    let count = |s: Heat3d| overlays(&snap, 20.0, place(), &s).len();
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
            }
        )[0],
        Overlay::Lines(_)
    ));
}

#[test]
fn a_broken_snapshot_gives_no_overlays() {
    let mut snap = snapshot();
    snap.temperature_c.pop();
    assert!(overlays(&snap, 20.0, place(), &Heat3d::default()).is_empty());
}
