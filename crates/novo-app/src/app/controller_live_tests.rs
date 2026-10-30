//! Tests for the controller in the live view (lane C2): the radio-hoist fixture of `novo-tests`
//! settled with the controller driving the receiver, headless and through `LiveView::solved`,
//! the same path the live view draws from.

use std::collections::BTreeSet;

use novo_controller::{Action, Severity, SignalKey, Target};
use novo_core::{Document, SymbolId};
use novo_sim::{Direction, MotorRun, device_states};
use novo_tests::{RadioHoist, radio_hoist, radio_hoist_with_controller};

use super::controller_live::{Resolved, resolve_target};
use super::live::LiveView;

const ID: &str = "radio-1";

fn fixture() -> (Document, RadioHoist, LiveView) {
    let mut doc = Document::default();
    let (h, _) = radio_hoist_with_controller(&mut doc).expect("fixture builds");
    // The first frame reads the controllers from the project; acting before it would find none.
    let mut live = LiveView::default();
    live.reset_poles(doc.project());
    live.solved(doc.project(), 0.0);
    (doc, h, live)
}

fn act(live: &mut LiveView, action: Action) {
    live.ctl.act(ID, action);
}

fn press(c: &str) -> Action {
    Action::Press { control: c.into() }
}

fn position(c: &str, p: &str) -> Action {
    Action::SetPosition {
        control: c.into(),
        position: p.into(),
    }
}

/// Settles the circuit with a few frames (coils that were on hold themselves) and returns the
/// energized coils and the motor's run state.
fn settle(live: &mut LiveView, doc: &Document, h: &RadioHoist) -> (BTreeSet<SymbolId>, MotorRun) {
    let mut energized = BTreeSet::new();
    let mut motor = MotorRun::Stopped;
    for _ in 0..4 {
        let solved = live.solved(doc.project(), 0.05);
        energized = solved.settled.energized.clone();
        let states = device_states(doc.project(), &solved.netlist, &solved.settled);
        motor = states.motors[&h.motor];
    }
    (energized, motor)
}

fn lamp(live: &mut LiveView, doc: &Document, lamp: SymbolId) -> bool {
    let solved = live.solved(doc.project(), 0.05);
    device_states(doc.project(), &solved.netlist, &solved.settled).lamps[&lamp]
}

fn coils(h: &RadioHoist, which: [bool; 4]) -> BTreeSet<SymbolId> {
    [h.k1_coil, h.k2_coil, h.k3_coil, h.k4_coil]
        .into_iter()
        .zip(which)
        .filter_map(|(c, on)| on.then_some(c))
        .collect()
}

#[test]
fn nothing_runs_until_the_main_contact_closes() {
    let (doc, h, mut live) = fixture();
    let (on, motor) = settle(&mut live, &doc, &h);
    assert!(on.is_empty());
    assert_eq!(motor, MotorRun::Stopped);
    assert!(!lamp(&mut live, &doc, h.main_lamp));
    // A rocker moved before the main is on is refused by the receiver latch or has no effect.
    act(&mut live, position("hoist", "up1"));
    let (on, _) = settle(&mut live, &doc, &h);
    assert!(on.is_empty(), "no main, no output: {on:?}");
}

#[test]
fn hoist_up_step_one_runs_the_slow_winding() {
    let (doc, h, mut live) = fixture();
    act(&mut live, press("start"));
    assert!(settle(&mut live, &doc, &h).0.is_empty());
    assert!(lamp(&mut live, &doc, h.main_lamp));
    act(&mut live, position("hoist", "up1"));
    let (on, motor) = settle(&mut live, &doc, &h);
    assert_eq!(on, coils(&h, [true, false, true, false]));
    assert!(
        matches!(motor, MotorRun::Running { speed: 0, .. }),
        "{motor:?}"
    );
    assert!(lamp(&mut live, &doc, h.brake_lamp));
}

