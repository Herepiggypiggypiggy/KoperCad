//! Drive engine tests (lane L1). Expected values are worked out by hand in the comments
//! (the independent suite in `hand_validation.rs` covers the same physics from the L7 side).

use novo_drive::drive::*;
use std::f64::consts::PI;

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: got {a}, hand value {b}, tolerance {tol}"
    );
}

fn ramp(accel: f64, decel: f64) -> Ramp {
    Ramp {
        frequency_hz: 0.0,
        accel_time_s: accel,
        decel_time_s: decel,
        reference_hz: 50.0,
    }
}

// ---- ramp safety ----------------------------------------------------------------------

#[test]
fn ramp_is_safe_against_bad_numbers() {
    // Zero time = instant step; NaN and negative never panic and never produce NaN.
    for (a, d, refh) in [
        (0.0, 0.0, 50.0),
        (-1.0, -1.0, 50.0),
        (f64::NAN, f64::NAN, 50.0),
        (5.0, 5.0, 0.0),
        (5.0, 5.0, f64::NAN),
    ] {
        let mut r = Ramp {
            frequency_hz: 0.0,
            accel_time_s: a,
            decel_time_s: d,
            reference_hz: refh,
        };
        r.step(30.0, 0.01);
        assert_eq!(r.frequency_hz, 30.0, "instant for a={a} d={d} ref={refh}");
        r.step(-10.0, 0.01);
        assert_eq!(r.frequency_hz, -10.0);
    }
    let mut r = ramp(0.0, 0.0);
    r.step(20.0, 0.0); // the NaN/clamp panic case of finding F-3
    assert_eq!(r.frequency_hz, 0.0, "dt 0 changes nothing");
    r.step(20.0, -1.0);
    r.step(20.0, f64::NAN);
    r.step(f64::NAN, 0.01);
    r.step(f64::INFINITY, 0.01);
    assert_eq!(r.frequency_hz, 0.0);
    let mut r = Ramp {
        frequency_hz: f64::NAN,
        ..ramp(5.0, 5.0)
    };
    r.step(10.0, 0.1);
    assert!(r.frequency_hz.is_finite());
}

#[test]
fn ramp_reversal_decelerates_then_accelerates() {
    // +20 -> -20 Hz, accel 5 s, decel 3 s on 50 Hz: 20/(50/3) = 1.2 s down, 20/10 = 2.0 s up.
    let mut r = Ramp {
        frequency_hz: 20.0,
        ..ramp(5.0, 3.0)
    };
    for _ in 0..120 {
        r.step(-20.0, 0.01);
    }
    close(r.frequency_hz, 0.0, 1e-9, "zero at 1.2 s");
    for _ in 0..200 {
        r.step(-20.0, 0.01);
    }
    close(r.frequency_hz, -20.0, 1e-9, "-20 Hz at 3.2 s");
    // A coarse step carries the time left across zero: one 3.2 s step does it all.
    let mut r = Ramp {
        frequency_hz: 20.0,
        ..ramp(5.0, 3.0)
    };
    r.step(-20.0, 3.2);
    close(r.frequency_hz, -20.0, 1e-9, "single coarse step");
}

#[test]
fn ramp_instant_decel_then_timed_accel_on_reversal() {
    let mut r = Ramp {
        frequency_hz: 20.0,
        ..ramp(5.0, 0.0)
    };
    r.step(-20.0, 1.0); // instant to 0, then 1 s at 10 Hz/s
    close(r.frequency_hz, -10.0, 1e-9, "decel 0 s");
}

// ---- S-ramp -----------------------------------------------------------------------------

