//! Hand-calculated checks for lane L2c (closed-loop encoder hoist drive). Every expected number
//! was worked out on paper first; the formula is in the comment.

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::closed_loop::{
    ClFault, ClInputs, ClState, ClosedLoopConfig, ClosedLoopWinch, Encoder, EncoderHealth,
    PreTorque, Severity, StoMonitor, WinchSnapshot, check_winch,
};
use novo_drive::drive::MotorNameplate;
use novo_drive::mechanics::Hoist;

const TAU: f64 = std::f64::consts::TAU;
const G: f64 = novo_drive::GRAVITY;
const DT: f64 = 0.001;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b} (tolerance {tol})");
}

/// 30 kW, 1480 rpm, 400 V, 56 A motor. Rated torque = 30000 / (1480 * 2 pi / 60) = 193.6 N m.
fn motor() -> MotorNameplate {
    MotorNameplate {
        voltage_v: 400.0,
        current_a: 56.0,
        frequency_hz: 50.0,
        rpm: 1480.0,
        power_kw: 30.0,
        power_factor: 0.85,
    }
}

const MASS: f64 = 2500.0;
const DRUM_R: f64 = 0.2;
const RATIO: f64 = 31.5;
const ETA: f64 = 0.9;
const ROTOR_J: f64 = 0.5;

/// Weight torque on the shaft: m g r / i = 2500 * 9.80665 * 0.2 / 31.5 = 155.67 N m.
fn t_static() -> f64 {
    MASS * G * DRUM_R / RATIO
}

fn mpr() -> f64 {
    DRUM_R / RATIO
}

fn cfg() -> ClosedLoopConfig {
    let mut c = ClosedLoopConfig::generic_hoist(motor());
    c.pre_torque = PreTorque::Fixed(t_static());
    c.brake_close_wait_s = 0.5;
    c
}

fn brake(apply_delay: f64, rise: f64) -> SpringBrake {
    SpringBrake::new(
        400.0,
        0.15,
        apply_delay,
        apply_delay,
        rise,
        Switching::DcSide,
    )
    .unwrap()
}

fn winch_with(c: ClosedLoopConfig, b: SpringBrake) -> ClosedLoopWinch {
    let hoist = Hoist::new(MASS, DRUM_R, RATIO, 1, ETA).unwrap();
    ClosedLoopWinch::new(c, hoist, b, ROTOR_J).unwrap()
}

fn winch() -> ClosedLoopWinch {
    winch_with(cfg(), brake(0.2, 0.0))
}

fn run_input(rpm: f64) -> ClInputs {
    ClInputs {
        run: true,
        speed_ref_rpm: rpm,
        ..ClInputs::default()
    }
}

/// Runs `secs` of simulated time, feeding the brake feedback from the real brake when wired.
fn run(w: &mut ClosedLoopWinch, inp: ClInputs, secs: f64) -> Vec<WinchSnapshot> {
    let n = (secs / DT).round() as usize;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        let mut i = inp;
        if w.drive.cfg.brake_feedback && i.brake_feedback.is_none() {
            i.brake_feedback = Some(w.brake().holding_torque_nm() == 0.0);
        }
        v.push(w.step(&i, DT));
    }
    v
}

// ---- 1. encoder arithmetic --------------------------------------------------------------

#[test]
fn encoder_counts_and_resolution() {
    // 1024 ppr TTL, x4 quadrature = 4096 counts per revolution; one count per 5 ms window is
    // 60 / (4096 * 0.005) = 2.93 rpm.
    let mut e = Encoder::new(1024, 0.005).unwrap();
    close(e.counts_per_rev(), 4096.0, 0.0);
    close(e.speed_resolution_rpm(), 2.9297, 1e-3);
    e.update(TAU, 0.005);
    assert_eq!(e.counts(), 4096);
    close(e.position_rad(), TAU, 1e-9);
}

