//! Hand-calculated checks for two synchronous closed-loop winches on one beam (lane L2c).

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::closed_loop::{ClFault, ClState, ClosedLoopConfig, ClosedLoopWinch, PreTorque};
use novo_drive::drive::MotorNameplate;
use novo_drive::mechanics::Hoist;
use novo_machine::sync_winch::{
    Beam, PairCommand, PairSnapshot, PartnerAction, Rope, Supervision, SyncFault, SyncMode,
    WinchPair,
};

const G: f64 = novo_drive::GRAVITY;
const DT: f64 = 0.002;
const MASS: f64 = 5000.0;
const K: f64 = 2.0e6;
const SWL: f64 = 30_000.0;
const DRUM_R: f64 = 0.2;
const RATIO: f64 = 31.5;
const MPR: f64 = DRUM_R / RATIO;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b} (tolerance {tol})");
}

fn cfg() -> ClosedLoopConfig {
    let motor = MotorNameplate {
        voltage_v: 400.0,
        current_a: 56.0,
        frequency_hz: 50.0,
        rpm: 1480.0,
        power_kw: 30.0,
        power_factor: 0.85,
    };
    let mut c = ClosedLoopConfig::generic_hoist(motor);
    // Each winch holds half the weight: M g / 2 * r / i = 155.67 N m.
    c.pre_torque = PreTorque::Fixed(MASS * G / 2.0 * MPR);
    c.brake_close_wait_s = 0.5;
    c.bus.resistor_ohm = Some(40.0);
    c
}

fn winch(c: ClosedLoopConfig) -> ClosedLoopWinch {
    // The rope load is integrated by the pair; the hoist only gives geometry and efficiency.
    let hoist = Hoist::new(1.0, DRUM_R, RATIO, 1, 0.9).unwrap();
    let brake = SpringBrake::new(400.0, 0.15, 0.2, 0.2, 0.0, Switching::DcSide).unwrap();
    ClosedLoopWinch::with_shaft_inertia(c, hoist, brake, 0.5, 1.0).unwrap()
}

fn pair(guided: bool, cog: f64, mode: SyncMode, sup: Supervision) -> WinchPair {
    let beam = Beam::new(MASS, 4.0, cog, guided).unwrap();
    let rope = Rope::with_stiffness(K, 0.3).unwrap();
    WinchPair::new(winch(cfg()), winch(cfg()), [rope; 2], beam, mode, sup)
}

fn run(p: &mut WinchPair, cmd: PairCommand, secs: f64) -> Vec<PairSnapshot> {
    (0..(secs / DT).round() as usize)
        .map(|_| p.step(&cmd, DT))
        .collect()
}

fn sup() -> Supervision {
    Supervision::new(SWL)
}

#[test]
fn static_rope_forces_follow_the_centre_of_gravity() {
    // F_A = M g (1/2 + e/L) = 5000 * 9.80665 * 0.6 = 29419.95 N, F_B = 19613.3 N for e = 0.4, L = 4.
    let mut p = pair(false, 0.4, SyncMode::Independent, sup());
    let v = run(&mut p, PairCommand::default(), 2.0);
    let last = v.last().unwrap();
    close(last.force_n[0], 29419.95, 150.0);
    close(last.force_n[1], 19613.30, 150.0);
    close(last.share_a, 0.6, 0.005);
    assert!(last.height_m.abs() < 1e-4 && last.tilt_deg.abs() < 0.01);
    assert!(!last.overload[0] && !last.slack[0]);
}

#[test]
fn matched_winches_lift_the_beam_level_with_the_rope_forces_unchanged() {
    let mut p = pair(false, 0.4, SyncMode::Independent, sup());
    let v = run(&mut p, PairCommand::run(0.2), 6.0);
    let last = v.last().unwrap();
    assert!(last.height_m > 0.5, "lifted {}", last.height_m);
    assert!(last.tilt_deg.abs() < 0.05, "tilt {}", last.tilt_deg);
    close(last.share_a, 0.6, 0.01);
    // The hook follows the drums: speed within 2 % of the reference at the end.
    close(last.winches[0].hook_speed_m_s, 0.2, 0.004);
}

