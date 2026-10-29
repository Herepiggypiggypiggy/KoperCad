//! Tests of the induction motor at variable frequency (lane L2b). Expected values are worked
//! out by hand or by an independent route (the Thevenin closed form, a brute-force scan of the
//! phasor solution, energy balance); none comes from the function under test.

use std::f64::consts::TAU;

use novo_drive::InputError;
use novo_drive::drive::{
    ControlMode, Drive, DriveConfig, DriveInputs, DriveState, MotorNameplate, ReferenceSource,
    TerminalMacro, VfCurve,
};
use novo_drive::induction::{
    Circuit, FitOptions, InductionMotor, Saturation, SelfVentilation, Supply, TorqueLimitedBy,
};
use novo_drive::motor::Direction;

fn close(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: got {a}, expected {b}, tolerance {tol}"
    );
}

fn rel(a: f64, b: f64, tol: f64, what: &str) {
    assert!(
        (a - b).abs() <= tol * b.abs().max(1e-12),
        "{what}: got {a}, expected {b}, relative tolerance {tol}"
    );
}

/// The generic motor of hand case 4: 400 V star, 4 pole, 50 Hz, R1 0.45, X1 0.9, Xm 25,
/// R2' 0.35, X2' 1.1 ohm at 50 Hz.
fn case4(rs: f64) -> InductionMotor {
    InductionMotor::new(
        Circuit::new(rs, 0.9, 0.35, 1.1, 25.0),
        4,
        400.0,
        50.0,
        1450.0,
    )
    .unwrap()
}

fn supply(f: f64, v: f64) -> Supply {
    Supply::new(f, v, Direction::Positive).unwrap()
}

/// Constant V/f supply, 400 V at 50 Hz.
fn vf(f: f64) -> Supply {
    supply(f, 400.0 * f / 50.0)
}

// ---- hand case 4: breakdown against the Thevenin closed form ---------------------------

#[test]
fn breakdown_matches_hand_thevenin_values() {
    let m = case4(0.45);
    for (f, expected) in [(5.0, 51.519_435), (25.0, 157.780_862), (50.0, 194.485_480)] {
        close(
            m.breakdown(&vf(f)).motoring_nm,
            expected,
            1e-3,
            &format!("{f} Hz"),
        );
    }
    let m0 = case4(0.0);
    for f in [5.0, 25.0, 50.0] {
        close(
            m0.breakdown(&vf(f)).motoring_nm,
            241.026_809,
            1e-3,
            &format!("R1 = 0 at {f} Hz"),
        );
    }
}

#[test]
fn breakdown_slip_and_reachability_by_hand() {
    // R1 = 0, 50 Hz: Zth = j Xm X1 / (Xm + X1) = j 0.868..., slip at the peak is
    // R2 / (Xth + X2) = 0.35 / (0.86869 + 1.1) = 0.17776. Xth = 25*0.9/25.9 = 0.868726.
    let m0 = case4(0.0);
    let b = m0.breakdown(&vf(50.0));
    close(
        b.motoring_slip,
        0.35 / (25.0 * 0.9 / 25.9 + 1.1),
        1e-12,
        "slip 50 Hz",
    );
    assert!(b.motoring_reachable);
    // 5 Hz: reactances are a tenth, so the peak slip is ten times larger: 1.7776 > 1.
    let b5 = m0.breakdown(&vf(5.0));
    close(b5.motoring_slip, 10.0 * b.motoring_slip, 1e-12, "slip 5 Hz");
    assert!(!b5.motoring_reachable);
    // Generating side mirrors it with Rs = 0.
    close(
        b.generating_nm,
        b.motoring_nm,
        1e-9,
        "symmetric with R1 = 0",
    );
    close(b.generating_slip, -b.motoring_slip, 1e-12, "mirror slip");
}

#[test]
fn breakdown_equals_brute_force_scan_of_the_phasor_solution() {
    // Independent of the closed form: scan the torque over slip and take the maximum.
    let mut c = Circuit::new(0.45, 0.9, 0.35, 1.1, 25.0);
    c.core_loss_w = 120.0;
    let m = InductionMotor::new(c, 4, 400.0, 50.0, 1450.0).unwrap();
    for f in [8.0, 30.0, 50.0, 87.0] {
        let s = supply(f, (400.0f64).min(400.0 * f / 50.0));
        let b = m.breakdown(&s);
        let mut best = 0.0f64;
        let mut best_s = 0.0;
        let mut slip = 1e-4;
        while slip < 6.0 {
            let t = m.at_slip(&s, slip).unwrap().air_gap_torque_nm;
            if t > best {
                best = t;
                best_s = slip;
            }
            slip *= 1.0005;
        }
        rel(b.motoring_nm, best, 1e-3, &format!("scan {f} Hz"));
        rel(b.motoring_slip, best_s, 2e-3, &format!("scan slip {f} Hz"));
    }
}