#[test]
fn encoder_speed_follows_a_constant_turn() {
    // 10 rad/s for 1 s: measured speed within one count per window (0.307 rad/s).
    let mut e = Encoder::new(1024, 0.005).unwrap();
    let mut angle = 0.0;
    let mut w = 0.0;
    for _ in 0..1000 {
        angle += 10.0 * DT;
        w = e.update(angle, DT);
    }
    close(w, 10.0, 4.0 * TAU / 4096.0 / 0.005 / 4.0 + 0.01);
}

#[test]
fn encoder_failures() {
    let mut e = Encoder::new(1024, 0.005).unwrap();
    e.health = EncoderHealth::SignalLost;
    e.update(5.0, 0.005);
    assert_eq!(e.counts(), 0);
    let mut e = Encoder::new(1024, 0.005).unwrap();
    e.health = EncoderHealth::DirectionLost;
    e.update(0.0, 0.005);
    e.update(-1.0, 0.005);
    assert!(e.counts() > 0, "reverse turn counts upward");
    assert!(Encoder::new(0, 0.005).is_err());
}

// ---- 2. safe torque off monitor ----------------------------------------------------------

#[test]
fn sto_one_low_channel_removes_torque_and_discrepancy_latches() {
    let mut m = StoMonitor::new(1.0);
    assert!(m.step(true, true, 0.1).torque_allowed);
    let s = m.step(true, false, 0.1);
    assert!(!s.torque_allowed && !s.requested && !s.discrepancy);
    // 1.0 s limit: after 1.1 s of mismatch the fault latches.
    let mut s = s;
    for _ in 0..11 {
        s = m.step(true, false, 0.1);
    }
    assert!(s.discrepancy);
    // Back to both high is not enough; a full cycle (both low, then both high) clears it.
    assert!(m.step(true, true, 0.1).discrepancy);
    assert!(m.step(false, false, 0.1).requested);
    let s = m.step(true, true, 0.1);
    assert!(!s.discrepancy && s.torque_allowed);
}

#[test]
fn sto_request_then_release_then_mismatch_stays_latched_until_a_new_cycle() {
    let mut m = StoMonitor::new(1.0);
    assert!(m.step(false, false, 0.1).requested);
    assert!(m.step(true, true, 0.1).torque_allowed);
    for _ in 0..30 {
        m.step(true, false, 0.1);
    }
    // Both high again without a fresh both-low cycle: the fault stays.
    let s = m.step(true, true, 0.1);
    assert!(s.discrepancy && !s.torque_allowed);
    m.step(false, false, 0.1);
    assert!(m.step(true, true, 0.1).torque_allowed);
}

// ---- 3. hoist arithmetic and a holding brake ---------------------------------------------

#[test]
fn weight_torque_and_closed_brake_hold() {
    close(t_static(), 155.67, 0.01);
    let mut w = winch();
    let v = run(&mut w, ClInputs::default(), 3.0);
    let last = v.last().unwrap();
    assert_eq!(last.drive.state, ClState::Off);
    assert!(last.hook_height_m.abs() < 1e-9, "brake holds the load");
}

// ---- 4. the sequence: torque first, brake second -----------------------------------------

#[test]
fn brake_releases_only_after_torque_is_proven_and_load_does_not_sag() {
    let mut w = winch();
    let v = run(&mut w, run_input(0.0), 2.0);
    let first = v.iter().position(|s| s.drive.brake_release_cmd).unwrap();
    // The relay comes after the flux time (0.3 s) and the torque loop (4 ms lag).
    assert!(first as f64 * DT >= 0.3);
    let s = &v[first];
    assert!(s.drive.torque_proven);
    assert!(
        s.drive.torque_nm >= 0.9 * t_static() - 1e-6,
        "torque {} at release",
        s.drive.torque_nm
    );
    // Pre-torque equals the weight: no sag at all after the brake lets go.
    let lowest = v.iter().map(|s| s.hook_height_m).fold(f64::MAX, f64::min);
    assert!(lowest > -0.001, "sag {lowest} m");
    assert_eq!(v.last().unwrap().drive.state, ClState::Running);
}

