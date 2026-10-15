//! Induction motor model: nameplate fit, torque against the textbook
//! Thevenin form, and the standard starters.

use std::f64::consts::PI;

use novo_sim::analog::Complex;
use novo_sim::analog::motor::{
    Changeover, Connection, Load, Motor, Nameplate, Stage, Starter, Supply, start,
};

fn close(got: f64, want: f64, rel: f64) {
    let tol = rel * want.abs().max(1e-12);
    assert!((got - want).abs() <= tol, "got {got}, want {want} (±{tol})");
}

/// A typical 7.5 kW, 400 V, 4-pole, 50 Hz motor.
fn plate() -> Nameplate {
    Nameplate {
        power_w: 7_500.0,
        v_ll: 400.0,
        hz: 50.0,
        rpm: 1_450.0,
        current: 14.6,
        power_factor: 0.83,
        efficiency: 0.893,
        lr_current_ratio: 7.0,
        connection: Connection::Delta,
    }
}

fn motor() -> Motor {
    Motor::from_nameplate(plate()).expect("a consistent nameplate fits")
}

#[test]
fn nameplate_fit_reproduces_the_nameplate() {
    let m = motor();
    assert_eq!(m.pole_pairs, 2);
    close(m.rated_current(), 14.6, 2e-3);
    close(m.locked_rotor_current(), 14.6 * 7.0, 2e-3);
    let p = m.point(
        Supply::stiff(400.0),
        Connection::Delta,
        1.0,
        false,
        m.rated_slip(),
    );
    close(p.power_factor, 0.83, 2e-3);
    close(p.torque * m.rated_rpm * 2.0 * PI / 60.0, 7_500.0, 2e-3);
    // The model, not the nameplate, supplies these; they must be plausible
    // for a design B motor.
    let (bd, _) = m.breakdown();
    let ratio_lr = m.locked_rotor_torque() / m.rated_torque();
    let ratio_bd = bd / m.rated_torque();
    assert!((0.8..3.0).contains(&ratio_lr), "T_LR/T_N = {ratio_lr}");
    assert!((1.8..4.0).contains(&ratio_bd), "T_BD/T_N = {ratio_bd}");
}

#[test]
fn contradicting_nameplates_are_refused() {
    let mut p = plate();
    p.efficiency = 0.5; // 7.5 kW out of 8.4 kW in does not fit 50 %
    assert!(Motor::from_nameplate(p).is_err());
    p = plate();
    p.current = -1.0;
    assert!(Motor::from_nameplate(p).is_err());
}

#[test]
fn torque_matches_the_textbook_thevenin_form() {
    let m = motor();
    let vw = 400.0;
    // Thevenin equivalent seen by the rotor branch.
    let z1 = Complex::new(m.r1, m.x1);
    let zm = Complex::new(0.0, m.xm);
    let zth = z1 * zm / (z1 + zm);
    let vth = Complex::real(vw) * zm / (z1 + zm);
    for s in [0.01, 0.03, 0.1, 0.3, 0.7, 1.0] {
        let t = 3.0 * vth.abs().powi(2) * (m.r2 / s)
            / (m.omega_sync() * ((zth.re + m.r2 / s).powi(2) + (zth.im + m.x2).powi(2)));
        close(m.torque(vw, s), t, 1e-9);
    }
    // Breakdown slip s = r2 / sqrt(rth² + (xth + x2)²).
    let (_, s_bd) = m.breakdown();
    close(
        s_bd,
        m.r2 / (zth.re.powi(2) + (zth.im + m.x2).powi(2)).sqrt(),
        2e-3,
    );
}

#[test]
fn torque_scales_with_voltage_squared() {
    let m = motor();
    close(m.torque(200.0, 0.05) * 4.0, m.torque(400.0, 0.05), 1e-9);
}

#[test]
fn star_start_draws_a_third_of_the_current_and_torque() {
    let m = motor();
    let s = Supply::stiff(400.0);
    let dol = m.point(s, Connection::Delta, 1.0, false, 1.0);
    let star = m.point(s, Connection::Star, 1.0, false, 1.0);
    close(star.line_current, dol.line_current / 3.0, 1e-9);
    close(star.torque, dol.torque / 3.0, 1e-9);
}

#[test]
fn direct_start_settles_where_motor_and_load_torque_meet() {
    let m = motor();
    let r = start(
        &m,
        Supply::stiff(400.0),
        Starter::Direct,
        Load::Constant(m.rated_torque()),
        0.15,
        6.0,
        1e-3,
    );
    assert!(!r.stalled);
    close(r.final_rpm, 1_450.0, 3e-3);
    let t = r.start_time.expect("it starts");
    assert!(t > 0.1 && t < 3.0, "start time {t}");
    close(r.peak_line_current, m.locked_rotor_current(), 1e-6);
}

