//! The gamepad in the live view (lane C5), driven by a fake device: the radio-hoist fixture of
//! `novo-tests` settled with the pad holding the rocker, headless, through the same
//! `LiveView::solved` path the live view draws from. The rules of the map itself (deadzone,
//! detents, dead-man, device loss) are tested in `novo-controller`; these tests check the host:
//! that what the mapper asks reaches the controller and the drawn circuit, that the pad lets go
//! when it must, and that the map is saved with the project.

use std::collections::BTreeSet;

use novo_controller::gamepad::{
    AxisMap, ButtonAction, ButtonMap, CalPhase, DeadmanMap, GamepadMap, LinkLoss,
};
use novo_controller::{ControlDef, ProjectControllers};
use novo_core::{Document, SymbolId};
use novo_sim::{MotorRun, device_states};
use novo_tests::{RadioHoist, radio_hoist_with_controller};

use super::controller_live::{ControllerLive, HoldBy};
use super::input_devices::{FakeBackend, FakeHandle, InputService};
use super::live::LiveView;

const ID: &str = "radio-1";

fn rig() -> (Document, RadioHoist, LiveView, FakeHandle) {
    let mut doc = Document::default();
    let (h, _) = radio_hoist_with_controller(&mut doc).expect("fixture builds");
    let mut live = LiveView::default();
    live.reset_poles(doc.project());
    live.solved(doc.project(), 0.0);
    let (backend, hand) = FakeBackend::new();
    live.ctl
        .pad
        .set_service(InputService::new(Box::new(backend)));
    (doc, h, live, hand)
}

fn map() -> GamepadMap {
    let mut m = GamepadMap::default();
    m.axes.push(AxisMap::new("left_y", "hoist"));
    m.buttons
        .push(ButtonMap::new("east", "estop", ButtonAction::Estop));
    m.buttons
        .push(ButtonMap::new("start", "start", ButtonAction::Toggle));
    m
}

/// One frame of the card: the pad step, the holds, then the circuit.
fn frame(live: &mut LiveView, doc: &Document, focused: bool) -> (BTreeSet<SymbolId>, MotorRun) {
    live.ctl.pad_frame(focused);
    live.ctl.apply_holds();
    let mut energized = BTreeSet::new();
    let mut motor = MotorRun::Stopped;
    for _ in 0..4 {
        let solved = live.solved(doc.project(), 0.05);
        energized = solved.settled.energized.clone();
        let states = device_states(doc.project(), &solved.netlist, &solved.settled);
        if let Some(run) = states.motors.values().next() {
            motor = *run;
        }
    }
    (energized, motor)
}

fn position(live: &LiveView, control: &str) -> String {
    live.ctl
        .set
        .instance(ID)
        .and_then(|i| i.controller.position(control))
        .unwrap()
        .to_owned()
}

fn drive(live: &mut LiveView) {
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.pad_set_drive(true);
}

fn start_main(live: &mut LiveView, doc: &Document, hand: &FakeHandle) {
    frame(live, doc, true);
    hand.set_button("start", true);
    frame(live, doc, true);
    hand.set_button("start", false);
    frame(live, doc, true);
    assert_eq!(position(live, "start"), "on");
}

fn running(m: &MotorRun) -> bool {
    matches!(m, MotorRun::Running { .. })
}

#[test]
fn the_pad_holds_the_hoist_rocker_and_the_drawn_circuit_runs_the_motor() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);

    hand.set_axis("left_y", 0.3);
    let (on, motor) = frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "up1");
    assert!(!on.is_empty());
    assert!(
        matches!(motor, MotorRun::Running { speed: 0, .. }),
        "{motor:?}"
    );

    hand.set_axis("left_y", 1.0);
    let (_, motor) = frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "up2");
    assert!(
        matches!(motor, MotorRun::Running { speed: 1, .. }),
        "{motor:?}"
    );

    // Stick back to the middle: the spring returns the rocker and the motor stops.
    hand.set_axis("left_y", 0.0);
    let (on, motor) = frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "off");
    assert!(on.is_empty() || !running(&motor), "{on:?} {motor:?}");
    assert_eq!(motor, MotorRun::Stopped);
}

#[test]
fn the_estop_button_drops_every_output_of_the_circuit() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    hand.set_axis("left_y", 1.0);
    let (_, motor) = frame(&mut live, &doc, true);
    assert!(running(&motor));
    hand.set_button("east", true);
    let (on, motor) = frame(&mut live, &doc, true);
    assert_eq!(position(&live, "estop"), "pressed");
    assert!(on.is_empty(), "{on:?}");
    assert_eq!(motor, MotorRun::Stopped);
}

#[test]
fn unplugging_the_pad_stops_the_motor_in_the_same_frame() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    hand.set_axis("left_y", 1.0);
    assert!(running(&frame(&mut live, &doc, true).1));
    hand.connect(false);
    let (_, motor) = frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "off");
    assert_eq!(motor, MotorRun::Stopped);
}