#[test]
fn hoist_up_step_two_swaps_slow_for_fast() {
    let (doc, h, mut live) = fixture();
    act(&mut live, press("start"));
    act(&mut live, position("hoist", "up1"));
    settle(&mut live, &doc, &h);
    act(&mut live, position("hoist", "up2"));
    let (on, motor) = settle(&mut live, &doc, &h);
    assert_eq!(on, coils(&h, [true, false, false, true]));
    assert!(
        matches!(motor, MotorRun::Running { speed: 1, .. }),
        "{motor:?}"
    );
}

#[test]
fn hoist_down_turns_the_motor_the_other_way() {
    let (doc, h, mut live) = fixture();
    act(&mut live, press("start"));
    act(&mut live, position("hoist", "up1"));
    let up = settle(&mut live, &doc, &h).1;
    act(&mut live, position("hoist", "off"));
    assert_eq!(settle(&mut live, &doc, &h).1, MotorRun::Stopped);
    act(&mut live, position("hoist", "down1"));
    let (on, down) = settle(&mut live, &doc, &h);
    assert_eq!(on, coils(&h, [false, true, true, false]));
    let (MotorRun::Running { direction: a, .. }, MotorRun::Running { direction: b, .. }) =
        (up, down)
    else {
        panic!("both run: {up:?} {down:?}");
    };
    assert_ne!(a, b);
    assert!(matches!(a, Direction::Forward | Direction::Reverse));
}

#[test]
fn the_emergency_stop_drops_every_output() {
    let (doc, h, mut live) = fixture();
    act(&mut live, press("start"));
    act(&mut live, position("hoist", "up2"));
    assert!(!settle(&mut live, &doc, &h).0.is_empty());
    act(&mut live, press("estop"));
    let (on, motor) = settle(&mut live, &doc, &h);
    assert!(on.is_empty(), "{on:?}");
    assert_eq!(motor, MotorRun::Stopped);
    assert!(!lamp(&mut live, &doc, h.main_lamp));
    // The chain stays open until its reset, even with the rocker still held.
    act(&mut live, position("hoist", "up1"));
    assert!(settle(&mut live, &doc, &h).0.is_empty());
}

#[test]
fn releasing_a_binding_lets_the_contact_go_back_to_rest() {
    let (doc, h, mut live) = fixture();
    act(&mut live, press("start"));
    act(&mut live, position("hoist", "up1"));
    assert!(!settle(&mut live, &doc, &h).0.is_empty());
    live.ctl.unbind(&SignalKey::new(ID, "HOIST_UP_SLOW"));
    let (on, _) = settle(&mut live, &doc, &h);
    assert!(on.is_empty(), "the unbound output is open again: {on:?}");
}

#[test]
fn the_controller_is_read_from_the_project_and_saved_back_unchanged() {
    let (doc, _h, mut live) = fixture();
    assert_eq!(live.ctl.set.instances().len(), 1);
    assert_eq!(live.ctl.table.bindings.len(), 6);
    // Controls are stored at rest, so a project saved with the E-stop pressed reopens clean.
    act(&mut live, press("estop"));
    let text = live.ctl.snapshot().expect("serialises");
    let mut other = LiveView::default();
    other.ctl.set = novo_controller::ControllerSet::new();
    let stored = super::controller_live::stored(doc.project()).to_owned();
    assert!(!stored.is_empty(), "the fixture stores its controllers");
    assert_eq!(text, stored);
}

#[test]
fn an_unknown_pin_is_reported_and_drives_nothing() {
    let (doc, _h, mut live) = fixture();
    live.ctl.bind(
        SignalKey::new(ID, "BRIDGE_FWD_SLOW"),
        Target::Pin("-A1:99".into()),
        false,
    );
    let findings = live.ctl.findings(doc.project());
    assert!(
        findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.text.contains("-A1:99")),
        "{findings:?}"
    );
    assert!(matches!(
        resolve_target(doc.project(), &Target::Pin("-A1:99".into())),
        Resolved::Broken(_)
    ));
}

