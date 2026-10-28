//! Independent hand-calculation validation of the drive, motor, mechanics and brake models
//! (lane L7, cases 1-10, 13 and 14 of `docs/design/drive-crane-validation.md`).
//!
//! Rules of this file:
//! - The expected numbers are worked out here, by hand, from the physics. They are never
//!   produced by calling the engine under test.
//! - A test with no ignore marker runs against the engine as merged today.
//! - A test marked `#[ignore = "Lx: ..."]` waits for the named lane; the body is the oracle and a
//!   `todo!` naming the API the lane has to offer. The lane un-ignores it and replaces the `todo!`.
//! - `oracle_*` tests check the hand arithmetic itself so that a typo here cannot hide an engine
//!   error.
//!
//! Values are generic (no manufacturer data, no customer data).

use novo_drive::{
    Fidelity, GRAVITY,
    drive::Ramp,
    mechanics::hoist_holding_torque_nm,
    motor::{SYNC_RPM_FIDELITY, sync_rpm},
};
use std::f64::consts::PI;

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: got {a}, hand value {b}, tolerance {tol}"
    );
}

/// Shared hoist data used by cases 5, 6, 13, 14 (generic values).
mod hoist {
    pub const MASS_KG: f64 = 5000.0;
    pub const DRUM_R_M: f64 = 0.2;
    pub const RATIO: f64 = 31.5;
    pub const ETA: f64 = 0.9;
    /// Motor rotor inertia, kg m2.
    pub const J_MOTOR: f64 = 0.5;
    /// Linear speed per motor rad/s: `r / i` = 0.2 / 31.5 m per rad.
    pub const M_PER_RAD: f64 = DRUM_R_M / RATIO;
    /// Payload inertia reflected to the motor shaft: `m (r/i)^2` = 5000 / 157.5^2 = 0.20156.
    pub fn j_payload() -> f64 {
        MASS_KG * M_PER_RAD * M_PER_RAD
    }
    /// Total inertia on the motor shaft = 0.70156 kg m2.
    pub fn j_total() -> f64 {
        J_MOTOR + j_payload()
    }
    /// Gravity torque on the motor shaft with efficiency counted against hoisting.
    pub fn gravity_torque_hoisting() -> f64 {
        MASS_KG * super::GRAVITY * DRUM_R_M / (RATIO * ETA)
    }
    /// Static torque the brake must hold (efficiency not credited, conservative) = 311.32 N m.
    pub fn holding_torque_static() -> f64 {
        MASS_KG * super::GRAVITY * DRUM_R_M / RATIO
    }
}

// ---------------------------------------------------------------- case 1: synchronous speed

#[test]
fn case01_sync_rpm_table() {
    // n = 120 f / p. Rows poles 2, 4, 6, 8; columns 10, 25, 50, 87 Hz.
    let hz = [10.0, 25.0, 50.0, 87.0];
    let table: [(u32, [f64; 4]); 4] = [
        (2, [600.0, 1500.0, 3000.0, 5220.0]),
        (4, [300.0, 750.0, 1500.0, 2610.0]),
        (6, [200.0, 500.0, 1000.0, 1740.0]),
        (8, [150.0, 375.0, 750.0, 1305.0]),
    ];
    for (poles, row) in table {
        for (f, expected) in hz.iter().zip(row) {
            close(
                sync_rpm(*f, poles),
                expected,
                1e-9,
                &format!("{poles} poles at {f} Hz"),
            );
        }
    }
    assert_eq!(SYNC_RPM_FIDELITY, Fidelity::Exact);
}

#[test]
fn case01_pole_change_pair_speeds() {
    // 12 pole (fine) and 2 pole (coarse) windings on 50 Hz: 500 and 3000 rpm.
    close(sync_rpm(50.0, 12), 500.0, 1e-9, "fine");
    close(sync_rpm(50.0, 2), 3000.0, 1e-9, "coarse");
}

#[test]
#[ignore = "L2b: motor at frequency must offer rated_rpm_at(frequency_hz) (constant slip speed convention)"]
fn case01_slip_speed_constant_in_constant_flux_region() {
    // Model convention (neglects the stator drop, so it is least exact at 5-10 Hz).
    // Nameplate 4 pole, 50 Hz, 1450 rpm: slip speed 1500 - 1450 = 50 rpm. At constant V/f and
    // constant torque the slip speed is constant (slip frequency fixed by torque / flux^2), so
    // at 25 Hz: 750 - 50 = 700 rpm. Scaling the slip FRACTION (3.33 %) would give 725 rpm and
    // is the common mistake. At 10 Hz: 300 - 50 = 250 rpm.
    let expected = [(10.0, 250.0), (25.0, 700.0), (50.0, 1450.0)];
    assert_eq!(expected.len(), 3);
    todo!("L2b: compare engine rated-torque rpm at each frequency with the hand values");
}