#[test]
fn without_pre_torque_a_proportional_only_loop_creeps_at_weight_over_kp() {
    // No pre-torque, no integral: the drive builds only the minimum release torque (0.1 x rated
    // = 19.4 N m). After the brake opens the load settles where
    // 19.4 + kp * |w| = T_static * eta (the lowering losses take the rest):
    // |w| = (155.67 * 0.9 - 19.4) / kp.
    let mut c = cfg();
    c.pre_torque = PreTorque::None;
    c.ki = 0.0;
    c.speed_deviation_rpm = 1e6;
    c.rollback_limit_rad = 1e6;
    let kp = c.kp;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let v = run(&mut w, run_input(0.0), 4.0);
    let last = v.last().unwrap();
    let expect_rad_s = -(t_static() * ETA - 0.1 * 193.6) / kp;
    let got = last.shaft_rpm * TAU / 60.0;
    close(got, expect_rad_s, 0.08 * expect_rad_s.abs());
}

#[test]
fn pre_torque_from_a_load_cell_matches_a_fixed_value() {
    let mut c = cfg();
    c.pre_torque = PreTorque::LoadCell;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let v = run(&mut w, run_input(0.0), 2.0);
    let lowest = v.iter().map(|s| s.hook_height_m).fold(f64::MAX, f64::min);
    assert!(lowest > -0.001, "sag {lowest} m");
}

#[test]
fn integrator_after_release_removes_the_sag_of_a_weak_pre_torque() {
    // Half the needed pre-torque: sag, then the integral catches the load and holds it.
    let mut c = cfg();
    c.pre_torque = PreTorque::Fixed(0.5 * t_static());
    c.rollback_limit_rad = 1e6;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let v = run(&mut w, run_input(0.0), 6.0);
    let lowest = v.iter().map(|s| s.hook_height_m).fold(f64::MAX, f64::min);
    assert!(lowest < -0.0005, "some sag expected, got {lowest}");
    let last = v.last().unwrap();
    // It stops sinking inside the stiction band [T_s eta, T_s / eta] = [140.1, 173.0] N m.
    let t = last.drive.torque_nm;
    assert!(
        t >= t_static() * ETA - 1.0 && t <= t_static() / ETA + 1.0,
        "{t}"
    );
    assert!(last.shaft_rpm.abs() < 3.0);
}

#[test]
fn zero_speed_torque_control_holds_the_load_with_the_brake_open() {
    let mut w = winch();
    let v = run(&mut w, run_input(0.0), 4.0);
    let a = v[v.len() - 2000].hook_height_m;
    let b = v.last().unwrap().hook_height_m;
    assert!((a - b).abs() < 0.001, "drift {}", (a - b).abs());
    close(
        v.last().unwrap().drive.torque_nm,
        t_static(),
        0.02 * t_static(),
    );
    assert!(v.last().unwrap().brake_torque_nm == 0.0, "brake open");
}

// ---- 5. speed reference, ramp, feedback --------------------------------------------------

#[test]
fn ramp_arithmetic_and_speed_tracking() {
    // 740 rpm in 2 s (accel 370 rpm/s): half way at 1 s after the ramp starts.
    let mut c = cfg();
    c.accel_rpm_s = 370.0;
    c.decel_rpm_s = 370.0;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let v = run(&mut w, run_input(740.0), 6.0);
    let start = v
        .iter()
        .position(|s| s.drive.state == ClState::Running)
        .unwrap();
    let at_1s = &v[start + 1000];
    close(at_1s.drive.ref_rpm, 370.0, 2.0);
    let last = v.last().unwrap();
    close(last.drive.ref_rpm, 740.0, 1e-9);
    close(last.shaft_rpm, 740.0, 6.0);
    // Hook speed = w * r / (falls * i) = 740 * 2 pi / 60 * 0.2 / 31.5 = 0.492 m/s.
    close(last.hook_speed_m_s, 740.0 * TAU / 60.0 * mpr(), 0.005);
}