#[test]
fn s_ramp_end_points_midpoint_and_jerk() {
    // 0 -> 50 Hz in 5 s with 1 s edges: a_max = 50/(5-1) = 12.5 Hz/s, jerk 12.5 Hz/s2,
    // 25 Hz at 2.5 s, slope 0 at both ends. The trapezoid is exact in the continuous case; the
    // 1 ms sampling keeps it within 0.01 Hz.
    let mut s = SRamp::new(5.0, 5.0, 50.0, 0.2);
    let dt = 0.001;
    let mut max_jerk: f64 = 0.0;
    let mut prev_a = 0.0;
    let mut mid = 0.0;
    let mut t_end = None;
    for k in 1..=7000 {
        s.step(50.0, dt);
        max_jerk = max_jerk.max(((s.accel_hz_s - prev_a) / dt).abs());
        prev_a = s.accel_hz_s;
        if k == 2500 {
            mid = s.frequency_hz;
        }
        if t_end.is_none() && s.frequency_hz == 50.0 {
            t_end = Some(f64::from(k) * dt);
        }
    }
    close(mid, 25.0, 0.01, "midpoint");
    close(t_end.unwrap(), 5.0, 0.01, "arrival time");
    close(s.frequency_hz, 50.0, 0.0, "stays");
    close(s.accel_hz_s, 0.0, 0.0, "end slope");
    assert!(max_jerk <= 12.5 + 1e-6, "jerk {max_jerk}");
    assert!(max_jerk >= 12.4, "jerk is actually used: {max_jerk}");
}

#[test]
fn s_ramp_never_overshoots_and_reverses_through_zero() {
    let mut s = SRamp::new(4.0, 2.0, 50.0, 0.25);
    let mut peak: f64 = 0.0;
    for _ in 0..20000 {
        s.step(30.0, 0.001);
        peak = peak.max(s.frequency_hz);
    }
    assert!(peak <= 30.0 + 1e-12, "overshoot {peak}");
    for _ in 0..30000 {
        s.step(-30.0, 0.001);
    }
    close(s.frequency_hz, -30.0, 1e-9, "reversed");
    // Bad numbers are safe too.
    let mut b = SRamp::new(0.0, f64::NAN, 50.0, 0.3);
    b.step(10.0, 0.001);
    assert_eq!(b.frequency_hz, 10.0);
    b.step(f64::NAN, 0.001);
    b.step(5.0, -1.0);
    assert_eq!(b.frequency_hz, 10.0);
}

// ---- V/f --------------------------------------------------------------------------------

#[test]
fn vf_law_values_and_flux() {
    let v = VfCurve::new(400.0, 50.0);
    for (f, volts) in [
        (0.0, 0.0),
        (10.0, 80.0),
        (25.0, 200.0),
        (50.0, 400.0),
        (87.0, 400.0),
        (-25.0, 200.0),
    ] {
        close(v.volts_for_frequency(f), volts, 1e-9, &format!("{f} Hz"));
    }
    close(v.flux_ratio(25.0), 1.0, 1e-12, "flux below base");
    close(v.flux_ratio(87.0), 50.0 / 87.0, 1e-12, "flux above base");
    close(
        v.torque_capability_ratio(87.0),
        0.330_294_6,
        1e-6,
        "capability above base",
    );
    // Boost: 20 V at 0 Hz fading to nothing at 25 Hz; nothing at and above the end frequency.
    let b = VfCurve { boost_v: 20.0, ..v };
    close(b.volts_for_frequency(0.0), 20.0, 1e-12, "boost at 0 Hz");
    close(
        b.volts_for_frequency(10.0),
        80.0 + 20.0 * 0.6,
        1e-12,
        "boost at 10 Hz",
    );
    close(
        b.volts_for_frequency(25.0),
        200.0,
        1e-12,
        "boost gone at 25 Hz",
    );
    close(b.volts_for_frequency(60.0), 400.0, 1e-12, "capped at rated");
    // Quadratic: 25 Hz is a quarter of rated.
    let q = VfCurve {
        shape: VfShape::Quadratic,
        ..v
    };
    close(q.volts_for_frequency(25.0), 100.0, 1e-12, "quadratic");
    close(
        VfCurve::new(400.0, 0.0).volts_for_frequency(10.0),
        0.0,
        0.0,
        "bad base",
    );
}

// ---- braking resistor and sizing --------------------------------------------------------

