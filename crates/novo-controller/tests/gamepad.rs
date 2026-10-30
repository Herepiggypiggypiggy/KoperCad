//! The gamepad layer: deadzone and detent maths, calibration, the dead-man gate, the E-stop
//! button, device loss, the map file and the calibration wizard. Written by hand against the
//! rules in `gamepad/mod.rs`, not generated from the code.

use std::collections::BTreeMap;

use novo_controller::gamepad::*;
use novo_controller::*;

const RADIO: &str = include_str!("data/generic-radio-3x2.controller.toml");

/// The generic radio plus a dead-man bar, a horn button, a three-step crab rocker, a joystick
/// axis with detents and a spring-less lift lever.
fn pad_profile() -> Profile {
    let extra = r#"
[[control]]
id = "deadman"
kind = "deadman"

[[control]]
id = "horn"
kind = "button"

[[control]]
id = "crab"
kind = "rocker3"
positions = ["off", "fwd1", "fwd2", "fwd3", "rev1", "rev2", "rev3"]

[[control]]
id = "joy"
kind = "axis"
positions = ["off", "up1", "up2", "dn1", "dn2"]

[[control]]
id = "lever"
kind = "axis"
spring = false
positions = ["mid", "hi", "lo"]

[[signal]]
name = "HORN"
control = "horn"
when = ["on"]

[[signal]]
name = "CRAB_F3"
control = "crab"
group = "fwd"
step = 3
"#;
    let mut text = RADIO.to_owned();
    text.push_str(extra);
    Profile::load(&text).expect("pad test profile loads")
}

fn controller() -> Controller {
    Controller::new(pad_profile()).unwrap()
}

fn map_of(toml: &str) -> GamepadMap {
    GamepadMap::parse(toml).expect("map parses")
}

/// The map most tests use: no dead-man, so sticks act at once.
fn plain_map() -> GamepadMap {
    map_of(
        r#"
[[axis]]
input = "left_y"
control = "hoist"

[[axis]]
input = "left_x"
control = "trolley"

[[axis]]
input = "right_y"
control = "crab"

[[button]]
input = "east"
control = "estop"
action = "estop"

[[button]]
input = "north"
control = "estop"
action = "reset"

[[button]]
input = "start"
control = "start"
action = "toggle"

[[button]]
input = "south"
control = "horn"
action = "hold"
"#,
    )
}

fn deadman_map() -> GamepadMap {
    let mut m = plain_map();
    m.deadman = Some(DeadmanMap {
        input: "left_trigger".into(),
        control: Some("deadman".into()),
    });
    m
}

/// The host: applies actions, then holds, like the live view does.
fn apply(ctl: &mut Controller, out: &PadOutput) {
    for a in &out.actions {
        match a {
            Action::Press { control } => {
                let _ = ctl.press(control);
            }
            Action::Reset { control } => {
                let _ = ctl.reset(control);
            }
            Action::SetPosition { control, position } => {
                let _ = ctl.set_position(control, position);
            }
            _ => panic!("unexpected action {a:?}"),
        }
    }
    if let Some(ok) = out.link {
        ctl.set_link(ok);
    }
    for (control, pos) in &out.holds {
        match pos {
            Some(p) => {
                let _ = ctl.set_position(control, p);
            }
            None => {
                let _ = ctl.release(control);
            }
        }
    }
}

struct Rig {
    ctl: Controller,
    map: GamepadMap,
    mapper: PadMapper,
}

impl Rig {
    fn new(map: GamepadMap) -> Rig {
        Rig {
            ctl: controller(),
            map,
            mapper: PadMapper::new(),
        }
    }

    fn step(&mut self, input: &InputState) -> PadOutput {
        let out = self.mapper.update(&self.map, &self.ctl, input);
        apply(&mut self.ctl, &out);
        out
    }

    fn at(&self, control: &str) -> String {
        self.ctl.position(control).unwrap().to_owned()
    }
}

fn pad() -> InputState {
    InputState::connected()
}

// ---- step_of: deadzone and detents ----

#[test]
fn deadzone_edge_is_rest_and_just_beyond_is_step_one() {
    let dz = 0.2;
    assert_eq!(
        step_of(0, 0.2, dz, 0.0, 2, 2),
        0,
        "exactly at the deadzone is rest"
    );
    assert_eq!(step_of(0, 0.2000001, dz, 0.0, 2, 2), 1);
    assert_eq!(step_of(0, -0.2, dz, 0.0, 2, 2), 0);
    assert_eq!(step_of(0, -0.2000001, dz, 0.0, 2, 2), -1);
    assert_eq!(step_of(0, 0.0, dz, 0.0, 2, 2), 0);
    assert_eq!(step_of(0, 0.05, dz, 0.0, 2, 2), 0);
}

#[test]
fn zero_deadzone_acts_on_any_deflection() {
    assert_eq!(step_of(0, 0.0, 0.0, 0.0, 1, 1), 0);
    assert_eq!(step_of(0, 0.001, 0.0, 0.0, 1, 1), 1);
    assert_eq!(step_of(0, -0.001, 0.0, 0.0, 1, 1), -1);
}

#[test]
fn two_steps_split_the_throw_beyond_the_deadzone_in_half() {
    // deadzone 0.2: step 1 from 0.2 up to 0.6, step 2 from 0.6 (0.2 + 0.8 / 2).
    let s = |v| step_of(0, v, 0.2, 0.0, 2, 2);
    assert_eq!(s(0.3), 1);
    assert_eq!(s(0.5999), 1);
    assert_eq!(s(0.6), 2);
    assert_eq!(s(1.0), 2);
    assert_eq!(s(-0.5999), -1);
    assert_eq!(s(-0.6), -2);
    assert_eq!(s(-1.0), -2);
}

#[test]
fn three_steps_split_the_throw_in_thirds() {
    // deadzone 0.1, throw 0.9: edges at 0.1, 0.4, 0.7.
    let s = |v| step_of(0, v, 0.1, 0.0, 3, 3);
    assert_eq!(s(0.1), 0);
    assert_eq!(s(0.39), 1);
    assert_eq!(s(0.4), 2);
    assert_eq!(s(0.69), 2);
    assert_eq!(s(0.7), 3);
    assert_eq!(s(1.0), 3);
}

#[test]
fn a_single_step_side_has_one_band() {
    let s = |v| step_of(0, v, 0.1, 0.0, 1, 1);
    assert_eq!(s(0.2), 1);
    assert_eq!(s(1.0), 1);
}

#[test]
fn sides_have_their_own_step_counts() {
    // Two steps up, one down.
    assert_eq!(step_of(0, 1.0, 0.1, 0.0, 1, 2), 2);
    assert_eq!(step_of(0, -1.0, 0.1, 0.0, 1, 2), -1);
    assert_eq!(step_of(0, -0.5, 0.1, 0.0, 1, 2), -1);
}