#[test]
fn jerk_limited_ramp_has_no_overshoot_and_bounded_jerk() {
    let mut c = cfg();
    c.jerk_rpm_s2 = 1000.0;
    c.accel_rpm_s = 400.0;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let v = run(&mut w, run_input(600.0), 5.0);
    let refs: Vec<f64> = v.iter().map(|s| s.drive.ref_rpm).collect();
    assert!(refs.iter().all(|r| *r <= 600.0 + 1e-6), "overshoot");
    let acc: Vec<f64> = refs.windows(2).map(|p| (p[1] - p[0]) / DT).collect();
    assert!(acc.iter().all(|a| a.abs() <= 400.0 * 1.02));
    let jerk = acc
        .windows(2)
        .map(|p| ((p[1] - p[0]) / DT).abs())
        .fold(0.0, f64::max);
    assert!(jerk <= 1000.0 * 1.05, "jerk {jerk}");
    close(*refs.last().unwrap(), 600.0, 1e-3);
}

// ---- 6. stopping: zero-speed hold, brake closes under torque ------------------------------

#[test]
fn stop_holds_zero_speed_until_the_brake_has_the_load() {
    let mut w = winch();
    run(&mut w, run_input(600.0), 4.0);
    let v = run(&mut w, ClInputs::default(), 6.0);
    let h_stop = v.first().unwrap().hook_height_m;
    let last = v.last().unwrap();
    assert_eq!(last.drive.state, ClState::Off);
    // Brake holds: the hook moved at most the ramp-down distance plus a creep of a millimetre.
    let closing = v
        .iter()
        .position(|s| s.drive.state == ClState::BrakeClosing)
        .unwrap();
    let h_close = v[closing].hook_height_m;
    assert!((last.hook_height_m - h_close).abs() < 0.002);
    assert!(last.hook_height_m > h_stop);
    // Torque memory: the torque held when the brake closed is inside the stiction band of the
    // drivetrain, [T_s eta, T_s / eta] = [140.1, 173.0] N m (it stops from above).
    let m = w.drive.memory_torque_nm();
    assert!(
        m >= t_static() * ETA - 1.0 && m <= t_static() / ETA + 1.0,
        "{m}"
    );
}

#[test]
fn torque_memory_removes_the_sag_of_the_next_start() {
    let mut c = cfg();
    c.pre_torque = PreTorque::Memory;
    c.rollback_limit_rad = 1e6;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let first = run(&mut w, run_input(0.0), 2.0);
    let sag1 = first
        .iter()
        .map(|s| s.hook_height_m)
        .fold(f64::MAX, f64::min);
    run(&mut w, ClInputs::default(), 3.0);
    let base = w.shaft().angle_rad * mpr();
    let second = run(&mut w, run_input(0.0), 2.0);
    let sag2 = second
        .iter()
        .map(|s| s.hook_height_m - base)
        .fold(f64::MAX, f64::min);
    assert!(sag1 < -0.0005, "first start has no memory yet: {sag1}");
    assert!(sag2 > sag1 && sag2 > -0.0005, "second start sag {sag2}");
}

// ---- 7. the classic drop: torque removed before the brake holds ---------------------------

#[test]
fn torque_removed_before_the_brake_applies_drops_the_load() {
    // Brake apply delay 0.25 s, drive holds only 0.1 s after the relay opens: the load runs
    // free for gap = 0.15 s. Shaft acceleration a = T_s eta / J with J = J_rotor + m mpr^2;
    // angle = a gap^2 / 2, then the brake stops it with a_dec = (T_brake - T_s eta) / J:
    // angle += v^2 / (2 a_dec), v = a gap.
    let mut c = cfg();
    c.brake_close_wait_s = 0.1;
    let mut w = winch_with(c.clone(), brake(0.25, 0.0));
    let v = run(&mut w, run_input(0.0), 2.0);
    let _ = v;
    let before = w.shaft().angle_rad;
    run(&mut w, ClInputs::default(), 3.0);
    let moved = (w.shaft().angle_rad - before) * mpr();
    let j = ROTOR_J + MASS * mpr() * mpr();
    let a = t_static() * ETA / j;
    let gap = 0.15;
    let v_end = a * gap;
    let a_dec = (400.0 - t_static() * ETA) / j;
    let expect = (0.5 * a * gap * gap + v_end * v_end / (2.0 * a_dec)) * mpr();
    // The load moves down (negative) and the hold phase before gives a sub-millimetre creep.
    close(-moved, expect, 0.15 * expect + 0.0015);
    // The design check names this setting.
    let f = check_winch(
        &c,
        &Hoist::new(MASS, DRUM_R, RATIO, 1, ETA).unwrap(),
        &brake(0.25, 0.0),
    );
    assert!(
        f.iter()
            .any(|x| x.severity == Severity::Problem && x.text.contains("falls about")),
        "{f:?}"
    );
}

