//! The heat-up timeline: the run, the slider and the playback.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui;
use novo_cfd::{CfdSnapshot, Fidelity, TransientSummary};

use super::heat::{HeatState, Meta};
use super::heat_run::Solution;
use super::heat_run_tests::scenario;
use super::heat_time::{Event, TimeLine, default_solver, plot, spec_for_minutes};

fn meta() -> Meta {
    Meta {
        scenario_key: 1,
        layout_key: 2,
        ambient: 35.0,
        plate: [20.0, 20.0, 360.0, 360.0],
        messages: Vec::new(),
    }
}

fn dummy(id: u64) -> Arc<Solution> {
    // Not a real field; the timeline never looks inside.
    let snap = novo_cfd::StubSolver;
    let snapshot = novo_cfd::Solver::solve(
        &snap,
        &scenario(),
        Fidelity::Coarse,
        &novo_cfd::Control::new(),
        &mut |_| {},
    )
    .unwrap();
    Arc::new(Solution {
        snapshot: Arc::new(snapshot),
        id,
        scenario_key: 1,
        layout_key: 2,
        ambient: 35.0,
        plate: [20.0, 20.0, 360.0, 360.0],
        messages: Vec::new(),
    })
}

fn filled(n: usize) -> TimeLine {
    let mut t = TimeLine::new();
    let sol = dummy(1);
    for i in 0..n {
        t.push(i as f64 * 60.0, 1.0, sol.clone());
    }
    t
}

#[test]
fn a_spec_has_sixty_one_frames_and_clamps_the_length() {
    let s = spec_for_minutes(60.0);
    assert_eq!(s.frame_count(), 61);
    assert!((s.duration_s - 3600.0).abs() < 1e-9 && s.is_valid());
    assert!((spec_for_minutes(0.0).duration_s - 60.0).abs() < 1e-9);
    assert!((spec_for_minutes(1e9).duration_s - 86_400.0).abs() < 1e-9);
}

#[test]
fn the_newest_frame_is_shown_until_the_user_moves() {
    let mut t = filled(3);
    assert_eq!(t.at, 2);
    assert!(t.take_selection().is_some());
    assert!(t.take_selection().is_none(), "once per change");
    t.seek(0);
    t.push(180.0, 1.0, dummy(2));
    assert_eq!(t.at, 0, "after seek the view stays");
    assert!(t.take_selection().is_some());
}

#[test]
fn seeking_stops_the_playback_and_stays_in_range() {
    let mut t = filled(5);
    t.play(0.0);
    assert!(t.playing);
    t.seek(99);
    assert!(!t.playing);
    assert_eq!(t.at, 4);
}

#[test]
fn playing_takes_about_ten_seconds_and_stops_at_the_end() {
    let mut t = filled(11);
    t.seek(0);
    t.take_selection();
    t.play(100.0);
    t.tick(100.0);
    assert_eq!(t.at, 0, "no time has passed");
    t.tick(105.0);
    assert!(t.at >= 4 && t.at <= 6, "half way after 5 s: {}", t.at);
    t.tick(130.0);
    assert_eq!(t.at, 10);
    assert!(!t.playing);
    // Playing again from the end starts over.
    t.play(200.0);
    assert_eq!(t.at, 0);
    assert!(t.playing);
}

#[test]
fn times_read_naturally() {
    assert_eq!(TimeLine::time_label(45.0), "45 s");
    assert_eq!(TimeLine::time_label(23.0 * 60.0), "23 min");
    assert_eq!(TimeLine::time_label(3900.0), "1 h 05 min");
}

fn summary() -> TransientSummary {
    TransientSummary {
        fidelity: Fidelity::Coarse,
        times_s: vec![0.0, 100.0, 200.0],
        load_factor: vec![1.0; 3],
        mean_air_c: vec![30.0, 34.0, 36.0],
        max_air_c: vec![30.0, 40.0, 50.0],
        devices: Vec::new(),
        time_constant_s: Some(90.0),
    }
}

#[test]
fn the_plot_rises_with_the_temperature_and_marks_the_shown_time() {
    let p = plot(&summary(), 100.0, 100.0, 20.0).unwrap();
    assert_eq!(p.max.len(), 3);
    // Hotter is higher: smaller y.
    assert!(p.max[2].1 < p.max[1].1 && p.max[1].1 < p.max[0].1);
    assert!(p.mean.iter().zip(&p.max).all(|(m, x)| m.1 >= x.1 - 1e-4));
    assert!((p.cursor_x - 50.0).abs() < 1e-4);
    assert_eq!(p.max[0].0, 0.0);
    assert!((p.max[2].0 - 100.0).abs() < 1e-4);
    let mut bad = summary();
    bad.max_air_c.pop();
    assert!(plot(&bad, 0.0, 10.0, 10.0).is_none());
}

fn drain(t: &mut TimeLine) -> Vec<Event> {
    let t0 = Instant::now();
    let mut all = Vec::new();
    while t0.elapsed() < Duration::from_secs(60) {
        all.extend(t.poll());
        if matches!(all.last(), Some(Event::Finished(_) | Event::Lost)) {
            return all;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no end event");
}

#[test]
fn the_stub_publishes_frames_in_time_order_then_a_summary() {
    let mut t = TimeLine::new();
    let spec = spec_for_minutes(10.0);
    let expected = spec.frame_count();
    t.start(default_solver(), scenario(), spec, meta());
    assert!(t.running());
    let events = drain(&mut t);
    let times: Vec<f64> = events
        .iter()
        .filter_map(|e| match e {
            Event::Frame(f) => Some(f.time_s),
            _ => None,
        })
        .collect();
    assert!(
        times.len() >= 2 && times.len() <= expected,
        "{}",
        times.len()
    );
    assert!(times.windows(2).all(|w| w[1] > w[0]));
    assert!(matches!(events.last(), Some(Event::Finished(Ok(s))) if s.times_s.len() > 1));
}

#[test]
fn cancelling_a_heat_up_ends_it() {
    let mut t = TimeLine::new();
    t.start(default_solver(), scenario(), spec_for_minutes(10.0), meta());
    t.cancel();
    let events = drain(&mut t);
    assert!(matches!(events.last(), Some(Event::Finished(_))));
}

#[test]
fn a_heat_up_through_the_heat_state_shows_the_latest_frame_and_the_slider_changes_it() {
    let ctx = egui::Context::default();
    let mut state = HeatState::new();
    state.request_transient();
    assert!(state.take_request());
    state.start(scenario(), meta());
    assert!(state.running());
    let t0 = Instant::now();
    while state.running() && t0.elapsed() < Duration::from_secs(60) {
        state.poll(&ctx);
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!state.running());
    assert!(state.time.frames.len() >= 2);
    let last = state.time.frames.len() - 1;
    let shown = state.solution().unwrap().id;
    assert_eq!(shown, state.time.frames[last].solution.id);
    state.time.seek(0);
    state.poll(&ctx);
    assert_eq!(
        state.solution().unwrap().id,
        state.time.frames[0].solution.id
    );
    assert_ne!(shown, state.solution().unwrap().id);
    // A steady run replaces the timeline.
    state.start(scenario(), meta());
    assert!(state.time.is_empty());
}

#[test]
fn a_snapshot_type_is_shared_not_copied() {
    // Frames hold the same Arc the Heat view shows.
    let t = filled(2);
    assert!(Arc::ptr_eq(&t.frames[0].solution, &t.frames[1].solution));
    let _: &CfdSnapshot = &t.frames[0].solution.snapshot;
}