#[test]
fn a_side_without_positions_stays_at_rest() {
    assert_eq!(step_of(0, -1.0, 0.1, 0.0, 0, 2), 0);
    assert_eq!(step_of(0, 1.0, 0.1, 0.0, 2, 0), 0);
    assert_eq!(step_of(0, 1.0, 0.1, 0.0, 0, 0), 0);
}

#[test]
fn hysteresis_keeps_a_step_until_the_value_falls_back() {
    let (dz, h) = (0.2, 0.1);
    // Entered step 2 at 0.6. Falling back: stays until below 0.5.
    assert_eq!(step_of(2, 0.55, dz, h, 2, 2), 2);
    assert_eq!(step_of(2, 0.5, dz, h, 2, 2), 2);
    assert_eq!(step_of(2, 0.4999, dz, h, 2, 2), 1);
    // The way up has no hysteresis: from step 1 it needs the edge itself.
    assert_eq!(step_of(1, 0.59, dz, h, 2, 2), 1);
    assert_eq!(step_of(1, 0.6, dz, h, 2, 2), 2);
    // Leaving step 1 for rest: needs to fall to deadzone - hysteresis.
    assert_eq!(step_of(1, 0.15, dz, h, 2, 2), 1);
    assert_eq!(step_of(1, 0.1, dz, h, 2, 2), 0);
    assert_eq!(step_of(1, 0.05, dz, h, 2, 2), 0);
}

#[test]
fn hysteresis_stops_chatter_around_an_edge() {
    // A value wobbling around the step 2 edge at 0.6 changes the step once, not every sample.
    let mut step = 0;
    let mut changes = 0;
    for v in [0.55, 0.61, 0.59, 0.61, 0.58, 0.62, 0.57, 0.61] {
        let next = step_of(step, v, 0.2, 0.1, 2, 2);
        if next != step {
            changes += 1;
        }
        step = next;
    }
    assert_eq!(step, 2);
    assert_eq!(changes, 2, "rest -> 1 -> 2 once each");
}

#[test]
fn hysteresis_larger_than_the_deadzone_does_not_go_negative() {
    // Leaving step 1 needs value <= max(dz - h, 0) = 0, so a stick that returns exactly to
    // centre does release, and a tiny deflection holds the step.
    assert_eq!(step_of(1, 0.0, 0.05, 0.3, 2, 2), 0);
    assert_eq!(step_of(1, 0.01, 0.05, 0.3, 2, 2), 1);
}

#[test]
fn big_jumps_pass_several_steps_at_once() {
    assert_eq!(step_of(0, 1.0, 0.1, 0.05, 3, 3), 3);
    assert_eq!(step_of(3, 0.0, 0.1, 0.05, 3, 3), 0);
    assert_eq!(step_of(-3, 0.0, 0.1, 0.05, 3, 3), 0);
}

#[test]
fn crossing_the_centre_changes_side_without_hysteresis_from_the_old_side() {
    assert_eq!(step_of(2, -0.9, 0.1, 0.1, 2, 2), -2);
    assert_eq!(step_of(-2, 0.9, 0.1, 0.1, 2, 2), 2);
    // Value exactly 0 while on the negative side falls back through the negative hysteresis.
    assert_eq!(step_of(-1, 0.0, 0.1, 0.05, 2, 2), 0);
}

#[test]
fn not_a_number_and_out_of_range_values_are_safe() {
    assert_eq!(step_of(2, f64::NAN, 0.1, 0.05, 2, 2), 0);
    assert_eq!(step_of(0, f64::INFINITY, 0.1, 0.05, 2, 2), 0);
    assert_eq!(
        step_of(0, 7.0, 0.1, 0.05, 2, 2),
        2,
        "7 clamps to full deflection"
    );
}

// ---- normalise: calibration and invert ----

#[test]
fn normalise_scales_each_side_on_its_own() {
    // Centre 0.1, reaches +0.9 and -0.5: the two ends are different distances away.
    let n = |raw| normalise(raw, 0.1, -0.5, 0.9, false);
    assert_eq!(n(0.1), 0.0);
    assert!((n(0.9) - 1.0).abs() < 1e-12);
    assert!((n(-0.5) + 1.0).abs() < 1e-12);
    assert!((n(0.5) - 0.5).abs() < 1e-12);
    assert!((n(-0.2) + 0.5).abs() < 1e-12);
}

#[test]
fn normalise_clamps_and_inverts() {
    assert_eq!(normalise(2.0, 0.0, -1.0, 1.0, false), 1.0);
    assert_eq!(normalise(-2.0, 0.0, -1.0, 1.0, false), -1.0);
    assert_eq!(normalise(0.5, 0.0, -1.0, 1.0, true), -0.5);
    assert_eq!(normalise(f64::NAN, 0.0, -1.0, 1.0, false), 0.0);
}

#[test]
fn normalise_with_a_broken_range_uses_the_nominal_throw() {
    assert_eq!(normalise(0.5, 0.0, 0.0, 0.0, false), 0.5);
    assert_eq!(normalise(-0.5, 0.3, 0.9, 0.1, false), -0.8 / 1.0);
}

#[test]
fn a_stick_that_rests_off_centre_reads_zero_after_calibration() {
    let mut a = AxisMap::new("left_y", "hoist");
    a.centre = 0.08;
    a.low = -0.9;
    a.high = 0.95;
    assert_eq!(a.value(0.08), 0.0);
    assert!(a.value(0.95) > 0.999);
}

// ---- selector bands ----

#[test]
fn selector_bands_are_equal_and_have_hysteresis() {
    // Three positions: the value range -1..1 is cut at -1/3 and 1/3.
    assert_eq!(band_of(None, -1.0, 3, 0.0), 0);
    assert_eq!(band_of(None, -0.4, 3, 0.0), 0);
    assert_eq!(band_of(None, -0.3, 3, 0.0), 1);
    assert_eq!(band_of(None, 0.3, 3, 0.0), 1);
    assert_eq!(band_of(None, 0.4, 3, 0.0), 2);
    assert_eq!(band_of(None, 1.0, 3, 0.0), 2);
    // From band 1, stays a little beyond the edge, leaves further out.
    assert_eq!(band_of(Some(1), 0.36, 3, 0.2), 1);
    assert_eq!(band_of(Some(1), 0.6, 3, 0.2), 2);
    assert_eq!(band_of(Some(0), -0.28, 3, 0.2), 0);
}

// ---- the mapper on a real controller ----