// ---- 8. safe torque off ------------------------------------------------------------------

#[test]
fn sto_removes_torque_at_once_and_the_brake_stops_the_load() {
    let mut w = winch();
    run(&mut w, run_input(600.0), 4.0);
    let mut inp = run_input(600.0);
    inp.sto = (false, false);
    let v = run(&mut w, inp, 3.0);
    assert_eq!(v[0].drive.state, ClState::StoActive);
    assert_eq!(v[0].drive.torque_nm, 0.0);
    assert!(!v[0].drive.brake_release_cmd);
    assert!(v[0].drive.sto_requested);
    let last = v.last().unwrap();
    assert!(last.shaft_rpm.abs() < 1e-6, "brake stopped the load");
    // After the request ends and the run is removed the drive is ready again.
    run(&mut w, ClInputs::default(), 1.0);
    let s = run(&mut w, ClInputs::default(), 0.1);
    assert_eq!(s.last().unwrap().drive.state, ClState::Off);
}

#[test]
fn sto_channel_mismatch_trips_after_the_limit() {
    let mut w = winch();
    run(&mut w, run_input(300.0), 3.0);
    let mut inp = run_input(300.0);
    inp.sto = (true, false);
    let v = run(&mut w, inp, 1.5);
    assert_eq!(v[100].drive.state, ClState::StoActive, "torque off at once");
    assert_eq!(v.last().unwrap().drive.fault, Some(ClFault::StoDiscrepancy));
}

// ---- 9. torque proof timeout ----------------------------------------------------------------

#[test]
fn a_load_beyond_the_drive_never_gets_its_brake_released() {
    // The pre-torque asks for more than the torque limit (1.5 x 56 A is about 240 N m).
    let mut c = cfg();
    c.pre_torque = PreTorque::Fixed(400.0);
    let mut w = winch_with(c.clone(), brake(0.2, 0.0));
    let v = run(&mut w, run_input(600.0), 3.0);
    let last = v.last().unwrap();
    assert_eq!(last.drive.fault, Some(ClFault::TorqueProofTimeout));
    assert!(v.iter().all(|s| !s.drive.brake_release_cmd));
    assert!(last.hook_height_m.abs() < 1e-9);
}

// ---- 10. encoder trouble ---------------------------------------------------------------------

#[test]
fn encoder_signal_lost_while_running_trips_on_the_plausibility_check() {
    let mut w = winch();
    run(&mut w, run_input(700.0), 3.0);
    w.drive.encoder.health = EncoderHealth::SignalLost;
    let v = run(&mut w, run_input(700.0), 3.0);
    assert_eq!(
        v.last().unwrap().drive.fault,
        Some(ClFault::EncoderFeedback)
    );
    // The tolerance is 148 rpm and the time 0.1 s: it trips fast.
    let t = v.iter().position(|s| s.drive.fault.is_some()).unwrap() as f64 * DT;
    assert!(t < 0.8, "tripped after {t}");
}

#[test]
fn unmonitored_encoder_loss_at_low_speed_runs_away() {
    let mut c = cfg();
    c.encoder_monitor = false;
    let mut w = winch_with(c, brake(0.2, 0.0));
    run(&mut w, run_input(100.0), 3.0);
    w.drive.encoder.health = EncoderHealth::SignalLost;
    let v = run(&mut w, run_input(100.0), 6.0);
    let last = v.last().unwrap();
    assert!(last.drive.fault.is_none(), "{:?}", last.drive.fault);
    assert!(last.shaft_rpm > 1.2 * 1480.0, "{} rpm", last.shaft_rpm);
}