#[test]
fn torque_scales_with_voltage_squared() {
    let m = case4(0.45);
    let full = m.breakdown(&supply(50.0, 400.0)).motoring_nm;
    let low = m.breakdown(&supply(50.0, 320.0)).motoring_nm;
    rel(low / full, 0.64, 1e-12, "(V/V0)^2");
    // The same holds at any slip.
    let a = m
        .at_slip(&supply(50.0, 400.0), 0.05)
        .unwrap()
        .air_gap_torque_nm;
    let b = m
        .at_slip(&supply(50.0, 200.0), 0.05)
        .unwrap()
        .air_gap_torque_nm;
    rel(b / a, 0.25, 1e-12, "(V/V0)^2 at fixed slip");
}

#[test]
fn field_weakening_breakdown_falls_with_one_over_f_squared() {
    // With R1 = 0 and voltage held at rated, T_max = 3 Vth^2 / (2 w_s (Xth + X2)). Vth is
    // independent of frequency, w_s and the reactances grow with f: T_max ~ 1/f^2 exactly.
    let m = case4(0.0);
    let t50 = m.breakdown(&supply(50.0, 400.0)).motoring_nm;
    for f in [60.0, 75.0, 100.0, 150.0] {
        let t = m.breakdown(&supply(f, 400.0)).motoring_nm;
        rel(t, t50 * (50.0 / f).powi(2), 1e-12, &format!("{f} Hz"));
    }
    // Air-gap flux in per unit falls as f_base / f above base (no load).
    let p = m.at_slip(&supply(100.0, 400.0), 0.0).unwrap();
    close(p.flux_pu, 0.5, 0.03, "flux at double frequency");
}

// ---- hand case 1: slip speed convention ----------------------------------------------

#[test]
fn slip_speed_convention_keeps_slip_speed_not_slip_fraction() {
    let m = case4(0.45);
    // 4 pole 50 Hz 1450 rpm: slip speed 50 rpm. 10 Hz: 300 - 50, 25 Hz: 750 - 50.
    close(m.rated_rpm_at(10.0), 250.0, 1e-9, "10 Hz");
    close(m.rated_rpm_at(25.0), 700.0, 1e-9, "25 Hz");
    close(m.rated_rpm_at(50.0), 1450.0, 1e-9, "50 Hz");
    // Never negative.
    close(m.rated_rpm_at(1.0), 0.0, 1e-9, "1 Hz");
}

#[test]
fn circuit_slip_speed_is_close_to_the_convention_above_25_hz() {
    // The circuit includes the stator drop, so it departs from the convention at low frequency
    // but follows it in the constant-flux region. Torque: the circuit's own torque at 50 Hz.
    let m = case4(0.45);
    let t_n = m
        .at_slip(&vf(50.0), 1.0 - 1450.0 / 1500.0)
        .unwrap()
        .torque_nm;
    for f in [25.0, 50.0] {
        let p = m
            .steady_state(&vf(f), t_n)
            .unwrap()
            .expect("below breakdown");
        let conv = m.rated_rpm_at(f);
        assert!(
            (p.speed_rpm - conv).abs() < 0.04 * conv,
            "{f} Hz: circuit {} rpm, convention {conv} rpm",
            p.speed_rpm
        );
    }
    // At 10 Hz the stator drop matters: the circuit slip speed is larger than the convention.
    let p10 = m.steady_state(&vf(10.0), t_n).unwrap().unwrap();
    assert!(p10.speed_rpm < m.rated_rpm_at(10.0));
}

// ---- energy balance and signs ---------------------------------------------------------