#[test]
fn braking_resistor_flags() {
    // J 2 kg m2, 1500 rpm, stop in 4 s, one stop per 60 s: E = 24674 J, average 6168.5 W,
    // peak 12337 W, duty average 411.2 W. Bus threshold 780 V.
    let duty = BrakingDuty {
        inertia_kg_m2: 2.0,
        speed_rpm: 1500.0,
        stop_time_s: 4.0,
        cycle_time_s: 60.0,
    };
    let r47 = BrakingResistor {
        ohm: 47.0,
        continuous_w: 1000.0,
        thermal_capacity_j: 0.0,
    };
    let c = check_braking_resistor(&duty, &r47, 780.0, 40.0);
    close(c.energy_j, 24_674.011, 1e-3, "energy");
    close(c.peak_w, 12_337.006, 1e-3, "peak");
    close(c.duty_average_w, 411.234, 1e-3, "duty");
    close(c.capacity_w, 12_944.681, 1e-3, "V2/R");
    assert!(c.flags.is_empty(), "{:?}", c.flags);
    let r100 = BrakingResistor { ohm: 100.0, ..r47 };
    assert_eq!(
        check_braking_resistor(&duty, &r100, 780.0, 40.0).flags,
        vec![ResistorFlag::PeakExceedsCapacity]
    );
    let weak = BrakingResistor {
        continuous_w: 300.0,
        ..r47
    };
    assert_eq!(
        check_braking_resistor(&duty, &weak, 780.0, 40.0).flags,
        vec![ResistorFlag::DutyExceedsContinuous]
    );
    let low = BrakingResistor { ohm: 30.0, ..r47 };
    assert!(
        check_braking_resistor(&duty, &low, 780.0, 40.0)
            .flags
            .contains(&ResistorFlag::BelowMinimumOhm)
    );
    let small = BrakingResistor {
        thermal_capacity_j: 10_000.0,
        ..r47
    };
    assert!(
        check_braking_resistor(&duty, &small, 780.0, 40.0)
            .flags
            .contains(&ResistorFlag::EnergyExceedsThermalCapacity)
    );
}

#[test]
fn sizing_rule_two_times_sum_of_motor_currents() {
    // Three motors of 6.5, 6.5 and 4.2 A with factor 2: 2 x 17.2 = 34.4 A, above a 33 A boundary.
    let c = check_drive_sizing(32.0, &[6.5, 6.5, 4.2], 2.0, 33.0);
    close(c.required_a, 34.4, 1e-12, "required");
    assert!(!c.ok);
    assert_eq!(c.class, DriveClass::Process);
    let c = check_drive_sizing(16.0, &[3.0, 3.0], 2.0, 33.0);
    assert!(c.ok && c.class == DriveClass::Compact);
    close(
        required_drive_current_a(&[f64::NAN, -1.0, 2.0], 2.0),
        4.0,
        1e-12,
        "bad values ignored",
    );
}

// ---- I2t ---------------------------------------------------------------------------------

fn time_to_trip(ratio: f64) -> Option<f64> {
    let mut o = I2tOverload::new(10.0, 1.5, 60.0);
    let dt = 0.01;
    for k in 1..=200_000 {
        if o.step(10.0 * ratio, dt) {
            return Some(f64::from(k) * dt);
        }
    }
    None
}

#[test]
fn i2t_trip_times() {
    // K = (1.5^2 - 1) * 60 = 75; 150 % trips at 60 s (the entered point), 200 % at 75/3 = 25 s,
    // 100 % never.
    close(time_to_trip(1.5).unwrap(), 60.0, 0.011, "150 %");
    close(time_to_trip(2.0).unwrap(), 25.0, 0.011, "200 %");
    assert!(time_to_trip(1.0).is_none());
    assert!(time_to_trip(0.5).is_none());
    let o = I2tOverload::new(10.0, 1.5, 60.0);
    close(
        o.time_to_trip_s(1.2).unwrap(),
        170.454_545,
        1e-5,
        "120 % model value",
    );
    assert!(o.time_to_trip_s(1.0).is_none());
}

#[test]
fn invalid_inputs_are_flagged_not_silent() {
    let duty = BrakingDuty {
        inertia_kg_m2: 2.0,
        speed_rpm: 1500.0,
        stop_time_s: 4.0,
        cycle_time_s: 60.0,
    };
    for ohm in [0.0, -5.0, f64::NAN] {
        let r = BrakingResistor {
            ohm,
            continuous_w: 1000.0,
            thermal_capacity_j: 0.0,
        };
        let c = check_braking_resistor(&duty, &r, 780.0, 40.0);
        assert!(c.flags.contains(&ResistorFlag::InvalidResistance), "{ohm}");
        assert!(c.capacity_w.is_finite());
    }
    assert!(I2tOverload::new(10.0, 1.5, 60.0).is_valid());
    assert!(!I2tOverload::new(10.0, 1.0, 60.0).is_valid());
    assert!(!I2tOverload::new(10.0, 1.5, 0.0).is_valid());
    assert!(!I2tOverload::new(0.0, 1.5, 60.0).is_valid());
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    assert!(Drive::new(cfg.clone()).config_problems().is_empty());
    cfg.motor_overload = (1.0, 60.0);
    assert_eq!(Drive::new(cfg).config_problems().len(), 1);
}

