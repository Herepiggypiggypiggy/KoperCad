//! The crane simulation example runs: a radio presses the receiver contacts, the drawn relays
//! answer, and the hoist, the bridge and the crab move through Simulate > Hoist run and
//! Simulate > Drive run.

use novo_controller::{Action, Target};
use novo_core::Document;
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::HoistLoop;

use super::controller_live::{Resolved, resolve_target};
use super::drive_run::{self, DriveRun};
use super::example_crane_sim::{RADIO, build_crane_sim_example};
use super::hoist_run::{self, HoistRun};
use super::live::LiveView;

struct Rig {
    doc: Document,
    live: LiveView,
    hoist: HoistRun,
    drive: DriveRun,
}

fn rig() -> Rig {
    let mut doc = Document::default();
    build_crane_sim_example(&mut doc).expect("the example builds");
    let project = doc.project();
    let (hoists, notes) = hoist_run::hoists_of(project);
    assert!(notes.is_empty(), "{notes:?}");
    assert_eq!(hoists.len(), 1, "one hoist");
    let (axes, notes) = drive_run::axes_of(project);
    assert!(
        notes.iter().all(|(l, _)| matches!(l, super::Level::Info)),
        "{notes:?}"
    );
    assert_eq!(axes.len(), 2, "bridge and crab");
    let mut lp = HoistLoop::new(StepConfig::default()).unwrap();
    lp.hoists = hoists;
    let mut live = LiveView::default();
    live.on = true;
    live.reset_poles(project);
    // The first frame reads the controllers from the project.
    live.solved(project, 0.0);
    Rig {
        hoist: HoistRun::for_test(lp),
        drive: DriveRun::for_test(axes, Vec::new()),
        live,
        doc,
    }
}

impl Rig {
    fn act(&mut self, action: Action) {
        self.live.ctl.act(RADIO, action);
    }

    fn set(&mut self, control: &str, position: &str) {
        self.act(Action::SetPosition {
            control: control.into(),
            position: position.into(),
        });
    }

    fn run(&mut self, seconds: f64) {
        let project = self.doc.project();
        for _ in 0..(seconds / 0.016) as usize {
            hoist_run::advance(&mut self.hoist, &mut self.live, project, 0.016);
            drive_run::advance(&mut self.drive, &mut self.live, project, 0.016);
        }
    }

    fn start(&mut self) {
        self.act(Action::Press {
            control: "start".into(),
        });
        self.run(0.5);
    }
}

#[test]
fn every_radio_signal_reaches_a_receiver_contact_of_the_drawing() {
    use novo_controller::SignalKey;
    let mut r = rig();
    r.live.solved(r.doc.project(), 0.0);
    let signals = [
        "ESTOP_OK",
        "MAIN",
        "HOIST_UP_SLOW",
        "HOIST_UP_FAST",
        "HOIST_DN_SLOW",
        "HOIST_DN_FAST",
        "BRIDGE_FWD_SLOW",
        "BRIDGE_FWD_FAST",
        "BRIDGE_BACK_SLOW",
        "BRIDGE_BACK_FAST",
        "TROLLEY_RIGHT_SLOW",
        "TROLLEY_RIGHT_FAST",
        "TROLLEY_LEFT_SLOW",
        "TROLLEY_LEFT_FAST",
    ];
    for signal in signals {
        let targets: Vec<Target> = r
            .live
            .ctl
            .table
            .targets_of(&SignalKey::new(RADIO, signal))
            .cloned()
            .collect();
        assert_eq!(targets.len(), 1, "{signal}: {targets:?}");
        let resolved = resolve_target(r.doc.project(), &targets[0]);
        assert!(
            matches!(resolved, Resolved::Pole(_)),
            "{signal} -> {}: {resolved:?}",
            targets[0]
        );
    }
}

#[test]
fn nothing_moves_before_the_radio_is_started() {
    let mut r = rig();
    r.set("hoist", "up2");
    r.set("bridge", "forward2");
    r.run(5.0);
    assert!(r.hoist.height_m(0).abs() < 0.01, "{}", r.hoist.height_m(0));
    assert!(r.drive.axis_position_m(0).abs() < 0.01);
}

