//! Loop checks for lane L5a part 3: a drive, its motor and a wheeled load, with a scripted
//! circuit. Expected values are worked out on paper in the comments.

use novo_drive::brake::{SpringBrake, Switching};
use novo_drive::drive::ReferenceSource;
use novo_drive::drive::{Drive, DriveConfig, MotorNameplate, TerminalMacro};
use novo_drive::induction::{FitOptions, InductionMotor};
use novo_machine::bindings::SensorBinding;
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::{LimitKind, LimitSwitch};
use novo_machine::port::FakeCircuit;
use novo_machine::travel_loop::{
    Grip, Gust, StopRecord, TravelBinding, TravelFindingKind, TravelLoad, TravelLoop, TravelModule,
    Wind,
};

const DT: f64 = 0.005;

fn plate() -> MotorNameplate {
    MotorNameplate {
        voltage_v: 400.0,
        current_a: 3.6,
        frequency_hz: 50.0,
        rpm: 1430.0,
        power_kw: 1.5,
        power_factor: 0.80,
    }
}

/// 5000 kg on 0.15 m wheels, gearbox 30: 0.005 m per rad/s of motor speed.
fn load() -> TravelLoad {
    TravelLoad::new(5000.0, 0.15, 30.0, 0.9, 0.01, 0.0, 0.005).unwrap()
}

fn module(limits: Vec<LimitSwitch>) -> TravelModule {
    let np = plate();
    let (motor, _) = InductionMotor::from_nameplate(&np, None, &FitOptions::typical()).unwrap();
    let mut cfg = DriveConfig::generic_travel(np, 4.0);
    // Step 1 runs the main reference (10 Hz), step 2 selects preset 2 (50 Hz).
    cfg.reference = ReferenceSource::Panel(10.0);
    cfg.macro_ = TerminalMacro::two_wire_two_speed();
    cfg.accel_time_s = 4.0;
    cfg.decel_time_s = 4.0;
    TravelModule::new(
        "=A-TRAVEL",
        Drive::new(cfg),
        motor,
        load(),
        TravelBinding {
            inputs: vec![
                Some("=K-FWD".into()),
                Some("=K-REV".into()),
                Some("=K-HI".into()),
                Some("=K-EN".into()),
            ],
            mains: Some("=K-MAIN".into()),
            limits,
            ..TravelBinding::default()
        },
    )
}

fn the_loop(limits: Vec<LimitSwitch>) -> TravelLoop {
    let mut l = TravelLoop::new(StepConfig { dt_s: DT }).unwrap();
    l.modules.push(module(limits));
    l
}

fn circuit(tags: &[&str]) -> FakeCircuit {
    let mut c = FakeCircuit::new();
    for t in ["=K-MAIN", "=K-EN"].iter().chain(tags) {
        c.energised.insert((*t).to_owned());
    }
    c
}

fn seconds(s: f64) -> usize {
    (s / DT).round() as usize
}

#[test]
fn load_arithmetic_is_the_closed_form() {
    let l = load();
    // r / i = 0.15 / 30 = 0.005 m per rad.
    assert!((l.metres_per_rad() - 0.005).abs() < 1e-15);
    // J = 0.005 + 5000 * 0.005^2 = 0.13 kg m2.
    assert!((l.inertia_kg_m2() - 0.13).abs() < 1e-12);
    // Rolling: 5000 * g * 0.01 * 0.005 / 0.9 N m.
    let t = 5000.0 * novo_drive::GRAVITY * 0.01 * 0.005 / 0.9;
    assert!((l.rolling_torque_nm() - t).abs() < 1e-12);
    assert!((l.torque_nm(100.0) + t).abs() < 1e-12);
    assert!((l.torque_nm(-100.0) - t).abs() < 1e-12);
    assert_eq!(l.torque_nm(0.0), 0.0);
}