// ---------------------------------------------------------------- case 2: ramps

#[test]
fn case02_linear_ramp_points() {
    // 0 -> 50 Hz in 5 s is 10 Hz/s. At 2.5 s: 25 Hz. At 5 s: 50 Hz, then it stays.
    let mut r = Ramp {
        frequency_hz: 0.0,
        accel_time_s: 5.0,
        decel_time_s: 3.0,
        reference_hz: 50.0,
    };
    for _ in 0..250 {
        r.step(50.0, 0.01);
    }
    close(r.frequency_hz, 25.0, 1e-9, "2.5 s");
    for _ in 0..250 {
        r.step(50.0, 0.01);
    }
    close(r.frequency_hz, 50.0, 1e-9, "5 s");
    for _ in 0..100 {
        r.step(50.0, 0.01);
    }
    close(r.frequency_hz, 50.0, 1e-12, "no overshoot after 6 s");
}

#[test]
fn case02_decel_uses_its_own_time() {
    // Decel 50 -> 0 in 3 s is 16.667 Hz/s: after 1 s it is 50 - 16.667 = 33.333 Hz.
    let mut r = Ramp {
        frequency_hz: 50.0,
        accel_time_s: 5.0,
        decel_time_s: 3.0,
        reference_hz: 50.0,
    };
    for _ in 0..100 {
        r.step(0.0, 0.01);
    }
    close(r.frequency_hz, 100.0 / 3.0, 1e-9, "1 s of decel");
}

#[test]
fn case02_ramp_between_two_set_points_uses_reference_slope() {
    // The times refer to the reference frequency: 10 -> 20 Hz is 10 Hz of a 50 Hz span, so
    // 10 Hz / (10 Hz/s) = 1 s, not 5 s.
    let mut r = Ramp {
        frequency_hz: 10.0,
        accel_time_s: 5.0,
        decel_time_s: 5.0,
        reference_hz: 50.0,
    };
    for _ in 0..100 {
        r.step(20.0, 0.01);
    }
    close(r.frequency_hz, 20.0, 1e-9, "10 to 20 Hz in 1 s");
}

#[test]
fn case02_ramp_does_not_overshoot_with_a_coarse_step() {
    // A 0.3 s step is 3 Hz at 10 Hz/s; target 2 Hz must be hit exactly, not passed.
    let mut r = Ramp {
        frequency_hz: 0.0,
        accel_time_s: 5.0,
        decel_time_s: 5.0,
        reference_hz: 50.0,
    };
    r.step(2.0, 0.3);
    close(r.frequency_hz, 2.0, 1e-12, "clamped to target");
}

#[test]
#[ignore = "L1 finding F-1: Ramp::step uses the accel rate for a reversal through zero (see docs/design/drive-crane-validation.md)"]
fn case02_reversal_decelerates_then_accelerates() {
    // +20 Hz -> -20 Hz with accel 5 s and decel 3 s on a 50 Hz reference.
    // Stage 1 decelerates 20 -> 0 at 50/3 Hz/s: 20 / 16.667 = 1.2 s.
    // Stage 2 accelerates 0 -> -20 at 10 Hz/s: 2.0 s. Total 3.2 s.
    // Using the accel rate for both stages gives 4.0 s (the merged engine does this).
    let mut r = Ramp {
        frequency_hz: 20.0,
        accel_time_s: 5.0,
        decel_time_s: 3.0,
        reference_hz: 50.0,
    };
    for _ in 0..320 {
        r.step(-20.0, 0.01);
    }
    close(r.frequency_hz, -20.0, 1e-9, "reaches -20 Hz after 3.2 s");
}

#[test]
#[ignore = "L1: S-ramp (jerk limited) must exist in drive.rs"]
fn case02_s_ramp_end_points_and_jerk_limit() {
    // Jerk-limited ramp 0 -> 50 Hz in 5 s with the acceleration rising and falling over 1 s each.
    // The area under the trapezoid of acceleration is the frequency step: a_max (T - t_r) = f,
    // so a_max = 50 / (5 - 1) = 12.5 Hz/s and the jerk is a_max / t_r = 12.5 Hz/s^2.
    // Frequency at 2.5 s (symmetric midpoint) is 25 Hz; the slope is 0 at 0 s and at 5 s.
    todo!(
        "L1: sample the S-ramp every 1 ms and compare end slope 0, midpoint 25 Hz, max jerk 12.5"
    );
}