#[test]
fn start_time_follows_inertia() {
    let m = motor();
    let run = |j| {
        start(
            &m,
            Supply::stiff(400.0),
            Starter::Direct,
            Load::Constant(0.0),
            j,
            20.0,
            1e-3,
        )
        .start_time
        .unwrap()
    };
    let t1 = run(0.1);
    let t2 = run(0.2);
    close(t2 / t1, 2.0, 0.02);
    // With no load the acceleration time is J * w / T_avg; the average
    // accelerating torque lies between the locked-rotor and breakdown torque.
    let (bd, _) = m.breakdown();
    let w = m.omega_sync();
    assert!(t1 > 0.1 * w / bd && t1 < 0.1 * w / m.locked_rotor_torque().min(bd));
}

#[test]
fn overloaded_motor_stalls_and_keeps_drawing_locked_rotor_current() {
    let m = motor();
    let r = start(
        &m,
        Supply::stiff(400.0),
        Starter::Direct,
        Load::Constant(10.0 * m.rated_torque()),
        0.15,
        3.0,
        1e-3,
    );
    assert!(r.stalled);
    assert!(r.start_time.is_none());
    close(
        r.samples.last().unwrap().line_current,
        m.locked_rotor_current(),
        1e-6,
    );
    // i²t is the heating the protection sees: locked current squared times 3 s.
    close(r.i2t, m.locked_rotor_current().powi(2) * 3.0, 2e-3);
}

#[test]
fn star_delta_start_cuts_the_inrush_and_changes_over() {
    let m = motor();
    let load = Load::Quadratic(m.rated_torque() * 1.1);
    let supply = Supply::stiff(400.0);
    let dol = start(&m, supply, Starter::Direct, load, 0.2, 20.0, 1e-3);
    let sd = start(
        &m,
        supply,
        Starter::StarDelta {
            changeover: Changeover::AtSpeed(0.85),
            open_s: 0.08,
        },
        load,
        0.2,
        20.0,
        1e-3,
    );
    assert!(!sd.stalled);
    close(
        sd.samples[0].line_current,
        dol.peak_line_current / 3.0,
        1e-6,
    );
    // The changeover to delta at speed draws more than the star start did,
    // but still well below direct-on-line.
    assert!(sd.peak_line_current < 0.9 * dol.peak_line_current);
    close(sd.final_rpm, dol.final_rpm, 2e-3);
    let stages: Vec<_> = sd.samples.iter().map(|s| s.stage).collect();
    assert_eq!(stages[0], Stage::First);
    assert!(stages.contains(&Stage::Open));
    assert_eq!(*stages.last().unwrap(), Stage::Run);
    // Speed at changeover is at least the 85 % asked for.
    let open = sd.samples.iter().find(|s| s.stage == Stage::Open).unwrap();
    assert!(open.rpm >= 0.85 * m.sync_rpm() * 0.999);
    // Off the supply there is no current.
    assert_eq!(open.line_current, 0.0);
}

#[test]
fn weak_supply_dips_the_voltage_and_the_torque() {
    let m = motor();
    let stiff = m.point(Supply::stiff(400.0), Connection::Delta, 1.0, false, 1.0);
    let weak = m.point(
        Supply {
            v_ll: 400.0,
            source: Complex::new(0.15, 0.25),
        },
        Connection::Delta,
        1.0,
        false,
        1.0,
    );
    assert!(weak.winding_voltage < 0.9 * stiff.winding_voltage);
    assert!(weak.line_current < stiff.line_current);
    close(
        weak.torque / stiff.torque,
        (weak.winding_voltage / stiff.winding_voltage).powi(2),
        1e-9,
    );
}

#[test]
fn soft_start_trades_peak_current_for_time() {
    let m = motor();
    let load = Load::Quadratic(m.rated_torque() * 1.1);
    let supply = Supply::stiff(400.0);
    let dol = start(&m, supply, Starter::Direct, load, 0.2, 20.0, 1e-3);
    let soft = start(
        &m,
        supply,
        Starter::SoftStart {
            initial: 0.4,
            ramp_s: 6.0,
        },
        load,
        0.2,
        20.0,
        1e-3,
    );
    assert!(!soft.stalled);
    assert!(soft.peak_line_current < 0.6 * dol.peak_line_current);
    assert!(soft.start_time.unwrap() > dol.start_time.unwrap());
}

#[test]
fn auto_transformer_start_scales_line_current_by_the_tap_squared() {
    let m = motor();
    let s = Supply::stiff(400.0);
    let tap = 0.65;
    let dol = m.point(s, Connection::Delta, 1.0, false, 1.0);
    let at = m.point(s, Connection::Delta, tap, true, 1.0);
    close(at.line_current, dol.line_current * tap * tap, 1e-9);
    close(at.torque, dol.torque * tap * tap, 1e-9);
}