#[test]
fn i2t_cools_below_rated_current() {
    let mut o = I2tOverload::new(10.0, 1.5, 60.0);
    for _ in 0..3000 {
        o.step(15.0, 0.01); // 30 s at 150 %: half of K
    }
    close(o.used(), 0.5, 1e-9, "half used");
    for _ in 0..100 {
        o.step(0.0, 0.1); // 10 s at zero current gives back 10 s of (0 - 1)
    }
    close(o.accumulated, 37.5 - 10.0, 1e-9, "cooled");
}

// ---- DC bus energy balance (property) ---------------------------------------------------

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.next()
    }
}

#[test]
fn bus_energy_balance_over_random_runs() {
    let mut rng = Lcg(42);
    for run in 0..200 {
        let mut cfg = BusConfig::generic_400v();
        cfg.capacitance_f = rng.range(0.4e-3, 3.0e-3);
        cfg.resistor_ohm = if rng.next() > 0.3 {
            Some(rng.range(30.0, 120.0))
        } else {
            None
        };
        cfg.regen_to_mains = rng.next() > 0.8;
        cfg.inverter_efficiency = rng.range(0.9, 0.99);
        let mut bus = DcBus::new(cfg);
        let e0 = bus.stored_j();
        let dt = rng.range(0.0005, 0.005);
        let mut p = 0.0;
        for k in 0..4000 {
            if k % 200 == 0 {
                p = rng.range(-30_000.0, 15_000.0);
            }
            bus.step(p, rng.next() > 0.01, dt);
        }
        let resid = bus.balance_residual_j(e0);
        let scale = bus.book.mains_in + bus.book.to_load.abs() + bus.book.resistor + 1.0;
        assert!(
            resid.abs() <= 1e-6 * scale,
            "run {run}: residual {resid} of {scale}"
        );
    }
}

#[test]
fn bus_chopper_holds_voltage_and_resistor_heats() {
    // A regenerating load of 8 kW with a 47 ohm resistor: the bus rises to the chopper level and
    // is held there; the resistor takes about the regen power and its thermal model fills.
    let mut cfg = BusConfig::generic_400v();
    cfg.resistor_ohm = Some(47.0);
    cfg.resistor_continuous_w = 500.0;
    cfg.resistor_thermal_j = 20_000.0;
    let mut bus = DcBus::new(cfg);
    let mut vmax: f64 = 0.0;
    for _ in 0..20_000 {
        let s = bus.step(-8000.0, true, 0.001);
        vmax = vmax.max(s.volts);
    }
    assert!(vmax < 800.0, "chopper limits the bus: {vmax}");
    assert!(bus.chopper_on || bus.volts() < 780.0);
    assert!(
        bus.resistor_overloaded(),
        "7 kW into a 500 W / 20 kJ resistor must overload in 20 s"
    );
    // Without a resistor the same regeneration drives the bus up beyond the trip level.
    let mut bare = DcBus::new(BusConfig::generic_400v());
    for _ in 0..2000 {
        bare.step(-8000.0, true, 0.001);
    }
    assert!(bare.volts() > BusConfig::generic_400v().over_voltage_v);
}

// ---- drive state machine ----------------------------------------------------------------

fn nameplate() -> MotorNameplate {
    // Generic 1.5 kW 4-pole: 400 V, 3.6 A, 50 Hz, 1430 rpm, pf 0.8.
    MotorNameplate {
        voltage_v: 400.0,
        current_a: 3.6,
        frequency_hz: 50.0,
        rpm: 1430.0,
        power_kw: 1.5,
        power_factor: 0.8,
    }
}