#[test]
fn the_radio_lifts_the_hook_through_the_drawn_contactors() {
    let mut r = rig();
    r.start();
    r.set("hoist", "up2");
    r.run(6.0);
    let up = r.hoist.height_m(0);
    assert!(up > 0.5, "the hook rose {up} m");
    r.set("hoist", "off");
    r.run(3.0);
    let rest = r.hoist.height_m(0);
    r.run(3.0);
    assert!((r.hoist.height_m(0) - rest).abs() < 0.02, "the brake holds");
}

#[test]
fn the_upper_limit_drops_the_fast_step_and_the_slow_step_goes_on() {
    let mut r = rig();
    r.start();
    r.set("hoist", "up2");
    r.run(40.0);
    let at_limit = r.hoist.height_m(0);
    assert!(at_limit > 4.4, "the hook reached the limit: {at_limit}");
    // Past the limit the fast step is cut, so the hook only creeps on (fine step, 5 m/min or less).
    r.run(10.0);
    let later = r.hoist.height_m(0);
    assert!(later - at_limit < 1.0, "{at_limit} -> {later}");
}

#[test]
fn the_radio_drives_the_bridge_and_the_crab_forward_and_back() {
    let mut r = rig();
    r.start();
    r.set("bridge", "forward1");
    r.set("trolley", "right1");
    r.run(20.0);
    let (b, c) = (r.drive.axis_position_m(0), r.drive.axis_position_m(1));
    assert!(b > 0.3 && c > 0.3, "bridge {b} m, crab {c} m");
    r.set("bridge", "back1");
    r.set("trolley", "left1");
    r.run(40.0);
    assert!(r.drive.axis_position_m(0) < b, "the bridge came back");
    assert!(r.drive.axis_position_m(1) < c, "the crab came back");
}

#[test]
fn the_emergency_stop_of_the_radio_stops_everything() {
    let mut r = rig();
    r.start();
    r.set("hoist", "up2");
    r.set("bridge", "forward2");
    r.run(4.0);
    r.act(Action::Press {
        control: "estop".into(),
    });
    r.run(15.0);
    let (h, b) = (r.hoist.height_m(0), r.drive.axis_position_m(0));
    r.run(5.0);
    assert!((r.hoist.height_m(0) - h).abs() < 0.02);
    assert!((r.drive.axis_position_m(0) - b).abs() < 0.02);
}

#[test]
fn the_3d_picture_is_set_to_the_bridge_and_the_crab_with_their_end_limits() {
    let r = rig();
    let project = r.doc.project();
    let setup = super::crane_setup::CraneSetup::from_properties(&project.info().properties);
    assert_eq!(setup.bridge_axis.as_deref(), Some("=CRN+FLD-M21"));
    assert_eq!(setup.trolley_axis.as_deref(), Some("=CRN+FLD-M22"));
    let axes = drive_run::travel_ids(project);
    assert!(axes.contains(&"=CRN+FLD-M21".to_owned()), "{axes:?}");
    assert!(axes.contains(&"=CRN+FLD-M22".to_owned()), "{axes:?}");
    for axis in ["=CRN+FLD-M21", "=CRN+FLD-M22"] {
        assert_eq!(
            drive_run::travel_limits_of(project, axis).len(),
            2,
            "{axis}"
        );
    }
}

#[test]
fn the_example_still_carries_the_plain_crane_pages_it_is_drawn_from() {
    let mut doc = Document::default();
    build_crane_sim_example(&mut doc).unwrap();
    let pages = doc
        .project()
        .pages()
        .filter(|p| !matches!(p.kind, novo_core::PageKind::Generated { .. }))
        .count();
    assert_eq!(pages, 6);
}

#[test]
fn the_hook_stays_where_it_stopped_while_the_bridge_travels() {
    let mut r = rig();
    r.start();
    r.set("hoist", "up2");
    r.run(2.0);
    r.set("hoist", "off");
    r.run(4.0);
    let at_rest = r.hoist.height_m(0);
    r.set("bridge", "forward2");
    r.run(20.0);
    assert!(r.drive.axis_position_m(0) > 1.0);
    assert!(
        (r.hoist.height_m(0) - at_rest).abs() < 0.02,
        "{at_rest} -> {}",
        r.hoist.height_m(0)
    );
}
