//! A controller signal bound to a machine port commands the hoist and drive runs (lane L5b):
//! press, release, E-stop and dead-man through fixed steps, and the checks, catalog and storage
//! around them. No circuit is drawn: the ports replace the drawn inputs.

use novo_controller::{Action, ProjectControllers, Severity, SignalKey, Target};
use novo_core::{AddonBlock, DeviceTag, Document, Project};
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::HoistLoop;

use super::controller_live::{generic_profile, profile_from_table};
use super::drive_run::{self, DriveRun};
use super::hoist_run::{self, HoistRun};
use super::live::LiveView;

const RADIO: &str = "generic-radio-3x2";
const DT: f64 = 0.016;

fn doc() -> Document {
    let mut doc = Document::default();
    doc.transact("Machine", |tx| {
        let m1 = tx.add_device(DeviceTag::product("M1"), "")?;
        let m2 = tx.add_device(DeviceTag::product("M2"), "")?;
        let m1 = tx.project().device(m1).map(|d| d.id).unwrap();
        let m2 = tx.project().device(m2).map(|d| d.id).unwrap();
        tx.set_property(m1, "mech_mass_kg", Some(100.0.into()))?;
        tx.set_property(m2, "trav_mass_kg", Some(2000.0.into()))?;
        Ok(())
    })
    .unwrap();
    doc
}

fn target(text: &str) -> Target {
    Target::ModulePath(text.to_owned())
}

/// A live view with a radio bound to the hoist `-M1` and the travel axis `-M2`.
fn radio(doc: &Document) -> (LiveView, String) {
    let project = doc.project();
    let mut live = LiveView::default();
    live.on = true;
    live.reset_poles(project);
    live.solved(project, 0.0);
    let id = live
        .ctl
        .add_instance(&generic_profile(RADIO).expect("shipped profile"))
        .unwrap();
    let bind = |live: &mut LiveView, signal: &str, to: &str| {
        live.ctl
            .bind(SignalKey::new(&id, signal), target(to), false);
    };
    for s in ["HOIST_UP_SLOW", "HOIST_UP_FAST"] {
        bind(&mut live, s, "hoist/-M1@up");
    }
    for s in ["HOIST_DN_SLOW", "HOIST_DN_FAST"] {
        bind(&mut live, s, "hoist/-M1@down");
    }
    bind(&mut live, "HOIST_UP_FAST", "hoist/-M1@fast");
    bind(&mut live, "HOIST_DN_FAST", "hoist/-M1@fast");
    bind(&mut live, "ESTOP_OK", "hoist/-M1@permit");
    for s in ["BRIDGE_FWD_SLOW", "BRIDGE_FWD_FAST"] {
        bind(&mut live, s, "travel/-M2@forward");
    }
    for s in ["BRIDGE_BACK_SLOW", "BRIDGE_BACK_FAST"] {
        bind(&mut live, s, "travel/-M2@reverse");
    }
    bind(&mut live, "BRIDGE_FWD_FAST", "travel/-M2@fast");
    bind(&mut live, "BRIDGE_BACK_FAST", "travel/-M2@fast");
    bind(&mut live, "ESTOP_OK", "travel/-M2@permit");
    (live, id)
}

fn act(live: &mut LiveView, id: &str, action: Action) {
    live.ctl.act(id, action);
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

fn hoist_run() -> HoistRun {
    let project_modules = |p: &Project| hoist_run::hoists_of(p).0;
    let doc = doc();
    let mut lp = HoistLoop::new(StepConfig::default()).unwrap();
    lp.hoists = project_modules(doc.project());
    HoistRun::for_test(lp)
}

fn hoist_frames(run: &mut HoistRun, live: &mut LiveView, project: &Project, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        live.solved(project, DT);
        hoist_run::advance(run, live, project, DT);
    }
}

fn drive_run(doc: &Document) -> DriveRun {
    DriveRun::for_test(drive_run::axes_of(doc.project()).0, Vec::new())
}

fn drive_frames(run: &mut DriveRun, live: &mut LiveView, project: &Project, seconds: f64) {
    for _ in 0..(seconds / DT).round() as usize {
        live.solved(project, DT);
        drive_run::advance(run, live, project, DT);
    }
}