#[test]
fn auto_bind_finds_the_receiver_outputs_by_pin_name() {
    let mut doc = Document::default();
    radio_hoist(&mut doc).expect("fixture builds");
    let mut live = LiveView::default();
    let profile = live
        .ctl
        .profiles()
        .iter()
        .find(|e| e.profile.controller.id == "generic-radio-3x2")
        .map(|e| e.profile.clone())
        .expect("shipped profile");
    let id = live.ctl.add_instance(&profile).expect("added");
    let (bound, missed) = live.ctl.auto_bind(doc.project(), &id, "-A1");
    assert!(bound >= 4, "bound {bound}, missed {missed:?}");
    assert!(missed.iter().any(|p| p == "7"), "{missed:?}");
    // A second run binds nothing new.
    assert_eq!(live.ctl.auto_bind(doc.project(), &id, "-A1").0, 0);
}

#[test]
fn unused_signals_are_one_line_per_controller() {
    let (doc, _h, mut live) = fixture();
    let findings = live.ctl.findings(doc.project());
    let idle: Vec<_> = findings
        .iter()
        .filter(|f| f.text.contains("drive nothing"))
        .collect();
    // The fixture binds six of the fifteen signals; the other nine are reported once.
    assert_eq!(idle.len(), 1, "{findings:?}");
    assert!(idle[0].text.contains("9 signals"), "{}", idle[0].text);
    assert!(
        findings.iter().all(|f| f.severity != Severity::Error),
        "{findings:?}"
    );
}

#[test]
fn every_kind_of_target_resolves_to_what_it_drives() {
    let (doc, _h, _live) = fixture();
    let p = doc.project();
    assert!(matches!(
        resolve_target(p, &Target::Pin("-A1:2".into())),
        Resolved::Pole(_)
    ));
    // The common is shared by every output: bind an output pin instead.
    assert!(matches!(
        resolve_target(p, &Target::Pin("-A1:COM".into())),
        Resolved::Broken(why) if why.contains("common")
    ));
    assert!(matches!(
        resolve_target(p, &Target::Symbol("-A1".into())),
        Resolved::Hand(list) if list.len() >= 8
    ));
    assert!(matches!(
        resolve_target(p, &Target::PlcAddress("%IX0.3".into())),
        Resolved::Plc(_)
    ));
    assert!(matches!(
        resolve_target(p, &Target::PlcAddress("%QX0.3".into())),
        Resolved::Broken(_)
    ));
    // The fixture has no motor with mechanical data, so the machine model has no such module.
    assert!(matches!(
        resolve_target(p, &Target::ModulePath("crane/hoist@up".into())),
        Resolved::Broken(why) if why.contains("no module")
    ));
}

#[test]
fn the_card_draws_for_the_fixture_in_the_live_view() {
    use super::gate1_tests::Drive;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("radio-hoist.novo");
    let mut doc = Document::default();
    radio_hoist_with_controller(&mut doc).expect("fixture builds");
    novo_storage::ProjectFile::create(&file, doc.project()).unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.open_path(&file);
    d.frame();
    d.app.wb.live.on = true;
    d.app.wb.live.ctl.open = true;
    for _ in 0..3 {
        d.frame();
    }
    assert!(d.app.wb.live.ctl.set.instances().len() == 1);
    // Working the controls is not editing the drawing: no new undo step, no new revision.
    let revision = d.doc().project().revision();
    d.app.wb.live.ctl.act(ID, press("start"));
    d.app.wb.live.ctl.act(ID, press("estop"));
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(
        d.doc().project().revision(),
        revision,
        "pressing controls edits nothing"
    );
}

fn open_fixture() -> (tempfile::TempDir, super::gate1_tests::Drive) {
    use super::gate1_tests::Drive;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("radio-hoist.novo");
    let mut doc = Document::default();
    radio_hoist_with_controller(&mut doc).expect("fixture builds");
    novo_storage::ProjectFile::create(&file, doc.project()).unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.open_path(&file);
    d.frame();
    d.app.wb.live.on = true;
    d.app.wb.live.ctl.open = true;
    for _ in 0..3 {
        d.frame();
    }
    (dir, d)
}

