//! Helpers on a CFD snapshot, checked on analytic fields.

use novo_cfd::{
    AccuracyStatement, CfdSnapshot, Fidelity, Grid3, MaterialInfo, MaterialKind, Progress,
    Residuals,
};
use novo_heatview::cfd::{self, Axis, Plane, Trace};

/// 100 x 80 x 60 mm, 20 x 16 x 12 cells; `T = x / 10 + 20` and a wind of
/// 1 m/s along +x. A 20 mm cube device stands on the plate at the origin.
fn snapshot() -> CfdSnapshot {
    let grid = Grid3::uniform([20, 16, 12], [100.0, 80.0, 60.0]);
    let n = grid.cells();
    let (mut temp, mut mat) = (Vec::new(), Vec::new());
    for k in 0..grid.nz {
        for j in 0..grid.ny {
            for i in 0..grid.nx {
                let c = grid.centre_mm(i, j, k);
                temp.push((c[0] / 10.0 + 20.0) as f32);
                let device = c[0] < 20.0 && c[1] < 20.0 && c[2] < 20.0;
                mat.push(u16::from(device));
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

#[test]
fn a_linear_field_is_reproduced_between_cell_centres() {
    let s = snapshot();
    let t = cfd::temperature_at(&s, [52.5, 40.0, 30.0]);
    assert!((t - (52.5 / 10.0 + 20.0)).abs() < 1e-4, "{t}");
    // Beyond the outermost centre the value is held, not extrapolated.
    let edge = cfd::temperature_at(&s, [0.0, 40.0, 30.0]);
    assert!((edge - 20.25).abs() < 1e-4, "{edge}");
}

#[test]
fn a_plate_side_slice_has_the_window_size_and_the_wind() {
    let s = snapshot();
    let plane = Plane {
        axis: Axis::Z,
        offset_mm: 40.0,
    };
    let w = cfd::whole_window(&s, Axis::Z);
    let sl = cfd::slice(&s, plane, w, 5.0).unwrap();
    assert_eq!(
        (sl.temperature.grid.cols, sl.temperature.grid.rows),
        (20, 16)
    );
    assert!((sl.temperature.sample(52.5, 40.0) - 25.25).abs() < 0.01);
    let (u, v) = sl.flow.sample(50.0, 40.0);
    assert!((u - 1.0).abs() < 1e-5 && v.abs() < 1e-5);
    assert!(!sl.is_solid(50.0, 40.0));
}

#[test]
fn a_vertical_slice_swaps_the_axes() {
    let s = snapshot();
    let plane = Plane {
        axis: Axis::X,
        offset_mm: 50.0,
    };
    let w = cfd::whole_window(&s, Axis::X);
    assert_eq!((w.w, w.h), (60.0, 80.0));
    let sl = cfd::slice(&s, plane, w, 5.0).unwrap();
    // Constant in the cut plane; the wind is across it, so no in-plane flow.
    let (lo, hi) = sl.temperature.extent().unwrap();
    assert!((hi - lo).abs() < 1e-4);
    assert!(sl.flow.max_speed() < 1e-6);
    assert!((sl.speed.at(3, 3) - 1.0).abs() < 1e-5);
}

#[test]
fn the_front_plane_stands_just_before_the_device_fronts() {
    let s = snapshot();
    let z = cfd::front_plane_z(&s);
    // The device occupies z < 20 mm; the plane is in the first air layer.
    assert!(z > 20.0 && z < 30.0, "{z}");
    let w = cfd::Window {
        u0: 0.0,
        v0: 0.0,
        w: 100.0,
        h: 80.0,
    };
    let two_d = cfd::as_2d(&s, w, 5.0).unwrap();
    assert!(!two_d.is_solid(10.0, 10.0), "air in front of the device");
}

#[test]
fn without_devices_the_plane_is_a_quarter_of_the_depth() {
    let mut s = snapshot();
    s.material.iter_mut().for_each(|m| *m = 0);
    assert!((cfd::front_plane_z(&s) - 15.0).abs() < 1e-9);
}

#[test]
fn streamlines_follow_the_wind_and_stop_at_a_solid() {
    let s = snapshot();
    let lines = cfd::streamlines(
        &s,
        &[[30.0, 40.0, 40.0], [2.0, 5.0, 5.0], [200.0, 0.0, 0.0]],
        Trace::default(),
    );
    let free = &lines[0];
    assert!(free.len() > 5);
    assert!(
        free.iter()
            .all(|p| (p[1] - 40.0).abs() < 1e-9 && p[2] == 40.0)
    );
    assert!(free.windows(2).all(|w| w[1][0] > w[0][0]));
    assert!(free.last().unwrap()[0] > 90.0, "runs to the far wall");
    assert_eq!(lines[1].len(), 1, "a seed inside the device");
    assert_eq!(lines[2].len(), 1, "a seed outside the cabinet");
}

#[test]
fn a_still_field_gives_no_lines() {
    let mut s = snapshot();
    s.velocity.iter_mut().for_each(|v| *v = [0.0; 3]);
    let lines = cfd::streamlines(&s, &[[50.0, 40.0, 40.0]], Trace::default());
    assert_eq!(lines[0].len(), 1);
}

#[test]
fn the_iso_surface_of_a_linear_field_is_a_plane() {
    let s = snapshot();
    // T = 25 at x = 50 mm. The lattice of centres spans 2.5 .. 97.5 in x,
    // 2.5 .. 77.5 in y (dy = 5) and 2.5 .. 57.5 in z (dz = 5).
    let tris = cfd::iso_surface(&s, 25.0);
    assert!(!tris.is_empty());
    assert!(tris.iter().flatten().all(|p| (p[0] - 50.0).abs() < 1e-3));
    let area = cfd::area_mm2(&tris);
    assert!((area - 75.0 * 55.0).abs() < 1.0, "{area}");
}

#[test]
fn a_level_outside_the_field_gives_nothing() {
    let s = snapshot();
    assert!(cfd::iso_surface(&s, 5.0).is_empty());
    assert!(cfd::iso_surface(&s, 90.0).is_empty());
}

#[test]
fn an_inconsistent_snapshot_is_refused_not_a_panic() {
    let mut s = snapshot();
    s.temperature_c.pop();
    let w = cfd::whole_window(&s, Axis::Z);
    assert!(
        cfd::slice(
            &s,
            Plane {
                axis: Axis::Z,
                offset_mm: 10.0
            },
            w,
            5.0
        )
        .is_none()
    );
    assert!(cfd::iso_surface(&s, 25.0).is_empty());
    assert!(cfd::streamlines(&s, &[[10.0, 10.0, 10.0]], Trace::default()).is_empty());
}