#[test]
fn the_radio_signals_are_what_the_tests_assume() {
    let (live, id) = radio(&doc());
    let names: Vec<String> = live
        .ctl
        .set
        .instance(&id)
        .unwrap()
        .controller
        .signals()
        .iter()
        .map(|s| s.name.clone())
        .collect();
    for n in [
        "HOIST_UP_SLOW",
        "HOIST_UP_FAST",
        "HOIST_DN_SLOW",
        "HOIST_DN_FAST",
        "BRIDGE_FWD_SLOW",
        "BRIDGE_FWD_FAST",
        "BRIDGE_BACK_SLOW",
        "BRIDGE_BACK_FAST",
        "ESTOP_OK",
    ] {
        assert!(names.iter().any(|x| x == n), "{n} in {names:?}");
    }
}

#[test]
fn the_catalog_lists_the_hoist_and_the_travel_axis_with_their_ports() {
    let doc = doc();
    let (mut live, _) = radio(&doc);
    let catalog = live.ctl.catalog(doc.project()).clone();
    let hoist = catalog
        .modules
        .iter()
        .find(|m| m.path == "hoist/-M1")
        .unwrap();
    let ports: Vec<&str> = hoist.ports.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(ports, ["up", "down", "fast", "permit", "deadman"]);
    let travel = catalog
        .modules
        .iter()
        .find(|m| m.path == "travel/-M2")
        .unwrap();
    let ports: Vec<&str> = travel.ports.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(ports, ["forward", "reverse", "fast", "permit", "deadman"]);
}

#[test]
fn the_axis_the_crane_picture_uses_for_the_bridge_is_listed_as_the_bridge() {
    let mut doc = doc();
    doc.transact("Bridge axis", |tx| {
        tx.set_info(|info| {
            info.properties
                .insert("crane3d.bridge_axis".to_owned(), "-M2".into());
        })
    })
    .unwrap();
    let (mut live, _) = radio(&doc);
    let catalog = live.ctl.catalog(doc.project()).clone();
    assert!(catalog.modules.iter().any(|m| m.path == "bridge/-M2"));
}

#[test]
fn pressing_the_rocker_lifts_the_hook_and_letting_go_holds_it() {
    let doc = doc();
    let project = doc.project();
    let (mut live, id) = radio(&doc);
    let mut run = hoist_run();
    act(&mut live, &id, press("start"));
    hoist_frames(&mut run, &mut live, project, 0.5);
    assert_eq!(run.height_m(0), 0.0, "nothing is held");
    act(&mut live, &id, position("hoist", "up1"));
    hoist_frames(&mut run, &mut live, project, 3.0);
    let rising = run.height_m(0);
    assert!(rising > 0.1, "the hook rose {rising} m");
    act(&mut live, &id, position("hoist", "off"));
    hoist_frames(&mut run, &mut live, project, 3.0);
    let rest = run.height_m(0);
    hoist_frames(&mut run, &mut live, project, 2.0);
    assert_eq!(run.height_m(0), rest, "the brake holds the load");
    act(&mut live, &id, position("hoist", "down1"));
    hoist_frames(&mut run, &mut live, project, 3.0);
    assert!(run.height_m(0) < rest - 0.1, "down lowers the hook");
}

#[test]
fn step_two_is_faster_than_step_one() {
    let height_at = |p: &str| {
        let doc = doc();
        let project = doc.project();
        let (mut live, id) = radio(&doc);
        let mut run = hoist_run();
        act(&mut live, &id, press("start"));
        act(&mut live, &id, position("hoist", p));
        hoist_frames(&mut run, &mut live, project, 4.0);
        run.height_m(0)
    };
    let (one, two) = (height_at("up1"), height_at("up2"));
    assert!(two > 2.0 * one && one > 0.1, "{one} {two}");
}

#[test]
fn a_pressed_estop_stops_the_hoist_even_with_the_rocker_held_and_the_reset_releases_it() {
    let doc = doc();
    let project = doc.project();
    let (mut live, id) = radio(&doc);
    let mut run = hoist_run();
    act(&mut live, &id, press("start"));
    act(&mut live, &id, position("hoist", "up2"));
    hoist_frames(&mut run, &mut live, project, 2.0);
    let before = run.height_m(0);
    assert!(before > 0.1);
    act(&mut live, &id, press("estop"));
    hoist_frames(&mut run, &mut live, project, 3.0);
    let stopped = run.height_m(0);
    assert!(
        stopped - before < 0.5,
        "it coasts a little: {before} {stopped}"
    );
    hoist_frames(&mut run, &mut live, project, 2.0);
    assert_eq!(
        run.height_m(0),
        stopped,
        "it stays put while the E-stop is pressed"
    );
    act(
        &mut live,
        &id,
        Action::Reset {
            control: "estop".into(),
        },
    );
    act(&mut live, &id, press("start"));
    act(&mut live, &id, position("hoist", "up2"));
    hoist_frames(&mut run, &mut live, project, 2.0);
    assert!(
        run.height_m(0) > stopped + 0.1,
        "it lifts again after the reset"
    );
}