// ---------------------------------------------------------------- case 3: V/f

/// Hand V/f law: linear to the corner, constant above.
fn vf_volts(f: f64, v_rated: f64, f_base: f64) -> f64 {
    v_rated * (f / f_base).min(1.0)
}

#[test]
fn oracle_case03_vf_values() {
    // 400 V, 50 Hz corner.
    close(vf_volts(10.0, 400.0, 50.0), 80.0, 1e-9, "10 Hz");
    close(vf_volts(25.0, 400.0, 50.0), 200.0, 1e-9, "25 Hz");
    close(vf_volts(50.0, 400.0, 50.0), 400.0, 1e-9, "50 Hz");
    close(vf_volts(87.0, 400.0, 50.0), 400.0, 1e-9, "87 Hz");
    // Flux above base is V/f relative to rated: 50/87 = 0.5747. Breakdown torque goes as the
    // square: 0.3303.
    let flux = 50.0 / 87.0;
    close(flux, 0.574_712_643, 1e-8, "flux ratio");
    close(flux * flux, 0.330_294_6, 1e-6, "torque capability ratio");
}

#[test]
#[ignore = "L1: drive V/f law (volts_for_frequency) not in the engine yet"]
fn case03_vf_law_in_the_drive() {
    todo!(
        "L1: volts at 10, 25, 50, 87 Hz must equal 80, 200, 400, 400 V; boost adds the entered volts at 0 Hz only"
    );
}

// ---------------------------------------------------------------- case 4: torque-speed

/// Closed-form breakdown torque of the per-phase equivalent circuit with the Thevenin
/// reduction, supply scaled `f/50` with a V/f ratio factor, reactances scaled with f.
fn breakdown_torque(f: f64, vf_factor: f64, r1: f64) -> f64 {
    let (x1_50, xm_50, r2, x2_50) = (0.9, 25.0, 0.35, 1.1);
    let (poles, v_line): (f64, f64) = (4.0, 400.0);
    let s = f / 50.0;
    let (x1, xm, x2) = (x1_50 * s, xm_50 * s, x2_50 * s);
    let _ = r2; // r2 sets where the peak is, not how high it is
    let v = v_line / 3f64.sqrt() * s * vf_factor;
    // Z_th = jXm (R1 + jX1) / (R1 + j(X1 + Xm)); V_th = V * jXm / (R1 + j(X1 + Xm))
    let den_re = r1;
    let den_im = x1 + xm;
    let den2 = den_re * den_re + den_im * den_im;
    // numerator jXm*(R1 + jX1) = -Xm X1 + j Xm R1
    let (nre, nim) = (-xm * x1, xm * r1);
    let zth_re = (nre * den_re + nim * den_im) / den2;
    let zth_im = (nim * den_re - nre * den_im) / den2;
    let vth = v * xm / den2.sqrt();
    let omega_s = 2.0 * PI * f * 2.0 / poles;
    3.0 * vth * vth / (2.0 * omega_s * (zth_re + (zth_re * zth_re + (zth_im + x2).powi(2)).sqrt()))
}

#[test]
fn oracle_case04_breakdown_scaling() {
    // Generic motor: 400 V star, 4 pole, R1 0.45, X1 0.9, Xm 25, R2' 0.35, X2' 1.1 ohm at 50 Hz.
    // R1 = 0: the closed form is the unrestricted maximum and every term scales with f at
    // constant V/f, so it is 241.027 N m wherever the peak slip is <= 1 (25 and 50 Hz: peak slip
    // 0.36 and 0.18). At 5 Hz the peak sits at slip 1.78 (> 1, not reachable when motoring), so the
    // motoring maximum is the standstill torque T(s = 1) = 205.980 N m instead.
    for f in [25.0, 50.0] {
        close(
            breakdown_torque(f, 1.0, 0.0),
            241.026_809,
            1e-3,
            &format!("R1=0 at {f} Hz"),
        );
    }
    assert!(
        breakdown_torque(5.0, 1.0, 0.0) > 241.0,
        "closed form is the unrestricted maximum"
    );
    // V/f lowered to 80 %: torque goes as 0.8^2 = 0.64 exactly.
    close(
        breakdown_torque(50.0, 0.8, 0.45) / breakdown_torque(50.0, 1.0, 0.45),
        0.64,
        1e-12,
        "(V/f)^2",
    );
    // With the stator resistance the low-frequency capability drops (stator drop):
    close(
        breakdown_torque(50.0, 1.0, 0.45),
        194.485_480,
        1e-3,
        "50 Hz",
    );
    close(
        breakdown_torque(25.0, 1.0, 0.45),
        157.780_862,
        1e-3,
        "25 Hz",
    );
    close(breakdown_torque(5.0, 1.0, 0.45), 51.519_435, 1e-3, "5 Hz");
}

