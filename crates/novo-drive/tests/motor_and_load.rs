//! Hand-calculated checks for lane L2a (two-speed motor, sequencing, hoist load, brake).
//! Every expected number below was worked out on paper first; the formula is in the comment.

use novo_drive::brake::{BrakeState, SpringBrake, Switching};
use novo_drive::hoist_unit::{BrakeFeed, HoistInput, TwoSpeedHoistUnit};
use novo_drive::mechanics::{
    Hoist, Shaft, hoist_lowering_torque_nm, hoist_raising_torque_nm, hoist_static_torque_nm,
};
use novo_drive::motor::{Direction, PoleChangeMotor, Winding, WindingSel, sync_rpm};
use novo_drive::sequencer::{Command, Sequencer, SequencerConfig, SpeedStep};
use novo_drive::{GRAVITY, InputError};

const TAU: f64 = std::f64::consts::TAU;

fn close(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol, "{a} vs {b} (tolerance {tol})");
}

// ---- 1. synchronous speed -------------------------------------------------------------

#[test]
fn sync_speed_table_120_f_over_p() {
    // (poles, Hz, rpm) = 120 f / p
    let table = [
        (2, 10.0, 600.0),
        (2, 25.0, 1500.0),
        (2, 50.0, 3000.0),
        (2, 87.0, 5220.0),
        (4, 10.0, 300.0),
        (4, 25.0, 750.0),
        (4, 50.0, 1500.0),
        (4, 87.0, 2610.0),
        (6, 10.0, 200.0),
        (6, 25.0, 500.0),
        (6, 50.0, 1000.0),
        (6, 87.0, 1740.0),
        (8, 10.0, 150.0),
        (8, 25.0, 375.0),
        (8, 50.0, 750.0),
        (8, 87.0, 1305.0),
        (12, 50.0, 500.0),
    ];
    for (poles, hz, rpm) in table {
        close(sync_rpm(hz, poles).unwrap(), rpm, 1e-9);
    }
}

#[test]
fn sync_speed_rejects_bad_input() {
    assert_eq!(sync_rpm(50.0, 0), Err(InputError::BadPoles { poles: 0 }));
    assert_eq!(sync_rpm(50.0, 3), Err(InputError::BadPoles { poles: 3 }));
    assert!(matches!(
        sync_rpm(-1.0, 4),
        Err(InputError::Negative { .. })
    ));
    assert!(matches!(
        sync_rpm(f64::NAN, 4),
        Err(InputError::NotFinite { .. })
    ));
}

// ---- 2. torque-speed curve ----------------------------------------------------------------

/// 4 kW, 4 poles, 50 Hz, 1440 rpm, breakdown 2.5 x rated.
/// s_n = (1500 - 1440)/1500 = 0.04; w_n = 1440 * 2 pi / 60 = 150.7964 rad/s;
/// T_n = 4000 / 150.7964 = 26.5258 N m; T_b = 2.5 T_n = 66.3146 N m;
/// s_b = 0.04 (2.5 + sqrt(5.25)) = 0.04 * 4.791288 = 0.191652.
fn four_pole() -> Winding {
    Winding::new(4, 50.0, 4000.0, 1440.0, 2.5).unwrap()
}

#[test]
fn kloss_fit_matches_hand_values() {
    let w = four_pole();
    close(w.rated_slip(), 0.04, 1e-12);
    close(w.rated_torque_nm(), 26.5258, 1e-3);
    close(w.breakdown_torque_nm(), 66.3146, 1e-3);
    close(w.breakdown_slip(), 0.191652, 1e-5);
    // The curve passes through the rated point and the breakdown point.
    close(
        w.torque_nm(Direction::Positive, 1440.0 * TAU / 60.0),
        26.5258,
        1e-3,
    );
    let w_at_sb = 1500.0 * (1.0 - 0.191652) * TAU / 60.0;
    close(w.torque_nm(Direction::Positive, w_at_sb), 66.3146, 1e-2);
}

#[test]
fn kloss_is_zero_at_sync_and_negative_above() {
    let w = four_pole();
    let ws = 1500.0 * TAU / 60.0;
    assert_eq!(w.torque_nm(Direction::Positive, ws), 0.0);
    // Mirror of the rated point: slip -0.04 gives -T_n (generator).
    close(
        w.torque_nm(Direction::Positive, 1560.0 * TAU / 60.0),
        -26.5258,
        1e-3,
    );
    // Reverse field mirrors everything.
    close(
        w.torque_nm(Direction::Negative, -1440.0 * TAU / 60.0),
        -26.5258,
        1e-3,
    );
}

