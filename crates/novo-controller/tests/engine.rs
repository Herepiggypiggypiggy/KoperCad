//! Hand-written truth tables for the controller engine. Every expectation here comes from how a
//! real two-step radio, E-stop and dead-man behave, not from reading the engine.

use novo_controller::*;

const RADIO: &str = include_str!("data/generic-radio-3x2.controller.toml");

fn radio() -> Controller {
    Controller::new(Profile::load(RADIO).expect("fixture loads")).unwrap()
}

/// Radio with main switched on (E-stop out, link up).
fn radio_on() -> Controller {
    let mut c = radio();
    assert_eq!(c.press("start").unwrap(), Applied::Changed);
    c
}

fn highs(c: &Controller) -> Vec<String> {
    c.signals()
        .into_iter()
        .filter(SignalState::is_high)
        .map(|s| s.name)
        .collect()
}

fn set(c: &mut Controller, control: &str, pos: &str) {
    assert_eq!(
        c.set_position(control, pos).unwrap(),
        Applied::Changed,
        "{control}={pos}"
    );
}

// ---- 2-step truth tables ----

#[test]
fn hoist_rocker_truth_table_step2_includes_step1() {
    // (position, [UP_SLOW, UP_FAST, DN_SLOW, DN_FAST])
    let table = [
        ("off", [false, false, false, false]),
        ("up1", [true, false, false, false]),
        ("up2", [true, true, false, false]),
        ("down1", [false, false, true, false]),
        ("down2", [false, false, true, true]),
    ];
    let mut c = radio_on();
    for (pos, want) in table {
        if pos != "off" {
            set(&mut c, "hoist", pos);
        } else {
            c.release("hoist").unwrap();
        }
        let got = ["H_UP_SLOW", "H_UP_FAST", "H_DN_SLOW", "H_DN_FAST"].map(|s| c.is_high(s));
        assert_eq!(got, want, "hoist at {pos}");
    }
}

#[test]
fn step2_drops_slow_when_profile_says_so() {
    let text = RADIO.replace(
        "generic = true",
        "generic = true\nstep2_includes_step1 = false",
    );
    let mut c = Controller::new(Profile::load(&text).unwrap()).unwrap();
    c.press("start").unwrap();
    set(&mut c, "bridge", "fwd2");
    assert_eq!(highs(&c), ["MAIN", "ESTOP_OK", "B_FWD_FAST"]);
    set(&mut c, "bridge", "fwd1");
    assert_eq!(highs(&c), ["MAIN", "ESTOP_OK", "B_FWD_SLOW"]);
}

#[test]
fn a_rocker_cannot_make_both_directions() {
    let mut c = radio_on();
    for pos in ["up1", "up2", "down1", "down2"] {
        set(&mut c, "hoist", pos);
        let up = c.is_high("H_UP_SLOW") || c.is_high("H_UP_FAST");
        let down = c.is_high("H_DN_SLOW") || c.is_high("H_DN_FAST");
        assert!(!(up && down), "both directions at {pos}");
    }
}

#[test]
fn spring_rocker_returns_to_off_on_release() {
    let mut c = radio_on();
    set(&mut c, "hoist", "up2");
    assert_eq!(c.release("hoist").unwrap(), Applied::Changed);
    assert_eq!(c.position("hoist"), Some("off"));
    assert!(!c.is_high("H_UP_SLOW"));
    assert_eq!(c.release("hoist").unwrap(), Applied::Unchanged);
}