#[test]
#[ignore = "L2b: variable-frequency torque-speed curve not in the engine yet"]
fn case04_engine_breakdown_matches_thevenin() {
    todo!(
        "L2b: engine breakdown torque at 5, 25, 50 Hz with V/f constant = 51.519, 157.781, 194.485 N m (R1 0.45); R1 = 0 gives 241.027 at all three"
    );
}

// ---------------------------------------------------------------- case 5: hoist torque and holding

#[test]
fn case05_hoisting_torque_by_hand() {
    // T = m g r / (i eta) = 5000 * 9.80665 * 0.2 / (31.5 * 0.9) = 9806.65 / 28.35 = 345.9136 N m
    let t = hoist_holding_torque_nm(5000.0, 0.2, 31.5, 0.9);
    close(t, 345.913_580, 1e-5, "hoisting torque");
}

#[test]
fn oracle_case05_lowering_and_holding() {
    // Lowering: the load drives the motor, efficiency now works for the shaft:
    // T = m g r eta / i = 9806.65 * 0.9 / 31.5 = 280.19 N m, sign opposite (overhauling).
    close(
        hoist::MASS_KG * GRAVITY * hoist::DRUM_R_M * hoist::ETA / hoist::RATIO,
        280.19,
        1e-9,
        "lowering magnitude",
    );
    // Static hold without crediting efficiency = 311.3222 N m; a 500 N m brake gives 1.606.
    close(
        hoist::holding_torque_static(),
        311.322_222,
        1e-5,
        "static hold",
    );
    close(
        500.0 / hoist::holding_torque_static(),
        1.606_0,
        1e-3,
        "brake ratio",
    );
}

#[test]
#[ignore = "L2a: lowering sign, overhauling flag and brake holding ratio not in the engine yet"]
fn case05_lowering_sign_and_brake_hold() {
    todo!("L2a: lowering torque -280.19 N m (overhauling), hold ratio 1.606 for a 500 N m brake");
}

// ---------------------------------------------------------------- case 6: brake drop

#[test]
fn oracle_case06_free_fall_bound_and_with_inertia() {
    // Brake released 150 ms before the motor holds the load. Upper bound with no inertia is
    // free fall 0.5 g t^2 = 0.5 * 9.80665 * 0.15^2 = 110.32 mm.
    close(
        0.5 * GRAVITY * 0.15 * 0.15,
        0.110_324_8,
        1e-7,
        "free fall bound",
    );
    // Real drop is smaller because the motor rotor must be spun up through the gearbox:
    // J reflected as mass = J (i/r)^2 = 0.5 * 157.5^2 = 12403.125 kg-equivalent.
    // a = g m / (m + J_ref) = 9.80665 * 5000 / 17403.125 = 2.8175 m/s^2; drop 0.5 a t^2 = 31.70 mm.
    let j_ref = hoist::J_MOTOR * (hoist::RATIO / hoist::DRUM_R_M).powi(2);
    close(j_ref, 12_403.125, 1e-9, "reflected inertia");
    let a = GRAVITY * hoist::MASS_KG / (hoist::MASS_KG + j_ref);
    close(a, 2.817_497, 1e-6, "acceleration");
    close(0.5 * a * 0.15 * 0.15, 0.031_697, 1e-6, "drop with inertia");
}

#[test]
#[ignore = "L2a: brake-open-before-torque drop (hoist mechanics step) not in the engine yet"]
fn case06_brake_opens_before_torque() {
    todo!(
        "L2a: drop in 150 ms is 31.70 mm with J_motor 0.5 kg m2 and must never exceed 110.32 mm; with J_motor 0 it equals 110.32 mm"
    );
}

// ---------------------------------------------------------------- case 7: braking resistor