#[test]
fn slip_for_torque_inverts_the_curve() {
    let w = four_pole();
    close(w.slip_for_torque(26.5258).unwrap(), 0.04, 1e-5);
    close(w.slip_for_torque(-26.5258).unwrap(), -0.04, 1e-5);
    assert_eq!(w.slip_for_torque(0.0), Some(0.0));
    assert_eq!(w.slip_for_torque(70.0), None, "above breakdown");
    let omega = w.steady_omega_rad_s(Direction::Positive, 26.5258).unwrap();
    close(omega, 1440.0 * TAU / 60.0, 1e-3);
}

#[test]
fn winding_rejects_bad_nameplate() {
    assert!(matches!(
        Winding::new(0, 50.0, 4000.0, 1440.0, 2.5),
        Err(InputError::BadPoles { .. })
    ));
    assert!(matches!(
        Winding::new(4, 50.0, 4000.0, 1500.0, 2.5),
        Err(InputError::OutOfRange { .. })
    ));
    assert!(matches!(
        Winding::new(4, 50.0, 0.0, 1440.0, 2.5),
        Err(InputError::NotPositive { .. })
    ));
    assert!(matches!(
        Winding::new(4, 50.0, 4000.0, 1440.0, 1e200),
        Err(InputError::OutOfRange { .. })
    ));
    let fine = Winding::new(12, 50.0, 800.0, 480.0, 2.0).unwrap();
    let coarse = Winding::new(2, 50.0, 6000.0, 2880.0, 2.2).unwrap();
    assert_eq!(
        PoleChangeMotor::new(coarse, fine, 0.1),
        Err(InputError::PolesNotDescending {
            fine: 2,
            coarse: 12
        })
    );
    assert!(matches!(
        Winding::new(4, 50.0, 4000.0, 1440.0, 1.0),
        Err(InputError::OutOfRange { .. })
    ));
}

// ---- 3. run-up and changeover ---------------------------------------------------------

/// Time for a free rotor of inertia `j` to go from slip `s1` to `s2` on a Kloss curve:
/// t = J w_s / (2 T_b) * [ (s1^2 - s2^2) / (2 s_b) + s_b ln(s1 / s2) ]
/// (from J dw = -J w_s ds and T = 2 T_b / (s/s_b + s_b/s)).
fn closed_form_time(w: &Winding, j: f64, s1: f64, s2: f64) -> f64 {
    let sb = w.breakdown_slip();
    j * w.sync_rad_s() / (2.0 * w.breakdown_torque_nm())
        * ((s1 * s1 - s2 * s2) / (2.0 * sb) + sb * (s1 / s2).ln())
}

fn simulate_free_run(w: &Winding, j: f64, omega0: f64, s_end: f64, dt: f64) -> f64 {
    let mut shaft = Shaft::new(j).unwrap();
    shaft.omega_rad_s = omega0;
    let mut t = 0.0;
    while (w.sync_rad_s() - shaft.omega_rad_s) / w.sync_rad_s() > s_end {
        let torque = w.torque_nm(Direction::Positive, shaft.omega_rad_s);
        shaft.step(dt, torque, 0.0, 0.0);
        t += dt;
        assert!(t < 100.0, "run-up never finished");
    }
    t
}

fn pole_change() -> PoleChangeMotor {
    // 12/2 pole hoist motor. Fine: 0.8 kW, 480 rpm of 500, breakdown 2.0.
    // Coarse: 6 kW, 2880 rpm of 3000, breakdown 2.2.
    PoleChangeMotor::new(
        Winding::new(12, 50.0, 800.0, 480.0, 2.0).unwrap(),
        Winding::new(2, 50.0, 6000.0, 2880.0, 2.2).unwrap(),
        0.05,
    )
    .unwrap()
}