fn travel_drive() -> Drive {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro::two_wire_two_speed();
    cfg.accel_time_s = 4.0;
    cfg.decel_time_s = 2.0;
    cfg.presets_hz = [0.0, 10.0, 50.0, 25.0];
    cfg.reference = ReferenceSource::Panel(10.0);
    Drive::new(cfg)
}

fn di(fwd: bool, rev: bool, fast: bool) -> DriveInputs {
    DriveInputs {
        di: vec![fwd, rev, fast, true],
        ..Default::default()
    }
}

fn run_for(d: &mut Drive, inp: &DriveInputs, secs: f64) -> DriveOutputs {
    let n = (secs / 0.001).round() as usize;
    let mut out = d.step(inp, 0.001);
    for _ in 1..n {
        out = d.step(inp, 0.001);
    }
    out
}

#[test]
fn radio_step_one_runs_low_then_step_two_runs_high() {
    let mut d = travel_drive();
    // Step 1: direction only = main reference 10 Hz. Ramp 0 -> 10 Hz on a 4 s / 50 Hz slope
    // (12.5 Hz/s) takes 0.8 s.
    let o = run_for(&mut d, &di(true, false, false), 0.4);
    close(o.frequency_hz, 5.0, 1e-6, "0.4 s into step 1");
    close(o.voltage_v, 40.0, 1e-6, "V/f: 5 Hz is 40 V");
    assert_eq!(o.state, DriveState::Running);
    let o = run_for(&mut d, &di(true, false, false), 1.0);
    close(o.frequency_hz, 10.0, 1e-9, "step 1 reached");
    assert!(o.at_reference);
    // Step 2: DI3 selects preset bit 1 = preset 2 = 50 Hz. 40 Hz at 12.5 Hz/s = 3.2 s.
    let o = run_for(&mut d, &di(true, false, true), 1.6);
    close(o.frequency_hz, 10.0 + 12.5 * 1.6, 1e-6, "halfway up");
    let o = run_for(&mut d, &di(true, false, true), 2.0);
    close(o.frequency_hz, 50.0, 1e-9, "step 2 reached");
    close(o.voltage_v, 400.0, 1e-9, "rated volts");
    // Back to step 1: decel 40 Hz at 25 Hz/s = 1.6 s.
    let o = run_for(&mut d, &di(true, false, false), 1.6);
    close(o.frequency_hz, 10.0, 1e-9, "back to step 1");
    // Release: 10 Hz at 25 Hz/s = 0.4 s, then stopped.
    let o = run_for(&mut d, &di(false, false, false), 0.5);
    assert_eq!(o.state, DriveState::Stopped);
    assert_eq!(o.frequency_hz, 0.0);
    assert!(!o.running);
}

#[test]
fn reversal_through_zero_and_both_directions_at_once() {
    let mut d = travel_drive();
    run_for(&mut d, &di(true, false, true), 5.0); // 50 Hz forward
    // Reverse direction at step 2: decel 50 -> 0 in 2 s, then accel 0 -> -50 in 4 s.
    let o = run_for(&mut d, &di(false, true, true), 2.0);
    close(o.frequency_hz, 0.0, 1e-6, "through zero");
    let o = run_for(&mut d, &di(false, true, true), 2.0);
    close(o.frequency_hz, -25.0, 1e-6, "reverse halfway");
    // Forward and reverse together is no command: stop.
    let o = run_for(&mut d, &di(true, true, true), 3.0);
    assert_eq!(o.state, DriveState::Stopped);
}

#[test]
fn min_max_and_skip_frequency() {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro::two_wire_presets();
    cfg.min_hz = 5.0;
    cfg.max_hz = 40.0;
    cfg.skip_bands = vec![(18.0, 22.0)];
    cfg.accel_time_s = 1.0;
    cfg.presets_hz = [0.0, 2.0, 60.0, 19.0];
    let mut d = Drive::new(cfg);
    let mk = |p0: bool, p1: bool| DriveInputs {
        di: vec![true, false, p0, p1],
        ..Default::default()
    };
    let o = run_for(&mut d, &mk(true, false), 3.0);
    close(o.frequency_hz, 5.0, 1e-9, "min frequency lifts 2 Hz to 5");
    let o = run_for(&mut d, &mk(false, true), 3.0);
    close(o.frequency_hz, 40.0, 1e-9, "max frequency caps 60 Hz");
    let o = run_for(&mut d, &mk(true, true), 3.0);
    close(
        o.frequency_hz,
        18.0,
        1e-9,
        "19 Hz sits in the skip band: nearer edge is 18",
    );
}