#[test]
fn input_power_equals_losses_plus_air_gap_power() {
    let mut c = Circuit::new(0.45, 0.9, 0.35, 1.1, 25.0);
    c.core_loss_w = 90.0;
    c.rot_loss_w = 40.0;
    let m = InductionMotor::new(c, 4, 400.0, 50.0, 1450.0).unwrap();
    for f in [3.0, 12.0, 50.0, 90.0] {
        for slip in [-0.2, -0.03, 0.001, 0.02, 0.15, 0.6, 1.0] {
            let s = supply(f, (400.0f64).min(400.0 * f / 50.0 + 15.0));
            let p = m.at_slip(&s, slip).unwrap();
            let balance = p.loss_stator_w + p.loss_core_w + p.air_gap_power_w - p.input_power_w;
            assert!(
                balance.abs() < 1e-9 * p.input_power_w.abs().max(1.0),
                "{f} Hz slip {slip}: {balance}"
            );
            // Shaft power = air gap power x (1 - s) - friction.
            let shaft = p.air_gap_power_w * (1.0 - slip) - p.loss_rot_w;
            assert!(
                (shaft - p.shaft_power_w).abs() < 1e-9 * shaft.abs().max(1.0) + 1e-9,
                "{f} Hz slip {slip}: shaft {shaft} vs {}",
                p.shaft_power_w
            );
            // Rotor copper = s x air gap power.
            assert!(
                (p.loss_rotor_w - slip * p.air_gap_power_w).abs()
                    < 1e-9 * p.loss_rotor_w.abs().max(1.0),
                "rotor copper"
            );
        }
    }
}

#[test]
fn above_synchronous_speed_the_motor_generates_and_feeds_back() {
    let m = case4(0.45);
    let s = vf(50.0);
    let p = m.at_slip(&s, -0.03).unwrap();
    assert!(p.torque_nm < 0.0 && p.input_power_w < 0.0 && p.power_factor < 0.0);
    assert!(p.speed_rpm > p.sync_rpm);
    let eff = p.efficiency.unwrap();
    assert!(eff > 0.5 && eff < 1.0, "generating efficiency {eff}");
    // Overhauling load: a steady state with negative torque is found above synchronous speed.
    let st = m.steady_state(&s, -30.0).unwrap().unwrap();
    assert!(st.speed_rpm > st.sync_rpm);
    close(st.torque_nm, -30.0, 1e-6, "torque");
}

#[test]
fn reverse_direction_gives_mirror_image() {
    let m = case4(0.45);
    let fwd = Supply::new(30.0, 240.0, Direction::Positive).unwrap();
    let rev = Supply::new(30.0, 240.0, Direction::Negative).unwrap();
    let a = m.evaluate(&fwd, 100.0).unwrap();
    let b = m.evaluate(&rev, -100.0).unwrap();
    close(b.torque_nm, -a.torque_nm, 1e-9, "torque");
    close(b.speed_rpm, -a.speed_rpm, 1e-9, "speed");
    close(b.stator_current_a, a.stator_current_a, 1e-9, "current");
    close(b.input_power_w, a.input_power_w, 1e-9, "power");
    // Positive speed against a reverse field is plugging: braking torque in the shaft sense.
    let plug = m.evaluate(&rev, 100.0).unwrap();
    assert!(plug.torque_nm < 0.0);
}

#[test]
fn steady_state_returns_none_beyond_breakdown() {
    let m = case4(0.45);
    let b = m.breakdown(&vf(50.0));
    assert!(
        m.steady_state(&vf(50.0), 0.99 * b.motoring_nm)
            .unwrap()
            .is_some()
    );
    assert!(
        m.steady_state(&vf(50.0), 1.01 * b.motoring_nm)
            .unwrap()
            .is_none()
    );
    assert!(
        m.steady_state(&vf(50.0), -1.01 * b.generating_nm)
            .unwrap()
            .is_none()
    );
}

#[test]
fn dc_hold_below_a_hundredth_of_a_hertz() {
    let m = case4(0.45);
    let s = supply(0.0, 12.0);
    let p = m.evaluate(&s, 0.0).unwrap();
    close(p.torque_nm, 0.0, 0.0, "torque");
    // 12 V / sqrt(3) / 0.45 ohm.
    close(
        p.stator_current_a,
        12.0 / 3f64.sqrt() / 0.45,
        1e-9,
        "current",
    );
    close(
        p.input_power_w,
        3.0 * p.stator_current_a.powi(2) * 0.45,
        1e-9,
        "power",
    );
}

// ---- current-limited torque ---------------------------------------------------------