#[test]
fn two_speed_run_up_and_changeover_times_match_the_closed_form() {
    let m = pole_change();
    let j = 0.3;
    let fine = m.winding(WindingSel::Fine);
    let coarse = m.winding(WindingSel::Coarse);

    // Stage 1: fine from rest (slip 1) to slip 0.05.
    let t1 = closed_form_time(fine, j, 1.0, 0.05);
    let sim1 = simulate_free_run(fine, j, 0.0, 0.05, 0.0002);
    close(sim1, t1, t1 * 0.005);

    // Changeover: the rotor coasts (no torque, no load) at 0.95 x fine sync speed, then the
    // coarse winding takes over at slip 1 - 0.95 w_fs / w_cs.
    let omega0 = 0.95 * fine.sync_rad_s();
    let s_start = 1.0 - omega0 / coarse.sync_rad_s();
    close(s_start, 1.0 - 0.95 * 500.0 / 3000.0, 1e-12);
    let t2 = closed_form_time(coarse, j, s_start, 0.05);
    let sim2 = simulate_free_run(coarse, j, omega0, 0.05, 0.0002);
    close(sim2, t2, t2 * 0.005);
    assert!(t2 > 0.0 && t1 > 0.0);
}

// ---- 4. hoist load ------------------------------------------------------------------------

#[test]
fn hoist_torques_by_hand() {
    // 5000 kg, r 0.2 m, i 31.5, eta 0.9: m g r = 9806.65 N m, / 31.5 = 311.322 N m static.
    close(
        hoist_static_torque_nm(5000.0, 0.2, 31.5).unwrap(),
        311.322,
        1e-3,
    );
    // raising  311.322 / 0.9 = 345.914, lowering 311.322 * 0.9 = 280.190
    close(
        hoist_raising_torque_nm(5000.0, 0.2, 31.5, 0.9).unwrap(),
        345.914,
        1e-3,
    );
    close(
        hoist_lowering_torque_nm(5000.0, 0.2, 31.5, 0.9).unwrap(),
        280.190,
        1e-3,
    );
}

#[test]
fn hoist_rejects_bad_input() {
    assert!(matches!(
        hoist_raising_torque_nm(5000.0, 0.2, 0.0, 0.9),
        Err(InputError::NotPositive { field: "ratio", .. })
    ));
    assert!(matches!(
        hoist_raising_torque_nm(5000.0, 0.2, 31.5, 0.0),
        Err(InputError::OutOfRange {
            field: "efficiency",
            ..
        })
    ));
    assert!(matches!(
        hoist_raising_torque_nm(5000.0, 0.2, 31.5, 1.2),
        Err(InputError::OutOfRange { .. })
    ));
    assert!(matches!(
        hoist_raising_torque_nm(5000.0, -0.2, 31.5, 0.9),
        Err(InputError::NotPositive { .. })
    ));
    assert!(Hoist::new(100.0, 0.1, 10.0, 0, 0.9).is_err());
}

#[test]
fn reeving_divides_hook_travel_and_torque() {
    // 2 falls halve the weight torque and the hook speed.
    let one = Hoist::new(1000.0, 0.1, 20.0, 1, 1.0).unwrap();
    let two = Hoist::new(1000.0, 0.1, 20.0, 2, 1.0).unwrap();
    close(two.static_torque_nm(), one.static_torque_nm() / 2.0, 1e-9);
    close(two.metres_per_rad(), one.metres_per_rad() / 2.0, 1e-12);
    close(
        two.reflected_inertia_kg_m2(),
        one.reflected_inertia_kg_m2() / 4.0,
        1e-12,
    );
}

// ---- 5. brake and the hoist unit --------------------------------------------------------

fn sequencer() -> Sequencer {
    Sequencer::new(SequencerConfig {
        changeover_dead_time_s: 0.1,
        fine_first_s: 0.5,
        reversal_dead_time_s: 0.2,
    })
    .unwrap()
}

fn brake(torque: f64, switching: Switching) -> SpringBrake {
    // release 20 ms, apply 200 ms AC side or 20 ms DC side, torque steps
    SpringBrake::new(torque, 0.02, 0.2, 0.02, 0.0, switching).unwrap()
}

fn unit(
    mass: f64,
    eta: f64,
    rotor_j: f64,
    brake_nm: f64,
    feed: BrakeFeed,
    switching: Switching,
) -> TwoSpeedHoistUnit {
    let m = PoleChangeMotor::new(
        *pole_change().winding(WindingSel::Fine),
        *pole_change().winding(WindingSel::Coarse),
        rotor_j,
    )
    .unwrap();
    // r 0.1 m, i 40: 0.0025 m per rad
    let hoist = Hoist::new(mass, 0.1, 40.0, 1, eta).unwrap();
    TwoSpeedHoistUnit::new(m, hoist, sequencer(), brake(brake_nm, switching), feed).unwrap()
}

