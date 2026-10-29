//! Design findings for a closed-loop winch: settings that work on paper and drop a load in
//! practice. Pure checks of the numbers entered (no run needed); each finding says what it
//! computed. The factors used are entered planning rules, not a standard.

use std::f64::consts::TAU;

use super::controller::{ClosedLoopConfig, PreTorque};
use crate::GRAVITY;
use crate::brake::SpringBrake;
use crate::mechanics::Hoist;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    /// A setting that lets the load move or the drive trip in normal use.
    Problem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub severity: Severity,
    pub text: String,
}

fn f(severity: Severity, text: String) -> Finding {
    Finding { severity, text }
}

/// Planning rule: a holding brake should hold at least this multiple of the static load torque.
pub const BRAKE_MARGIN: f64 = 1.5;

pub fn check_winch(cfg: &ClosedLoopConfig, hoist: &Hoist, brake: &SpringBrake) -> Vec<Finding> {
    let mut out = Vec::new();
    let t_static = hoist.static_torque_nm();
    let t_rated = cfg.rated_torque_nm();

    // Torque is removed before the brake has taken the load.
    let takes_over = brake.apply_delay_s() + brake.torque_rise_s();
    if cfg.brake_close_wait_s < takes_over {
        let gap = takes_over - cfg.brake_close_wait_s;
        let drop = 0.5 * GRAVITY * gap * gap;
        out.push(f(
            Severity::Problem,
            format!(
                "Torque is removed {gap:.2} s before the brake has full torque (hold {:.2} s, brake needs {takes_over:.2} s): the load falls about {drop:.3} m (free fall estimate).",
                cfg.brake_close_wait_s
            ),
        ));
    }
    // The drive believes the brake is open before it is.
    if !cfg.brake_feedback && cfg.release_wait_s < brake.release_delay_s() {
        out.push(f(
            Severity::Problem,
            format!(
                "The drive starts the ramp {:.2} s after the relay, the brake needs {:.2} s to open: it pulls against a closed brake. Wire the brake feedback or raise the wait.",
                cfg.release_wait_s,
                brake.release_delay_s()
            ),
        ));
    }
    if cfg.brake_feedback && cfg.feedback_timeout_s <= brake.release_delay_s() {
        out.push(f(
            Severity::Problem,
            format!(
                "Feedback timeout {:.2} s is not longer than the brake release time {:.2} s: every start trips.",
                cfg.feedback_timeout_s,
                brake.release_delay_s()
            ),
        ));
    }
    // Brake holds the load?
    if brake.rated_torque_nm() < t_static {
        out.push(f(
            Severity::Problem,
            format!(
                "Brake torque {:.0} N m is below the weight torque {t_static:.0} N m: the brake cannot hold the load.",
                brake.rated_torque_nm()
            ),
        ));
    } else if brake.rated_torque_nm() < BRAKE_MARGIN * t_static {
        out.push(f(
            Severity::Warning,
            format!(
                "Brake torque {:.0} N m is {:.2} x the weight torque {t_static:.0} N m (planning rule {BRAKE_MARGIN} x).",
                brake.rated_torque_nm(),
                brake.rated_torque_nm() / t_static
            ),
        ));
    }
    // Drive torque.
    let t_raise = hoist.raising_torque_nm();
    let t_max = cfg.torque_limit_nm(0.0);
    if t_max < t_raise {
        out.push(f(
            Severity::Problem,
            format!(
                "Drive torque limit {t_max:.0} N m is below the torque to raise the load at constant speed {t_raise:.0} N m: it cannot lift this load."
            ),
        ));
    }
    // Pre-torque.
    match cfg.pre_torque {
        PreTorque::None if t_static > 0.1 * t_rated => out.push(f(
            Severity::Warning,
            format!(
                "No pre-torque at brake release with a load of {:.0} % of rated torque: the load sags until the speed loop has wound up. Use a load cell, a fixed value or torque memory.",
                100.0 * t_static / t_rated
            ),
        )),
        PreTorque::Fixed(t) if (t - t_static).abs() > 0.2 * t_static => out.push(f(
            Severity::Warning,
            format!(
                "Fixed pre-torque {t:.0} N m differs from the weight torque {t_static:.0} N m by more than 20 %."
            ),
        )),
        _ => {}
    }
    if cfg.min_release_torque_nm > cfg.torque_limit_nm(0.0) {
        out.push(f(
            Severity::Problem,
            "Minimum release torque is above the torque limit: the brake never releases.".into(),
        ));
    }
    // Encoder resolution against creep speed.
    let counts = 4.0 * f64::from(cfg.encoder_ppr);
    let res_rpm = 60.0 / (counts * cfg.encoder_window_s);
    if res_rpm > cfg.zero_speed_rpm {
        out.push(f(
            Severity::Warning,
            format!(
                "Encoder speed resolution {res_rpm:.2} rpm (one count per {:.0} ms window) is coarser than the zero speed level {:.2} rpm: zero speed is never seen cleanly.",
                cfg.encoder_window_s * 1000.0,
                cfg.zero_speed_rpm
            ),
        ));
    }
    if !cfg.encoder_monitor {
        out.push(f(
            Severity::Warning,
            "Encoder monitoring is off: a lost encoder reads standstill and the drive can accelerate the motor without a trip.".into(),
        ));
    }
    // Regeneration when lowering.
    let w_max = cfg.motor.rpm * TAU / 60.0;
    let p_regen = hoist.lowering_torque_nm() * w_max * cfg.efficiency;
    if cfg.bus.resistor_ohm.is_none()
        && !cfg.bus.regen_to_mains
        && p_regen > 0.1 * cfg.motor.power_kw * 1000.0
    {
        out.push(f(
            Severity::Problem,
            format!(
                "Lowering this load at rated speed returns about {:.1} kW to the DC bus and there is no braking resistor or regenerative front end: the bus trips on overvoltage.",
                p_regen / 1000.0
            ),
        ));
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.severity));
    out
}