#[test]
fn a_drum_one_percent_too_big_tilts_a_free_beam_by_the_difference_over_the_length() {
    // Free beam, equal drums commissioned but winch B's drum is 1 % larger: after the same turns
    // B has taken 1 % more rope. Tilt (small angle) = (x_A - x_B) / L; forces stay static.
    let mut p = pair(false, 0.0, SyncMode::Independent, sup()).with_drum_error([1.0, 1.01]);
    let v = run(&mut p, PairCommand::run(0.3), 5.0);
    let last = v.last().unwrap();
    let dx = last.true_position_m[0] - last.true_position_m[1];
    assert!(dx < -0.005, "B must be ahead: {dx}");
    close(
        last.tilt_deg.to_radians(),
        dx / 4.0,
        0.06 * (dx / 4.0).abs(),
    );
    close(last.share_a, 0.5, 0.01);
}

#[test]
fn a_drum_error_on_a_guided_beam_loads_the_rope_that_took_more() {
    // Guided beam, equal stiffness: F_B - F_A = k (x_B - x_A) and F_A + F_B = M g.
    let mut p = pair(true, 0.0, SyncMode::Independent, sup()).with_drum_error([1.0, 1.01]);
    let v = run(&mut p, PairCommand::run(0.3), 5.0);
    let last = v.last().unwrap();
    let dx = last.true_position_m[1] - last.true_position_m[0];
    assert!(dx > 0.008, "{dx}");
    let expect_b = 0.5 * (MASS * G + K * dx);
    close(last.force_n[1], expect_b, 0.06 * expect_b);
    close(last.force_n[0] + last.force_n[1], MASS * G, 0.03 * MASS * G);
    assert!(
        last.overload[1],
        "rope B is over its rating: {:?}",
        last.force_n
    );
}

#[test]
fn a_position_lock_keeps_the_encoders_together_but_not_a_wrong_drum() {
    // The lock compares encoder counts: with a drum 1 % too big the counts agree, the ropes do not.
    let mode = SyncMode::PositionLock {
        gain_per_s: 20.0,
        limit_m_s: 0.05,
    };
    let mut p = pair(true, 0.0, mode, sup()).with_drum_error([1.0, 1.01]);
    let v = run(&mut p, PairCommand::run(0.3), 5.0);
    let last = v.last().unwrap();
    assert!(
        last.position_error_m.abs() < 0.002,
        "encoders {}",
        last.position_error_m
    );
    let dx = last.true_position_m[1] - last.true_position_m[0];
    assert!(dx > 0.008, "true difference {dx}");
    assert!((last.force_n[1] - last.force_n[0]) > 0.2 * MASS * G);
}

#[test]
fn a_position_lock_cancels_a_slow_ramp_on_one_drive() {
    let mode = SyncMode::PositionLock {
        gain_per_s: 20.0,
        limit_m_s: 0.05,
    };
    let mut p = pair(true, 0.0, mode, sup());
    // B accelerates at 600 rpm/s, A at 740: a difference of 140 rpm/s for about 0.6 s.
    p.b.drive.cfg.accel_rpm_s = 600.0;
    let v = run(&mut p, PairCommand::run(0.3), 6.0);
    let last = v.last().unwrap();
    let worst = v
        .iter()
        .map(|s| s.position_error_m.abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.02, "{worst}");
    assert!(
        last.position_error_m.abs() < 0.003,
        "{}",
        last.position_error_m
    );
    close(last.share_a, 0.5, 0.03);
}

#[test]
fn a_torque_follower_shares_the_load_by_torque_and_ignores_the_drum_error() {
    // Follower torque = leader torque: F_B * mpr_B = F_A * mpr_A, so F_B / F_A = 1 / 1.01.
    let mode = SyncMode::TorqueFollower {
        share: 1.0,
        margin: 0.15,
    };
    let mut p = pair(true, 0.0, mode, sup()).with_drum_error([1.0, 1.01]);
    let v = run(&mut p, PairCommand::run(0.3), 5.0);
    let last = v.last().unwrap();
    close(last.force_n[1] / last.force_n[0], 1.0 / 1.01, 0.03);
    assert!(!last.overload[0] && !last.overload[1], "{:?}", last.force_n);
    assert!(last.winches[1].drive.fault.is_none() && last.winches[0].drive.fault.is_none());
}