fn idle(coil: bool) -> HoistInput {
    HoistInput {
        command: Command::OFF,
        mains_on: true,
        brake_coil_fed: coil,
    }
}

#[test]
fn brake_drop_without_rotor_inertia_is_the_half_g_t_squared_bound() {
    // This is the upper bound only (zero rotor inertia, lossless): see the next tests for
    // the real figure with inertia reflected through the gearbox.
    // Coil fed for 0.15 s, release delay 20 ms, no motor torque: the load falls for
    // t = 0.15 - 0.02 = 0.13 s. Lossless drum, no rotor inertia: a = g.
    // drop = 0.5 * 9.80665 * 0.13^2 = 0.082862 m
    let mut u = unit(
        500.0,
        1.0,
        0.0,
        400.0,
        BrakeFeed::Separate,
        Switching::AcSide,
    );
    let mut snap = None;
    for _ in 0..150 {
        snap = Some(u.step(idle(true), 0.001));
    }
    close(snap.unwrap().hook_height_m, -0.082862, 2e-5);
}

#[test]
fn brake_drop_with_losses_and_rotor_inertia() {
    // a = m g eta k^2 / (J + m k^2) with k = 0.0025 m/rad: m k^2 = 500 * 6.25e-6 = 0.003125,
    // J = 0.01, eta = 0.9: a = 9.80665 * 0.9 * 0.003125 / 0.013125 = 2.10143 m/s2.
    // After 0.13 s of fall: 0.5 * 2.10143 * 0.0169 = 0.017757 m.
    let mut u = unit(
        500.0,
        0.9,
        0.01,
        400.0,
        BrakeFeed::Separate,
        Switching::AcSide,
    );
    let mut snap = None;
    for _ in 0..150 {
        snap = Some(u.step(idle(true), 0.001));
    }
    close(snap.unwrap().hook_height_m, -0.017757, 2e-5);
}

#[test]
fn brake_drop_150_ms_matches_the_l7_hand_value() {
    // 5000 kg, r 0.2 m, i 31.5, rotor 0.5 kg m2, lossless; fall for 150 ms (release delay 20 ms).
    // Reflected mass J (i/r)^2 = 0.5 * 157.5^2 = 12403.1 kg; a = g m / (m + 12403.1) = 2.8175;
    // drop = 0.5 * 2.8175 * 0.15^2 = 0.031697 m (the 0.5 g t^2 bound would be 0.1103 m).
    let m = PoleChangeMotor::new(
        *pole_change().winding(WindingSel::Fine),
        *pole_change().winding(WindingSel::Coarse),
        0.5,
    )
    .unwrap();
    let hoist = Hoist::new(5000.0, 0.2, 31.5, 1, 1.0).unwrap();
    let mut u = TwoSpeedHoistUnit::new(
        m,
        hoist,
        sequencer(),
        brake(400.0, Switching::AcSide),
        BrakeFeed::Separate,
    )
    .unwrap();
    let mut snap = None;
    for _ in 0..170 {
        snap = Some(u.step(idle(true), 0.001));
    }
    close(snap.unwrap().hook_height_m, -0.031697, 5e-5);
}

#[test]
fn applied_brake_holds_when_strong_enough_and_slips_when_not() {
    // Static torque 500 * 9.80665 * 0.0025 = 12.2583 N m.
    let mut strong = unit(
        500.0,
        0.9,
        0.01,
        20.0,
        BrakeFeed::Separate,
        Switching::AcSide,
    );
    let mut weak = unit(
        500.0,
        0.9,
        0.01,
        5.0,
        BrakeFeed::Separate,
        Switching::AcSide,
    );
    for _ in 0..3000 {
        strong.step(idle(false), 0.001);
        weak.step(idle(false), 0.001);
    }
    assert_eq!(strong.shaft().omega_rad_s, 0.0);
    assert_eq!(strong.shaft().angle_rad, 0.0);
    assert!(weak.shaft().omega_rad_s < 0.0, "load runs away down");
}

#[test]
fn brake_apply_delay_depends_on_switching_side() {
    let mut ac = brake(100.0, Switching::AcSide);
    let mut dc = brake(100.0, Switching::DcSide);
    for _ in 0..100 {
        ac.step(true, 0.001);
        dc.step(true, 0.001);
    }
    assert_eq!(ac.state(), BrakeState::Released);
    // Coil off: DC side torque is back after 20 ms, AC side after 200 ms.
    let mut t_ac = 0;
    let mut t_dc = 0;
    for k in 1..=400 {
        ac.step(false, 0.001);
        dc.step(false, 0.001);
        if t_dc == 0 && dc.state() == BrakeState::Applied {
            t_dc = k;
        }
        if t_ac == 0 && ac.state() == BrakeState::Applied {
            t_ac = k;
        }
    }
    assert_eq!(t_dc, 20);
    assert_eq!(t_ac, 200);
}