#[test]
fn several_instances_keep_name_colour_and_group_in_the_project() {
    let (_dir, mut d) = open_fixture();
    let ctl = &mut d.app.wb.live.ctl;
    let second = ctl.duplicate_instance(ID).unwrap();
    assert_eq!(second, "radio-1-2");
    assert_eq!(ctl.selected.as_deref(), Some("radio-1-2"));
    assert!(
        ctl.table.bindings.iter().all(|b| b.key.controller == ID),
        "a duplicate starts with no bindings"
    );
    ctl.rename_instance(&second, "  Crane 2 radio ");
    ctl.set_colour(&second, Some("#e06cb4"));
    ctl.set_target_group(&second, "runway1/bridge2");
    assert_eq!(ctl.colour_of(&second), [0xe0, 0x6c, 0xb4]);
    assert_ne!(ctl.colour_of(ID), ctl.colour_of(&second));
    for _ in 0..3 {
        d.frame();
    }
    // The card wrote the structure into the project; a view that starts empty reads it back.
    let mut fresh = LiveView::default();
    fresh.ctl.sync(d.doc().project());
    let inst = fresh.ctl.set.instance(&second).expect("restored");
    assert_eq!(inst.label, "Crane 2 radio");
    assert_eq!(inst.colour.as_deref(), Some("#e06cb4"));
    assert_eq!(inst.target_group.as_deref(), Some("runway1/bridge2"));
    assert_eq!(fresh.ctl.set.instances().len(), 2);
}

#[test]
fn two_popped_panels_work_independently_and_dock_back() {
    let (_dir, mut d) = open_fixture();
    let second = d.app.wb.live.ctl.duplicate_instance(ID).unwrap();
    d.app.wb.live.ctl.set_popped(ID, true);
    d.app.wb.live.ctl.set_popped(&second, true);
    d.app.wb.live.ctl.open = false; // the panels draw without the card
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(d.app.wb.live.ctl.popped.len(), 2);
    let ctl = &mut d.app.wb.live.ctl;
    ctl.act(ID, press("start"));
    assert!(ctl.set.is_active(ID));
    assert!(!ctl.set.is_active(&second), "the other panel is untouched");
    ctl.act(&second, press("estop"));
    assert!(ctl.set.is_active(&second));
    ctl.set_popped(&second, false);
    assert_eq!(ctl.popped.len(), 1);
    // Removing an instance takes its panel with it.
    ctl.remove_instance(ID);
    assert!(ctl.popped.is_empty());
    for _ in 0..3 {
        d.frame();
    }
}

#[test]
fn two_instances_on_one_pin_are_a_conflict_the_panel_can_name() {
    let (_dir, mut d) = open_fixture();
    let second = d.app.wb.live.ctl.duplicate_instance(ID).unwrap();
    let ctl = &mut d.app.wb.live.ctl;
    let (pin, key) = {
        let b = &ctl.table.bindings[0];
        (b.target.clone(), b.key.signal.clone())
    };
    ctl.bind(SignalKey::new(&second, &key), pin.clone(), false);
    let shared = ctl.shared_targets(&second);
    assert_eq!(shared.len(), 1);
    assert_eq!(shared[0].target, pin);
    assert!(shared[0].conflict, "the generic radio is not marked shared");
    assert_eq!(ctl.shared_targets(ID).len(), 1);
    let project = d.doc().project().clone();
    let findings = d.app.wb.live.ctl.findings(&project);
    assert!(
        findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.text.contains("not all are marked shared")),
        "{findings:?}"
    );
}

#[test]
fn colour_text_reads_back_and_bad_text_falls_back_to_the_presets() {
    use super::controller_live::parse_hex;
    assert_eq!(parse_hex("#4c9aff"), Some([0x4c, 0x9a, 0xff]));
    assert_eq!(parse_hex("4c9aff"), None);
    assert_eq!(parse_hex("#4c9af"), None);
    assert_eq!(parse_hex("#zzzzzz"), None);
    assert_eq!(parse_hex("#ééé"), None);
}

#[test]
fn a_long_instance_name_is_cut_for_its_tab_only() {
    use super::controller_card::tab_text;
    assert_eq!(tab_text("Crane 1"), "Crane 1");
    let cut = tab_text("Radio, 3 motions, 2-step copy");
    assert!(cut.ends_with('…') && cut.chars().count() <= 16, "{cut}");
}