#[test]
fn torque_at_current_limit_hits_the_limit_exactly() {
    let m = case4(0.45);
    let s = vf(40.0);
    // No-load current at 40 Hz is about 8.9 A (low magnetising reactance of this generic motor).
    let i_lim = 14.0;
    let t = m.torque_at_current_limit(&s, i_lim, false).unwrap();
    assert_eq!(t.limited_by, TorqueLimitedBy::Current);
    close(t.point.stator_current_a, i_lim, 1e-9, "current at limit");
    // More allowed current gives more torque, and the torque is at most breakdown.
    let more = m.torque_at_current_limit(&s, 25.0, false).unwrap();
    assert!(more.torque_nm > t.torque_nm);
    let huge = m.torque_at_current_limit(&s, 1000.0, false).unwrap();
    assert_eq!(huge.limited_by, TorqueLimitedBy::Breakdown);
    rel(
        huge.torque_nm,
        m.breakdown(&s).motoring_nm,
        1e-9,
        "breakdown",
    );
    // A limit below the magnetising current gives no torque.
    let none = m.torque_at_current_limit(&s, 0.5, false).unwrap();
    close(none.torque_nm, 0.0, 1e-3, "below magnetising current");
    // Generating side mirrors in sign.
    let g = m.torque_at_current_limit(&s, i_lim, true).unwrap();
    assert!(g.point.torque_nm < 0.0 && g.slip < 0.0);
}

// ---- cooling derating ---------------------------------------------------------------

#[test]
fn continuous_torque_derates_at_low_speed() {
    let m = case4(0.45);
    let cool = SelfVentilation::typical();
    let t_n = m.rated_torque_nm();
    // T = T_n sqrt(a + (1 - a) x), a = 0.4.
    close(
        m.continuous_torque_nm(0.0, &cool),
        t_n * 0.4f64.sqrt(),
        1e-9,
        "standstill",
    );
    close(
        m.continuous_torque_nm(725.0, &cool),
        t_n * (0.4f64 + 0.6 * 0.5).sqrt(),
        1e-9,
        "half speed",
    );
    close(
        m.continuous_torque_nm(1450.0, &cool),
        t_n,
        1e-9,
        "rated speed",
    );
    close(
        m.continuous_torque_nm(2900.0, &cool),
        t_n,
        1e-9,
        "capped above rated",
    );
    // Forced ventilation (fraction 1): no derating at all.
    let forced = SelfVentilation {
        standstill_loss_fraction: 1.0,
    };
    close(m.continuous_torque_nm(0.0, &forced), t_n, 1e-9, "forced");
}

// ---- saturation ---------------------------------------------------------------------

#[test]
fn saturation_is_off_by_default_and_costs_torque_when_overexcited() {
    let lin = case4(0.45);
    let mut c = Circuit::new(0.45, 0.9, 0.35, 1.1, 25.0);
    // Knee exactly at rated flux: any extra flux saturates.
    c.saturation = Some(Saturation {
        knee_flux_pu: 1.0,
        exponent: 2.0,
    });
    let sat = InductionMotor::new(c, 4, 400.0, 50.0, 1450.0).unwrap();
    // At rated and below the two agree exactly (flux <= 1).
    let a = lin.at_slip(&vf(50.0), lin.rated_slip()).unwrap();
    let b = sat.at_slip(&vf(50.0), lin.rated_slip()).unwrap();
    rel(b.torque_nm, a.torque_nm, 1e-6, "rated flux unchanged");
    // Overexcited (+25 % volts per hertz): more magnetising current, less torque than linear.
    let over = supply(40.0, 400.0);
    let pl = lin.at_slip(&over, 0.03).unwrap();
    let ps = sat.at_slip(&over, 0.03).unwrap();
    assert!(ps.flux_pu > 1.0);
    assert!(ps.magnetising_current_a > pl.magnetising_current_a);
    assert!(ps.stator_current_a > pl.stator_current_a);
    // The numeric breakdown search works under saturation and stays near the linear value.
    let bs = sat.breakdown(&over).motoring_nm;
    let bl = lin.breakdown(&over).motoring_nm;
    assert!(bs > 0.5 * bl && bs <= bl * 1.001, "sat {bs} lin {bl}");
}

// ---- fit from a nameplate -----------------------------------------------------------