#[test]
fn brake_validation() {
    assert!(SpringBrake::new(0.0, 0.0, 0.0, 0.0, 0.0, Switching::AcSide).is_err());
    assert!(SpringBrake::new(10.0, -0.1, 0.0, 0.0, 0.0, Switching::AcSide).is_err());
}

fn run_command(
    u: &mut TwoSpeedHoistUnit,
    cmd: Command,
    seconds: f64,
) -> novo_drive::hoist_unit::HoistSnapshot {
    let n = (seconds / 0.001).round() as usize;
    let mut last = None;
    for _ in 0..n {
        last = Some(u.step(
            HoistInput {
                command: cmd,
                mains_on: true,
                brake_coil_fed: false,
            },
            0.001,
        ));
    }
    last.unwrap()
}

#[test]
fn raising_at_coarse_settles_where_motor_torque_equals_load() {
    // 500 kg on k = 0.0025 m/rad, eta 0.9: raising torque = 12.2583 / 0.9 = 13.6204 N m.
    let mut u = unit(
        500.0,
        0.9,
        0.05,
        40.0,
        BrakeFeed::MotorTerminals,
        Switching::AcSide,
    );
    let snap = run_command(
        &mut u,
        Command {
            direction: Direction::Positive,
            step: SpeedStep::Two,
        },
        15.0,
    );
    assert_eq!(snap.energised.unwrap().winding, WindingSel::Coarse);
    let coarse = *pole_change().winding(WindingSel::Coarse);
    let omega = coarse
        .steady_omega_rad_s(Direction::Positive, 13.6204)
        .unwrap();
    close(snap.hook_speed_m_s, omega * 0.0025, 1e-3);
    close(snap.motor_torque_nm, 13.6204, 1e-2);
    assert!(snap.hook_height_m > 1.0);
}

#[test]
fn lowering_at_coarse_overhauls_above_sync_speed() {
    // Holding back torque when lowering: 12.2583 * 0.9 = 11.0325 N m, field sense -11.0325,
    // so the rotor turns faster than synchronous speed (generator braking).
    let mut u = unit(
        500.0,
        0.9,
        0.05,
        40.0,
        BrakeFeed::MotorTerminals,
        Switching::AcSide,
    );
    let snap = run_command(
        &mut u,
        Command {
            direction: Direction::Negative,
            step: SpeedStep::Two,
        },
        15.0,
    );
    let coarse = *pole_change().winding(WindingSel::Coarse);
    let sync_hook = coarse.sync_rad_s() * 0.0025;
    assert!(snap.hook_speed_m_s < -sync_hook, "{}", snap.hook_speed_m_s);
    let omega = coarse
        .steady_omega_rad_s(Direction::Negative, -11.0325)
        .unwrap();
    close(snap.hook_speed_m_s, omega * 0.0025, 1e-3);
}

#[test]
fn releasing_to_step_one_drops_to_fine_and_off_stops_with_the_brake() {
    let mut u = unit(
        500.0,
        0.9,
        0.05,
        40.0,
        BrakeFeed::MotorTerminals,
        Switching::DcSide,
    );
    run_command(
        &mut u,
        Command {
            direction: Direction::Positive,
            step: SpeedStep::Two,
        },
        10.0,
    );
    let slow = run_command(
        &mut u,
        Command {
            direction: Direction::Positive,
            step: SpeedStep::One,
        },
        10.0,
    );
    assert_eq!(slow.energised.unwrap().winding, WindingSel::Fine);
    // Fine winding sync is 500 rpm = 52.36 rad/s = 0.1309 m/s at k = 0.0025; steady slightly below.
    assert!(slow.hook_speed_m_s > 0.05 && slow.hook_speed_m_s < 0.1309);
    let stopped = run_command(&mut u, Command::OFF, 3.0);
    assert_eq!(stopped.energised, None);
    assert_eq!(stopped.hook_speed_m_s, 0.0);
    assert!(stopped.brake_torque_nm > 0.0);
    // And it holds afterwards.
    let later = run_command(&mut u, Command::OFF, 5.0);
    close(later.hook_height_m, stopped.hook_height_m, 0.0);
}