#[test]
fn three_step_rocker_with_analog_levels() {
    let text = r#"
[controller]
id = "j"
name = "Joystick"
kind = "joystick"
[[control]]
id = "x"
kind = "rocker3"
positions = ["off", "right1", "right2", "right3", "left1", "left2", "left3"]
[[signal]]
name = "REF"
kind = "analog"
control = "x"
unit = "V"
levels = { right1 = 2.0, right2 = 5.0, right3 = 10.0, left1 = -2.0, left2 = -5.0, left3 = -10.0 }
[[signal]]
name = "R3"
kind = "voltage"
control = "x"
group = "right"
step = 3
"#;
    let mut c = Controller::new(Profile::load(text).unwrap()).unwrap();
    assert_eq!(c.level("REF"), Some(0.0));
    for (pos, v) in [
        ("right2", 5.0),
        ("right3", 10.0),
        ("left1", -2.0),
        ("left3", -10.0),
    ] {
        c.set_position("x", pos).unwrap();
        assert_eq!(c.level("REF"), Some(v), "{pos}");
    }
    c.set_position("x", "right3").unwrap();
    assert!(c.is_high("R3"));
    c.set_position("x", "right2").unwrap();
    assert!(!c.is_high("R3"));
}

// ---- main, stop, E-stop ----

#[test]
fn nothing_moves_until_main_is_on() {
    let mut c = radio();
    set(&mut c, "hoist", "up2");
    assert!(!c.is_high("H_UP_SLOW") && !c.is_high("H_UP_FAST"));
    assert!(
        c.is_high("ESTOP_OK"),
        "the E-stop chain does not wait for main"
    );
    c.release("hoist").unwrap();
    c.press("start").unwrap();
    set(&mut c, "hoist", "up2");
    assert!(c.is_high("H_UP_FAST"));
}

#[test]
fn estop_latches_drops_main_and_needs_a_reset() {
    let mut c = radio_on();
    set(&mut c, "hoist", "up1");
    assert!(c.is_high("H_UP_SLOW") && c.is_high("MAIN") && c.is_high("ESTOP_OK"));

    c.press("estop").unwrap();
    assert!(!c.is_high("ESTOP_OK"), "NC contact opens");
    assert_eq!(c.position("start"), Some("off"), "main drops");
    assert!(!c.is_high("MAIN") && !c.is_high("H_UP_SLOW"));

    // releasing the button does nothing, the mushroom stays in
    assert_eq!(c.release("estop").unwrap(), Applied::Unchanged);
    assert_eq!(c.position("estop"), Some("pressed"));
    assert_eq!(
        c.set_position("estop", "released").unwrap(),
        Applied::Blocked(Block::NeedsReset)
    );

    // main cannot be set with the E-stop in
    assert_eq!(
        c.press("start").unwrap(),
        Applied::Blocked(Block::Requires("estop=released".into()))
    );

    // twist: E-stop is out, but main stays off until it is pressed again
    assert_eq!(c.reset("estop").unwrap(), Applied::Changed);
    assert!(c.is_high("ESTOP_OK"));
    assert!(!c.is_high("MAIN"));
    c.press("start").unwrap();
    assert!(c.is_high("MAIN"));
}

#[test]
fn stop_button_drops_main_and_cannot_start_while_held() {
    let mut c = radio_on();
    c.press("stop").unwrap();
    assert_eq!(c.position("start"), Some("off"));
    c.release("stop").unwrap();
    assert!(!c.is_high("MAIN"), "main stays off after stop is released");
    c.press("stop").unwrap();
    assert_eq!(
        c.press("start").unwrap(),
        Applied::Blocked(Block::Inhibited("stop=on".into()))
    );
}

#[test]
fn latch_toggles_and_keeps_its_state_on_release() {
    let mut c = radio();
    c.press("start").unwrap();
    assert_eq!(c.release("start").unwrap(), Applied::Unchanged);
    assert_eq!(c.position("start"), Some("on"));
    c.press("start").unwrap();
    assert_eq!(c.position("start"), Some("off"));
}

// ---- dead-man ----

const DEADMAN: &str = r#"
[controller]
id = "pendant"
name = "Pendant with enabling switch"
kind = "pendant"
common = "C"
gate = ["enable=held"]
[[control]]
id = "estop"
kind = "estop"
[[control]]
id = "enable"
kind = "deadman"
positions = ["released", "enable", "held"]
rest = "released"
[[control]]
id = "fwd"
kind = "button"
[[signal]]
name = "FWD"
control = "fwd"
when = ["on"]
"#;