fn plate(kw: f64, v: f64, i: f64, f: f64, rpm: f64, pf: f64) -> MotorNameplate {
    MotorNameplate {
        voltage_v: v,
        current_a: i,
        frequency_hz: f,
        rpm,
        power_kw: kw,
        power_factor: pf,
    }
}

#[test]
fn fit_reproduces_the_nameplate_for_a_range_of_motors() {
    // Generic nameplates from 0.37 kW to 250 kW (typical values, no manufacturer data).
    let plates = [
        plate(0.37, 400.0, 1.1, 50.0, 1370.0, 0.72),
        plate(1.5, 400.0, 3.6, 50.0, 1430.0, 0.80),
        plate(5.5, 400.0, 11.2, 50.0, 1445.0, 0.84),
        plate(11.0, 400.0, 21.5, 50.0, 1460.0, 0.85),
        plate(30.0, 400.0, 56.0, 50.0, 1475.0, 0.86),
        plate(90.0, 400.0, 160.0, 50.0, 1485.0, 0.88),
        plate(250.0, 690.0, 255.0, 50.0, 1488.0, 0.88),
        plate(2.2, 400.0, 4.7, 50.0, 2850.0, 0.86),
        plate(7.5, 400.0, 16.0, 50.0, 960.0, 0.78),
        plate(3.7, 460.0, 6.6, 60.0, 1760.0, 0.83),
    ];
    for np in plates {
        let (m, rep) = InductionMotor::from_nameplate(&np, None, &FitOptions::default())
            .unwrap_or_else(|e| panic!("{np:?}: {e}"));
        assert!(rep.residual < 1e-9, "{np:?}: residual {}", rep.residual);
        let s = supply(np.frequency_hz, np.voltage_v);
        let p = m.at_slip(&s, m.rated_slip()).unwrap();
        rel(p.stator_current_a, np.current_a, 1e-8, "current");
        rel(p.power_factor, np.power_factor, 1e-8, "pf");
        rel(p.shaft_power_w, np.power_kw * 1000.0, 1e-8, "power");
        rel(
            m.rated_torque_nm(),
            np.power_kw * 1000.0 / (np.rpm * TAU / 60.0),
            1e-8,
            "rated torque = P / w",
        );
        close(p.speed_rpm, np.rpm, 1e-8, "speed");
        // Efficiency from the circuit equals the nameplate's own ratio.
        let eta =
            np.power_kw * 1000.0 / (3f64.sqrt() * np.voltage_v * np.current_a * np.power_factor);
        rel(rep.rated_efficiency, eta, 1e-8, "efficiency");
        assert!(rep.breakdown_ratio > 1.0);
    }
}

#[test]
fn fit_with_a_breakdown_ratio_reproduces_it() {
    let np = plate(11.0, 400.0, 21.5, 50.0, 1460.0, 0.85);
    for k in [2.0, 2.6, 3.2] {
        let opts = FitOptions {
            breakdown_ratio: Some(k),
            ..FitOptions::default()
        };
        let (m, rep) = InductionMotor::from_nameplate(&np, None, &opts)
            .unwrap_or_else(|e| panic!("k {k}: {e}"));
        rel(rep.breakdown_ratio, k, 1e-7, "breakdown ratio");
        rel(
            m.breakdown(&supply(50.0, 400.0)).motoring_nm / m.rated_torque_nm(),
            k,
            1e-7,
            "from the motor",
        );
        let p = m.at_slip(&supply(50.0, 400.0), m.rated_slip()).unwrap();
        rel(p.stator_current_a, 21.5, 1e-8, "current");
    }
}

#[test]
fn fit_with_losses_still_reproduces_the_nameplate() {
    let np = plate(15.0, 400.0, 28.5, 50.0, 1465.0, 0.86);
    let (m, rep) = InductionMotor::from_nameplate(&np, None, &FitOptions::typical()).unwrap();
    assert!(rep.residual < 1e-9);
    let p = m.at_slip(&supply(50.0, 400.0), m.rated_slip()).unwrap();
    rel(p.shaft_power_w, 15_000.0, 1e-8, "power");
    assert!(p.loss_core_w > 0.0 && p.loss_rot_w > 0.0);
    assert!(
        p.loss_rot_w >= 150.0 - 1e-6,
        "friction at least 1 % of rated power"
    );
    assert!(
        p.loss_core_w >= 300.0 - 1e-6,
        "core loss at least 2 % of rated power"
    );
    // Energy balance holds with the fitted losses.
    let bal = p.losses_w() + p.shaft_power_w - p.input_power_w;
    assert!(bal.abs() < 1e-6, "balance {bal}");
}