#[test]
fn stick_up_drives_the_hoist_rocker_through_its_steps() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    // Main on first so the signals can be seen.
    rig.step(&pad().with_button("start", true));
    assert!(rig.ctl.is_high("MAIN"));
    rig.step(&pad());

    rig.step(&pad().with_axis("left_y", 0.3));
    assert_eq!(rig.at("hoist"), "up1");
    assert!(rig.ctl.is_high("H_UP_SLOW"));
    rig.step(&pad().with_axis("left_y", 0.9));
    assert_eq!(rig.at("hoist"), "up2");
    assert!(rig.ctl.is_high("H_UP_FAST"));
    rig.step(&pad().with_axis("left_y", -0.9));
    assert_eq!(rig.at("hoist"), "down2");
    rig.step(&pad().with_axis("left_y", -0.3));
    assert_eq!(rig.at("hoist"), "down1");
    rig.step(&pad().with_axis("left_y", 0.05));
    assert_eq!(
        rig.at("hoist"),
        "off",
        "inside the deadzone the spring returns it"
    );
    assert!(!rig.ctl.is_high("H_UP_SLOW"));
}

#[test]
fn the_first_direction_is_positive_unless_its_name_says_left_or_down() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    // trolley lists left first, but left is the negative side: pushing right gives right2.
    rig.step(&pad().with_axis("left_x", 1.0));
    assert_eq!(rig.at("trolley"), "right2");
    rig.step(&pad().with_axis("left_x", -1.0));
    assert_eq!(rig.at("trolley"), "left2");
}

#[test]
fn invert_swaps_the_directions() {
    let mut map = plain_map();
    map.axes[0].invert = true;
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "down2");
}

#[test]
fn three_step_rocker_gets_three_detents() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    for (v, want) in [
        (0.3, "fwd1"),
        (0.6, "fwd2"),
        (1.0, "fwd3"),
        (-0.3, "rev1"),
        (-1.0, "rev3"),
    ] {
        rig.step(&pad().with_axis("right_y", v));
        assert_eq!(rig.at("crab"), want, "at {v}");
    }
}

#[test]
fn calibration_moves_the_centre_and_the_ends() {
    let mut map = plain_map();
    map.axes[0].centre = 0.2;
    map.axes[0].low = -0.6;
    map.axes[0].high = 0.8;
    let mut rig = Rig::new(map);
    rig.step(&pad().with_axis("left_y", 0.2));
    // The worn stick rests at 0.2: no motion.
    rig.step(&pad().with_axis("left_y", 0.2));
    assert_eq!(rig.at("hoist"), "off");
    // Its full-up is 0.8 raw: step 2.
    rig.step(&pad().with_axis("left_y", 0.8));
    assert_eq!(rig.at("hoist"), "up2");
    // Its full-down is -0.6 raw.
    rig.step(&pad().with_axis("left_y", -0.6));
    assert_eq!(rig.at("hoist"), "down2");
}

#[test]
fn two_axes_on_one_control_prefer_the_active_one() {
    let mut map = plain_map();
    map.axes.push(AxisMap::new("right_x", "hoist"));
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("right_x", 1.0));
    assert_eq!(
        rig.at("hoist"),
        "up2",
        "the second axis acts while the first is at rest"
    );
}

#[test]
fn hold_button_presses_the_push_button_while_down() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_button("south", true));
    assert_eq!(rig.at("horn"), "on");
    rig.step(&pad().with_button("south", false));
    assert_eq!(rig.at("horn"), "off");
}

#[test]
fn a_trigger_can_work_as_a_button() {
    let mut map = plain_map();
    map.buttons[3].input = "right_trigger".into();
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("right_trigger", 0.4));
    assert_eq!(rig.at("horn"), "off", "under half travel");
    rig.step(&pad().with_axis("right_trigger", 0.8));
    assert_eq!(rig.at("horn"), "on");
}

// ---- dead-man gating ----

#[test]
fn nothing_moves_without_the_deadman() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    let out = rig.step(&pad().with_axis("left_y", 1.0).with_button("south", true));
    assert!(!out.gate_open);
    assert_eq!(rig.at("hoist"), "off");
    assert_eq!(rig.at("horn"), "off");
    assert_eq!(rig.at("deadman"), "released");
}

#[test]
fn deadman_trigger_opens_the_gate_and_holds_the_deadman_control() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    let out = rig.step(&pad().with_axis("left_trigger", 1.0));
    assert!(out.gate_open);
    assert_eq!(rig.at("deadman"), "held");
    // A half-pressed trigger is not enough.
    rig.step(&pad().with_axis("left_trigger", 0.4));
    assert_eq!(rig.at("deadman"), "released");
}

#[test]
fn with_the_deadman_held_the_stick_acts() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    rig.step(&pad().with_axis("left_trigger", 1.0));
    rig.step(
        &pad()
            .with_axis("left_trigger", 1.0)
            .with_axis("left_y", 1.0),
    );
    assert_eq!(rig.at("hoist"), "up2");
}

#[test]
fn letting_go_of_the_deadman_stops_motion_in_the_same_frame() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    let held = pad().with_axis("left_trigger", 1.0);
    rig.step(&held.clone());
    rig.step(
        &held
            .clone()
            .with_axis("left_y", 1.0)
            .with_button("south", true),
    );
    assert_eq!(rig.at("hoist"), "up2");
    assert_eq!(rig.at("horn"), "on");
    rig.step(&pad().with_axis("left_y", 1.0).with_button("south", true));
    assert_eq!(rig.at("hoist"), "off");
    assert_eq!(rig.at("horn"), "off");
    assert_eq!(rig.at("deadman"), "released");
}

#[test]
fn after_the_deadman_returns_a_deflected_stick_must_go_to_neutral_first() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    let held = pad().with_axis("left_trigger", 1.0);
    rig.step(&held.clone().with_axis("left_y", 1.0));
    // Gate lost with the stick still over, grasped again with the stick still over.
    rig.step(&pad().with_axis("left_y", 1.0));
    let out = rig.step(&held.clone().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "off", "must not start by itself");
    assert!(out.axes[0].waiting_for_neutral);
    // Back to neutral, then out again.
    rig.step(&held.clone());
    rig.step(&held.clone().with_axis("left_y", 0.4));
    assert_eq!(rig.at("hoist"), "up1");
}

#[test]
fn a_held_button_must_also_be_released_after_the_gate_returns() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    // Horn pressed first, deadman grasped after: horn waits for a fresh press.
    rig.step(&pad().with_button("south", true));
    rig.step(
        &pad()
            .with_axis("left_trigger", 1.0)
            .with_button("south", true),
    );
    assert_eq!(rig.at("horn"), "off");
    rig.step(&pad().with_axis("left_trigger", 1.0));
    rig.step(
        &pad()
            .with_axis("left_trigger", 1.0)
            .with_button("south", true),
    );
    assert_eq!(rig.at("horn"), "on");
}

