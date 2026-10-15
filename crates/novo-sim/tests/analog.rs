//! The analog solver against textbook circuits with closed-form answers.

use std::f64::consts::PI;

use novo_sim::analog::{
    Circuit, Complex, Element, Error, Event, GROUND, Method, Wave, solve_ac, solve_dc, transient,
};

fn close(got: f64, want: f64, rel: f64) {
    let tol = rel * want.abs().max(1e-12);
    assert!((got - want).abs() <= tol, "got {got}, want {want} (±{tol})");
}

#[test]
fn voltage_divider() {
    let mut c = Circuit::new();
    let (top, mid) = (c.node(), c.node());
    c.voltage(top, GROUND, Wave::Dc(12.0));
    c.resistor(top, mid, 1_000.0);
    c.resistor(mid, GROUND, 2_000.0);
    let s = solve_dc(&c).unwrap();
    close(s.voltage(mid).re, 8.0, 1e-7);
    close(s.current(1).re, 4e-3, 1e-7);
    // The source delivers: current p -> n through it is negative.
    close(s.current(0).re, -4e-3, 1e-7);
}

#[test]
fn wheatstone_bridge_balance_and_offset() {
    // R1/R2 = R3/R4 balances; with R4 = 1.1k the detector sees a known voltage.
    let build = |r4: f64| {
        let mut c = Circuit::new();
        let (top, a, b) = (c.node(), c.node(), c.node());
        c.voltage(top, GROUND, Wave::Dc(10.0));
        c.resistor(top, a, 1_000.0);
        c.resistor(a, GROUND, 1_000.0);
        c.resistor(top, b, 1_000.0);
        c.resistor(b, GROUND, r4);
        (solve_dc(&c).unwrap(), a, b)
    };
    let (s, a, b) = build(1_000.0);
    assert!(s.between(a, b).abs() < 1e-9);
    let (s, a, b) = build(1_100.0);
    // 5 V on one side, 10 * 1100/2100 on the other.
    close(s.between(b, a).re, 10.0 * 1100.0 / 2100.0 - 5.0, 1e-7);
}

#[test]
fn thevenin_equivalent_of_a_loaded_network() {
    // 10 V behind 100 ohm with a 100 ohm shunt: Vth = 5 V, Rth = 50 ohm.
    // Loaded with 50 ohm the load sees 2.5 V (maximum power transfer point).
    let mut c = Circuit::new();
    let (src, out) = (c.node(), c.node());
    c.voltage(src, GROUND, Wave::Dc(10.0));
    c.resistor(src, out, 100.0);
    c.resistor(out, GROUND, 100.0);
    c.resistor(out, GROUND, 50.0);
    close(
        solve_dc(&c).unwrap().voltage(out).re,
        10.0 * 33.333333333 / 133.333333333,
        1e-6,
    );
}

#[test]
fn rc_charging_follows_one_minus_exp() {
    let mut c = Circuit::new();
    let (src, out) = (c.node(), c.node());
    c.voltage(src, GROUND, Wave::Dc(10.0));
    c.resistor(src, out, 1_000.0);
    c.capacitor(out, GROUND, 1e-6);
    let tau = 1e-3;
    let t = transient(&c, 1e-6, 5.0 * tau, Method::Trapezoidal, &[]).unwrap();
    for k in [1.0, 2.0, 3.0, 5.0] {
        let i = t.index_at(k * tau);
        close(t.voltages[i][out], 10.0 * (1.0 - (-k).exp()), 2e-3);
    }
    // Charging current at tau: (V/R) e^-1.
    let i = t.index_at(tau);
    close(t.currents[i][1], 10.0 / 1000.0 * (-1.0f64).exp(), 5e-3);
}

#[test]
fn rl_current_rise() {
    let mut c = Circuit::new();
    let (src, out) = (c.node(), c.node());
    c.voltage(src, GROUND, Wave::Dc(24.0));
    c.resistor(src, out, 12.0);
    let l = c.inductor(out, GROUND, 0.5);
    let tau = 0.5 / 12.0;
    let t = transient(&c, tau / 2000.0, 4.0 * tau, Method::Trapezoidal, &[]).unwrap();
    for k in [1.0, 2.0, 4.0] {
        let i = t.index_at(k * tau);
        close(t.currents[i][l], 2.0 * (1.0 - (-k).exp()), 2e-3);
    }
}