#[test]
fn fit_rejects_contradicting_or_bad_nameplates() {
    let ok = plate(1.5, 400.0, 3.6, 50.0, 1430.0, 0.80);
    let d = FitOptions::default();
    // Efficiency above 100 %.
    let bad = plate(5.0, 400.0, 3.6, 50.0, 1430.0, 0.80);
    assert!(matches!(
        InductionMotor::from_nameplate(&bad, None, &d),
        Err(InputError::Inconsistent { .. })
    ));
    // Rated speed above synchronous.
    let fast = plate(1.5, 400.0, 3.6, 50.0, 1520.0, 0.80);
    assert!(InductionMotor::from_nameplate(&fast, Some(4), &d).is_err());
    for broken in [
        MotorNameplate {
            voltage_v: 0.0,
            ..ok
        },
        MotorNameplate {
            current_a: f64::NAN,
            ..ok
        },
        MotorNameplate { rpm: -1.0, ..ok },
        MotorNameplate {
            power_factor: 1.2,
            ..ok
        },
        MotorNameplate {
            frequency_hz: 0.0,
            ..ok
        },
    ] {
        assert!(
            InductionMotor::from_nameplate(&broken, None, &d).is_err(),
            "{broken:?}"
        );
    }
    assert!(InductionMotor::from_nameplate(&ok, Some(3), &d).is_err());
    assert!(
        InductionMotor::from_nameplate(
            &ok,
            None,
            &FitOptions {
                breakdown_ratio: Some(0.5),
                ..d
            }
        )
        .is_err()
    );
}

#[test]
fn bad_circuits_and_supplies_return_errors_not_nan() {
    assert!(Supply::new(f64::NAN, 400.0, Direction::Positive).is_err());
    assert!(Supply::new(-1.0, 400.0, Direction::Positive).is_err());
    assert!(Supply::new(50.0, f64::INFINITY, Direction::Positive).is_err());
    let bad = Circuit::new(0.45, 0.9, 0.0, 1.1, 25.0);
    assert!(InductionMotor::new(bad, 4, 400.0, 50.0, 1450.0).is_err());
    let c = Circuit::new(0.45, 0.9, 0.35, 1.1, 25.0);
    assert!(InductionMotor::new(c, 4, 400.0, 50.0, 1500.0).is_err());
    assert!(InductionMotor::new(c, 3, 400.0, 50.0, 1450.0).is_err());
    let m = case4(0.45);
    assert!(m.evaluate(&vf(50.0), f64::NAN).is_err());
    assert!(m.steady_state(&vf(50.0), f64::NAN).is_err());
    let mut w = 0.0;
    assert!(m.step_shaft(&mut w, &vf(50.0), 0.0, 0.1, |_| 0.0).is_err());
    assert!(m.step_shaft(&mut w, &vf(50.0), 1.0, -0.1, |_| 0.0).is_err());
    // Zero voltage: no current, no torque, no NaN.
    let z = m.at_slip(&supply(50.0, 0.0), 0.1).unwrap();
    assert!(z.torque_nm == 0.0 && z.stator_current_a == 0.0);
    // Slip exactly zero is finite.
    let p = m.at_slip(&vf(50.0), 0.0).unwrap();
    assert!(p.stator_current_a.is_finite() && p.torque_nm.abs() < 1e-3);
}

// ---- shaft dynamics -----------------------------------------------------------------

#[test]
fn shaft_reaches_the_steady_state_speed_and_the_energy_balance_closes() {
    let m = case4(0.45);
    let s = vf(50.0);
    let j = 0.5;
    let load = 20.0; // N m opposing
    let target = m.steady_state(&s, load).unwrap().unwrap();
    let mut w = 0.0;
    let (mut e_in, mut e_loss, mut w_load) = (0.0, 0.0, 0.0);
    for _ in 0..400 {
        let st = m.step_shaft(&mut w, &s, j, 0.01, |_| -load).unwrap();
        e_in += st.electrical_energy_j;
        e_loss += st.loss_energy_j;
        w_load += st.load_work_j;
    }
    let rpm = w * 60.0 / TAU;
    close(
        rpm,
        target.speed_rpm,
        1.0,
        "settles at the steady-state speed",
    );
    // E_in - losses = delta KE - work done by the load (negative here).
    let ke = 0.5 * j * w * w;
    let lhs = e_in - e_loss;
    let rhs = ke - w_load;
    assert!(
        (lhs - rhs).abs() < 2e-3 * lhs.abs(),
        "energy: in-loss {lhs} vs KE-work {rhs}"
    );
}

