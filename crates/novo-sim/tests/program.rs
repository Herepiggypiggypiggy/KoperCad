//! The PLC program interpreter against the circuits a control cabinet runs:
//! seal-in, timers, counters, star-delta, a state machine.

use novo_sim::program::{Address, Program, Runtime, Value};

fn rt(src: &str) -> Runtime {
    match Program::parse(src) {
        Ok(p) => Runtime::new(p),
        Err(e) => panic!("{e}\n--- source ---\n{src}"),
    }
}

fn a(text: &str) -> Address {
    Address::parse(text).unwrap()
}

/// Scans `ms` milliseconds in 10 ms steps.
fn run_ms(r: &mut Runtime, ms: i64) {
    for _ in 0..ms / 10 {
        r.scan(10_000).unwrap();
    }
}

fn press(r: &mut Runtime, input: &str) {
    r.set_input_bool(a(input), true);
    r.scan(10_000).unwrap();
    r.set_input_bool(a(input), false);
    r.scan(10_000).unwrap();
}

const SEAL_IN: &str = r#"
PROGRAM Motor
VAR
    start AT %IX0.0 : BOOL;
    stop  AT %IX0.1 : BOOL;     (* stop button, pressed = TRUE *)
    ovl   AT %IX0.2 : BOOL;     (* overload relay, tripped = TRUE *)
    run   AT %QX0.0 : BOOL;
END_VAR
RUNG "Seal-in"
    { [start] | [run] } -- [/stop] -- [/ovl] -- (run);
END_PROGRAM
"#;

#[test]
fn seal_in_holds_after_the_button_is_released() {
    let mut r = rt(SEAL_IN);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")));
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")), "start latched the motor");
    run_ms(&mut r, 500);
    assert!(r.output_bool(a("%QX0.0")), "and it stays on");
    press(&mut r, "%IX0.1");
    assert!(!r.output_bool(a("%QX0.0")), "stop releases it");
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")));
    r.set_input_bool(a("%IX0.2"), true);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")), "overload trips it");
    r.set_input_bool(a("%IX0.2"), false);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")), "and it does not restart by itself");
}

#[test]
fn seal_in_in_structured_text_behaves_the_same() {
    let mut r = rt(r#"
        VAR start AT %IX0.0 : BOOL; stop AT %IX0.1 : BOOL; run AT %QX0.0 : BOOL; END_VAR
        run := (start OR run) AND NOT stop;
    "#);
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")));
    press(&mut r, "%IX0.1");
    assert!(!r.output_bool(a("%QX0.0")));
}

#[test]
fn ladder_probes_show_where_power_flows() {
    let mut r = rt(SEAL_IN);
    let rung = r.program().rungs[0].clone();
    r.scan(10_000).unwrap();
    // Nothing started: the branch passes no power, so no element after it does.
    assert!(rung.elements.iter().all(|e| !r.power(e.probe)));
    r.set_input_bool(a("%IX0.0"), true);
    r.scan(10_000).unwrap();
    assert!(rung.elements.iter().all(|e| r.power(e.probe)));
    // Released: power now flows through the seal-in contact only.
    r.set_input_bool(a("%IX0.0"), false);
    r.scan(10_000).unwrap();
    assert!(r.power(rung.elements.last().unwrap().probe));
}

#[test]
fn on_delay_timer_counts_real_time() {
    let mut r = rt(r#"
        VAR go AT %IX0.0 : BOOL; t : TON; lamp AT %QX0.0 : BOOL; END_VAR
        t(IN := go, PT := T#2s500ms);
        lamp := t.Q;
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 2_000);
    assert!(!r.output_bool(a("%QX0.0")), "not yet at 2 s");
    assert_eq!(
        r.block_value("t", "ET"),
        Some(Value::Time(2_000_000 - 10_000))
    );
    run_ms(&mut r, 600);
    assert!(r.output_bool(a("%QX0.0")), "on after 2.5 s");
    assert_eq!(r.block_value("t", "ET"), Some(Value::Time(2_500_000)));
    r.set_input_bool(a("%IX0.0"), false);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")), "drops at once");
    // A short pulse never gets there.
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 1_000);
    r.set_input_bool(a("%IX0.0"), false);
    run_ms(&mut r, 100);
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 2_000);
    assert!(!r.output_bool(a("%QX0.0")), "the timer restarted");
}