#[test]
fn an_ungated_axis_ignores_the_deadman() {
    let mut map = deadman_map();
    map.axes[0].gated = false;
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "up2", "no trigger held and it still acts");
    // The gated horn on the same pad stays dead.
    rig.step(&pad().with_button("south", true));
    assert_eq!(rig.at("horn"), "off");
}

#[test]
fn neutral_required_off_lets_a_deflected_stick_act_at_once() {
    let mut map = plain_map();
    map.neutral_required = false;
    let mut rig = Rig::new(map);
    rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "up2");
}

#[test]
fn connecting_with_the_stick_over_does_not_move_anything() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad().with_axis("left_y", 1.0).with_button("south", true));
    assert_eq!(rig.at("hoist"), "off");
    assert_eq!(rig.at("horn"), "off");
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "up2");
}

// ---- E-stop and other buttons ----

#[test]
fn the_estop_button_presses_on_the_edge_even_without_the_deadman() {
    let mut rig = Rig::new(deadman_map());
    rig.step(&pad());
    let out = rig.step(&pad().with_button("east", true));
    assert!(!out.gate_open);
    assert_eq!(rig.at("estop"), "pressed");
    assert_eq!(
        out.actions,
        vec![Action::Press {
            control: "estop".into()
        }]
    );
    // Held down: no second press. Released and pressed again: another one (a no-op for the
    // latched button, but it is asked for).
    let held = rig.step(&pad().with_button("east", true));
    assert!(held.actions.is_empty());
    rig.step(&pad());
    let again = rig.step(&pad().with_button("east", true));
    assert_eq!(again.actions.len(), 1);
}

#[test]
fn estop_drops_main_and_the_motion_signals() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_button("start", true));
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0));
    assert!(rig.ctl.is_high("H_UP_FAST"));
    rig.step(&pad().with_axis("left_y", 1.0).with_button("east", true));
    assert!(!rig.ctl.is_high("MAIN"));
    assert!(!rig.ctl.is_high("H_UP_FAST"));
    assert!(!rig.ctl.is_high("ESTOP_OK"));
}

#[test]
fn the_reset_button_releases_the_estop_and_main_stays_off() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_button("east", true));
    assert_eq!(rig.at("estop"), "pressed");
    rig.step(&pad());
    rig.step(&pad().with_button("north", true));
    assert_eq!(rig.at("estop"), "released");
    assert!(!rig.ctl.is_high("MAIN"), "reset does not switch main on");
}

#[test]
fn a_button_already_down_when_the_pad_connects_does_not_fire() {
    let mut rig = Rig::new(plain_map());
    let out = rig.step(&pad().with_button("east", true).with_button("start", true));
    assert!(out.actions.is_empty());
    assert_eq!(rig.at("estop"), "released");
}

#[test]
fn toggle_button_switches_the_latch_on_and_off() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_button("start", true));
    assert_eq!(rig.at("start"), "on");
    rig.step(&pad());
    rig.step(&pad().with_button("start", true));
    assert_eq!(rig.at("start"), "off");
}

#[test]
fn the_engine_still_refuses_what_its_rules_refuse() {
    // Main cannot be switched on while the E-stop is pressed; the pad does not bypass that.
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_button("east", true));
    rig.step(&pad());
    rig.step(&pad().with_button("start", true));
    assert_eq!(rig.at("start"), "off");
}

#[test]
fn selector_buttons_step_through_the_positions() {
    let mut map = plain_map();
    map.buttons.push(ButtonMap::new(
        "dpad_right",
        "hoist_sel",
        ButtonAction::Next,
    ));
    map.buttons
        .push(ButtonMap::new("dpad_left", "hoist_sel", ButtonAction::Prev));
    let mut rig = Rig::new(map);
    rig.step(&pad());
    assert_eq!(rig.at("hoist_sel"), "main");
    rig.step(&pad().with_button("dpad_right", true));
    assert_eq!(rig.at("hoist_sel"), "mast");
    rig.step(&pad());
    rig.step(&pad().with_button("dpad_right", true));
    assert_eq!(rig.at("hoist_sel"), "mast", "stops at the end");
    rig.step(&pad());
    rig.step(&pad().with_button("dpad_left", true));
    assert_eq!(rig.at("hoist_sel"), "main");
}

#[test]
fn set_button_puts_a_selector_at_its_position() {
    let mut map = plain_map();
    let mut b = ButtonMap::new("west", "hoist_sel", ButtonAction::Set);
    b.position = Some("mast".into());
    map.buttons.push(b);
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_button("west", true));
    assert_eq!(rig.at("hoist_sel"), "mast");
}

#[test]
fn an_axis_picks_a_selector_position_by_band_and_only_on_change() {
    let mut map = GamepadMap::default();
    map.axes.push(AxisMap::new("right_x", "hoist_sel"));
    let mut rig = Rig::new(map);
    // Connect with the stick at the far right: the selector is not touched.
    let out = rig.step(&pad().with_axis("right_x", 1.0));
    assert!(out.actions.is_empty());
    assert_eq!(rig.at("hoist_sel"), "main");
    // Moving it across the middle changes it.
    let out = rig.step(&pad().with_axis("right_x", -1.0));
    assert_eq!(out.actions.len(), 1);
    assert_eq!(rig.at("hoist_sel"), "main", "band 0 is main");
    rig.step(&pad().with_axis("right_x", 1.0));
    assert_eq!(rig.at("hoist_sel"), "mast");
}

#[test]
fn a_lever_without_spring_is_set_back_to_rest_by_the_pad() {
    let mut map = GamepadMap::default();
    map.axes.push(AxisMap::new("right_y", "lever"));
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("right_y", 1.0));
    assert_eq!(rig.at("lever"), "hi");
    rig.step(&pad());
    assert_eq!(
        rig.at("lever"),
        "mid",
        "no spring, so the pad sets rest explicitly"
    );
}

// ---- disconnect and link loss ----

fn driving_rig(map: GamepadMap) -> Rig {
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(
        &pad()
            .with_axis("left_trigger", 1.0)
            .with_button("start", true),
    );
    rig.step(&pad().with_axis("left_trigger", 1.0));
    rig.step(
        &pad()
            .with_axis("left_trigger", 1.0)
            .with_axis("left_y", 1.0)
            .with_axis("left_x", -1.0)
            .with_button("south", true),
    );
    assert_eq!(rig.at("hoist"), "up2");
    assert_eq!(rig.at("trolley"), "left2");
    assert_eq!(rig.at("horn"), "on");
    assert_eq!(rig.at("start"), "on");
    assert_eq!(rig.at("deadman"), "held");
    rig
}