#[test]
fn shaft_step_is_independent_of_how_the_time_is_split() {
    let m = case4(0.45);
    let s = vf(30.0);
    let mut w1 = 5.0;
    m.step_shaft(&mut w1, &s, 0.8, 1.0, |_| -10.0).unwrap();
    let mut w2 = 5.0;
    for _ in 0..10 {
        m.step_shaft(&mut w2, &s, 0.8, 0.1, |_| -10.0).unwrap();
    }
    close(w1, w2, 2e-3, "one second in one or ten steps");
}

#[test]
fn deceleration_below_the_field_speed_feeds_energy_back() {
    // Frequency steps down while the rotor still turns fast: negative torque, negative power,
    // and the kinetic energy lost is accounted for.
    let m = case4(0.45);
    let j = 1.0;
    let mut w = 1450.0 * TAU / 60.0;
    let s = vf(40.0);
    let first = m.step_shaft(&mut w, &s, j, 0.01, |_| 0.0).unwrap();
    assert!(first.point.torque_nm < 0.0, "braking torque");
    assert!(first.point.input_power_w < 0.0, "feeds back");
    let ke0 = 0.5 * j * (1450.0 * TAU / 60.0f64).powi(2);
    let (mut e_in, mut e_loss) = (first.electrical_energy_j, first.loss_energy_j);
    for _ in 0..300 {
        let st = m.step_shaft(&mut w, &s, j, 0.01, |_| 0.0).unwrap();
        e_in += st.electrical_energy_j;
        e_loss += st.loss_energy_j;
    }
    let ke = 0.5 * j * w * w;
    let lhs = e_in - e_loss;
    let rhs = ke - ke0;
    assert!((lhs - rhs).abs() < 3e-3 * rhs.abs(), "{lhs} vs {rhs}");
    assert!(
        w * 60.0 / TAU < 1250.0,
        "has slowed towards the 1200 rpm field"
    );
}

// ---- joined with the drive engine (feedback of current and power) ----------------------

#[test]
fn drive_and_motor_close_the_loop_with_current_and_power_feedback() {
    let np = plate(1.5, 400.0, 3.6, 50.0, 1430.0, 0.80);
    let (motor, _) = InductionMotor::from_nameplate(&np, None, &FitOptions::typical()).unwrap();
    let mut cfg = DriveConfig::generic_travel(np, 7.0);
    cfg.mode = ControlMode::VoltsPerHertz;
    cfg.vf = VfCurve::new(400.0, 50.0);
    cfg.reference = ReferenceSource::Panel(40.0);
    cfg.macro_ = TerminalMacro::two_wire_presets();
    cfg.accel_time_s = 4.0;
    cfg.decel_time_s = 4.0;
    let mut drive = Drive::new(cfg);
    let j = 0.2;
    let load = 0.3; // N m friction-like
    let dt = 0.005;
    let mut w = 0.0;
    let mut inp = DriveInputs {
        di: vec![true, false, false, false, false, false],
        ..DriveInputs::default()
    };
    let mut feedback = (0.0, 0.0);
    let mut peak_i: f64 = 0.0;
    let mut min_power: f64 = 0.0;
    let mut last = None;
    for step in 0..6000 {
        // Run command for the first 20 s, then stop.
        if step == 4000 {
            inp.di = vec![false; 6];
        }
        inp.motor_current_a = feedback.0;
        inp.motor_power_w = feedback.1;
        let out = drive.step(&inp, dt);
        let dir = if out.frequency_hz < 0.0 {
            Direction::Negative
        } else {
            Direction::Positive
        };
        let sup = Supply::new(out.frequency_hz.abs(), out.voltage_v, dir).unwrap();
        let st = motor
            .step_shaft(&mut w, &sup, j, dt, |om| {
                -load * (om / 0.5).clamp(-1.0, 1.0)
            })
            .unwrap();
        feedback = (st.point.stator_current_a, st.point.input_power_w);
        peak_i = peak_i.max(feedback.0);
        min_power = min_power.min(feedback.1);
        last = Some((out, st));
        if step == 3999 {
            let (o, s) = last.as_ref().unwrap();
            assert_eq!(o.state, DriveState::Running);
            // Settled at 40 Hz: speed = 1200 rpm minus the slip for the load torque.
            assert!(
                s.point.speed_rpm > 1100.0 && s.point.speed_rpm < 1200.0,
                "{}",
                s.point.speed_rpm
            );
            assert!(s.point.stator_current_a < 3.6, "partly loaded");
        }
    }
    assert!(
        peak_i < 7.0 * 1.5,
        "stays inside the drive current limit: {peak_i}"
    );
    // After the stop command the motor decelerates on the ramp (regenerating at first).
    let (o, s) = last.unwrap();
    assert!(s.point.speed_rpm < 50.0 || o.state != DriveState::Running);
    assert!(min_power < 0.0, "motor fed energy back while slowing");
}