#[test]
fn series_rlc_resonance() {
    let (r, l, cap) = (10.0f64, 10e-3f64, 1e-6f64);
    let f0 = 1.0 / (2.0 * PI * (l * cap).sqrt());
    let mut c = Circuit::new();
    let (src, a, b) = (c.node(), c.node(), c.node());
    c.voltage(src, GROUND, Wave::rms(10.0, f0, 0.0));
    c.resistor(src, a, r);
    c.inductor(a, b, l);
    c.capacitor(b, GROUND, cap);
    let s = solve_ac(&c, f0).unwrap();
    let i = s.current(1);
    close(i.abs(), 1.0, 1e-6);
    assert!(i.arg().abs() < 1e-6, "in phase at resonance");
    // Quality factor Q = w0 L / R multiplies the voltage across C.
    let q = 2.0 * PI * f0 * l / r;
    close(s.voltage(b).abs(), 10.0 * q, 1e-6);
    assert!((s.voltage(b).arg() + PI / 2.0).abs() < 1e-6);
    // Below resonance the circuit is capacitive: current leads.
    let s = solve_ac(
        &{
            let mut c2 = Circuit::new();
            let (src, a, b) = (c2.node(), c2.node(), c2.node());
            c2.voltage(src, GROUND, Wave::rms(10.0, 50.0, 0.0));
            c2.resistor(src, a, r);
            c2.inductor(a, b, l);
            c2.capacitor(b, GROUND, cap);
            c2
        },
        50.0,
    )
    .unwrap();
    assert!(s.current(1).arg() > 0.0);
}

#[test]
fn ac_complex_power_balance() {
    // 230 V into R = 20, X_L = 15: Z = 25 at 36.87 deg, S = V^2 / Z*.
    let hz = 50.0;
    let mut c = Circuit::new();
    let (src, mid) = (c.node(), c.node());
    c.voltage(src, GROUND, Wave::rms(230.0, hz, 0.0));
    c.resistor(src, mid, 20.0);
    c.inductor(mid, GROUND, 15.0 / (2.0 * PI * hz));
    let s = solve_ac(&c, hz).unwrap();
    let i = -s.current(0);
    let power = s.voltage(src) * i.conj();
    close(power.re, 230.0 * 230.0 / 25.0 * (20.0 / 25.0), 1e-9);
    close(power.im, 230.0 * 230.0 / 25.0 * (15.0 / 25.0), 1e-9);
    // P equals I^2 R.
    close(power.re, i.abs().powi(2) * 20.0, 1e-9);
}

fn three_phase(c: &mut Circuit, vll: f64, hz: f64) -> [usize; 3] {
    let vph = vll / 3f64.sqrt();
    let l = [c.node(), c.node(), c.node()];
    for (k, &n) in l.iter().enumerate() {
        c.voltage(n, GROUND, Wave::rms(vph, hz, -120.0 * k as f64));
    }
    l
}

#[test]
fn balanced_star_load_floating_neutral() {
    let (vll, hz) = (400.0, 50.0);
    let mut c = Circuit::new();
    let lines = three_phase(&mut c, vll, hz);
    let star = c.node();
    let mut loads = Vec::new();
    for &n in &lines {
        let mid = c.node();
        loads.push(c.resistor(n, mid, 10.0));
        c.inductor(mid, star, 5.0 / (2.0 * PI * hz));
    }
    let s = solve_ac(&c, hz).unwrap();
    assert!(
        s.voltage(star).abs() < 1e-6,
        "balanced load has no neutral shift"
    );
    let z = (10f64 * 10.0 + 5.0 * 5.0).sqrt();
    let iline = vll / 3f64.sqrt() / z;
    for &r in &loads {
        close(s.current(r).abs(), iline, 1e-9);
    }
    // P = sqrt(3) VLL I cos(phi).
    let p: f64 = loads
        .iter()
        .map(|&r| s.current(r).abs().powi(2) * 10.0)
        .sum();
    close(p, 3f64.sqrt() * vll * iline * (10.0 / z), 1e-9);
}

#[test]
fn delta_load_line_current_is_root_three_phase_current() {
    let (vll, hz) = (400.0, 50.0);
    let mut c = Circuit::new();
    let l = three_phase(&mut c, vll, hz);
    let ph = [
        c.resistor(l[0], l[1], 20.0),
        c.resistor(l[1], l[2], 20.0),
        c.resistor(l[2], l[0], 20.0),
    ];
    let s = solve_ac(&c, hz).unwrap();
    let iph = vll / 20.0;
    close(s.current(ph[0]).abs(), iph, 1e-9);
    // Line current = phasor difference of two phase currents.
    let iline = s.current(ph[0]) - s.current(ph[2]);
    close(iline.abs(), iph * 3f64.sqrt(), 1e-9);
}

#[test]
fn open_phase_on_a_three_wire_star_splits_the_line_voltage() {
    // One star leg open: the other two are in series across VLL.
    let (vll, hz) = (400.0, 50.0);
    let mut c = Circuit::new();
    let l = three_phase(&mut c, vll, hz);
    let star = c.node();
    let a = c.resistor(l[0], star, 10.0);
    c.resistor(l[1], star, 10.0);
    let s = solve_ac(&c, hz).unwrap();
    close(s.current(a).abs(), vll / 20.0, 1e-7);
}