#[test]
fn lost_direction_channel_is_caught_when_lowering() {
    let mut w = winch();
    w.drive.encoder.health = EncoderHealth::DirectionLost;
    let up = run(&mut w, run_input(500.0), 4.0);
    assert!(
        up.last().unwrap().drive.fault.is_none(),
        "raising reads right"
    );
    let down = run(&mut w, run_input(-500.0), 6.0);
    assert_eq!(
        down.last().unwrap().drive.fault,
        Some(ClFault::EncoderFeedback)
    );
}

// ---- 11. brake feedback ---------------------------------------------------------------------

#[test]
fn brake_feedback_confirms_the_release_and_a_stuck_brake_trips() {
    let mut c = cfg();
    c.brake_feedback = true;
    let mut w = winch_with(c.clone(), brake(0.2, 0.0));
    let v = run(&mut w, run_input(300.0), 3.0);
    assert!(v.last().unwrap().drive.fault.is_none());
    assert_eq!(v.last().unwrap().drive.state, ClState::Running);
    // The brake never opens: feedback says closed.
    let mut w = winch_with(c, brake(0.2, 0.0));
    let mut inp = run_input(300.0);
    inp.brake_feedback = Some(false);
    let v = run(&mut w, inp, 3.0);
    assert_eq!(v.last().unwrap().drive.fault, Some(ClFault::BrakeFeedback));
}

#[test]
fn a_brake_coil_that_the_circuit_does_not_feed_leaves_the_brake_closed() {
    // The drive commands the release (relay on) but the drawing's brake contactor never feeds
    // the coil. With feedback wired the drive trips; without it, the shaft stays at rest.
    let mut c = cfg();
    c.brake_feedback = true;
    let mut w = winch_with(c, brake(0.2, 0.0));
    let mut inp = run_input(300.0);
    inp.brake_coil = Some(false);
    inp.brake_feedback = Some(false);
    let v = run(&mut w, inp, 3.0);
    assert_eq!(v.last().unwrap().drive.fault, Some(ClFault::BrakeFeedback));
    assert!(v.iter().all(|s| s.hook_height_m.abs() < 0.001));
    // A coil fed from the start (a welded brake contactor) opens the brake before the drive has
    // built its torque: the load runs down, whatever the relay says.
    let mut w = winch();
    let mut inp = run_input(300.0);
    inp.brake_coil = Some(true);
    let v = run(&mut w, inp, 3.0);
    assert!(v.iter().any(|s| s.hook_speed_m_s < -0.01));
}

// ---- 12. regeneration: lowering a load ---------------------------------------------------------

#[test]
fn lowering_without_a_braking_resistor_trips_the_bus() {
    let mut w = winch();
    let v = run(&mut w, run_input(-1000.0), 6.0);
    assert_eq!(v.last().unwrap().drive.fault, Some(ClFault::BusOverVoltage));
    let f = check_winch(
        &w.drive.cfg,
        &Hoist::new(MASS, DRUM_R, RATIO, 1, ETA).unwrap(),
        &brake(0.2, 0.0),
    );
    assert!(f.iter().any(|x| x.text.contains("overvoltage")));
}

#[test]
fn a_braking_resistor_takes_the_energy_of_lowering() {
    // Lowering power: T_s eta w at 1000 rpm = 155.67 * 0.9 * 104.7 = 14.7 kW at the shaft.
    // 780 V across 40 ohm dissipates 15.2 kW.
    let mut c = cfg();
    c.bus.resistor_ohm = Some(40.0);
    let mut w = winch_with(c, brake(0.2, 0.0));
    let v = run(&mut w, run_input(-1000.0), 8.0);
    let last = v.last().unwrap();
    assert!(last.drive.fault.is_none(), "{:?}", last.drive.fault);
    assert!(v.iter().any(|s| s.drive.chopper_on));
    assert!(v.iter().all(|s| s.drive.bus_v < 820.0));
    close(last.shaft_rpm, -1000.0, 8.0);
    // The bus energy balance closes by construction.
    let e0 = 0.5 * 1.0e-3 * (400.0_f64 * 2.0_f64.sqrt()).powi(2);
    assert!(w.drive.bus.balance_residual_j(e0).abs() < 1e-3);
}