#[test]
fn oracle_case07_braking_energy_and_power() {
    // J 2.0 kg m2 total, 1500 rpm = 157.08 rad/s, stopped in 4 s at constant decel torque.
    let w = 2.0 * PI * 1500.0 / 60.0;
    let e = 0.5 * 2.0 * w * w;
    close(e, 24_674.011, 1e-3, "kinetic energy");
    // Average over the stop 6168.5 W; power is proportional to speed so the peak at the start
    // is twice that: 12337.0 W (upper bound: motor and drive losses ignored).
    close(e / 4.0, 6_168.503, 1e-3, "average");
    close(2.0 * e / 4.0, 12_337.006, 1e-3, "peak");
    // Resistor limit at 780 V bus threshold: V^2/R. 47 ohm -> 12944.7 W (enough for the peak),
    // 100 ohm -> 6084 W (below the peak: the bus rises, flag it).
    close(780f64.powi(2) / 47.0, 12_944.681, 1e-3, "47 ohm");
    close(780f64.powi(2) / 100.0, 6_084.0, 1e-9, "100 ohm");
    // One stop per 60 s: average 411.2 W against the continuous rating.
    close(e / 60.0, 411.234, 1e-3, "duty average");
}

#[test]
#[ignore = "L1: braking chopper and resistor check not in the engine yet"]
fn case07_resistor_flags() {
    todo!(
        "L1: 47 ohm / 1000 W continuous / 60 s cycle passes peak, fails nothing at 411 W; 100 ohm flags peak > V^2/R; 47 ohm with 300 W continuous flags duty"
    );
}

// ---------------------------------------------------------------- case 8: DC bus energy

#[test]
fn oracle_case08_capacitor_energy() {
    // 1000 uF from 565 V (rectified 400 V) to 780 V stores 0.5 C (V2^2 - V1^2) = 144.59 J.
    close(
        0.5e-3 * (780f64.powi(2) - 565f64.powi(2)),
        144.5875,
        1e-9,
        "stored energy",
    );
}

#[test]
#[ignore = "L1: DC bus model not in the engine yet"]
fn case08_energy_balance_property() {
    todo!(
        "L1: for 200 random runs (inertia, ramp, load) E_in - E_loss == E_mech + dE_bus within 1e-6 relative"
    );
}

// ---------------------------------------------------------------- case 9: current limit

#[test]
fn oracle_case09_accel_under_current_limit() {
    // J 0.5 kg m2 to 1500 rpm (157.08 rad/s) with available torque 30 N m: t = J w / T = 2.618 s.
    // With a 10 N m constant load torque the available torque is 20 N m: 3.927 s.
    let w = 2.0 * PI * 1500.0 / 60.0;
    close(0.5 * w / 30.0, 2.617_994, 1e-6, "no load");
    close(0.5 * w / 20.0, 3.926_991, 1e-6, "with load");
}

#[test]
#[ignore = "L1 + L2b: current limit at 150 % with torque from the motor model"]
fn case09_current_limit_acceleration_time() {
    todo!(
        "L1/L2b: engine acceleration time to 1500 rpm equals 2.618 s (30 N m limit) within one sample"
    );
}

// ---------------------------------------------------------------- case 10: I2t trip

#[test]
fn oracle_case10_i2t_times() {
    // The user enters one point: 150 % trips in 60 s. Model: trip when the integral of
    // (I/In)^2 - 1 over time reaches K. K = (1.5^2 - 1) * 60 = 75.
    let k = (1.5f64 * 1.5 - 1.0) * 60.0;
    close(k, 75.0, 1e-12, "K");
    // Then 120 % trips in 75 / 0.44 = 170.45 s and 200 % in 75 / 3 = 25 s. Those two are
    // model values (stated so in the app); only the entered 150 / 60 s point is exact.
    close(k / (1.2 * 1.2 - 1.0), 170.454_545, 1e-5, "120 %");
    close(k / (2.0 * 2.0 - 1.0), 25.0, 1e-12, "200 %");
}

#[test]
#[ignore = "L1: motor thermal I2t trip not in the engine yet"]
fn case10_i2t_trip_time() {
    todo!(
        "L1: constant 150 % trips at 60.0 s within one sample; 200 % at 25.0 s; 100 % never; recovery model separately"
    );
}

// ---------------------------------------------------------------- case 13: two-speed run-up