#[test]
fn mains_off_drops_motor_and_brake() {
    let mut u = unit(
        500.0,
        0.9,
        0.05,
        40.0,
        BrakeFeed::MotorTerminals,
        Switching::DcSide,
    );
    run_command(
        &mut u,
        Command {
            direction: Direction::Positive,
            step: SpeedStep::Two,
        },
        8.0,
    );
    let mut last = None;
    for _ in 0..3000 {
        last = Some(u.step(
            HoistInput {
                command: Command {
                    direction: Direction::Positive,
                    step: SpeedStep::Two,
                },
                mains_on: false,
                brake_coil_fed: true,
            },
            0.001,
        ));
    }
    let s = last.unwrap();
    assert_eq!(s.energised, None);
    assert_eq!(s.hook_speed_m_s, 0.0);
}

// ---- 6. sequencing ------------------------------------------------------------------------

fn seq_trace(cmds: &[(Command, f64)]) -> Vec<(f64, Option<(WindingSel, Direction)>)> {
    let mut s = sequencer();
    let dt = 0.01;
    let mut t = 0.0;
    let mut out = Vec::new();
    for (cmd, dur) in cmds {
        let n = (dur / dt).round() as usize;
        for _ in 0..n {
            t += dt;
            out.push((t, s.step(*cmd, dt).map(|e| (e.winding, e.direction))));
        }
    }
    out
}

fn at(trace: &[(f64, Option<(WindingSel, Direction)>)], t: f64) -> Option<(WindingSel, Direction)> {
    trace
        .iter()
        .min_by(|a, b| (a.0 - t).abs().partial_cmp(&(b.0 - t).abs()).unwrap())
        .unwrap()
        .1
}

#[test]
fn step_two_from_standstill_goes_fine_first_then_dead_time_then_coarse() {
    let up2 = Command {
        direction: Direction::Positive,
        step: SpeedStep::Two,
    };
    let tr = seq_trace(&[(up2, 1.5)]);
    let fine = Some((WindingSel::Fine, Direction::Positive));
    let coarse = Some((WindingSel::Coarse, Direction::Positive));
    assert_eq!(at(&tr, 0.2), fine);
    assert_eq!(at(&tr, 0.45), fine);
    assert_eq!(at(&tr, 0.55), None, "dead time between fine and coarse");
    assert_eq!(at(&tr, 0.8), coarse);
    assert_eq!(at(&tr, 1.4), coarse);
    // Never both: a trace holds one winding at a time by construction; count the dead steps.
    let dead = tr.iter().filter(|x| x.1.is_none()).count();
    assert!((9..=11).contains(&dead), "dead steps {dead}");
}

#[test]
fn release_to_step_one_and_off_and_reversal() {
    let up1 = Command {
        direction: Direction::Positive,
        step: SpeedStep::One,
    };
    let up2 = Command {
        direction: Direction::Positive,
        step: SpeedStep::Two,
    };
    let dn1 = Command {
        direction: Direction::Negative,
        step: SpeedStep::One,
    };
    let tr = seq_trace(&[
        (up2, 1.5),
        (up1, 1.0),
        (Command::OFF, 0.3),
        (up1, 0.3),
        (dn1, 0.6),
    ]);
    let fine = Some((WindingSel::Fine, Direction::Positive));
    // After release of step 2 (t = 1.5): dead 0.1 s, then fine.
    assert_eq!(at(&tr, 1.55), None);
    assert_eq!(at(&tr, 1.8), fine);
    // Off at t = 2.5.
    assert_eq!(at(&tr, 2.7), None);
    // Up 1 at 2.8..3.1, reverse to down 1 at 3.1: dead 0.2 s, then fine negative.
    assert_eq!(at(&tr, 3.0), fine);
    assert_eq!(at(&tr, 3.2), None);
    assert_eq!(at(&tr, 3.6), Some((WindingSel::Fine, Direction::Negative)));
}

#[test]
fn sequencer_rejects_negative_times() {
    assert!(
        Sequencer::new(SequencerConfig {
            changeover_dead_time_s: -0.1,
            fine_first_s: 0.0,
            reversal_dead_time_s: 0.0,
        })
        .is_err()
    );
}

#[test]
fn gravity_constant_is_standard() {
    close(GRAVITY, 9.80665, 1e-12);
}