#[test]
fn analog_references_scale_and_4_20_ma_loss_trips() {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro::run_and_direction();
    cfg.accel_time_s = 0.5;
    cfg.reference = ReferenceSource::AnalogVoltage {
        min_v: 0.0,
        max_v: 10.0,
        min_hz: 0.0,
        max_hz: 50.0,
    };
    let mut d = Drive::new(cfg.clone());
    let inp = DriveInputs {
        di: vec![true, false, false],
        analog: 4.0,
        ..Default::default()
    };
    close(
        run_for(&mut d, &inp, 2.0).frequency_hz,
        20.0,
        1e-9,
        "4 V of 10 V is 20 Hz",
    );
    cfg.reference = ReferenceSource::AnalogCurrent {
        min_ma: 4.0,
        max_ma: 20.0,
        min_hz: 0.0,
        max_hz: 50.0,
        loss_ma: Some(3.0),
    };
    let mut d = Drive::new(cfg);
    let inp = DriveInputs {
        di: vec![true, false, false],
        analog: 12.0,
        ..Default::default()
    };
    close(
        run_for(&mut d, &inp, 3.0).frequency_hz,
        25.0,
        1e-9,
        "12 mA is half scale",
    );
    let lost = DriveInputs {
        di: vec![true, false, false],
        analog: 0.5,
        ..Default::default()
    };
    let o = run_for(&mut d, &lost, 0.01);
    assert_eq!(o.fault, Some(Fault::ReferenceLost));
    assert!(!o.fault_relay_ok);
}

#[test]
fn fast_stop_uses_its_own_time_and_enable_blocks_start() {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro {
        name: "test".into(),
        inputs: vec![
            DigitalInput::new(TerminalFunction::Forward),
            DigitalInput::nc(TerminalFunction::FastStop),
            DigitalInput::new(TerminalFunction::Enable),
        ],
    };
    cfg.accel_time_s = 1.0;
    cfg.decel_time_s = 10.0;
    cfg.fast_stop_time_s = 0.5;
    let mut d = Drive::new(cfg);
    let on = DriveInputs {
        di: vec![true, true, true],
        ..Default::default()
    };
    let o = run_for(&mut d, &on, 1.5);
    close(o.frequency_hz, 50.0, 1e-9, "at speed");
    // NC fast-stop input de-energised: 50 Hz in 0.5 s -> after 0.25 s it is 25 Hz.
    let stop = DriveInputs {
        di: vec![true, false, true],
        ..Default::default()
    };
    let o = run_for(&mut d, &stop, 0.25);
    close(o.frequency_hz, 25.0, 1e-6, "fast stop slope");
    let o = run_for(&mut d, &stop, 0.5);
    assert_eq!(o.state, DriveState::Stopped);
    // Enable missing: will not start.
    let no_en = DriveInputs {
        di: vec![true, true, false],
        ..Default::default()
    };
    let o = run_for(&mut d, &no_en, 0.5);
    assert_eq!(o.state, DriveState::Stopped);
    assert_eq!(o.frequency_hz, 0.0);
}

#[test]
fn external_fault_latches_until_reset_edge_with_cause_gone() {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro {
        name: "test".into(),
        inputs: vec![
            DigitalInput::new(TerminalFunction::Forward),
            DigitalInput::nc(TerminalFunction::ExternalFault),
            DigitalInput::new(TerminalFunction::FaultReset),
        ],
    };
    cfg.accel_time_s = 1.0;
    let mut d = Drive::new(cfg);
    let ok = DriveInputs {
        di: vec![true, true, false],
        ..Default::default()
    };
    run_for(&mut d, &ok, 1.0);
    let bad = DriveInputs {
        di: vec![true, false, false],
        ..Default::default()
    };
    let o = run_for(&mut d, &bad, 0.01);
    assert_eq!(o.fault, Some(Fault::External));
    assert_eq!(o.frequency_hz, 0.0);
    assert_eq!(o.state, DriveState::Faulted);
    // Cause gone but no reset: stays tripped even with the run command held.
    assert_eq!(run_for(&mut d, &ok, 0.5).state, DriveState::Faulted);
    // Reset while the cause is still there: refused.
    let reset_bad = DriveInputs {
        di: vec![false, false, true],
        ..Default::default()
    };
    assert_eq!(run_for(&mut d, &reset_bad, 0.1).state, DriveState::Faulted);
    // Reset edge with the cause gone: ready again.
    let off = DriveInputs {
        di: vec![false, true, false],
        ..Default::default()
    };
    run_for(&mut d, &off, 0.1);
    let reset = DriveInputs {
        di: vec![false, true, true],
        ..Default::default()
    };
    let o = run_for(&mut d, &reset, 0.01);
    assert_eq!(o.state, DriveState::Stopped);
    assert!(o.fault_relay_ok);
}