#[test]
fn deadman_gates_signals_and_only_lives_while_held() {
    let mut c = Controller::new(Profile::load(DEADMAN).unwrap()).unwrap();
    c.press("fwd").unwrap();
    assert!(!c.is_high("FWD"), "not enabled yet");
    c.set_position("enable", "enable").unwrap();
    assert!(
        !c.is_high("FWD"),
        "middle position is not 'held' for this gate"
    );
    c.set_position("enable", "held").unwrap();
    assert!(c.is_high("FWD"));
    c.release("enable").unwrap();
    assert!(
        !c.is_high("FWD"),
        "letting go of the bar drops the signal at once"
    );
    assert_eq!(
        c.position("fwd"),
        Some("on"),
        "the button itself is still held"
    );
}

// ---- link loss and other radio effects ----

#[test]
fn link_loss_drops_every_signal_and_main_and_needs_a_new_start() {
    let mut c = radio_on();
    set(&mut c, "bridge", "fwd2");
    assert!(c.is_high("B_FWD_FAST"));

    assert_eq!(c.set_link(false), Applied::Changed);
    assert!(
        highs(&c).is_empty(),
        "everything low, E-stop chain open: {:?}",
        highs(&c)
    );
    assert_eq!(c.position("start"), Some("off"));
    assert_eq!(
        c.position("bridge"),
        Some("fwd2"),
        "the person still holds the rocker"
    );
    assert_eq!(c.warnings(), ["radio link lost"]);
    assert_eq!(c.press("start").unwrap(), Applied::Blocked(Block::LinkDown));

    // link returns: the rocker is still held but main is off, so the crane stays stopped
    c.set_link(true);
    assert!(!c.is_high("B_FWD_FAST"));
    assert!(c.is_high("ESTOP_OK"));
    c.press("start").unwrap();
    assert!(c.is_high("B_FWD_FAST"));
}

#[test]
fn link_loss_only_matters_for_signals_that_need_the_link() {
    let text = RADIO.replace("kind = \"radio\"", "kind = \"pendant\"");
    let mut c = Controller::new(Profile::load(&text).unwrap()).unwrap();
    c.press("start").unwrap();
    set(&mut c, "bridge", "fwd1");
    // a wired pendant has no radio link: signals do not depend on it (the main latch still does,
    // because it was declared receiver-side)
    c.set_link(false);
    assert!(c.is_high("ESTOP_OK"));
    assert!(!c.is_high("B_FWD_SLOW"), "main dropped, gate fails");
}

#[test]
fn self_test_delay_holds_everything_low_until_it_ends() {
    let text = RADIO.replace("self_test_ms = 0", "self_test_ms = 1500");
    let mut c = Controller::new(Profile::load(&text).unwrap()).unwrap();
    assert!(c.in_self_test());
    assert!(highs(&c).is_empty());
    c.advance(1499);
    assert!(highs(&c).is_empty());
    c.advance(1);
    assert_eq!(highs(&c), ["ESTOP_OK"]);
    assert!(c.warnings().is_empty());

    // power cycle restarts it, and main drops with power
    c.press("start").unwrap();
    c.power(false);
    assert_eq!(c.position("start"), Some("off"));
    assert!(highs(&c).is_empty());
    c.power(true);
    assert!(c.in_self_test());
    c.advance(1500);
    assert_eq!(highs(&c), ["ESTOP_OK"]);
}

#[test]
fn battery_low_is_a_warning_only() {
    let mut c = radio_on();
    let before = highs(&c);
    c.set_battery_low(true);
    assert_eq!(highs(&c), before);
    assert_eq!(c.warnings(), ["transmitter battery low"]);
}

// ---- selector mapping ----

#[test]
fn selector_decides_which_signal_set_the_rocker_drives() {
    let mut c = radio_on();
    set(&mut c, "hoist", "up1");
    assert!(c.is_high("H_UP_SLOW") && !c.is_high("M_UP_SLOW"));
    set(&mut c, "hoist_sel", "mast");
    assert!(!c.is_high("H_UP_SLOW") && c.is_high("M_UP_SLOW"));
    // selector stays where it is
    assert_eq!(c.release("hoist_sel").unwrap(), Applied::Unchanged);
    assert_eq!(c.position("hoist_sel"), Some("mast"));
    set(&mut c, "hoist_sel", "main");
    assert!(c.is_high("H_UP_SLOW") && !c.is_high("M_UP_SLOW"));
}