#[test]
fn off_delay_timer_holds_the_output() {
    let mut r = rt(r#"
        VAR fan_req AT %IX0.0 : BOOL; t : TOF; fan AT %QX0.0 : BOOL; END_VAR
        t(IN := fan_req, PT := T#3s);
        fan := t.Q;
    "#);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")));
    r.set_input_bool(a("%IX0.0"), true);
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")));
    r.set_input_bool(a("%IX0.0"), false);
    run_ms(&mut r, 2_900);
    assert!(r.output_bool(a("%QX0.0")), "still running on");
    run_ms(&mut r, 200);
    assert!(!r.output_bool(a("%QX0.0")), "off after 3 s");
}

#[test]
fn pulse_timer_gives_a_fixed_pulse_and_ignores_retriggers() {
    let mut r = rt(r#"
        VAR trig AT %IX0.0 : BOOL; t : TP; horn AT %QX0.0 : BOOL; END_VAR
        t(IN := trig, PT := T#1s);
        horn := t.Q;
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")));
    // Wiggle the input during the pulse: no effect.
    r.set_input_bool(a("%IX0.0"), false);
    run_ms(&mut r, 300);
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 300);
    assert!(r.output_bool(a("%QX0.0")));
    run_ms(&mut r, 500);
    assert!(!r.output_bool(a("%QX0.0")), "pulse ended after 1 s");
}

#[test]
fn up_counter_counts_edges_and_resets() {
    let mut r = rt(r#"
        VAR
            sensor AT %IX0.0 : BOOL;
            reset  AT %IX0.1 : BOOL;
            c : CTU;
            full AT %QX0.0 : BOOL;
        END_VAR
        c(CU := sensor, R := reset, PV := 3);
        full := c.Q;
    "#);
    for n in 1..=3 {
        press(&mut r, "%IX0.0");
        assert_eq!(r.block_value("c", "CV"), Some(Value::Int(n)));
    }
    assert!(r.output_bool(a("%QX0.0")));
    // Holding the input high counts once, not every scan.
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 100);
    assert_eq!(r.block_value("c", "CV"), Some(Value::Int(4)));
    r.set_input_bool(a("%IX0.0"), false);
    press(&mut r, "%IX0.1");
    assert_eq!(r.block_value("c", "CV"), Some(Value::Int(0)));
    assert!(!r.output_bool(a("%QX0.0")));
}

#[test]
fn down_counter_loads_and_counts_to_zero() {
    let mut r = rt(r#"
        VAR use AT %IX0.0 : BOOL; load AT %IX0.1 : BOOL; c : CTD; empty AT %QX0.0 : BOOL; END_VAR
        c(CD := use, LD := load, PV := 2);
        empty := c.Q;
    "#);
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")), "CV 0 is empty");
    press(&mut r, "%IX0.1");
    assert_eq!(r.block_value("c", "CV"), Some(Value::Int(2)));
    assert!(!r.output_bool(a("%QX0.0")));
    press(&mut r, "%IX0.0");
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")));
}

#[test]
fn up_down_counter_in_ladder_boxes() {
    let mut r = rt(r#"
        VAR
            in_gate AT %IX0.0 : BOOL;
            out_gate AT %IX0.1 : BOOL;
            c : CTUD;
            places AT %QW2 : INT;
            full AT %QX0.0 : BOOL;
        END_VAR
        c(CU := in_gate, CD := out_gate, PV := 2);
        places := c.CV;
        full := c.QU;
    "#);
    press(&mut r, "%IX0.0");
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")), "car park full at 2");
    assert_eq!(r.output(a("%QW2")), Value::Int(2));
    press(&mut r, "%IX0.1");
    assert!(!r.output_bool(a("%QX0.0")));
    assert_eq!(r.output(a("%QW2")), Value::Int(1));
}

/// Star-delta start: main contactor and star on first, star off after the
/// start time, delta on after a changeover pause. Star and delta interlock.
const STAR_DELTA: &str = r#"
PROGRAM StarDelta
VAR
    start   AT %IX0.0 : BOOL;
    stop    AT %IX0.1 : BOOL;
    ovl     AT %IX0.2 : BOOL;
    k_main  AT %QX0.0 : BOOL;
    k_star  AT %QX0.1 : BOOL;
    k_delta AT %QX0.2 : BOOL;
    running : BOOL;
    t_star  : TON;
    t_gap   : TON;
END_VAR

RUNG "Start latch"
    { [start] | [running] } -- [/stop] -- [/ovl] -- (running);

(* Star time 5 s, then a 100 ms pause before delta closes. *)
RUNG "Star time"
    [running] -- <t_star: TON, PT := T#5s>;
RUNG "Changeover gap"
    [t_star.Q] -- <t_gap: TON, PT := T#100ms>;

RUNG "Main contactor"   [running] -- (k_main);
RUNG "Star contactor"   [running] -- [/t_star.Q] -- [/k_delta] -- (k_star);
RUNG "Delta contactor"  [running] -- [t_gap.Q] -- [/k_star] -- (k_delta);
END_PROGRAM
"#;

#[test]
fn star_delta_start_sequence() {
    let mut r = rt(STAR_DELTA);
    let (main, star, delta) = (a("%QX0.0"), a("%QX0.1"), a("%QX0.2"));
    run_ms(&mut r, 100);
    assert!(!r.output_bool(main) && !r.output_bool(star) && !r.output_bool(delta));

    press(&mut r, "%IX0.0");
    assert!(r.output_bool(main) && r.output_bool(star) && !r.output_bool(delta));

    run_ms(&mut r, 4_900);
    assert!(
        r.output_bool(star) && !r.output_bool(delta),
        "still in star after ~5 s"
    );
    // Through the changeover: star must drop before delta closes.
    let mut seen_gap = false;
    for _ in 0..40 {
        r.scan(10_000).unwrap();
        assert!(
            !(r.output_bool(star) && r.output_bool(delta)),
            "star and delta never close together"
        );
        if !r.output_bool(star) && !r.output_bool(delta) {
            seen_gap = true;
        }
    }
    assert!(seen_gap, "there is a pause between star and delta");
    assert!(r.output_bool(main) && !r.output_bool(star) && r.output_bool(delta));

    press(&mut r, "%IX0.1");
    assert!(!r.output_bool(main) && !r.output_bool(star) && !r.output_bool(delta));

    // Restarts in star, with the timers starting over.
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(star) && !r.output_bool(delta));
}

#[test]
fn overload_drops_the_whole_star_delta_start() {
    let mut r = rt(STAR_DELTA);
    press(&mut r, "%IX0.0");
    run_ms(&mut r, 6_000);
    assert!(r.output_bool(a("%QX0.2")));
    r.set_input_bool(a("%IX0.2"), true);
    r.scan(10_000).unwrap();
    for out in ["%QX0.0", "%QX0.1", "%QX0.2"] {
        assert!(!r.output_bool(a(out)), "{out} dropped");
    }
}

#[test]
fn edge_contacts_fire_for_one_scan() {
    let mut r = rt(r#"
        VAR btn AT %IX0.0 : BOOL; hits : INT; END_VAR
        RUNG [P btn] -- (hits := hits + 1);
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 200);
    assert_eq!(r.get("hits"), Some(Value::Int(1)));
    r.set_input_bool(a("%IX0.0"), false);
    run_ms(&mut r, 100);
    r.set_input_bool(a("%IX0.0"), true);
    run_ms(&mut r, 100);
    assert_eq!(r.get("hits"), Some(Value::Int(2)));
}

#[test]
fn set_and_reset_coils_latch() {
    let mut r = rt(r#"
        VAR a_btn AT %IX0.0 : BOOL; b_btn AT %IX0.1 : BOOL; q AT %QX0.0 : BOOL; END_VAR
        RUNG [a_btn] -- (S q);
        RUNG [b_btn] -- (R q);
    "#);
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")));
    run_ms(&mut r, 100);
    assert!(r.output_bool(a("%QX0.0")));
    press(&mut r, "%IX0.1");
    assert!(!r.output_bool(a("%QX0.0")));
}

#[test]
fn nested_branches_and_compare_contacts() {
    let mut r = rt(r#"
        VAR
            a1 AT %IX0.0 : BOOL; a2 AT %IX0.1 : BOOL; b1 AT %IX0.2 : BOOL;
            level AT %IW4 : INT;
            q AT %QX0.0 : BOOL;
        END_VAR
        (* q = ((a1 AND a2) OR b1) AND level > 50 *)
        RUNG { { [a1] -- [a2] } | [b1] } -- [level > 50] -- (q);
    "#);
    let ev = |r: &mut Runtime, a1, a2, b1, level| {
        r.set_input_bool(a("%IX0.0"), a1);
        r.set_input_bool(a("%IX0.1"), a2);
        r.set_input_bool(a("%IX0.2"), b1);
        r.set_input(a("%IW4"), Value::Int(level));
        r.scan(10_000).unwrap();
        r.output_bool(a("%QX0.0"))
    };
    assert!(!ev(&mut r, false, false, false, 80));
    assert!(!ev(&mut r, true, false, false, 80));
    assert!(ev(&mut r, true, true, false, 80));
    assert!(ev(&mut r, false, false, true, 80));
    assert!(!ev(&mut r, true, true, true, 50));
    assert!(ev(&mut r, true, true, true, 51));
}

#[test]
fn negated_and_pulse_coils() {
    let mut r = rt(r#"
        VAR en AT %IX0.0 : BOOL; idle AT %QX0.0 : BOOL; blip AT %QX0.1 : BOOL; END_VAR
        RUNG [en] -- (/idle);
        RUNG [en] -- (P blip);
    "#);
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")) && !r.output_bool(a("%QX0.1")));
    r.set_input_bool(a("%IX0.0"), true);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")) && r.output_bool(a("%QX0.1")));
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.1")), "the pulse lasts one scan");
}

#[test]
fn flasher_from_two_timers() {
    let mut r = rt(r#"
        VAR
            enable AT %IX0.0 : BOOL;
            on_t : TON; off_t : TON;
            lamp AT %QX0.0 : BOOL;
        END_VAR
        off_t(IN := enable AND NOT on_t.Q, PT := T#500ms);
        on_t(IN := off_t.Q, PT := T#500ms);
        lamp := off_t.Q;
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    let mut edges = 0;
    let mut last = false;
    for _ in 0..400 {
        r.scan(10_000).unwrap();
        let now = r.output_bool(a("%QX0.0"));
        if now != last {
            edges += 1;
        }
        last = now;
    }
    // 4 s at a 1 s period: on 4 times, off 4 times (minus the final one).
    assert!((6..=8).contains(&edges), "{edges} edges");
}

#[test]
fn traffic_light_state_machine() {
    let mut r = rt(r#"
        VAR
            state : INT := 0;
            t : TON;
            red AT %QX0.0 : BOOL;
            amber AT %QX0.1 : BOOL;
            green AT %QX0.2 : BOOL;
            hold : TIME;
        END_VAR
        CASE state OF
            0: hold := T#4s;  red := TRUE;  amber := FALSE; green := FALSE;
            1: hold := T#1s;  red := TRUE;  amber := TRUE;  green := FALSE;
            2: hold := T#3s;  red := FALSE; amber := FALSE; green := TRUE;
            3: hold := T#1s;  red := FALSE; amber := TRUE;  green := FALSE;
        ELSE
            state := 0;
        END_CASE;
        t(IN := TRUE, PT := hold);
        IF t.Q THEN
            state := (state + 1) MOD 4;
            t(IN := FALSE);
        END_IF;
    "#);
    let lights = |r: &Runtime| {
        (
            r.output_bool(a("%QX0.0")),
            r.output_bool(a("%QX0.1")),
            r.output_bool(a("%QX0.2")),
        )
    };
    run_ms(&mut r, 100);
    assert_eq!(lights(&r), (true, false, false));
    run_ms(&mut r, 4_000);
    assert_eq!(lights(&r), (true, true, false));
    run_ms(&mut r, 1_000);
    assert_eq!(lights(&r), (false, false, true));
    run_ms(&mut r, 3_000);
    assert_eq!(lights(&r), (false, true, false));
    run_ms(&mut r, 1_000);
    assert_eq!(lights(&r), (true, false, false), "back to red");
}

#[test]
fn st_control_flow_and_math() {
    let mut r = rt(r#"
        VAR
            sum : INT; i : INT; n : INT := 0; x : REAL; w : WORD; dint_v : DINT;
            small : SINT := 120; clamp : INT; fact : DINT := 1; k : INT;
        END_VAR
        FOR i := 1 TO 10 DO
            IF i MOD 2 = 0 THEN CONTINUE; END_IF;
            IF i > 7 THEN EXIT; END_IF;
            sum := sum + i;          (* 1+3+5+7 *)
        END_FOR;
        WHILE n < 5 DO n := n + 1; END_WHILE;
        REPEAT k := k + 3; UNTIL k >= 10 END_REPEAT;
        FOR i := 5 TO 1 BY -1 DO fact := fact * i; END_FOR;
        x := SQRT(16.0) + 2.0 ** 3 + ABS(-1.5);
        w := 16#00F0 OR 16#000F;
        small := small + 10;           (* wraps in SINT *)
        clamp := LIMIT(0, 250, 100);
        dint_v := REAL_TO_INT(2.5) + TRUNC(2.9);
    "#);
    r.scan(10_000).unwrap();
    assert_eq!(r.get("sum"), Some(Value::Int(16)));
    assert_eq!(r.get("n"), Some(Value::Int(5)));
    assert_eq!(r.get("k"), Some(Value::Int(12)));
    assert_eq!(r.get("fact"), Some(Value::Int(120)));
    assert_eq!(r.get("x"), Some(Value::Real(4.0 + 8.0 + 1.5)));
    assert_eq!(r.get("w"), Some(Value::Int(0xFF)));
    assert_eq!(r.get("small"), Some(Value::Int(-126)));
    assert_eq!(r.get("clamp"), Some(Value::Int(100)));
    assert_eq!(r.get("dint_v"), Some(Value::Int(5)));
}

#[test]
fn analog_scaling_with_real_math() {
    // A 4..20 mA level transmitter on a 0..27648 word, scaled to 0..100 %.
    let mut r = rt(r#"
        VAR
            raw AT %IW0 : INT;
            pct : REAL;
            high_alarm AT %QX0.0 : BOOL;
        END_VAR
        pct := LIMIT(0.0, INT_TO_REAL(raw) * 100.0 / 27648.0, 100.0);
        high_alarm := pct > 90.0;
    "#);
    r.set_input(a("%IW0"), Value::Int(13824));
    r.scan(10_000).unwrap();
    assert_eq!(r.get("pct"), Some(Value::Real(50.0)));
    assert!(!r.output_bool(a("%QX0.0")));
    r.set_input(a("%IW0"), Value::Int(26000));
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")));
}

#[test]
fn direct_addresses_work_inline() {
    let mut r = rt(r#"
        %QX0.0 := %IX0.0 AND NOT %IX0.1;
        %MX0.0 := %IX0.0;
        %QX0.1 := %MX0.0;
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")) && r.output_bool(a("%QX0.1")));
    r.set_input_bool(a("%IX0.1"), true);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")));
}

#[test]
fn bistables_have_a_dominant_input() {
    let mut r = rt(r#"
        VAR s AT %IX0.0 : BOOL; rr AT %IX0.1 : BOOL; f : RS; g : SR; q1 AT %QX0.0 : BOOL; q2 AT %QX0.1 : BOOL; END_VAR
        f(S := s, R1 := rr);
        g(S1 := s, R := rr);
        q1 := f.Q1;
        q2 := g.Q1;
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    r.set_input_bool(a("%IX0.1"), true);
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")), "RS: reset wins");
    assert!(r.output_bool(a("%QX0.1")), "SR: set wins");
}

#[test]
fn a_division_by_zero_faults_and_keeps_the_outputs() {
    let mut r = rt(r#"
        VAR d AT %IW0 : INT; q AT %QX0.0 : BOOL; r : INT; END_VAR
        q := d <> 0;
        r := 100 / d;
    "#);
    r.set_input(a("%IW0"), Value::Int(5));
    r.scan(10_000).unwrap();
    assert!(r.output_bool(a("%QX0.0")));
    r.set_input(a("%IW0"), Value::Int(0));
    let e = r.scan(10_000).unwrap_err();
    assert_eq!(e.line, 4);
    // `q := d <> 0` ran (FALSE) before the fault; the scan is rolled back.
    assert!(e.message.contains("division by zero"));
    assert!(r.output_bool(a("%QX0.0")), "outputs hold through a fault");
}

#[test]
fn runaway_loops_are_stopped() {
    let mut r = rt("VAR x : INT; END_VAR WHILE TRUE DO x := x + 1; END_WHILE;");
    let e = r.scan(10_000).unwrap_err();
    assert!(e.message.contains("watchdog"));
}

#[test]
fn mistakes_are_reported_with_their_place() {
    let cases = [
        ("VAR x : BOOL; END_VAR\ny := TRUE;", "line 2", "`y` is not declared"),
        ("VAR x : INT; END_VAR\nRUNG [x] -- (x);", "line 2", "BOOL"),
        ("VAR x : BOOL; END_VAR\nx := ;", "line 2", "expected a value"),
        ("VAR x : FLOAT; END_VAR", "line 1", "unknown type"),
        ("VAR i AT %IX0.0 : BOOL; END_VAR\ni := TRUE;", "line 2", "input"),
        ("VAR x : BOOL; x : BOOL; END_VAR", "line 1", "twice"),
        ("VAR t : TON; END_VAR\nt(Q := TRUE);", "line 2", "no input"),
        ("VAR x : INT; END_VAR\nx := ABS(1, 2);", "line 2", "argument"),
        ("VAR x : BOOL; END_VAR\nIF x THEN x := TRUE;", "", "END_IF"),
    ];
    for (src, place, what) in cases {
        let e = Program::parse(src).expect_err(src);
        let text = e.to_string();
        assert!(text.contains(place), "{src}: {text}");
        assert!(text.contains(what), "{src}: {text}");
    }
}

#[test]
fn a_cold_start_clears_everything() {
    let mut r = rt(SEAL_IN);
    press(&mut r, "%IX0.0");
    assert!(r.output_bool(a("%QX0.0")));
    r.reset();
    r.scan(10_000).unwrap();
    assert!(!r.output_bool(a("%QX0.0")));
    assert_eq!(r.time_us(), 10_000);
}

#[test]
fn names_are_not_case_sensitive_and_comments_nest() {
    let mut r = rt(r#"
        var Go at %ix0.0 : bool; Q AT %QX0.0 : BOOL; end_var
        (* a (* nested *) comment *)
        if GO then q := true; else Q := FALSE; end_if;   // trailing
    "#);
    r.set_input_bool(a("%IX0.0"), true);
    r.scan(1).unwrap();
    assert!(r.output_bool(a("%QX0.0")));
}

#[test]
fn program_lists_the_addresses_it_uses() {
    let p = Program::parse(SEAL_IN).unwrap();
    let list: Vec<String> = p.addresses.iter().map(ToString::to_string).collect();
    assert_eq!(list, ["%IX0.0", "%IX0.1", "%IX0.2", "%QX0.0"]);
}