// ---- capability table -----------------------------------------------------------------

#[test]
fn frequency_row_follows_the_vf_law_and_the_regions() {
    let np = plate(1.5, 400.0, 3.6, 50.0, 1430.0, 0.80);
    let (m, _) = InductionMotor::from_nameplate(&np, None, &FitOptions::default()).unwrap();
    let vf = VfCurve::new(400.0, 50.0);
    let cool = SelfVentilation::typical();
    let i_lim = 1.5 * 3.6;
    let at50 = m.frequency_row(&vf, 50.0, i_lim, &cool).unwrap();
    close(at50.volts_ll, 400.0, 1e-9, "V/f at base");
    assert_eq!(at50.region, novo_drive::induction::Region::ConstantFlux);
    // Rated torque is reachable at 50 Hz and the speed is the nameplate speed.
    close(at50.rated_torque_rpm.unwrap(), 1430.0, 1e-3, "rated speed");
    close(
        at50.rated_torque_current_a.unwrap(),
        3.6,
        1e-6,
        "rated current",
    );
    let at100 = m.frequency_row(&vf, 100.0, i_lim, &cool).unwrap();
    assert_eq!(at100.region, novo_drive::induction::Region::FieldWeakening);
    close(at100.volts_ll, 400.0, 1e-9, "voltage capped");
    // Field weakening: flux about halves, the breakdown torque falls to about a quarter
    // (stator resistance makes it slightly better than the ideal 1/4).
    assert!(at100.flux_pu < 0.7, "flux {}", at100.flux_pu);
    let ratio = at100.breakdown_nm / at50.breakdown_nm;
    assert!(ratio > 0.2 && ratio < 0.32, "breakdown ratio {ratio}");
    // Rated torque is no longer reachable at 100 Hz on 400 V.
    assert!(at100.rated_torque_rpm.is_none());
    // The current limit never lets the current exceed itself, and power rises then flattens.
    let at75 = m.frequency_row(&vf, 75.0, i_lim, &cool).unwrap();
    assert!(at75.power_at_limit_w > 0.0 && at100.power_at_limit_w > 0.0);
    assert!(m.frequency_row(&vf, 50.0, -1.0, &cool).is_err());
}

#[test]
fn deriving_poles_from_a_tiny_rpm_is_refused_not_overflowed() {
    let d = FitOptions::default();
    for rpm in [1e-9, 1e-3, 1.0, 200.0] {
        let np = plate(1.5, 400.0, 3.6, 50.0, rpm, 0.80);
        assert!(
            matches!(
                InductionMotor::from_nameplate(&np, None, &d),
                Err(InputError::OutOfRange { field: "rpm", .. })
            ),
            "rpm {rpm}"
        );
    }
    // The message carries the true bound: 60 f / 13 = 230.77 rpm, below the 2 pole speed 3000 rpm.
    let np = plate(1.5, 400.0, 3.6, 50.0, 200.0, 0.80);
    let msg = InductionMotor::from_nameplate(&np, None, &d)
        .err()
        .unwrap()
        .to_string();
    assert!(msg.contains("230.7"), "{msg}");
    assert!(msg.contains("3000"), "{msg}");
    // A very slow 24-pole nameplate (250 rpm at 50 Hz) is still derived.
    let np = plate(1.5, 400.0, 3.6, 50.0, 245.0, 0.80);
    assert!(!matches!(
        InductionMotor::from_nameplate(&np, None, &d),
        Err(InputError::OutOfRange { field: "rpm", .. })
    ));
}