#[test]
fn losing_the_device_releases_every_held_control_at_once() {
    let mut rig = driving_rig(deadman_map());
    let out = rig.step(&InputState::disconnected());
    assert!(!out.connected);
    for c in ["hoist", "trolley", "horn", "deadman"] {
        assert_eq!(out.holds.get(c), Some(&None), "{c} released");
    }
    assert_eq!(rig.at("hoist"), "off");
    assert_eq!(rig.at("trolley"), "off");
    assert_eq!(rig.at("horn"), "off");
    assert_eq!(rig.at("deadman"), "released");
    // Release policy: nothing else happens, main stays on and the link stays up.
    assert!(out.link.is_none());
    assert!(out.actions.is_empty());
    assert!(rig.ctl.link_ok());
    assert_eq!(rig.at("start"), "on");
    // And it keeps saying so every frame while it is gone.
    let again = rig.step(&InputState::disconnected());
    assert_eq!(again.holds.get("hoist"), Some(&None));
    assert!(again.link.is_none());
}

#[test]
fn a_disconnected_device_ignores_stale_values() {
    let mut rig = driving_rig(deadman_map());
    let mut gone = pad()
        .with_axis("left_trigger", 1.0)
        .with_axis("left_y", 1.0);
    gone.connected = false;
    rig.step(&gone);
    assert_eq!(rig.at("hoist"), "off");
    assert_eq!(rig.at("deadman"), "released");
}

#[test]
fn drop_link_policy_drops_the_radio_link_once_and_restores_it_on_return() {
    let mut map = deadman_map();
    map.on_disconnect = LinkLoss::DropLink;
    let mut rig = driving_rig(map);
    let out = rig.step(&InputState::disconnected());
    assert_eq!(out.link, Some(false));
    assert!(!rig.ctl.link_ok());
    assert_eq!(
        rig.at("start"),
        "off",
        "the receiver-side main latch dropped with the link"
    );
    assert!(!rig.ctl.is_high("MAIN"));
    assert!(
        rig.step(&InputState::disconnected()).link.is_none(),
        "only once"
    );

    let back = rig.step(&pad());
    assert_eq!(back.link, Some(true));
    assert!(rig.ctl.link_ok());
    assert_eq!(rig.at("start"), "off", "main does not come back by itself");
    assert!(rig.step(&pad()).link.is_none());
}

#[test]
fn estop_policy_presses_the_estop_and_reconnecting_does_not_reset_it() {
    let mut map = deadman_map();
    map.on_disconnect = LinkLoss::Estop;
    let mut rig = driving_rig(map);
    let out = rig.step(&InputState::disconnected());
    assert_eq!(
        out.actions,
        vec![Action::Press {
            control: "estop".into()
        }]
    );
    assert_eq!(rig.at("estop"), "pressed");
    assert!(!rig.ctl.is_high("MAIN"));
    assert!(
        rig.step(&InputState::disconnected()).actions.is_empty(),
        "only once"
    );
    rig.step(&pad());
    rig.step(&pad());
    assert_eq!(
        rig.at("estop"),
        "pressed",
        "stays pressed until someone resets it"
    );
}

#[test]
fn reconnecting_with_the_stick_over_does_not_resume_motion() {
    let mut rig = driving_rig(deadman_map());
    rig.step(&InputState::disconnected());
    let over = pad()
        .with_axis("left_trigger", 1.0)
        .with_axis("left_y", 1.0);
    rig.step(&over.clone());
    assert_eq!(rig.at("hoist"), "off");
    rig.step(&pad().with_axis("left_trigger", 1.0));
    rig.step(&over);
    assert_eq!(rig.at("hoist"), "up2");
}

#[test]
fn a_device_that_never_connected_triggers_no_policy() {
    let mut map = plain_map();
    map.on_disconnect = LinkLoss::Estop;
    let mut rig = Rig::new(map);
    let out = rig.step(&InputState::disconnected());
    assert!(out.actions.is_empty());
    assert!(out.link.is_none());
    assert_eq!(rig.at("estop"), "released");
}

#[test]
fn estop_policy_without_a_mapped_button_uses_the_profiles_estop() {
    let mut map = GamepadMap::default();
    map.axes.push(AxisMap::new("left_y", "hoist"));
    map.on_disconnect = LinkLoss::Estop;
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&InputState::disconnected());
    assert_eq!(rig.at("estop"), "pressed");
}

#[test]
fn a_spring_less_lever_goes_to_rest_when_the_device_is_lost() {
    let mut map = GamepadMap::default();
    map.axes.push(AxisMap::new("right_y", "lever"));
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("right_y", 1.0));
    assert_eq!(rig.at("lever"), "hi");
    rig.step(&InputState::disconnected());
    assert_eq!(rig.at("lever"), "mid");
}

// ---- robustness ----

#[test]
fn entries_that_name_missing_controls_are_skipped_not_fatal() {
    let mut map = plain_map();
    map.axes.push(AxisMap::new("left_y", "nonexistent"));
    map.buttons
        .push(ButtonMap::new("west", "nonexistent", ButtonAction::Hold));
    let mut rig = Rig::new(map);
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0).with_button("west", true));
    assert_eq!(rig.at("hoist"), "up2");
}

#[test]
fn the_same_input_gives_the_same_output() {
    let run = || {
        let mut rig = Rig::new(deadman_map());
        let mut seen = Vec::new();
        let frames = [
            pad(),
            pad().with_axis("left_trigger", 1.0),
            pad()
                .with_axis("left_trigger", 1.0)
                .with_axis("left_y", 0.7),
            pad()
                .with_axis("left_trigger", 1.0)
                .with_axis("left_y", 0.2),
            pad().with_button("east", true),
            InputState::disconnected(),
        ];
        for f in &frames {
            seen.push(rig.step(f));
        }
        seen
    };
    assert_eq!(run(), run());
}

#[test]
fn the_monitor_reading_shows_raw_value_step_and_position() {
    let mut map = plain_map();
    map.axes[0].centre = 0.1;
    let mut rig = Rig::new(map);
    rig.step(&pad().with_axis("left_y", 0.1));
    let out = rig.step(&pad().with_axis("left_y", 0.1));
    let r = &out.axes[0];
    assert_eq!((r.input.as_str(), r.control.as_str()), ("left_y", "hoist"));
    assert_eq!(r.raw, 0.1);
    assert_eq!(r.value, 0.0);
    assert_eq!(r.step, 0);
    assert_eq!(r.position, None);
    let out = rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(out.axes[0].step, 2);
    assert_eq!(out.axes[0].position.as_deref(), Some("up2"));
}

