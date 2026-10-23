//! Ambient motion for the cabinet views: a spinning fan and air that travels
//! through an opening. Pure functions of the clock, so they are tested without
//! a window. They are a picture of "this fan runs, the air goes this way", not
//! a measured speed: the turn rate only grows with the datasheet flow so a big
//! fan reads as a big fan, and is capped so it never aliases on a 60 Hz screen.
//!
//! A caller that wants a still picture (reduced motion, the QA harness) passes
//! `None` for the clock and gets the same fixed frame every time.

use std::f32::consts::TAU;

/// Slowest and fastest turn shown, turns per second.
pub const MIN_TURNS: f32 = 0.5;
pub const MAX_TURNS: f32 = 1.5;
/// The flow, m3/h, at which a fan turns at the top rate.
pub const FULL_FLOW_M3H: f32 = 600.0;
/// Seconds one pulse of air takes to cross an opening.
pub const AIR_SECONDS: f64 = 1.6;
/// The angle a still picture shows a fan at, radians.
pub const REST_ANGLE: f32 = 0.0;

/// Turns per second shown for a fan of `flow_m3h`.
pub fn turns_per_second(flow_m3h: f32) -> f32 {
    let share = (flow_m3h / FULL_FLOW_M3H).clamp(0.0, 1.0);
    MIN_TURNS + (MAX_TURNS - MIN_TURNS) * share
}

/// The blade angle at `time` seconds, radians in `[0, TAU)`. `None` is the
/// still picture.
pub fn fan_angle(time: Option<f64>, flow_m3h: f32) -> f32 {
    let Some(t) = time else { return REST_ANGLE };
    let turns = (t * f64::from(turns_per_second(flow_m3h))).fract() as f32;
    turns * TAU
}

/// How far along its path a pulse of air is, `0..1`, for pulse `k` of `n`
/// spread evenly. `None` for the still picture: no pulses are shown at all.
pub fn air_phase(time: Option<f64>, k: usize, n: usize) -> Option<f32> {
    let t = time?;
    let n = n.max(1);
    Some((((t / AIR_SECONDS) + k as f64 / n as f64).fract()) as f32)
}

/// A pulse fades in at the start and out at the end: opacity `0..1`.
pub fn air_fade(phase: f32) -> f32 {
    (phase * (1.0 - phase) * 4.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bigger_fan_turns_faster_up_to_a_cap() {
        assert!(turns_per_second(50.0) < turns_per_second(300.0));
        assert_eq!(turns_per_second(5000.0), MAX_TURNS);
        assert_eq!(turns_per_second(0.0), MIN_TURNS);
        assert_eq!(turns_per_second(-10.0), MIN_TURNS);
    }

    #[test]
    fn the_still_picture_never_changes() {
        assert_eq!(fan_angle(None, 100.0), fan_angle(None, 900.0));
        assert_eq!(air_phase(None, 0, 3), None);
    }

    #[test]
    fn the_blades_come_round_once_per_period() {
        let rate = f64::from(turns_per_second(300.0));
        let a = fan_angle(Some(0.3), 300.0);
        let b = fan_angle(Some(0.3 + 1.0 / rate), 300.0);
        assert!((a - b).abs() < 1e-3 || (a - b).abs() > TAU - 1e-3);
        assert!((0.0..TAU).contains(&a));
    }

    #[test]
    fn pulses_are_spread_evenly_and_fade_at_the_ends() {
        let p: Vec<f32> = (0..4)
            .map(|k| air_phase(Some(0.0), k, 4).unwrap())
            .collect();
        assert_eq!(p, vec![0.0, 0.25, 0.5, 0.75]);
        assert_eq!(air_fade(0.0), 0.0);
        assert_eq!(air_fade(1.0), 0.0);
        assert!((air_fade(0.5) - 1.0).abs() < 1e-6);
    }
}
