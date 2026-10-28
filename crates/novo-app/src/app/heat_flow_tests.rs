//! The Flow control: scale, words, thinning, the speed map and the
//! through-flow verdict.

use glam::Vec3;
use novo_heatview::cfd::Axis;

use super::heat_3d::{Flow, FlowLine, Heat3d};
use super::heat_3d_tests::{place, snapshot};
use super::heat_flow::*;
use super::heat_settings::{Kind, VentSpec, Wall};

/// A straight line along x of `len` mm at a steady `speed`.
fn line(y: f32, len: f32, speed: f32) -> FlowLine {
    let n = 5;
    let points: Vec<Vec3> = (0..n)
        .map(|i| Vec3::new(len * i as f32 / (n - 1) as f32, y, 0.0))
        .collect();
    let along = (0..n).map(|i| len * i as f32 / (n - 1) as f32).collect();
    FlowLine {
        points,
        speed: vec![speed; n],
        along,
    }
}

fn flow_of(speeds: &[f32]) -> Flow {
    let lines: Vec<FlowLine> = speeds
        .iter()
        .enumerate()
        .map(|(i, s)| line(i as f32 * 10.0, 400.0, *s))
        .collect();
    let top = speeds.iter().copied().fold(0.0, f32::max);
    Flow { lines, top }
}

fn fan(wall: Wall, exhaust: bool) -> VentSpec {
    VentSpec {
        kind: Kind::Fan,
        wall,
        exhaust,
        ..VentSpec::new(wall)
    }
}

#[test]
fn the_words_follow_the_speed() {
    assert_eq!(speed_word(0.0), "still");
    assert_eq!(speed_word(0.1), "gentle");
    assert_eq!(speed_word(0.3), "moderate");
    assert_eq!(speed_word(1.0), "brisk");
    assert_eq!(speed_word(4.0), "fast");
}

#[test]
fn the_scale_ends_on_a_round_number_above_the_air() {
    assert_eq!(nice_top(0.0), 0.1);
    assert_eq!(nice_top(0.12), 0.2);
    assert_eq!(nice_top(0.5), 0.5);
    assert_eq!(nice_top(0.51), 1.0);
    assert_eq!(nice_top(7.0), 10.0);
    assert_eq!(nice_top(f32::NAN), 0.1);
    // The test snapshot has a 1 m/s wind everywhere.
    assert_eq!(speed_scale(&snapshot()), 1.0);
}

#[test]
fn one_jet_does_not_flatten_the_scale_of_the_rest() {
    let mut snap = snapshot();
    snap.velocity.iter_mut().for_each(|v| *v = [0.2, 0.0, 0.0]);
    // Two of the 3840 cells (about 0.05 %) are a fan jet.
    for v in snap.velocity.iter_mut().take(2) {
        *v = [8.0, 0.0, 0.0];
    }
    assert_eq!(speed_scale(&snap), 0.2);
}

#[test]
fn colour_is_the_speed_on_the_shared_scale() {
    // The same speed on the same scale is the same colour in every mode, and
    // a quarter of the scale is not the end colour.
    assert_eq!(colour(0.5, 1.0), colour(0.25, 0.5));
    assert_ne!(colour(0.25, 1.0), colour(1.0, 1.0));
}

#[test]
fn less_strength_keeps_the_main_streams_and_never_fewer_than_six() {
    let speeds: Vec<f32> = (1..=30).map(|i| i as f32 / 30.0).collect();
    let flow = flow_of(&speeds);
    let few = thinned(&flow, 0.0, 1.0);
    let many = thinned(&flow, 1.0, 1.0);
    assert_eq!(many.lines.len(), 30);
    assert!(few.lines.len() < many.lines.len() / 2 && few.lines.len() >= 6);
    // What stays is the fastest: nothing slower than the slowest kept line is
    // missing from a faster rank.
    let slowest_kept = few
        .lines
        .iter()
        .map(|l| l.speed[0])
        .fold(f32::MAX, f32::min);
    let dropped_faster = flow
        .lines
        .iter()
        .filter(|l| l.speed[0] > slowest_kept)
        .filter(|l| !few.lines.iter().any(|k| k.points == l.points))
        .count();
    assert_eq!(dropped_faster, 0);
    assert!((few.top - 1.0).abs() < 1e-6);
    let tiny = thinned(&flow_of(&[0.9, 0.8, 0.7]), 0.0, 1.0);
    assert_eq!(tiny.lines.len(), 3);
}

#[test]
fn nearly_still_air_is_left_out() {
    let flow = flow_of(&[0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.01, 0.01]);
    let kept = thinned(&flow, 1.0, 1.0);
    assert_eq!(kept.lines.len(), 6);
    assert!(Flow::default().lines.is_empty());
    assert!(thinned(&Flow::default(), 0.5, 1.0).lines.is_empty());
}

#[test]
fn faster_air_is_drawn_thicker() {
    let flow = flow_of(&[0.05, 0.3, 0.9, 0.9, 0.9, 0.9, 0.9, 0.9]);
    let view = FlowView {
        mode: FlowMode::Streamlines,
        strength: 1.0,
    };
    let widths: Vec<f32> = stream_overlays(&flow, &view, 1.0, 2)
        .into_iter()
        .map(|o| match o {
            novo_view3d::Overlay::Lines(l) => l.width_px,
            _ => panic!("lines only"),
        })
        .collect();
    assert!(widths.len() >= 2, "{widths:?}");
    assert!(widths.windows(2).all(|w| w[0] < w[1]), "{widths:?}");
}