// ---- the map: file, validation, suggestion ----

#[test]
fn a_map_round_trips_through_toml() {
    let mut map = deadman_map();
    map.device = "Test pad".into();
    map.on_disconnect = LinkLoss::DropLink;
    map.axes[0].invert = true;
    map.axes[0].deadzone = 0.2;
    map.axes[1].gated = false;
    map.neutral_required = false;
    map.buttons[0].position = Some("x".into());
    let text = map.to_toml().unwrap();
    assert_eq!(GamepadMap::parse(&text).unwrap(), map);
}

#[test]
fn defaults_are_filled_in_and_unknown_fields_are_refused() {
    let m = map_of("[[axis]]\ninput = \"left_y\"\ncontrol = \"hoist\"\n");
    let a = &m.axes[0];
    assert_eq!(a.deadzone, DEFAULT_DEADZONE);
    assert_eq!(a.hysteresis, DEFAULT_HYSTERESIS);
    assert_eq!((a.centre, a.low, a.high), (0.0, -1.0, 1.0));
    assert!(a.gated && !a.invert);
    assert!(m.neutral_required);
    assert_eq!(m.on_disconnect, LinkLoss::Release);
    assert!(
        GamepadMap::parse("[[axis]]\ninput = \"left_y\"\ncontrol = \"h\"\nspeed = 3\n").is_err()
    );
    assert!(GamepadMap::parse("on_disconnect = \"explode\"\n").is_err());
    assert!(
        GamepadMap::parse("[[button]]\ninput = \"a\"\ncontrol = \"b\"\naction = \"bang\"\n")
            .is_err()
    );
}

#[test]
fn an_empty_map_is_empty_and_serialises_to_nothing_much() {
    let m = GamepadMap::default();
    assert!(m.is_empty());
    assert!(!m.to_toml().unwrap().contains("axis"));
}

fn problems(map: &GamepadMap) -> Vec<(Severity, String)> {
    map.validate(&pad_profile())
        .into_iter()
        .map(|i| (i.severity, format!("{} {}", i.path, i.message)))
        .collect()
}

fn has(map: &GamepadMap, severity: Severity, text: &str) -> bool {
    problems(map)
        .iter()
        .any(|(s, m)| *s == severity && m.contains(text))
}

#[test]
fn a_good_map_has_no_errors() {
    for map in [plain_map(), deadman_map()] {
        let errors: Vec<_> = problems(&map)
            .into_iter()
            .filter(|(s, _)| *s == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "{errors:?}");
    }
    assert!(!plain_map().has_errors(&pad_profile()));
}

#[test]
fn validation_finds_each_kind_of_defect() {
    let e = Severity::Error;
    let w = Severity::Warning;

    let mut m = plain_map();
    m.axes[0].input = "banana".into();
    assert!(has(&m, e, "not a pad axis"));

    let mut m = plain_map();
    m.axes[0].control = "ghost".into();
    assert!(has(&m, e, "does not exist"));

    let mut m = plain_map();
    m.axes[0].control = "horn".into();
    assert!(has(&m, e, "cannot drive a button"));

    let mut m = plain_map();
    m.axes[0].deadzone = 0.95;
    assert!(has(&m, e, "deadzone must be"));
    m.axes[0].deadzone = f64::NAN;
    assert!(has(&m, e, "deadzone must be"));

    let mut m = plain_map();
    m.axes[0].hysteresis = 0.7;
    assert!(has(&m, e, "hysteresis must be"));

    let mut m = plain_map();
    m.axes[0].low = 0.2;
    assert!(has(&m, e, "low < centre < high"));
    let mut m = plain_map();
    m.axes[0].high = -2.0;
    assert!(has(&m, e, "low < centre < high"));

    let mut m = plain_map();
    m.axes[0].high = 0.1;
    assert!(has(&m, w, "under 0.3"));

    let mut m = plain_map();
    m.axes[0].deadzone = 0.6;
    assert!(has(&m, w, "over 0.5"));

    let mut m = plain_map();
    m.axes.push(AxisMap::new("left_y", "hoist"));
    assert!(has(&m, w, "mapped twice"));
    let mut m = plain_map();
    m.axes.push(AxisMap::new("right_x", "hoist"));
    assert!(has(&m, w, "has two axes"));

    let mut m = plain_map();
    m.buttons[3].action = ButtonAction::Toggle;
    assert!(has(&m, e, "does not fit a button"));
    let mut m = plain_map();
    m.buttons[3].position = Some("sideways".into());
    assert!(has(&m, e, "no position `sideways`"));
    let mut m = plain_map();
    m.buttons[3].input = "teleport".into();
    assert!(has(&m, e, "not a pad button"));
    let mut m = plain_map();
    m.buttons[0].control = "horn".into();
    assert!(has(&m, e, "does not fit a button"));
    let mut m = plain_map();
    m.buttons
        .push(ButtonMap::new("west", "hoist_sel", ButtonAction::Set));
    assert!(has(&m, e, "needs a position"));
    let mut m = plain_map();
    m.buttons
        .push(ButtonMap::new("west", "hoist", ButtonAction::Next));
    assert!(has(&m, e, "does not fit a rocker2"));
    let mut m = plain_map();
    m.buttons
        .push(ButtonMap::new("west", "hoist", ButtonAction::Hold));
    assert!(has(&m, w, "holds the first step"));

    let mut m = deadman_map();
    m.deadman.as_mut().unwrap().control = Some("horn".into());
    assert!(has(&m, e, "not a dead-man"));
    m.deadman.as_mut().unwrap().control = Some("zzz".into());
    assert!(has(&m, e, "does not exist"));
    m.deadman.as_mut().unwrap().input = "nope".into();
    assert!(has(&m, e, "not a pad input"));
}

#[test]
fn validation_warns_when_nothing_on_the_pad_can_press_the_estop() {
    let mut m = plain_map();
    m.buttons.retain(|b| b.action != ButtonAction::Estop);
    assert!(has(
        &m,
        Severity::Warning,
        "no pad button presses the E-stop"
    ));
    assert!(!has(
        &plain_map(),
        Severity::Warning,
        "no pad button presses"
    ));
}

#[test]
fn validation_warns_about_a_missing_deadman_input_when_the_profile_has_one() {
    assert!(has(
        &plain_map(),
        Severity::Warning,
        "dead-man control but no pad input"
    ));
    assert!(!has(
        &deadman_map(),
        Severity::Warning,
        "dead-man control but no pad input"
    ));
}

