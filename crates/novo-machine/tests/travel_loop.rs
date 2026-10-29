//! Loop checks for lane L5a part 3: a drive, its motor and a wheeled load, with a scripted
//! circuit. Expected values are worked out on paper in the comments.

use novo_drive::drive::ReferenceSource;
use novo_drive::drive::{Drive, DriveConfig, MotorNameplate, TerminalMacro};
use novo_drive::induction::{FitOptions, InductionMotor};
use novo_machine::bindings::SensorBinding;
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::{LimitKind, LimitSwitch};
use novo_machine::port::FakeCircuit;
use novo_machine::travel_loop::{TravelBinding, TravelLoad, TravelLoop, TravelModule};

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