#[test]
fn arrowheads_point_downwind_and_thin_out_with_strength() {
    let flow = flow_of(&[0.8; 8]);
    let count = |strength: f32| {
        let view = FlowView {
            mode: FlowMode::Streamlines,
            strength,
        };
        stream_overlays(&flow, &view, 1.0, 2)
            .into_iter()
            .map(|o| match o {
                novo_view3d::Overlay::Lines(l) => l.starts.len(),
                _ => 0,
            })
            .sum::<usize>()
    };
    assert!(count(1.0) > count(0.0));
}

#[test]
fn the_speed_map_colours_by_speed_and_skips_devices() {
    let snap = snapshot();
    let m = speed_map(&snap, place(), Axis::Z, 0.8, 1.0, 190).expect("a mesh");
    // 1 m/s on a 1 m/s scale is the end colour everywhere.
    let end = colour(1.0, 1.0);
    assert!(m.colors.iter().all(|c| c[..3] == end && c[3] == 190));
    // A quarter of the scale gives another colour.
    let slow = speed_map(&snap, place(), Axis::Z, 0.8, 4.0, 190).unwrap();
    assert_ne!(slow.colors[0][..3], end);
    // The cube at the origin is left out of a plane that cuts it.
    let cut = speed_map(&snap, place(), Axis::Z, 0.17, 1.0, 190).unwrap();
    assert!(cut.positions.len() < m.positions.len());
}

#[test]
fn the_mode_sets_the_switches_that_draw_it() {
    let mut h = Heat3d::default();
    assert_eq!(h.flow.mode, FlowMode::Off);
    h.set_flow(FlowMode::Streamlines);
    assert!(h.lines && !h.arrows);
    h.set_flow(FlowMode::Arrows);
    assert!(!h.lines && h.arrows);
    h.set_flow(FlowMode::SpeedMap);
    assert!(!h.lines && !h.arrows);
    h.set_flow(FlowMode::Off);
    assert!(!h.lines && !h.arrows);
}

#[test]
fn the_speed_map_replaces_the_temperature_slice_in_the_overlays() {
    let snap = snapshot();
    let flow = Flow::default();
    let mut h = Heat3d::default();
    let temp = super::heat_3d::overlays(&snap, 20.0, place(), &h, &flow, 1.0);
    h.set_flow(FlowMode::SpeedMap);
    let map = super::heat_3d::overlays(&snap, 20.0, place(), &h, &flow, 1.0);
    assert_eq!(temp.len(), 1);
    assert_eq!(map.len(), 1);
    assert_ne!(temp, map);
}

#[test]
fn enough_air_is_adequate_and_says_how_much_the_air_warms() {
    // 600 m3/h through for 300 W: 3.1 * 300 / 600 = 1.55 K.
    let specs = [fan(Wall::Left, false), fan(Wall::Right, true)];
    let t = through(&specs, &[Some(600.0), Some(-600.0)], 300.0, Some(240.0)).unwrap();
    assert_eq!(t.verdict, Verdict::Adequate);
    assert!((t.warming_k.unwrap() - 1.55).abs() < 0.01);
    assert!((t.total_m3h() - 600.0).abs() < 1e-9);
    let text = t.text();
    assert!(text.contains("600 m³/h") && text.contains("300 W") && text.contains("adequate"));
    assert!(text.contains("carries out about 240 W (80 %)"), "{text}");
    assert!(t.hover().contains("Fan") && t.hover().contains("not a standard"));
}

#[test]
fn less_air_is_low_then_very_low() {
    let specs = [fan(Wall::Left, false), fan(Wall::Right, true)];
    // 3.1 * 500 / 200 = 7.75 K.
    let low = through(&specs, &[Some(200.0), Some(-200.0)], 500.0, None).unwrap();
    assert_eq!(low.verdict, Verdict::Low);
    // 3.1 * 500 / 60 = 25.8 K.
    let very = through(&specs, &[Some(60.0), Some(-60.0)], 500.0, None).unwrap();
    assert_eq!(very.verdict, Verdict::VeryLow);
    assert!(very.text().contains("very low"));
}

#[test]
fn openings_that_move_no_air_say_so() {
    let specs = [fan(Wall::Left, false)];
    let t = through(&specs, &[Some(0.0)], 200.0, None).unwrap();
    assert_eq!(t.verdict, Verdict::None);
    assert!(t.warming_k.is_none());
    assert!(t.text().contains("mostly through the walls") && t.text().contains("(model)"));
}

#[test]
fn no_openings_or_no_reading_gives_no_summary() {
    assert!(through(&[], &[], 100.0, None).is_none());
    assert!(through(&[fan(Wall::Left, false)], &[None], 100.0, None).is_none());
}

#[test]
fn the_note_names_the_mode_and_the_scale() {
    let view = FlowView {
        mode: FlowMode::SpeedMap,
        strength: 0.4,
    };
    let n = note(&view, 0.5).unwrap();
    assert!(n.contains("0.5 m/s") && n.contains("model"));
    assert!(note(&FlowView::default(), 0.5).is_none());
}

#[test]
fn strength_sets_how_many_lines_the_flat_view_keeps() {
    assert_eq!(kept_count(100, 0.25), 40);
    assert_eq!(kept_count(100, 1.0), 100);
    assert_eq!(kept_count(100, 0.0), 20);
    assert_eq!(kept_count(10, 0.0), 6);
    assert_eq!(kept_count(3, 0.5), 3);
    assert_eq!(kept_count(0, 0.5), 0);
}

#[test]
fn the_scale_of_plain_speeds_is_the_same_rule() {
    assert_eq!(speed_scale_of(&[]), 0.1);
    assert_eq!(speed_scale_of(&[0.3; 50]), 0.5);
    assert_eq!(speed_scale_of(&[f32::NAN, 0.05]), 0.1);
}