#[test]
fn estop_policy_needs_an_estop_in_the_profile() {
    let profile = Profile::load(
        r#"
[controller]
id = "nostop"
name = "No stop"

[[control]]
id = "go"
kind = "button"

[[signal]]
name = "GO"
control = "go"
when = ["on"]
"#,
    )
    .unwrap();
    let mut m = GamepadMap {
        on_disconnect: LinkLoss::Estop,
        ..GamepadMap::default()
    };
    assert!(
        m.validate(&profile)
            .iter()
            .any(|i| i.severity == Severity::Error)
    );
    m.on_disconnect = LinkLoss::Release;
    assert!(m.validate(&profile).is_empty());
}

#[test]
fn a_one_directional_rocker_is_noted() {
    let profile = Profile::load(
        r#"
[controller]
id = "oneway"
name = "One way"

[[control]]
id = "lift"
kind = "rocker2"
positions = ["off", "up1", "up2"]

[[signal]]
name = "UP"
control = "lift"
group = "up"
step = 1
"#,
    )
    .unwrap();
    let mut m = GamepadMap::default();
    m.axes.push(AxisMap::new("left_y", "lift"));
    assert!(
        m.validate(&profile)
            .iter()
            .any(|i| i.severity == Severity::Warning && i.message.contains("one direction only"))
    );
}

#[test]
fn numbered_device_inputs_are_accepted() {
    for ok in ["axis0", "axis12", "left_x"] {
        assert!(is_axis_name(ok), "{ok}");
    }
    for bad in ["axis", "axisX", "left", "button3", ""] {
        assert!(!is_axis_name(bad), "{bad}");
    }
    assert!(is_button_name("button7") && is_button_name("south"));
    assert!(!is_button_name("button") && !is_button_name("axis1"));
    let mut m = plain_map();
    m.axes[0].input = "axis3".into();
    assert!(!m.has_errors(&pad_profile()));
}

#[test]
fn the_suggested_map_is_valid_for_every_shipped_profile_and_for_the_test_profile() {
    let mut profiles = vec![pad_profile()];
    for (id, _) in import::library::list() {
        let built = import::library::build(id).expect("shipped profile builds");
        profiles.push(Profile::load(&built.profile.to_toml()).unwrap());
    }
    assert!(profiles.len() > 1);
    for p in profiles {
        let m = GamepadMap::suggest(&p);
        let errors: Vec<_> = m
            .validate(&p)
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .collect();
        assert!(errors.is_empty(), "{}: {errors:?}", p.controller.id);
        assert!(!m.is_empty(), "{} gets something mapped", p.controller.id);
        // And a suggested E-stop button exists whenever the profile has an E-stop.
        if p.controls.iter().any(|c| c.kind == ControlKind::EStop) {
            assert!(
                m.buttons.iter().any(|b| b.action == ButtonAction::Estop),
                "{} has an E-stop button on the pad",
                p.controller.id
            );
        }
    }
}

#[test]
fn the_suggested_map_for_the_radio_drops_the_link_when_the_pad_goes() {
    let m = GamepadMap::suggest(&pad_profile());
    assert_eq!(m.on_disconnect, LinkLoss::DropLink);
    assert_eq!(
        m.deadman.as_ref().unwrap().control.as_deref(),
        Some("deadman")
    );
    assert_eq!(m.axes[0].control, "hoist");
}

#[test]
fn project_data_keeps_the_pad_maps_by_instance() {
    let mut set = ControllerSet::new();
    set.add_instance("radio-1", "Radio 1", &pad_profile())
        .unwrap();
    let mut data = ProjectControllers::capture(&set, &BindingTable::new()).unwrap();
    assert!(data.gamepads.is_empty());
    assert!(!data.to_text().unwrap().contains("gamepad"));
    data.gamepads.insert("radio-1".into(), deadman_map());
    let text = data.to_text().unwrap();
    let back = ProjectControllers::from_text(&text).unwrap();
    assert_eq!(back, data);
    assert_eq!(back.gamepads["radio-1"], deadman_map());
    // Projects saved before pad maps existed still read.
    let old = ProjectControllers::from_text("").unwrap();
    assert!(old.gamepads.is_empty());
}

#[test]
fn calibration_results_land_in_every_map_entry_of_that_input() {
    let mut m = plain_map();
    m.axes.push(AxisMap::new("left_y", "crab"));
    m.apply_calibration(&[AxisCal {
        input: "left_y".into(),
        centre: 0.05,
        low: -0.8,
        high: 0.9,
    }]);
    assert_eq!(
        (m.axes[0].centre, m.axes[0].low, m.axes[0].high),
        (0.05, -0.8, 0.9)
    );
    assert_eq!(m.axes[3].centre, 0.05);
    assert_eq!(m.axes[1].centre, 0.0, "other inputs are untouched");
}

// ---- the calibration wizard ----

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// Feed the centre phase with a stick that rests at (0.04, -0.03).
fn centred(cal: &mut Calibrator) {
    for _ in 0..40 {
        cal.feed(&pad().with_axis("left_x", 0.04).with_axis("left_y", -0.03));
    }
}

#[test]
fn the_centre_is_the_average_of_the_rest_samples() {
    let mut cal = Calibrator::new(&names(&["left_x", "left_y"]));
    assert_eq!(cal.phase(), CalPhase::Centre);
    assert!(!cal.start_range(), "no samples yet");
    for i in 0..30 {
        assert!(!cal.centre_ready() || i == 30);
        // Noise around the rest values that averages out.
        let n = if i % 2 == 0 { 0.01 } else { -0.01 };
        cal.feed(
            &pad()
                .with_axis("left_x", 0.04 + n)
                .with_axis("left_y", -0.03 - n),
        );
    }
    assert!(cal.centre_ready());
    assert_eq!(cal.centre_fraction(), 1.0);
    assert!(cal.start_range());
    assert_eq!(cal.phase(), CalPhase::Range);
    let p = cal.progress();
    assert!((p[0].centre - 0.04).abs() < 1e-9);
    assert!((p[1].centre + 0.03).abs() < 1e-9);
}