#[test]
fn a_bridge_button_drives_the_travel_axis_and_the_estop_stops_it() {
    let doc = doc();
    let project = doc.project();
    let (mut live, id) = radio(&doc);
    let mut run = drive_run(&doc);
    act(&mut live, &id, press("start"));
    act(&mut live, &id, position("bridge", "forward1"));
    drive_frames(&mut run, &mut live, project, 6.0);
    let there = run.axis_position_m(0);
    assert!(there > 0.05, "{there}");
    assert!(run.axis_speed_m_s(0) > 0.0);
    act(&mut live, &id, position("bridge", "off"));
    drive_frames(&mut run, &mut live, project, 12.0);
    let rest = run.axis_position_m(0);
    drive_frames(&mut run, &mut live, project, 2.0);
    assert!(
        (run.axis_position_m(0) - rest).abs() < 1e-3,
        "at rest after the ramp"
    );
    // reverse brings it back
    act(&mut live, &id, position("bridge", "back1"));
    drive_frames(&mut run, &mut live, project, 6.0);
    assert!(run.axis_position_m(0) < rest - 0.05);
    // E-stop while fast forward
    act(&mut live, &id, position("bridge", "forward2"));
    drive_frames(&mut run, &mut live, project, 3.0);
    act(&mut live, &id, press("estop"));
    drive_frames(&mut run, &mut live, project, 15.0);
    let stopped = run.axis_position_m(0);
    drive_frames(&mut run, &mut live, project, 2.0);
    assert!(
        (run.axis_position_m(0) - stopped).abs() < 1e-3,
        "stopped by the E-stop"
    );
}

#[test]
fn the_same_presses_give_the_same_motion_every_time() {
    let go = || {
        let doc = doc();
        let project = doc.project();
        let (mut live, id) = radio(&doc);
        let mut run = hoist_run();
        act(&mut live, &id, press("start"));
        act(&mut live, &id, position("hoist", "up2"));
        hoist_frames(&mut run, &mut live, project, 2.0);
        act(&mut live, &id, position("hoist", "down1"));
        hoist_frames(&mut run, &mut live, project, 2.0);
        run.height_m(0).to_bits()
    };
    assert_eq!(go(), go());
}

#[test]
fn the_deadman_bar_gates_every_motion() {
    // A pendant table with an enabling switch gives a dead-man control and signal.
    let table = "Terminal;Function;Step\nCOM;Common;\n1;Hoist up;1\n2;Hoist up;2\n3;Hoist down;1\n4;Hoist down;2\n5;Emergency stop;\n6;Enabling switch;\n";
    let doc = doc();
    let project = doc.project();
    let profile = profile_from_table(table, "pendant-dm", "Pendant with dead-man").unwrap();
    let mut live = LiveView::default();
    live.on = true;
    live.reset_poles(project);
    live.solved(project, 0.0);
    let id = live.ctl.add_instance(&profile).unwrap();
    let signals: Vec<String> = live
        .ctl
        .set
        .instance(&id)
        .unwrap()
        .controller
        .signals()
        .iter()
        .map(|s| s.name.clone())
        .collect();
    let dead = signals
        .iter()
        .find(|s| s.to_lowercase().contains("enab") || s.to_lowercase().contains("dead"))
        .unwrap_or_else(|| panic!("a dead-man signal in {signals:?}"))
        .clone();
    let up = signals.iter().find(|s| s.contains("UP")).unwrap().clone();
    let bind = |live: &mut LiveView, s: &str, to: &str| {
        live.ctl.bind(SignalKey::new(&id, s), target(to), false);
    };
    bind(&mut live, &up, "hoist/-M1@up");
    bind(&mut live, &dead, "hoist/-M1@deadman");
    let controls = live
        .ctl
        .set
        .instance(&id)
        .unwrap()
        .controller
        .profile()
        .controls
        .clone();
    let dm_control = controls
        .iter()
        .find(|c| c.kind == novo_controller::ControlKind::Deadman)
        .expect("a deadman control")
        .id
        .clone();
    let up_control = controls
        .iter()
        .find(|c| c.id.to_lowercase().contains("hoist"))
        .unwrap()
        .id
        .clone();
    let mut run = hoist_run();
    act(&mut live, &id, position(&up_control, "up1"));
    hoist_frames(&mut run, &mut live, project, 3.0);
    assert_eq!(run.height_m(0), 0.0, "no motion without the bar");
    act(&mut live, &id, press(&dm_control));
    act(&mut live, &id, position(&up_control, "up1"));
    hoist_frames(&mut run, &mut live, project, 3.0);
    assert!(run.height_m(0) > 0.1, "the bar held: it lifts");
    act(
        &mut live,
        &id,
        Action::Release {
            control: dm_control,
        },
    );
    hoist_frames(&mut run, &mut live, project, 3.0);
    let rest = run.height_m(0);
    hoist_frames(&mut run, &mut live, project, 2.0);
    assert_eq!(run.height_m(0), rest, "the bar let go: it holds");
}