#[test]
fn products_that_overflow_are_refused_not_run() {
    // Each value is finite, the derived inertia or torque is not.
    assert!(TravelLoad::new(5000.0, 1e300, 30.0, 0.9, 0.01, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(5000.0, 0.15, 1e-300, 0.9, 0.01, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(5000.0, 0.15, 30.0, 1e-300, 0.01, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(5000.0, 0.15, 30.0, 0.9, 1e300, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(5000.0, 0.15, 30.0, 0.9, 0.01, 1e300, 0.0).is_err());
    assert!(TravelLoad::new(1e300, 0.15, 1e-30, 0.9, 0.01, 0.0, 0.0).is_err());
}

#[test]
fn bad_load_data_is_refused() {
    assert!(TravelLoad::new(0.0, 0.15, 30.0, 0.9, 0.01, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(10.0, 0.15, 30.0, 1.2, 0.01, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(10.0, 0.15, 30.0, 0.9, -0.01, 0.0, 0.0).is_err());
    assert!(TravelLoad::new(10.0, 0.15, 30.0, 0.9, 0.01, f64::NAN, 0.0).is_err());
}

#[test]
fn step_one_settles_a_little_below_the_synchronous_speed_of_10_hz() {
    let mut l = the_loop(vec![]);
    let mut c = circuit(&["=K-FWD"]);
    l.run(&mut c, seconds(30.0));
    let s = l.module("=A-TRAVEL").unwrap().snapshot().unwrap().clone();
    // Synchronous at 10 Hz on 4 poles: 300 rpm = 31.4 rad/s = 0.157 m/s; slip takes a few %.
    assert!(s.speed_m_s > 0.12 && s.speed_m_s < 0.157, "{}", s.speed_m_s);
    assert!((s.drive.frequency_hz - 10.0).abs() < 1e-6);
    assert!(l.module("=A-TRAVEL").unwrap().position_m() > 2.0);
}

#[test]
fn step_two_reaches_the_high_speed_preset() {
    let mut l = the_loop(vec![]);
    let mut c = circuit(&["=K-FWD", "=K-HI"]);
    l.run(&mut c, seconds(40.0));
    let s = l.module("=A-TRAVEL").unwrap().snapshot().unwrap().clone();
    // Preset 2 is 50 Hz: synchronous 1500 rpm = 0.785 m/s.
    assert!(s.speed_m_s > 0.70 && s.speed_m_s < 0.785, "{}", s.speed_m_s);
}

#[test]
fn reverse_runs_the_other_way() {
    let mut l = the_loop(vec![]);
    let mut c = circuit(&["=K-REV"]);
    l.run(&mut c, seconds(20.0));
    let m = l.module("=A-TRAVEL").unwrap();
    assert!(m.position_m() < -1.0, "{}", m.position_m());
    assert!(m.snapshot().unwrap().speed_m_s < 0.0);
}

#[test]
fn stopping_ramps_down_and_feeds_energy_back() {
    let mut l = the_loop(vec![]);
    let mut c = circuit(&["=K-FWD", "=K-HI"]);
    l.run(&mut c, seconds(30.0));
    c.energised.remove("=K-FWD");
    let mut min_power: f64 = 0.0;
    for _ in 0..seconds(20.0) {
        l.step(&mut c);
        min_power = min_power.min(
            l.module("=A-TRAVEL")
                .unwrap()
                .snapshot()
                .unwrap()
                .motor
                .input_power_w,
        );
    }
    let m = l.module("=A-TRAVEL").unwrap();
    assert!(m.snapshot().unwrap().speed_m_s.abs() < 0.01);
    assert!(
        min_power < 0.0,
        "motor fed energy back on the ramp: {min_power}"
    );
}

#[test]
fn the_current_stays_inside_the_drive_limit() {
    let mut l = the_loop(vec![]);
    let mut c = circuit(&["=K-FWD", "=K-HI"]);
    let mut peak: f64 = 0.0;
    for _ in 0..seconds(30.0) {
        l.step(&mut c);
        peak = peak.max(
            l.module("=A-TRAVEL")
                .unwrap()
                .snapshot()
                .unwrap()
                .motor
                .stator_current_a,
        );
    }
    assert!(peak < 1.5 * 4.0 * 1.5, "peak {peak}");
}

#[test]
fn an_end_limit_that_opens_the_forward_contactor_stops_the_axis() {
    let limit = LimitSwitch {
        binding: SensorBinding {
            name: "end".into(),
            device_tag: "=S-END".into(),
            actuate_at_mm: 1000.0,
        },
        kind: LimitKind::Upper,
    };
    let mut l = the_loop(vec![limit]);
    // The forward input is only fed while the end limit conducts.
    let mut c = circuit(&[]).with_rule(|s| {
        if s.contacts.get("=S-END").copied().unwrap_or(true) {
            s.energised.insert("=K-FWD".into());
        } else {
            s.energised.remove("=K-FWD");
        }
    });
    l.run(&mut c, seconds(40.0));
    let m = l.module("=A-TRAVEL").unwrap();
    // 10 Hz is 0.15 m/s; the stop ramp is 4 s from 50 Hz, so about 0.4 s: well inside 5 cm.
    assert!(
        m.position_m() >= 1.0 && m.position_m() < 1.1,
        "{}",
        m.position_m()
    );
    assert!(m.snapshot().unwrap().speed_m_s.abs() < 0.01);
}

#[test]
fn a_run_is_deterministic() {
    let run = || {
        let mut l = the_loop(vec![]);
        let mut c = circuit(&["=K-FWD", "=K-HI"]);
        l.run(&mut c, seconds(15.0));
        let m = l.module("=A-TRAVEL").unwrap();
        (m.position_m().to_bits(), m.rpm().to_bits())
    };
    assert_eq!(run(), run());
}

#[test]
fn missing_bindings_are_reported_before_the_run() {
    let l = the_loop(vec![]);
    let c = circuit(&["=K-FWD"]);
    let findings = l.check_bindings(&c);
    assert!(findings.iter().any(|f| f.message.contains("=K-REV")));
    assert!(findings.iter().any(|f| f.message.contains("=K-HI")));
    assert!(!findings.iter().any(|f| f.message.contains("=K-FWD")));
}

#[test]
fn a_refused_motor_step_stops_that_module_with_a_message() {
    // A negative step time is refused by the motor model: the module stops and says why.
    let mut l = the_loop(vec![]);
    l.config.dt_s = -1.0;
    let mut c = circuit(&["=K-MAIN", "=K-EN", "=K-FWD"]);
    l.run(&mut c, 3);
    let m = l.module("=A-TRAVEL").unwrap();
    let why = m.fault().expect("the module has stopped");
    assert!(why.starts_with("the run stopped:"), "{why}");
    assert!(m.snapshot().is_none());
}

// ---- Brake, wind, gearbox and grip -----------------------------------------------------------

const MPR: f64 = 0.005;
const G: f64 = novo_drive::GRAVITY;

/// The reference axis with a wind and a brake. `brake` is torque, release delay, AC apply delay.
fn axis(load: TravelLoad, brake: Option<(f64, f64, f64)>, feed: Option<&str>) -> TravelLoop {
    let mut m = module(vec![]);
    m.load = load;
    m.binding.brake_feed = feed.map(str::to_owned);
    if let Some((torque, release, apply)) = brake {
        let b = SpringBrake::new(torque, release, apply, 0.03, 0.0, Switching::AcSide).unwrap();
        m = m.with_brake(b, Grip::default());
    }
    let mut l = TravelLoop::new(StepConfig { dt_s: DT }).unwrap();
    l.modules.push(m);
    l
}

fn the_stop(l: &TravelLoop) -> StopRecord {
    l.module("=A-TRAVEL")
        .unwrap()
        .snapshot()
        .unwrap()
        .last_stop
        .expect("a stop was recorded")
}

#[test]
fn the_gust_is_a_trapezoid_in_time() {
    let g = Gust {
        peak_m_s: 8.0,
        start_s: 10.0,
        rise_s: 2.0,
        hold_s: 5.0,
        fall_s: 4.0,
    };
    for (t, want) in [
        (0.0, 0.0),
        (10.0, 0.0),
        (11.0, 4.0),
        (12.0, 8.0),
        (17.0, 8.0),
        (19.0, 4.0),
        (21.0, 0.0),
        (50.0, 0.0),
    ] {
        assert!((g.speed_at(t) - want).abs() < 1e-12, "{t}");
    }
}

#[test]
fn the_wind_force_follows_the_air_speed_relative_to_the_axis() {
    // 1/2 * 1.2 * 1.2 * 20 m2 = 14.4 N per (m/s)^2.
    let w = Wind {
        speed_m_s: 10.0,
        gust: None,
        area_m2: 20.0,
        drag_coeff: 1.2,
    };
    assert!((w.force_n(0.0, 0.0) - 1440.0).abs() < 1e-9);
    // Running with the wind feels less of it, against it more; the force keeps its sign.
    assert!((w.force_n(0.0, 2.0) - 14.4 * 64.0).abs() < 1e-9);
    assert!((w.force_n(0.0, -2.0) - 14.4 * 144.0).abs() < 1e-9);
    assert!(w.force_n(0.0, 12.0) < 0.0);
    let back = Wind {
        speed_m_s: -10.0,
        ..w
    };
    assert!((back.force_n(0.0, 0.0) + 1440.0).abs() < 1e-9);
}

#[test]
fn the_constant_wind_force_adds_to_the_wind_of_the_site() {
    let w = Wind {
        speed_m_s: 10.0,
        gust: None,
        area_m2: 20.0,
        drag_coeff: 1.2,
    };
    let l = TravelLoad::new(5000.0, 0.15, 30.0, 0.9, 0.01, 100.0, 0.005)
        .unwrap()
        .with_wind(w)
        .unwrap();
    assert!((l.wind_force_n(0.0, 0.0) - 1540.0).abs() < 1e-9);
}

#[test]
fn bad_wind_data_is_refused() {
    let l = load();
    let w = Wind {
        speed_m_s: 10.0,
        gust: None,
        area_m2: 20.0,
        drag_coeff: 1.2,
    };
    assert!(l.with_wind(Wind { area_m2: -1.0, ..w }).is_err());
    assert!(
        l.with_wind(Wind {
            speed_m_s: f64::NAN,
            ..w
        })
        .is_err()
    );
    assert!(
        l.with_wind(Wind {
            speed_m_s: 500.0,
            ..w
        })
        .is_err()
    );
    assert!(
        l.with_wind(Wind {
            area_m2: 1e300,
            ..w
        })
        .is_err()
    );
    let flat = Gust {
        peak_m_s: 5.0,
        start_s: 1.0,
        rise_s: 0.0,
        hold_s: 1.0,
        fall_s: 1.0,
    };
    assert!(
        l.with_wind(Wind {
            gust: Some(flat),
            ..w
        })
        .is_err()
    );
}

#[test]
fn the_gearbox_costs_the_motor_when_the_load_opposes_and_gives_back_when_it_pushes() {
    let eta = 0.9;
    // 1000 N of wind forward at the wheel is 5 N m ideal on the shaft.
    let l = TravelLoad::new(5000.0, 0.15, 30.0, eta, 0.0, 1000.0, 0.005).unwrap();
    // Running forward the wind pushes: the shaft sees eta * 5.
    assert!((l.torque_nm(100.0) - 5.0 * eta).abs() < 1e-9);
    // Running backward it opposes: the motor must overcome 5 / eta.
    assert!((l.torque_nm(-100.0) - 5.0 / eta).abs() < 1e-9);
    // At rest it back-drives the gearbox about halfway between the two.
    let rest = l.torque_nm(0.0);
    assert!(rest > 5.0 * eta && rest < 5.0 / eta);
}

#[test]
fn a_closed_brake_holds_the_axis_against_a_wind_the_rolling_resistance_cannot() {
    // 2000 N of wind against 490 N of rolling: free, the axis rolls away.
    let wind = TravelLoad::new(5000.0, 0.15, 30.0, 0.9, 0.01, 2000.0, 0.005).unwrap();
    let mut free = axis(wind, None, None);
    let mut held = axis(wind, Some((60.0, 0.15, 0.25)), None);
    let mut c = circuit(&[]);
    free.run(&mut c, seconds(5.0));
    held.run(&mut c, seconds(20.0));
    // Net 2000 * 0.9 - 490 / 0.9 -> 6.275 N m on 0.13 kg m2: 0.241 m/s2, 3.0 m in 5 s.
    let x = free.module("=A-TRAVEL").unwrap().position_m();
    assert!((x - 3.0).abs() < 0.15, "{x}");
    assert!(
        free.module("=A-TRAVEL")
            .unwrap()
            .findings()
            .iter()
            .any(|f| matches!(f.kind, TravelFindingKind::RollAway { .. }))
    );
    let h = held.module("=A-TRAVEL").unwrap();
    assert_eq!(h.position_m(), 0.0);
    assert!(h.findings().is_empty());
}

#[test]
fn a_wind_beyond_the_grip_moves_the_axis_even_with_the_brake_closed() {
    // Grip = 0.15 * 0.5 * 5000 * g = 3679 N; the brake cannot pass more to the rail.
    let wind = TravelLoad::new(5000.0, 0.15, 30.0, 0.9, 0.01, 6000.0, 0.005).unwrap();
    let mut l = axis(wind, Some((600.0, 0.15, 0.25)), None);
    let mut c = circuit(&[]);
    l.run(&mut c, seconds(5.0));
    assert!(l.module("=A-TRAVEL").unwrap().position_m() > 1.0);
}

/// Runs the reference axis up to a speed, opens the brake, then sets the speed by hand through
/// the drive's output. The drive stays off: the brake and the rolling resistance are all that
/// stop it.
fn coasting(brake: Option<(f64, f64, f64)>, feed: bool) -> (TravelLoop, FakeCircuit) {
    let mut l = axis(load(), brake, feed.then_some("=K-BR"));
    let mut c = circuit(&["=K-FWD", "=K-HI"]);
    if feed {
        c.energised.insert("=K-BR".into());
    }
    // 50 Hz: 0.75 m/s after the ramp.
    l.run(&mut c, seconds(30.0));
    (l, c)
}

#[test]
fn the_stop_distance_with_the_mains_lost_matches_the_closed_form() {
    // Mains lost: the drive trips and its output is gone at once and only the brake and the rolling
    // resistance stop the axis. Brake 15 N m (inside the grip of 3.7 kN: 15 / 0.005 * 0.9 = 2.7 kN), AC apply delay 0.25 s, rolling 2.725 N m.
    let (mut l, mut c) = coasting(Some((15.0, 0.05, 0.25)), true);
    let v0 = l.module("=A-TRAVEL").unwrap().snapshot().unwrap().speed_m_s;
    assert!(v0 > 0.7, "{v0}");
    c.energised.remove("=K-FWD");
    c.energised.remove("=K-HI");
    c.energised.remove("=K-MAIN");
    c.energised.remove("=K-BR");
    l.run(&mut c, seconds(20.0));
    let stop = the_stop(&l);
    assert!((stop.from_speed_m_s - v0).abs() < 0.02, "{stop:?}");
    // During the delay: decel = 2.725 / 0.13 * 0.005 = 0.1048 m/s2. Then (15 + 2.725) N m.
    let a_roll = 2.725 / 0.13 * MPR;
    let a_brake = (15.0 + 2.725) / 0.13 * MPR;
    let v1 = stop.from_speed_m_s - a_roll * 0.25;
    let want = (stop.from_speed_m_s + v1) / 2.0 * 0.25 + v1 * v1 / (2.0 * a_brake);
    assert!(
        (stop.distance_m - want).abs() < 0.05 * want,
        "{} vs {want}",
        stop.distance_m
    );
}

#[test]
fn a_brake_stops_the_axis_in_a_shorter_distance_than_coasting_does() {
    let stop_with = |brake| {
        let (mut l, mut c) = coasting(brake, brake.is_some());
        for tag in ["=K-FWD", "=K-HI", "=K-MAIN", "=K-BR"] {
            c.energised.remove(tag);
        }
        l.run(&mut c, seconds(60.0));
        the_stop(&l).distance_m
    };
    let coast = stop_with(None);
    let braked = stop_with(Some((15.0, 0.05, 0.25)));
    // 0.75 m/s at 0.105 m/s2 is 2.7 m; the brake does it in a fraction.
    assert!(coast > 2.0, "{coast}");
    assert!(braked < 0.5 * coast, "{braked} vs {coast}");
}

#[test]
fn a_slow_brake_lengthens_the_stop_by_the_speed_times_the_delay() {
    let stop_with = |apply| {
        let (mut l, mut c) = coasting(Some((15.0, 0.05, apply)), true);
        for tag in ["=K-FWD", "=K-HI", "=K-MAIN", "=K-BR"] {
            c.energised.remove(tag);
        }
        l.run(&mut c, seconds(30.0));
        the_stop(&l).distance_m
    };
    let quick = stop_with(0.05);
    let slow = stop_with(0.50);
    // About 0.75 m/s over the extra 0.45 s, less what the rolling resistance takes off.
    assert!(slow - quick > 0.28 && slow - quick < 0.36, "{slow} {quick}");
}

#[test]
fn a_brake_beyond_the_grip_skids_and_the_stop_is_capped_by_the_rail() {
    let (mut l, mut c) = coasting(Some((600.0, 0.05, 0.05)), true);
    for tag in ["=K-FWD", "=K-HI", "=K-MAIN", "=K-BR"] {
        c.energised.remove(tag);
    }
    l.run(&mut c, seconds(30.0));
    let m = l.module("=A-TRAVEL").unwrap();
    let skid = m
        .findings()
        .iter()
        .find_map(|f| match f.kind {
            TravelFindingKind::BrakeSkid {
                decel_m_s2, grip_n, ..
            } => Some((decel_m_s2, grip_n)),
            _ => None,
        })
        .expect("the skid is reported");
    assert!((skid.1 - 0.15 * 0.5 * 5000.0 * G).abs() < 1e-6);
    // The stop is the grip decel plus the rolling resistance, not 600 N m worth.
    let v0 = the_stop(&l).from_speed_m_s;
    let a_roll = 2.725 / 0.13 * MPR;
    let v1 = v0 - a_roll * 0.05;
    let a = skid.0 + a_roll;
    let want = (v0 + v1) / 2.0 * 0.05 + v1 * v1 / (2.0 * a);
    let got = the_stop(&l).distance_m;
    assert!((got - want).abs() < 0.06 * want, "{got} vs {want}");
}

#[test]
fn the_motor_dragging_against_a_late_brake_heats_it_and_is_reported() {
    // Weak brake and a 3 s release delay: the drive starts and the motor drags the brake.
    let mut l = axis(load(), Some((8.0, 3.0, 0.25)), None);
    let mut c = circuit(&["=K-FWD"]);
    l.run(&mut c, seconds(6.0));
    let m = l.module("=A-TRAVEL").unwrap();
    assert!(m.brake_energy_j() > 100.0, "{}", m.brake_energy_j());
    assert!(
        m.findings()
            .iter()
            .any(|f| matches!(f.kind, TravelFindingKind::BrakeDrag { .. }))
    );
}

#[test]
fn a_strong_brake_with_the_release_late_holds_the_axis_until_it_opens() {
    // 60 N m against a motor of about 10 N m rated: nothing moves before the release.
    let mut l = axis(load(), Some((60.0, 0.5, 0.25)), None);
    let mut c = circuit(&["=K-FWD"]);
    l.run(&mut c, seconds(0.3));
    assert_eq!(l.module("=A-TRAVEL").unwrap().position_m(), 0.0);
    l.run(&mut c, seconds(10.0));
    assert!(l.module("=A-TRAVEL").unwrap().position_m() > 0.5);
}

#[test]
fn the_drive_brake_output_is_written_to_the_bound_relay_contact() {
    let mut l = axis(load(), Some((60.0, 0.05, 0.25)), Some("=K-BR"));
    l.modules[0].binding.brake_relay = Some("=K-R2".into());
    let mut c = circuit(&["=K-FWD"]);
    l.run(&mut c, seconds(0.05));
    assert_eq!(
        c.contacts.get("=K-R2"),
        Some(&false),
        "idle: the brake stays closed"
    );
    l.run(&mut c, seconds(5.0));
    assert_eq!(
        c.contacts.get("=K-R2"),
        Some(&true),
        "above the release frequency"
    );
    c.energised.remove("=K-FWD");
    l.run(&mut c, seconds(20.0));
    assert_eq!(c.contacts.get("=K-R2"), Some(&false));
}

#[test]
fn a_brake_contactor_without_a_relay_contact_is_a_binding_finding() {
    let l = axis(load(), Some((60.0, 0.05, 0.25)), Some("=K-BR"));
    let c = circuit(&["=K-FWD", "=K-BR"]);
    let f = l.check_bindings(&c);
    assert!(
        f.iter().any(|f| f.message.contains("no relay contact")),
        "{f:?}"
    );
}

#[test]
fn a_gust_pushes_the_running_axis_and_the_snapshot_shows_the_force() {
    let gust = Gust {
        peak_m_s: 15.0,
        start_s: 10.0,
        rise_s: 2.0,
        hold_s: 20.0,
        fall_s: 2.0,
    };
    let wind = Wind {
        speed_m_s: 0.0,
        gust: Some(gust),
        area_m2: 30.0,
        drag_coeff: 1.2,
    };
    let calm = axis(load(), None, None);
    let gusty = axis(load().with_wind(wind).unwrap(), None, None);
    let (mut a, mut b) = (calm, gusty);
    let mut c = circuit(&["=K-FWD"]);
    a.run(&mut c, seconds(20.0));
    b.run(&mut c, seconds(20.0));
    let (sa, sb) = (
        a.module("=A-TRAVEL").unwrap().snapshot().unwrap().clone(),
        b.module("=A-TRAVEL").unwrap().snapshot().unwrap().clone(),
    );
    assert!(sb.wind_speed_m_s > 14.9);
    // 0.5 * 1.2 * 1.2 * 30 * (15 - v)^2 at v ~ 0.15 m/s.
    let want = 0.5 * 1.2 * 1.2 * 30.0 * (15.0 - sb.speed_m_s).powi(2);
    assert!(
        (sb.wind_force_n - want).abs() < 1.0,
        "{} vs {want}",
        sb.wind_force_n
    );
    // A 4.8 kN push on a 10 Hz axis speeds it up well past what the drive's slip allows.
    assert!(
        sb.speed_m_s > sa.speed_m_s + 0.01,
        "{} vs {}",
        sb.speed_m_s,
        sa.speed_m_s
    );
}

#[test]
fn a_run_with_wind_and_brake_is_deterministic() {
    let run = || {
        let wind = Wind {
            speed_m_s: 6.0,
            gust: Some(Gust {
                peak_m_s: 6.0,
                start_s: 3.0,
                rise_s: 1.0,
                hold_s: 2.0,
                fall_s: 1.0,
            }),
            area_m2: 25.0,
            drag_coeff: 1.3,
        };
        let mut l = axis(
            load().with_wind(wind).unwrap(),
            Some((60.0, 0.1, 0.2)),
            None,
        );
        let mut c = circuit(&["=K-FWD", "=K-HI"]);
        l.run(&mut c, seconds(15.0));
        c.energised.remove("=K-FWD");
        l.run(&mut c, seconds(10.0));
        let m = l.module("=A-TRAVEL").unwrap();
        (m.position_m().to_bits(), m.rpm().to_bits())
    };
    assert_eq!(run(), run());
}