#[test]
fn position_error_supervision_stops_both_winches() {
    let mut s = sup();
    s.max_position_error_m = 0.02;
    let mut p = pair(true, 0.0, SyncMode::Independent, s);
    p.b.drive.cfg.accel_rpm_s = 100.0;
    let v = run(&mut p, PairCommand::run(0.3), 8.0);
    let last = v.last().unwrap();
    assert_eq!(last.sync_fault, Some(SyncFault::PositionError));
    assert_eq!(p.state(0), ClState::Off);
    assert_eq!(p.state(1), ClState::Off);
    assert!(last.winches[0].shaft_rpm.abs() < 1e-3 && last.winches[1].shaft_rpm.abs() < 1e-3);
}

fn fault_a_after(p: &mut WinchPair, lead_s: f64, secs: f64) -> Vec<PairSnapshot> {
    run(p, PairCommand::run(0.3), lead_s);
    let mut cmd = PairCommand::run(0.3);
    cmd.external_fault[0] = true;
    run(p, cmd, secs)
}

#[test]
fn one_drive_trips_and_the_link_stops_the_partner() {
    let mut s = sup();
    s.on_partner_fault = PartnerAction::StopBoth;
    let mut p = pair(false, 0.0, SyncMode::Independent, s);
    let v = fault_a_after(&mut p, 4.0, 4.0);
    let last = v.last().unwrap();
    assert_eq!(p.fault(0), Some(ClFault::External));
    assert_eq!(last.sync_fault, Some(SyncFault::PartnerFault));
    assert_eq!(
        p.state(1),
        ClState::Off,
        "partner stopped and held by its brake"
    );
    assert!(last.winches[1].shaft_rpm.abs() < 1e-3 && last.winches[0].shaft_rpm.abs() < 1e-3);
    // Free beam: the rope forces never leave the static shares.
    assert!(v.iter().all(|s| !s.beyond_model));
}

#[test]
fn one_drive_trips_without_a_link_and_the_partner_carries_a_guided_load_alone() {
    // Guided beam: A's rope goes slack as the beam rises, B takes the whole weight M g = 49.03 kN,
    // which is above the rope rating of 30 kN. B would need M g mpr / eta = 49033 * 0.006349 / 0.9
    // = 346 N m to keep lifting at constant speed and its torque limit is 320 N m: it slows.
    let mut s = sup();
    s.on_partner_fault = PartnerAction::KeepRunning;
    let mut p = pair(true, 0.0, SyncMode::Independent, s);
    let v = fault_a_after(&mut p, 3.0, 1.0);
    let last = v.last().unwrap();
    assert_eq!(last.sync_fault, None);
    assert!(last.slack[0], "rope A slack: {:?}", last.force_n);
    close(last.force_n[1], MASS * G, 0.05 * MASS * G);
    assert!(last.overload[1]);
    assert!(
        last.winches[1].drive.current_limit_active,
        "B is at its torque limit"
    );
    let needed = MASS * G * MPR / 0.9;
    assert!(needed > last.winches[1].drive.torque_limit_nm, "{needed}");
    let early = v[100].winches[1].shaft_rpm;
    assert!(
        last.winches[1].shaft_rpm < early,
        "B slows under the whole load"
    );
}

#[test]
fn one_drive_trips_without_a_link_and_a_free_beam_tilts_out_of_the_model() {
    let mut s = sup();
    s.on_partner_fault = PartnerAction::KeepRunning;
    let mut p = pair(false, 0.0, SyncMode::Independent, s);
    let v = fault_a_after(&mut p, 3.0, 6.0);
    let last = v.last().unwrap();
    // B keeps raising its end at 0.3 m/s about A's locked point: tilt ~ asin(dx / 4 m).
    assert!(last.tilt_deg.abs() > 5.0, "tilt {}", last.tilt_deg);
    assert!(
        last.beyond_model,
        "the snapshot says the vertical-rope model no longer applies"
    );
}

#[test]
fn rope_stiffness_from_steel_is_e_a_over_l() {
    // E = 100 GPa, A = 180 mm2, L = 20 m: k = 100e9 * 180e-6 / 20 = 900 kN/m.
    let r = Rope::from_steel(180.0, 20.0, 100.0, 0.3).unwrap();
    close(r.stiffness_n_m(), 900_000.0, 1.0);
    assert!(Rope::from_steel(-1.0, 20.0, 100.0, 0.3).is_err());
    assert!(Rope::with_stiffness(1e6, 5.0).is_err());
    assert!(Beam::new(1000.0, 4.0, 2.5, false).is_err());
}
