//! The Heat flow modes, the per device split and the moving cloud.

use novo_cfd::{DeviceHeatPaths, Fidelity};
use novo_heatview::cfd::Axis;
use novo_heatview::flux;
use novo_layout::Rect;
use novo_thermal::{DeviceResult, Status};

use super::heat_3d::{Heat3d, overlays};
use super::heat_3d_tests::{place, snapshot};
use super::heat_modes::*;

fn dev(id: u64, loss: f64, plate_share: f64) -> DeviceResult {
    DeviceResult {
        id,
        footprint: Rect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        },
        loss_w: loss,
        surface_c: 50.0,
        air_c: 40.0,
        h_w_m2k: 8.0,
        plate_share,
        surface_band_c: (50.0, 50.0),
        air_band_c: (40.0, 40.0),
        rated_max_ambient_c: None,
        rated_max_surface_c: None,
        margin_k: None,
        margin_pessimistic_k: None,
        status: Status::Ok,
    }
}

#[test]
fn each_mode_says_what_it_draws() {
    use HeatFlowMode::*;
    assert!(!Off.needs_flux() && Air.needs_flux() && Solids.needs_flux());
    assert!(Air.draws_air() && !Air.draws_solids() && !Air.draws_map());
    assert!(!Solids.draws_air() && Solids.draws_solids());
    assert!(Both.draws_air() && Both.draws_solids());
    assert!(Map.draws_map() && !Map.draws_air());
    // Every mode has a name and a sentence.
    for m in HeatFlowMode::ALL {
        assert!(!m.label().is_empty() && !m.hint().is_empty());
    }
}

#[test]
fn setting_the_mode_asks_for_the_data_and_off_does_not() {
    let mut h = Heat3d::default();
    assert!(!h.carry);
    h.set_heat_flow(HeatFlowMode::Both);
    assert!(h.carry && h.heat_flow == HeatFlowMode::Both);
    h.set_heat_flow(HeatFlowMode::Off);
    assert!(!h.carry);
}

#[test]
fn the_flux_map_is_brightest_where_the_air_carries_the_most_heat() {
    // A wind of 1 m/s over air that is 1 K warmer per 10 mm along x: the
    // flux grows with x.
    let s = snapshot();
    let field = flux::flux_field(&s, 20.0);
    let top = flux::top_air_flux(&s, &field);
    assert!(top > 0.0);
    let m = flux_map(&s, &field, place(), Axis::Z, 0.5, top, 200).unwrap();
    assert!(m.translucent && !m.indices.is_empty());
    // The strongest quad is more opaque than the weakest.
    let alphas: Vec<u8> = m.colors.iter().map(|c| c[3]).collect();
    assert!(alphas.iter().max() > alphas.iter().min());
    // Nothing is drawn without a scale or for a wrong-sized field.
    assert!(flux_map(&s, &field, place(), Axis::Z, 0.5, 0.0, 200).is_none());
    assert!(flux_map(&s, &field[1..], place(), Axis::Z, 0.5, top, 200).is_none());
}

#[test]
fn the_flux_map_leaves_out_the_solid_cells() {
    let s = snapshot();
    let field = flux::flux_field(&s, 20.0);
    let top = flux::top_air_flux(&s, &field);
    let m = flux_map(&s, &field, place(), Axis::Z, 0.1, top, 200).unwrap();
    // The 20 mm cube at the origin covers a corner of the plane z = 6 mm: the
    // mesh has fewer quads than the plane has cells.
    let full = flux_map(&s, &field, place(), Axis::Z, 0.9, top, 200).unwrap();
    assert!(m.indices.len() < full.indices.len());
}

#[test]
fn the_map_mode_replaces_the_temperature_slice() {
    let s = snapshot();
    let slice_only = Heat3d {
        slice: true,
        ..Heat3d::default()
    };
    let with_map = Heat3d {
        slice: true,
        heat_flow: HeatFlowMode::Map,
        carry: true,
        ..Heat3d::default()
    };
    let flow = super::heat_3d::Flow::default();
    let a = overlays(&s, 20.0, place(), &slice_only, &flow, 1.0);
    let b = overlays(&s, 20.0, place(), &with_map, &flow, 1.0);
    assert_eq!(a.len(), 1);
    assert!(
        b.is_empty(),
        "the map is added by the view, in place of the slice"
    );
}