#[test]
fn the_drop_link_policy_takes_the_radio_down_when_the_pad_goes() {
    let (doc, _h, mut live, hand) = rig();
    let mut m = map();
    m.on_disconnect = LinkLoss::DropLink;
    live.ctl.pad.maps.insert(ID.to_owned(), m);
    live.ctl.pad_set_drive(true);
    start_main(&mut live, &doc, &hand);
    hand.connect(false);
    frame(&mut live, &doc, true);
    let c = &live.ctl.set.instance(ID).unwrap().controller;
    assert!(!c.link_ok());
    assert_eq!(c.position("start"), Some("off"), "main drops with the link");
    hand.connect(true);
    frame(&mut live, &doc, true);
    let c = &live.ctl.set.instance(ID).unwrap().controller;
    assert!(c.link_ok());
    assert_eq!(
        c.position("start"),
        Some("off"),
        "and does not come back by itself"
    );
}

#[test]
fn a_pad_that_is_only_watched_moves_nothing_and_shows_what_it_would_do() {
    let (doc, _h, mut live, hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.pad.ensure_service();
    assert!(!live.ctl.pad.drive, "off until asked");
    frame(&mut live, &doc, true);
    hand.set_axis("left_y", 1.0);
    frame(&mut live, &doc, true);
    hand.set_axis("left_y", 0.0);
    frame(&mut live, &doc, true);
    hand.set_axis("left_y", 1.0);
    let (on, _) = frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "off");
    assert!(on.is_empty());
    let (who, out) = live.ctl.pad.last.clone().expect("the test result is kept");
    assert_eq!(who, ID);
    assert_eq!(out.holds.get("hoist"), Some(&Some("up2".to_owned())));
}

#[test]
fn switching_the_pad_on_with_the_stick_over_waits_for_neutral() {
    let (doc, _h, mut live, hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.pad.ensure_service();
    hand.set_axis("left_y", 1.0);
    frame(&mut live, &doc, true);
    live.ctl.pad_set_drive(true);
    frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "off");
    hand.set_axis("left_y", 0.0);
    frame(&mut live, &doc, true);
    hand.set_axis("left_y", 1.0);
    frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "up2");
}

#[test]
fn a_window_in_the_background_lets_go_without_counting_as_unplugged() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    hand.set_axis("left_y", 1.0);
    assert!(running(&frame(&mut live, &doc, true).1));
    let (_, motor) = frame(&mut live, &doc, false);
    assert_eq!(motor, MotorRun::Stopped);
    assert!(live.ctl.set.instance(ID).unwrap().controller.link_ok());
    // Back in the foreground the stick is still over: it must come back to neutral first.
    let (_, motor) = frame(&mut live, &doc, true);
    assert_eq!(motor, MotorRun::Stopped);
    hand.set_axis("left_y", 0.0);
    frame(&mut live, &doc, true);
    hand.set_axis("left_y", 1.0);
    assert!(running(&frame(&mut live, &doc, true).1));
}

#[test]
fn closing_the_card_lets_go_of_everything_the_pad_holds() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    hand.set_axis("left_y", 1.0);
    assert!(running(&frame(&mut live, &doc, true).1));
    live.ctl.pad_card_closed();
    assert_eq!(position(&live, "hoist"), "off");
    // Nothing else is touched on later frames of a closed card.
    live.ctl.pad_card_closed();
}

#[test]
fn the_pad_hold_wins_over_the_mouse_and_lets_the_mouse_have_it_back() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    live.ctl.hold(ID, "hoist", HoldBy::Mouse, Some("down1"));
    frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "down1");
    hand.set_axis("left_y", 1.0);
    frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "up2");
    hand.set_axis("left_y", 0.0);
    frame(&mut live, &doc, true);
    assert_eq!(
        position(&live, "hoist"),
        "down1",
        "the mouse still holds it"
    );
    live.ctl.hold(ID, "hoist", HoldBy::Mouse, None);
    frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "off");
}

#[test]
fn only_the_selected_controller_follows_the_pad_and_changing_it_lets_go() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    hand.set_axis("left_y", 1.0);
    assert!(running(&frame(&mut live, &doc, true).1));
    live.ctl.selected = None;
    frame(&mut live, &doc, true);
    assert_eq!(position(&live, "hoist"), "off");
}