#[test]
fn oracle_case13_two_speed_runup() {
    // Hoisting 5 t (gravity torque 345.91 N m). Pole-change motor, 12 pole fine 500 rpm
    // (52.360 rad/s) and 2 pole coarse 3000 rpm (314.159 rad/s). Constant motor torque
    // idealisation: fine 500 N m, coarse 700 N m. J_total = 0.70156 kg m2.
    let j = hoist::j_total();
    close(j, 0.701_562, 1e-6, "J total");
    let tl = hoist::gravity_torque_hoisting();
    let wf = 2.0 * PI * 500.0 / 60.0;
    let wc = 2.0 * PI * 3000.0 / 60.0;
    // Stage fine: t = J w / (T - TL) = 0.70156 * 52.360 / 154.087 = 0.2384 s
    close(j * wf / (500.0 - tl), 0.238_397, 1e-5, "fine run-up");
    // Changeover dead time 50 ms with no motor torque, brake open: dw = TL / J * dt = 24.65 rad/s,
    // so the speed falls to 27.707 rad/s.
    let w_after = wf - tl / j * 0.05;
    close(w_after, 27.706_780, 1e-5, "speed after dead time");
    // Stage coarse: t = J (wc - w_after) / (700 - TL) = 0.5676 s
    close(
        j * (wc - w_after) / (700.0 - tl),
        0.567_557,
        1e-5,
        "coarse run-up",
    );
}

#[test]
#[ignore = "L2a: two-speed hoist sequencing not in the engine yet"]
fn case13_two_speed_runup_and_changeover() {
    todo!(
        "L2a: fine stage 0.2384 s, speed 27.707 rad/s after 50 ms dead time, coarse stage 0.5676 s (constant-torque idealisation inputs in the oracle)"
    );
}

// ---------------------------------------------------------------- case 14: brake apply drop

/// Closed-form travel of the hook after the motor torque is cut at t = 0 while hoisting at
/// `v0` (m/s), with the brake applying after `delay_s`; brake torque `t_brake` on the motor
/// shaft. Returns (displacement at brake apply, hook speed then, total stop position).
/// Up is positive. Efficiency 1 (conservative for the drop), inertia 5 t payload + motor rotor.
fn brake_apply_stop(v0: f64, delay_s: f64, t_brake: f64) -> (f64, f64, f64) {
    let m = hoist::MASS_KG;
    let j_ref_mass = hoist::J_MOTOR * (hoist::RATIO / hoist::DRUM_R_M).powi(2);
    let a_free = GRAVITY * m / (m + j_ref_mass);
    let x1 = v0 * delay_s - 0.5 * a_free * delay_s * delay_s;
    let v1 = v0 - a_free * delay_s;
    let t_load = hoist::holding_torque_static();
    // Brake always opposes the motion; going up the load helps the brake, going down it fights it.
    let alpha = if v1 >= 0.0 {
        t_brake + t_load
    } else {
        t_brake - t_load
    } / hoist::j_total();
    let a_lin = alpha * hoist::M_PER_RAD;
    let stop = v1 * v1 / (2.0 * a_lin);
    let total = if v1 >= 0.0 { x1 + stop } else { x1 - stop };
    (x1, v1, total)
}

#[test]
fn oracle_case14_brake_apply_delay() {
    // Fine speed 0.05 m/s upward, motor torque cut, brake 500 N m.
    // AC-side switching, apply delay 80 ms: moves up 4.0 mm, loses 9.02 mm to the pull of the
    // load: -5.016 mm at apply, speed -0.1754 m/s (downward); the brake then needs 9.009 mm:
    // total -14.025 mm (below the start).
    let (x1, v1, total) = brake_apply_stop(0.05, 0.080, 500.0);
    close(x1, -0.005_016, 1e-6, "AC displacement at apply");
    close(v1, -0.175_400, 1e-6, "AC speed at apply");
    close(total, -0.014_025, 1e-6, "AC total");
    // DC-side switching, delay 12 ms: still moving up at apply, total +0.415 mm.
    let (_, v1, total) = brake_apply_stop(0.05, 0.012, 500.0);
    close(v1, 0.016_190, 1e-6, "DC speed at apply");
    close(total, 0.000_415, 1e-6, "DC total");
}

#[test]
#[ignore = "L2a: rectifier-fed brake with AC-side and DC-side delays not in the engine yet"]
fn case14_brake_ac_versus_dc_switching() {
    todo!(
        "L2a: total hook movement -14.025 mm (80 ms) and +0.415 mm (12 ms) at 500 N m brake, 0.05 m/s"
    );
}

// ---------------------------------------------------------------- case 11 (pendulum) lives in novo-machine