#[test]
fn the_split_comes_from_the_solver_when_it_has_one() {
    let mut s = snapshot();
    s.devices = vec![dev(1, 10.0, 0.2), dev(2, 30.0, 0.5)];
    s.heat_paths = vec![
        DeviceHeatPaths {
            id: 1,
            total_w: 10.0,
            conduction_w: 2.0,
            radiation_w: 1.0,
        },
        DeviceHeatPaths {
            id: 2,
            total_w: 30.0,
            conduction_w: 15.0,
            radiation_w: 3.0,
        },
    ];
    s.radiation_on = true;
    let rows = path_rows(&s);
    // Most heat first.
    assert_eq!(rows[0].id, 2);
    assert!((rows[0].air_w - 12.0).abs() < 1e-9 && rows[0].plate_w == 15.0);
    for r in &rows {
        let sum: f64 = r.shares().iter().sum();
        assert!((sum - 1.0).abs() < 1e-9, "{r:?}");
        assert!(r.radiation_split);
    }
    let t = path_text(&rows[0], "-K1");
    assert!(
        t.contains("-K1") && t.contains("30.0 W") && t.contains("radiation 10 %"),
        "{t}"
    );
}

#[test]
fn without_a_split_the_plate_share_is_used_and_radiation_is_not_claimed() {
    let mut s = snapshot();
    s.devices = vec![dev(1, 10.0, 0.25), dev(2, 0.0, 0.5)];
    let rows = path_rows(&s);
    // A device without loss has no row.
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].radiation_split);
    assert_eq!(rows[0].plate_w, 2.5);
    assert_eq!(rows[0].air_w, 7.5);
    let t = path_text(&rows[0], "-Q1");
    assert!(
        t.contains("air 75 %") && t.contains("plate 25 %") && !t.contains("radiation"),
        "{t}"
    );
}

/// The snapshot with the cube at 90 C in a 1 m/s wind and ambient air.
fn hot_cube() -> novo_cfd::CfdSnapshot {
    let mut s = snapshot();
    for (t, m) in s.temperature_c.iter_mut().zip(&s.material) {
        *t = if *m == 0 { 20.0 } else { 90.0 };
    }
    s.progress.fidelity = Fidelity::Coarse;
    s
}

#[test]
fn the_moving_cloud_starts_cold_and_warms_downwind_of_the_hot_part() {
    let s = hot_cube();
    let mut mc = MovingCloud::new(7, &s, 20.0)
        .unwrap()
        .with_budget(std::time::Duration::from_secs(30));
    assert_eq!(mc.result, 7);
    // Cold start: every air cell at ambient.
    let air_max = |mc: &MovingCloud| {
        mc.snapshot()
            .temperature_c
            .iter()
            .zip(&mc.snapshot().material)
            .filter(|(_, m)| **m == 0)
            .map(|(t, _)| *t)
            .fold(f32::MIN, f32::max)
    };
    assert!((air_max(&mc) - 20.0).abs() < 1e-6);
    let mut c = MoveControls::default();
    let mut now = 0.0;
    for _ in 0..40 {
        now += 0.05;
        mc.tick(now, &mut c);
    }
    assert!(mc.time_s() > 1.0, "time {}", mc.time_s());
    assert!(air_max(&mc) > 21.0, "no warming: {}", air_max(&mc));
    // The status line names the model and its distance from the solved air.
    assert!(
        c.status.contains("cold start") && c.status.contains("K from the solved air"),
        "{}",
        c.status
    );
}

#[test]
fn a_paused_cloud_stands_still_and_restart_goes_back_to_cold_air() {
    let s = hot_cube();
    let mut mc = MovingCloud::new(1, &s, 20.0)
        .unwrap()
        .with_budget(std::time::Duration::from_secs(30));
    let mut c = MoveControls::default();
    mc.tick(0.0, &mut c);
    for i in 1..20 {
        mc.tick(f64::from(i) * 0.05, &mut c);
    }
    let t = mc.time_s();
    assert!(t > 0.0);
    c.playing = false;
    for i in 20..40 {
        mc.tick(f64::from(i) * 0.05, &mut c);
    }
    assert_eq!(mc.time_s(), t);
    c.playing = true;
    c.restart = true;
    mc.tick(2.0, &mut c);
    assert!(!c.restart);
    assert!(mc.time_s() < t);
}

#[test]
fn a_long_gap_does_not_make_the_cloud_jump() {
    let s = hot_cube();
    let mut mc = MovingCloud::new(1, &s, 20.0)
        .unwrap()
        .with_budget(std::time::Duration::from_secs(30));
    let mut c = MoveControls {
        speed: 10.0,
        ..MoveControls::default()
    };
    mc.tick(0.0, &mut c);
    // A frozen window: ten seconds pass in the real world.
    mc.tick(10.0, &mut c);
    assert!(mc.time_s() <= 1.0 + 1e-6, "{}", mc.time_s());
}

#[test]
fn the_speed_range_is_sensible() {
    assert!(*SPEED_RANGE.start() >= 1.0 && *SPEED_RANGE.end() <= 60.0);
    assert!(SPEED_RANGE.contains(&MoveControls::default().speed));
}