#[test]
fn moving_to_all_corners_gives_centre_and_range() {
    let mut cal = Calibrator::new(&names(&["left_x", "left_y"])).with_pair("left_x", "left_y");
    centred(&mut cal);
    assert!(cal.start_range());
    assert!(!cal.complete());
    // A worn stick: x reaches -0.9 .. 0.8, y reaches -0.7 .. 0.95.
    for (x, y) in [
        (0.8, 0.95),
        (-0.9, 0.95),
        (-0.9, -0.7),
        (0.8, -0.7),
        (0.0, 0.0),
    ] {
        cal.feed(&pad().with_axis("left_x", x).with_axis("left_y", y));
    }
    assert!(cal.complete());
    assert!(cal.corners()[0].2.iter().all(|&c| c));
    let r = cal.finish().unwrap();
    let x = &r[0];
    assert_eq!(x.input, "left_x");
    assert!((x.centre - 0.04).abs() < 1e-9);
    assert!((x.high - (0.04 + 0.76 * END_SHARE_FOR_TESTS)).abs() < 1e-9);
    assert!((x.low - (0.04 - 0.94 * END_SHARE_FOR_TESTS)).abs() < 1e-9);
    let y = &r[1];
    assert!((y.high - (-0.03 + 0.98 * END_SHARE_FOR_TESTS)).abs() < 1e-9);

    // The calibrated map reaches full deflection at the observed ends, so the top step is
    // reachable although the stick never gets to 1.0.
    let mut a = AxisMap::new("left_x", "trolley");
    a.centre = x.centre;
    a.low = x.low;
    a.high = x.high;
    assert_eq!(a.value(0.8), 1.0);
    assert_eq!(a.value(-0.9), -1.0);
}

/// `calibrate::END_SHARE` is not public; the wizard documents 97 per cent.
const END_SHARE_FOR_TESTS: f64 = 0.97;

#[test]
fn an_unfinished_calibration_names_what_is_missing() {
    let mut cal = Calibrator::new(&names(&["left_x", "left_y"]));
    assert_eq!(cal.finish(), Err(CalError::CentreNotReady));
    centred(&mut cal);
    cal.start_range();
    cal.feed(&pad().with_axis("left_x", 0.9).with_axis("left_y", 0.1));
    let Err(CalError::Incomplete(missing)) = cal.finish() else {
        panic!("incomplete")
    };
    assert!(missing.contains(&("left_x".to_owned(), "low")));
    assert!(missing.contains(&("left_y".to_owned(), "low")));
    assert!(missing.contains(&("left_y".to_owned(), "high")));
    assert!(!missing.contains(&("left_x".to_owned(), "high")));
}

#[test]
fn a_small_wiggle_is_not_a_full_throw() {
    let mut cal = Calibrator::new(&names(&["left_x"]));
    for _ in 0..30 {
        cal.feed(&pad());
    }
    cal.start_range();
    cal.feed(&pad().with_axis("left_x", 0.45));
    cal.feed(&pad().with_axis("left_x", -0.45));
    assert!(matches!(cal.finish(), Err(CalError::Incomplete(_))));
    assert!(!cal.progress()[0].done());
}

#[test]
fn a_trigger_only_needs_its_pressed_side() {
    let mut cal = Calibrator::new(&names(&["left_trigger"]));
    for _ in 0..30 {
        cal.feed(&pad());
    }
    cal.start_range();
    assert!(!cal.progress()[0].done());
    cal.feed(&pad().with_axis("left_trigger", 1.0));
    assert!(cal.progress()[0].done());
    let r = cal.finish().unwrap();
    assert_eq!(r[0].centre, 0.0);
    assert!(r[0].low < r[0].centre && r[0].centre < r[0].high);
}

#[test]
fn a_disconnected_device_adds_no_samples_and_duplicates_are_folded() {
    let mut cal = Calibrator::new(&names(&["left_x", "left_x"]));
    for _ in 0..100 {
        cal.feed(&InputState::disconnected().with_axis("left_x", 1.0));
    }
    assert!(!cal.centre_ready());
    assert_eq!(cal.progress().len(), 1);
}

#[test]
fn a_pair_that_names_an_unknown_axis_is_ignored() {
    let cal = Calibrator::new(&names(&["left_x"])).with_pair("left_x", "left_y");
    assert!(cal.corners().is_empty());
}

// ---- end to end: calibrate, validate, drive ----

#[test]
fn calibrated_worn_stick_drives_all_steps_of_the_rocker() {
    // The stick rests at 0.06 and only reaches 0.74 up and -0.80 down.
    let mut cal = Calibrator::new(&names(&["left_y"]));
    for _ in 0..30 {
        cal.feed(&pad().with_axis("left_y", 0.06));
    }
    cal.start_range();
    cal.feed(&pad().with_axis("left_y", 0.74));
    cal.feed(&pad().with_axis("left_y", -0.80));
    cal.feed(&pad().with_axis("left_y", 0.06));
    let result = cal.finish().unwrap();

    let mut map = plain_map();
    map.apply_calibration(&result);
    assert!(!map.has_errors(&pad_profile()));
    let mut rig = Rig::new(map);
    rig.step(&pad().with_axis("left_y", 0.06));
    rig.step(&pad().with_axis("left_y", 0.06));
    assert_eq!(rig.at("hoist"), "off", "the worn rest position is rest");
    rig.step(&pad().with_axis("left_y", 0.74));
    assert_eq!(rig.at("hoist"), "up2", "the worn top reaches the last step");
    rig.step(&pad().with_axis("left_y", -0.80));
    assert_eq!(rig.at("hoist"), "down2");
}

#[test]
fn every_control_of_a_signal_table_follows_the_pad() {
    // The signal levels the pad makes by stick position, as a table.
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_button("start", true));
    rig.step(&pad());
    let mut seen: BTreeMap<&str, Vec<(String, bool, bool)>> = BTreeMap::new();
    for v in [0.0, 0.3, 0.9, -0.3, -0.9] {
        rig.step(&pad().with_axis("left_y", v));
        seen.entry("hoist").or_default().push((
            rig.at("hoist"),
            rig.ctl.is_high("H_UP_SLOW"),
            rig.ctl.is_high("H_UP_FAST"),
        ));
    }
    let table = &seen["hoist"];
    assert_eq!(table[0], ("off".into(), false, false));
    assert_eq!(table[1], ("up1".into(), true, false));
    assert_eq!(table[2].0, "up2");
    assert!(table[2].2);
    assert_eq!(table[3].0, "down1");
    assert!(!table[3].1 && !table[3].2);
    assert_eq!(table[4].0, "down2");
}

#[test]
fn after_hands_off_frames_a_deflected_stick_must_return_to_rest_first() {
    let mut rig = Rig::new(plain_map());
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "up2");
    // The host feeds a pad at rest while the window is in the background.
    rig.mapper.hands_off();
    rig.step(&pad());
    assert_eq!(rig.at("hoist"), "off");
    // Back with the stick still over, and a button down: nothing acts until both are let go.
    rig.step(&pad().with_axis("left_y", 1.0).with_button("south", true));
    assert_eq!(rig.at("hoist"), "off");
    assert_eq!(
        rig.at("horn"),
        "off",
        "a button held when the pad comes back fires nothing"
    );
    rig.step(&pad());
    rig.step(&pad().with_axis("left_y", 1.0));
    assert_eq!(rig.at("hoist"), "up2");
}