#[test]
fn the_checks_name_an_unknown_module_port_and_a_missing_port() {
    let doc = doc();
    let project = doc.project();
    let (mut live, id) = radio(&doc);
    for (signal, to) in [
        ("MAIN", "hoist/-M9@up"),
        ("STOP", "hoist/-M1@sideways"),
        ("HOIST_SEL_MAIN", "hoist/-M1"),
    ] {
        live.ctl
            .bind(SignalKey::new(&id, signal), target(to), false);
    }
    let findings = live.ctl.findings(project);
    let text: Vec<&str> = findings.iter().map(|f| f.text.as_str()).collect();
    assert!(
        text.iter().any(|t| t.contains("no module hoist/-M9")),
        "{text:?}"
    );
    assert!(
        text.iter().any(|t| t.contains("has no port sideways")),
        "{text:?}"
    );
    assert!(
        findings
            .iter()
            .any(|f| f.severity == Severity::Error && f.text.contains("names the module")),
        "{text:?}"
    );
}

#[test]
fn a_machine_binding_survives_saving_and_reopening_the_project() {
    let mut doc = doc();
    let (live, id) = radio(&doc);
    let text = live.ctl.snapshot().unwrap();
    assert!(text.contains("hoist/-M1@up"), "{text}");
    doc.transact("Store controller", |tx| {
        tx.set_addon_block(
            "controllers",
            "project",
            AddonBlock {
                format: 1,
                text: text.clone(),
            },
        )
    })
    .unwrap();
    let mut again = LiveView::default();
    again.reset_poles(doc.project());
    again.solved(doc.project(), 0.0);
    assert_eq!(again.ctl.table.bindings, live.ctl.table.bindings);
    assert!(again.ctl.set.instance(&id).is_some());
    let back = ProjectControllers::from_text(&text).unwrap();
    assert!(
        back.bindings
            .iter()
            .any(|b| b.target == target("travel/-M2@forward"))
    );
}

#[test]
fn a_machine_port_with_no_run_does_nothing_and_unbinding_returns_the_drawn_inputs() {
    let doc = doc();
    let project = doc.project();
    let (mut live, id) = radio(&doc);
    act(&mut live, &id, press("start"));
    act(&mut live, &id, position("hoist", "up1"));
    // The commands exist as soon as the frame is solved, whether or not a run reads them.
    live.solved(project, DT);
    assert_eq!(
        live.ctl
            .machine_commands()
            .level("-M1", novo_machine::commands::Port::Up),
        Some(true)
    );
    // Unbind everything: the next frames have no commands.
    for s in ["HOIST_UP_SLOW", "HOIST_UP_FAST"] {
        live.ctl.unbind(&SignalKey::new(&id, s));
    }
    live.solved(project, DT);
    assert_eq!(
        live.ctl
            .machine_commands()
            .level("-M1", novo_machine::commands::Port::Up),
        None
    );
}

#[test]
fn the_drivers_of_a_module_are_listed_for_the_run_windows() {
    let doc = doc();
    let project = doc.project();
    let (mut live, id) = radio(&doc);
    act(&mut live, &id, press("start"));
    act(&mut live, &id, position("hoist", "up2"));
    live.solved(project, DT);
    let drivers = live.ctl.machine_drivers();
    let hoist = &drivers["-M1"];
    assert!(
        hoist
            .iter()
            .any(|(p, k, high)| p.name() == "up" && k.signal == "HOIST_UP_FAST" && *high)
    );
    let line = super::controller_machine::driver_line(hoist);
    assert!(line.contains("HOIST_UP_FAST"), "{line}");
}