#[test]
fn ideal_transformer_ratio_and_reflected_load() {
    let mut c = Circuit::new();
    let (p, s) = (c.node(), c.node());
    c.voltage(p, GROUND, Wave::rms(230.0, 50.0, 0.0));
    c.add(Element::IdealTransformer {
        p1: p,
        p2: GROUND,
        s1: s,
        s2: GROUND,
        ratio: 10.0,
    });
    let load = c.resistor(s, GROUND, 2.3);
    let sol = solve_ac(&c, 50.0).unwrap();
    close(sol.voltage(s).abs(), 23.0, 1e-9);
    close(sol.current(load).abs(), 10.0, 1e-9);
    // Primary current = 10 A / 10 = 1 A (load reflected: 2.3 * 100 = 230 ohm).
    close(sol.current(0).abs(), 1.0, 1e-9);
}

#[test]
fn coupled_inductors_make_a_real_transformer() {
    // 10:1 by turns: L1 = 100 L2. Tightly coupled, loaded lightly.
    let hz = 50.0;
    let mut c = Circuit::new();
    let (p, s) = (c.node(), c.node());
    c.voltage(p, GROUND, Wave::rms(230.0, hz, 0.0));
    let l1 = c.inductor(p, GROUND, 10.0);
    let l2 = c.inductor(s, GROUND, 0.1);
    c.add(Element::Coupling { l1, l2, k: 0.9999 });
    c.resistor(s, GROUND, 100.0);
    let sol = solve_ac(&c, hz).unwrap();
    close(sol.voltage(s).abs(), 23.0, 5e-3);
    // Magnetising current alone: V / (w L1), 0.0732 A, plus the reflected load.
    let i_mag = 230.0 / (2.0 * PI * hz * 10.0);
    assert!(sol.current(l1).abs() > i_mag);
}

#[test]
fn contradicting_sources_have_no_solution() {
    let mut c = Circuit::new();
    let a = c.node();
    c.voltage(a, GROUND, Wave::Dc(5.0));
    c.voltage(a, GROUND, Wave::Dc(6.0));
    assert_eq!(solve_dc(&c).unwrap_err(), Error::Singular);
}

#[test]
fn bad_values_are_refused_not_solved() {
    let mut c = Circuit::new();
    let a = c.node();
    c.resistor(a, GROUND, -3.0);
    assert_eq!(solve_dc(&c).unwrap_err(), Error::BadValue(0));
}

#[test]
fn floating_part_solves_to_zero_volts() {
    let mut c = Circuit::new();
    let (a, b) = (c.node(), c.node());
    c.resistor(a, b, 10.0);
    let s = solve_dc(&c).unwrap();
    assert!(s.voltage(a).abs() < 1e-6);
}

#[test]
fn switch_event_in_a_transient() {
    // RC charges only after the switch closes at 2 ms.
    let mut c = Circuit::new();
    let (src, mid, out) = (c.node(), c.node(), c.node());
    c.voltage(src, GROUND, Wave::Dc(10.0));
    let sw = c.switch(src, mid, false);
    c.resistor(mid, out, 1_000.0);
    c.capacitor(out, GROUND, 1e-6);
    let ev = [Event {
        time: 2e-3,
        element: sw,
        closed: true,
    }];
    let t = transient(&c, 2e-6, 8e-3, Method::Trapezoidal, &ev).unwrap();
    assert!(t.voltages[t.index_at(1.9e-3)][out].abs() < 1e-3);
    close(
        t.voltages[t.index_at(3e-3)][out],
        10.0 * (1.0 - (-1.0f64).exp()),
        5e-3,
    );
}

#[test]
fn transient_sine_matches_ac_steady_state() {
    // After the start-up transient dies out a 50 Hz RL circuit settles to the
    // AC phasor solution.
    let hz = 50.0;
    let (r, l) = (10.0, 20e-3);
    let mut c = Circuit::new();
    let (src, mid) = (c.node(), c.node());
    c.voltage(src, GROUND, Wave::rms(100.0, hz, 0.0));
    c.resistor(src, mid, r);
    let ind = c.inductor(mid, GROUND, l);
    let ac = solve_ac(&c, hz).unwrap();
    let want_peak = ac.current(ind).abs() * 2f64.sqrt();
    let t = transient(&c, 1e-5, 0.6, Method::Trapezoidal, &[]).unwrap();
    let tail = &t.current(ind)[(0.5 / 1e-5) as usize..];
    let peak = tail.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    close(peak, want_peak, 5e-3);
    let _ = Complex::ZERO;
}