#[test]
fn overload_current_and_phase_loss_trip() {
    // Motor 3.6 A: 150 % for 60 s = K 75 -> at 5.4 A trips after 60 s.
    let mut d = travel_drive();
    let mut inp = di(true, false, true);
    inp.motor_current_a = 5.4;
    let mut tripped_at = None;
    for k in 0..70_000 {
        let o = d.step(&inp, 0.001);
        if o.fault == Some(Fault::MotorOverload) {
            tripped_at = Some(f64::from(k + 1) * 0.001);
            break;
        }
    }
    close(
        tripped_at.expect("motor overload trips"),
        60.0,
        0.01,
        "motor I2t at 150 %",
    );
    // Hardware over-current is immediate.
    let mut d = travel_drive();
    run_for(&mut d, &di(true, false, true), 0.1);
    let mut inp = di(true, false, true);
    inp.motor_current_a = 16.0; // above 2.2 x 7 A
    assert_eq!(d.step(&inp, 0.001).fault, Some(Fault::OverCurrent));
    // Phase loss while running.
    let mut d = travel_drive();
    run_for(&mut d, &di(true, false, false), 0.1);
    let mut inp = di(true, false, false);
    inp.mains_ok = false;
    let o = d.step(&inp, 0.001);
    assert_eq!(o.fault, Some(Fault::MainsPhaseLoss));
    // Output short from the load side.
    let mut d = travel_drive();
    let mut inp = di(false, false, false);
    inp.output_short = true;
    assert_eq!(d.step(&inp, 0.001).fault, Some(Fault::OutputShort));
}

#[test]
fn regeneration_without_resistor_trips_with_resistor_does_not() {
    // Fast stop of a high-inertia load: the motor feeds 6 kW back for several seconds.
    let regen = |resistor: bool| {
        let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
        cfg.macro_ = TerminalMacro::two_wire_presets();
        cfg.accel_time_s = 0.1;
        cfg.decel_time_s = 4.0;
        if resistor {
            cfg.bus.resistor_ohm = Some(60.0);
            cfg.bus.resistor_continuous_w = 3000.0;
            cfg.bus.resistor_thermal_j = 200_000.0;
        }
        let mut d = Drive::new(cfg);
        let run = DriveInputs {
            di: vec![true, false, false, false],
            ..Default::default()
        };
        run_for(&mut d, &run, 1.0);
        let stop = DriveInputs {
            di: vec![false, false, false, false],
            motor_power_w: -6000.0,
            ..Default::default()
        };
        let mut fault = None;
        for _ in 0..4000 {
            let o = d.step(&stop, 0.001);
            fault = fault.or(o.fault);
        }
        (fault, d.bus.volts())
    };
    assert_eq!(regen(false).0, Some(Fault::BusOverVoltage));
    let (fault, v) = regen(true);
    assert_eq!(fault, None);
    assert!(v < 800.0, "{v}");
}

#[test]
fn ramp_stretch_holds_decel_while_the_bus_is_high() {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro::two_wire_presets();
    cfg.accel_time_s = 0.1;
    cfg.decel_time_s = 1.0;
    cfg.ramp_stretch_v = 700.0;
    let mut d = Drive::new(cfg);
    run_for(
        &mut d,
        &DriveInputs {
            di: vec![true, false, false, false],
            ..Default::default()
        },
        1.0,
    );
    // Heavy regeneration lifts the bus above 700 V; the ramp then waits instead of tripping.
    let stop = DriveInputs {
        di: vec![false, false, false, false],
        motor_power_w: -20_000.0,
        ..Default::default()
    };
    let mut held = false;
    let mut last = 50.0;
    for _ in 0..300 {
        let o = d.step(&stop, 0.001);
        if o.bus_v >= 700.0 && (o.frequency_hz - last).abs() < 1e-12 {
            held = true;
        }
        last = o.frequency_hz;
    }
    assert!(held, "decel was held at least once while the bus was high");
}