#[test]
fn removing_a_controller_forgets_its_map() {
    let (_doc, _h, mut live, _hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.remove_instance(ID);
    assert!(live.ctl.pad.maps.is_empty());
}

#[test]
fn the_map_is_saved_with_the_project_and_read_back() {
    let (_doc, _h, mut live, _hand) = rig();
    let mut m = map();
    m.deadman = Some(DeadmanMap {
        input: "left_trigger".into(),
        control: None,
    });
    m.axes[0].deadzone = 0.22;
    live.ctl.pad.maps.insert(ID.to_owned(), m.clone());
    let text = live.ctl.snapshot().expect("serialises");
    let data = ProjectControllers::from_text(&text).unwrap();
    assert_eq!(data.gamepads.get(ID), Some(&m));

    // A map of an instance that does not exist is not written.
    live.ctl.pad.maps.insert("ghost".to_owned(), m);
    let data = ProjectControllers::from_text(&live.ctl.snapshot().unwrap()).unwrap();
    assert!(!data.gamepads.contains_key("ghost"));
}

#[test]
fn a_project_with_a_map_gives_it_back_on_sync_and_one_without_clears_it() {
    let (mut doc, _h, mut live, _hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    let text = live.ctl.snapshot().unwrap();
    doc.transact("Controllers", |tx| {
        tx.set_addon_block(
            super::controller_live::ADDON,
            super::controller_live::BLOCK,
            novo_core::AddonBlock {
                format: super::controller_live::BLOCK_FORMAT,
                text: text.clone(),
            },
        )
    })
    .expect("block is written");
    let mut fresh = ControllerLive::default();
    fresh.sync(doc.project());
    assert_eq!(fresh.pad.maps.get(ID), Some(&map()));
}

#[test]
fn listening_fills_the_input_from_the_first_thing_the_pad_does() {
    let (doc, _h, mut live, hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.pad.ensure_service();
    frame(&mut live, &doc, true);
    live.ctl.pad.listen = Some((
        super::controller_pad::Listen::Axis(0),
        live.ctl.pad.input.clone(),
    ));
    hand.set_axis("left_x", 0.3);
    frame(&mut live, &doc, true);
    assert!(live.ctl.pad.listen.is_some(), "a small move is not enough");
    hand.set_axis("right_y", -0.9);
    frame(&mut live, &doc, true);
    assert!(live.ctl.pad.listen.is_none());
    assert_eq!(live.ctl.pad.maps[ID].axes[0].input, "right_y");

    live.ctl.pad.listen = Some((
        super::controller_pad::Listen::Button(1),
        live.ctl.pad.input.clone(),
    ));
    hand.set_button("west", true);
    frame(&mut live, &doc, true);
    assert_eq!(live.ctl.pad.maps[ID].buttons[1].input, "west");

    live.ctl.pad.listen = Some((
        super::controller_pad::Listen::Deadman,
        live.ctl.pad.input.clone(),
    ));
    hand.set_axis("left_trigger", 1.0);
    frame(&mut live, &doc, true);
    assert_eq!(
        live.ctl.pad.maps[ID]
            .deadman
            .as_ref()
            .map(|d| d.input.as_str()),
        Some("left_trigger")
    );
}

#[test]
fn the_calibration_wizard_measures_a_worn_stick_and_writes_it_into_the_map() {
    let (doc, _h, mut live, hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.pad_open_wizard();
    assert!(live.ctl.pad.wizard.is_some());
    // The stick rests at 0.05.
    hand.set_axis("left_y", 0.05);
    for _ in 0..40 {
        frame(&mut live, &doc, true);
    }
    // Moves to both ends: -0.9 and +0.8.
    for v in [0.8, -0.9, 0.05] {
        hand.set_axis("left_y", v);
        frame(&mut live, &doc, true);
    }
    assert!(live.ctl.pad_wizard_finish());
    assert!(live.ctl.pad.wizard.is_none());
    let a = &live.ctl.pad.maps[ID].axes[0];
    assert!((a.centre - 0.05).abs() < 1e-9);
    assert!(a.high > 0.7 && a.high < 0.8, "{a:?}");
    assert!(a.low < -0.8 && a.low > -0.9, "{a:?}");
    assert!(live.ctl.pad.edit_pending);
}

#[test]
fn an_unfinished_calibration_stays_open_and_says_what_is_missing() {
    let (doc, _h, mut live, hand) = rig();
    live.ctl.pad.maps.insert(ID.to_owned(), map());
    live.ctl.pad_open_wizard();
    for _ in 0..40 {
        frame(&mut live, &doc, true);
    }
    hand.set_axis("left_y", 0.9);
    frame(&mut live, &doc, true);
    assert!(!live.ctl.pad_wizard_finish());
    assert!(live.ctl.pad.wizard.is_some());
    assert_eq!(
        live.ctl.pad.wizard.as_ref().map(|w| w.phase()),
        Some(CalPhase::Range)
    );
}

#[test]
fn the_pad_does_not_drive_the_controller_while_calibrating() {
    let (doc, _h, mut live, hand) = rig();
    drive(&mut live);
    start_main(&mut live, &doc, &hand);
    live.ctl.pad_open_wizard();
    hand.set_axis("left_y", 1.0);
    let (_, motor) = frame(&mut live, &doc, true);
    assert_eq!(motor, MotorRun::Stopped);
    assert_eq!(position(&live, "hoist"), "off");
}

#[test]
fn a_suggested_map_has_no_errors_for_the_fixture_profile() {
    let (_doc, _h, mut live, _hand) = rig();
    let profile = live
        .ctl
        .set
        .instance(ID)
        .unwrap()
        .controller
        .profile()
        .clone();
    live.ctl.pad_suggest(&profile);
    let m = &live.ctl.pad.maps[ID];
    assert!(!m.has_errors(&profile), "{:?}", m.validate(&profile));
    assert!(m.buttons.iter().any(|b| b.action == ButtonAction::Estop));
    let _: Option<&ControlDef> = profile.controls.first();
}