// ---- interlocks between separate controls ----

fn two_buttons(mode: &str) -> Controller {
    let text = format!(
        r#"
[controller]
id = "p"
name = "Two buttons"
common = "C"
[[control]]
id = "fwd"
kind = "button"
[[control]]
id = "rev"
kind = "button"
[[interlock]]
between = ["fwd", "rev"]
mode = "{mode}"
[[signal]]
name = "FWD"
control = "fwd"
when = ["on"]
[[signal]]
name = "REV"
control = "rev"
when = ["on"]
"#
    );
    Controller::new(Profile::load(&text).unwrap()).unwrap()
}

#[test]
fn interlock_first_wins() {
    let mut c = two_buttons("first_wins");
    c.press("fwd").unwrap();
    assert_eq!(
        c.press("rev").unwrap(),
        Applied::Blocked(Block::Interlock { with: "fwd".into() })
    );
    assert!(c.is_high("FWD") && !c.is_high("REV"));
    c.release("fwd").unwrap();
    assert_eq!(c.press("rev").unwrap(), Applied::Changed);
    assert!(c.is_high("REV") && !c.is_high("FWD"));
}

#[test]
fn interlock_both_off_when_both_are_down() {
    let mut c = two_buttons("both_off");
    c.press("fwd").unwrap();
    assert!(c.is_high("FWD"));
    c.press("rev").unwrap();
    assert!(!c.is_high("FWD") && !c.is_high("REV"));
    assert_eq!(c.effective_position("fwd").as_deref(), Some("off"));
    c.release("fwd").unwrap();
    assert!(c.is_high("REV"), "the remaining button works again");
}

// ---- errors and history ----

#[test]
fn unknown_names_are_errors_not_silence() {
    let mut c = radio();
    assert_eq!(
        c.press("nope"),
        Err(ControlError::UnknownControl("nope".into()))
    );
    assert_eq!(
        c.set_position("hoist", "sideways"),
        Err(ControlError::UnknownPosition {
            control: "hoist".into(),
            position: "sideways".into()
        })
    );
    assert_eq!(
        c.press("hoist"),
        Err(ControlError::NeedsPosition("hoist".into()))
    );
}

#[test]
fn history_lists_signal_changes_in_order() {
    let mut c = radio();
    c.advance(100);
    c.press("start").unwrap();
    c.advance(50);
    set(&mut c, "bridge", "fwd1");
    c.advance(25);
    set(&mut c, "bridge", "fwd2");
    let log: Vec<(u64, &str, f64)> = c
        .history()
        .iter()
        .map(|h| (h.t_ms, h.signal.as_str(), h.level))
        .collect();
    assert_eq!(
        log,
        [
            (100, "MAIN", 1.0),
            (150, "B_FWD_SLOW", 1.0),
            (175, "B_FWD_FAST", 1.0),
        ]
    );
}

#[test]
fn state_round_trips_and_an_edited_profile_still_restores() {
    let mut c = radio_on();
    set(&mut c, "hoist", "down2");
    let saved = c.state().clone();
    let json = serde_json::to_string(&saved).unwrap();
    let back: State = serde_json::from_str(&json).unwrap();
    let restored = Controller::restore(c.profile().clone(), back).unwrap();
    assert_eq!(restored.signals(), c.signals());

    // a position that no longer exists falls back to rest, a new control starts at rest
    let mut stale = saved;
    stale.positions.insert("hoist".into(), "gone".into());
    stale.positions.remove("trolley");
    let r = Controller::restore(c.profile().clone(), stale).unwrap();
    assert_eq!(r.position("hoist"), Some("off"));
    assert_eq!(r.position("trolley"), Some("off"));
}