// ---- current limit ----------------------------------------------------------------------

#[test]
fn vector_current_limit_gives_the_accel_time_by_hand() {
    // Choose a nameplate with pf 1 so the flux current is 0 and torque is proportional to
    // current: 20 N m at 4 A (P = 20 N m * 100 rad/s... use rpm 954.93 -> w = 100 rad/s,
    // P = 2 kW). Current limit 150 % = 6 A -> torque limit 30 N m.
    let m = MotorNameplate {
        voltage_v: 400.0,
        current_a: 4.0,
        frequency_hz: 50.0,
        rpm: 30.0 * 100.0 / PI,
        power_kw: 2.0,
        power_factor: 1.0,
    };
    close(m.rated_torque_nm(), 20.0, 1e-9, "rated torque");
    let mut cfg = DriveConfig::generic_travel(m, 6.0);
    cfg.mode = ControlMode::SensorlessVector;
    cfg.current_limit_a = 6.0;
    cfg.macro_ = TerminalMacro::two_wire_presets();
    cfg.accel_time_s = 0.01;
    let mut d = Drive::new(cfg);
    let o = d.step(
        &DriveInputs {
            di: vec![true, false, false, false],
            ..Default::default()
        },
        0.001,
    );
    close(o.torque_limit_nm, 30.0, 1e-9, "torque limit");
    // Plant: J 0.5 kg m2, accelerating to 1500 rpm = 157.08 rad/s at the torque limit:
    // t = J w / T = 0.5 * 157.0796 / 30 = 2.618 s. Then with a 10 N m load: 3.927 s.
    for (load, expect) in [(0.0, 2.617_994), (10.0, 3.926_991)] {
        let w_target = 2.0 * PI * 1500.0 / 60.0;
        let (mut w, mut t) = (0.0, 0.0);
        let dt = 0.001;
        while w < w_target {
            let torque = d
                .step(
                    &DriveInputs {
                        di: vec![true, false, false, false],
                        ..Default::default()
                    },
                    dt,
                )
                .torque_limit_nm;
            w += (torque - load) / 0.5 * dt;
            t += dt;
        }
        close(t, expect, 2.0 * dt, &format!("accel time with load {load}"));
    }
}

#[test]
fn volts_per_hertz_current_limit_holds_the_ramp() {
    let mut cfg = DriveConfig::generic_travel(nameplate(), 7.0);
    cfg.macro_ = TerminalMacro::two_wire_presets();
    cfg.accel_time_s = 5.0;
    cfg.current_limit_a = 8.0;
    let mut d = Drive::new(cfg);
    let free = DriveInputs {
        di: vec![true, false, false, false],
        motor_current_a: 4.0,
        ..Default::default()
    };
    let o = run_for(&mut d, &free, 1.0);
    close(o.frequency_hz, 10.0, 1e-6, "10 Hz/s while under the limit");
    assert!(!o.current_limit_active);
    let hot = DriveInputs {
        motor_current_a: 8.5,
        ..free.clone()
    };
    let o = run_for(&mut d, &hot, 1.0);
    close(o.frequency_hz, 10.0, 1e-6, "ramp held above the limit");
    assert!(o.current_limit_active);
    let o = run_for(&mut d, &free, 1.0);
    close(o.frequency_hz, 20.0, 1e-6, "continues when current falls");
}

#[test]
fn drive_input_current_and_logic_outputs() {
    let mut d = travel_drive();
    let mut inp = di(true, false, true);
    inp.motor_power_w = 1500.0;
    let o = run_for(&mut d, &inp, 5.0);
    assert!(o.brake_release && o.running && o.fault_relay_ok);
    assert!(
        o.mains_current_a > 0.0 && o.mains_current_a < 5.0,
        "{}",
        o.mains_current_a
    );
}