// ---- 13. findings ---------------------------------------------------------------------------------

#[test]
fn findings_name_the_dangerous_settings() {
    let h = Hoist::new(MASS, DRUM_R, RATIO, 1, ETA).unwrap();
    let weak = SpringBrake::new(120.0, 0.15, 0.2, 0.2, 0.0, Switching::DcSide).unwrap();
    let mut c = cfg();
    c.pre_torque = PreTorque::None;
    c.release_wait_s = 0.05;
    c.encoder_monitor = false;
    let f = check_winch(&c, &h, &weak);
    let text = f
        .iter()
        .map(|x| x.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("cannot hold the load"), "{text}");
    assert!(text.contains("closed brake"), "{text}");
    assert!(text.contains("No pre-torque"), "{text}");
    assert!(text.contains("monitoring is off"), "{text}");
    assert_eq!(f[0].severity, Severity::Problem, "worst first");
    // A sound setting has no problems.
    let mut ok = cfg();
    ok.bus.resistor_ohm = Some(40.0);
    let f = check_winch(&ok, &h, &brake(0.2, 0.0));
    assert!(f.iter().all(|x| x.severity != Severity::Problem), "{f:?}");
}

/// The winch with a 40 ohm braking resistor, as lowering a load needs one.
fn winch_resistor() -> ClosedLoopWinch {
    let mut c = cfg();
    c.bus.resistor_ohm = Some(40.0);
    winch_with(c, brake(0.2, 0.0))
}

// ---- 9. the hook on the ground: slack rope and the pick-up --------------------------------

/// Ground 0.3 m below the start. Lowering at 225.6 rpm (0.15 m/s of hook): the hook lands, the
/// drum keeps paying out rope. The rope paid out is the integral of the hook speed (rope speed =
/// omega x metres per rad); the slack is that minus the 0.3 m the hook could follow. Turning
/// back, the drum first takes the slack up and the hook leaves the ground at the same speed the
/// drum has then: once.
#[test]
fn a_hook_on_the_ground_leaves_the_rope_slack_and_is_picked_up_once() {
    let mut w = winch_resistor();
    w.ground_m = Some(-0.3);
    let down = run(&mut w, run_input(-225.6), 5.0);
    let landed: Vec<&WinchSnapshot> = down.iter().filter(|s| s.landed_m_s.is_some()).collect();
    assert_eq!(landed.len(), 1, "the hook lands once");
    close(landed[0].landed_m_s.unwrap(), 0.15, 0.02);
    let last = down.last().unwrap();
    assert!(last.resting);
    close(last.hook_height_m, -0.3, 1e-9);
    let paid_out: f64 = down.iter().map(|s| s.hook_speed_m_s * DT).sum();
    close(last.slack_m, -0.3 - paid_out, 2e-3);
    assert!(last.slack_m > 0.2, "{}", last.slack_m);
    // Nothing hangs on the rope, so the motor torque is the friction only, far below the weight.
    assert!(
        last.drive.torque_nm < 0.5 * t_static(),
        "{}",
        last.drive.torque_nm
    );

    let up = run(&mut w, run_input(225.6), 8.0);
    let picked: Vec<&WinchSnapshot> = up.iter().filter(|s| s.pickup_m_s.is_some()).collect();
    assert_eq!(picked.len(), 1, "the rope comes taut once");
    close(picked[0].pickup_m_s.unwrap(), 0.15, 0.03);
    let end = up.last().unwrap();
    assert!(!end.resting);
    close(end.slack_m, 0.0, 1e-9);
    assert!(end.hook_height_m > -0.3 + 0.05, "{}", end.hook_height_m);
}

#[test]
fn without_a_ground_there_is_never_slack() {
    let mut w = winch_resistor();
    let v = run(&mut w, run_input(-225.6), 4.0);
    assert!(
        v.iter()
            .all(|s| !s.resting && s.slack_m == 0.0 && s.pickup_m_s.is_none())
    );
    assert!(v.last().unwrap().hook_height_m < -0.3);
}
