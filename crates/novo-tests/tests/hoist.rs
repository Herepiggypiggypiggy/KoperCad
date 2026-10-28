//! The two-speed hoist fixture settles the way a hoist pendant should (lane L5a): step 1 runs
//! the fine winding, step 2 the coarse one, down reverses, and the upper limit switch cuts the
//! coarse up circuit only.

use std::collections::BTreeSet;

use novo_core::{Document, Project};
use novo_sim::{Direction, MotorRun, PoleState, device_states, rest_state, settle};
use novo_tests::hoist::{Hoist, hoist};

fn press(project: &Project, poles: &mut PoleState, tag: &str, closed: bool) {
    for symbol in project.symbols() {
        let hit = symbol
            .device()
            .and_then(|d| project.device(d))
            .is_some_and(|d| d.tag.to_string().ends_with(tag));
        if hit {
            poles.set_all(project, symbol.id, closed);
        }
    }
}

fn motor_after(doc: &Document, h: &Hoist, pressed: &[&str], limit_open: bool) -> MotorRun {
    let project = doc.project();
    let netlist = project.netlist();
    let mut poles = rest_state(project);
    for tag in pressed {
        press(project, &mut poles, tag, true);
    }
    if limit_open {
        press(project, &mut poles, "S9", false);
    }
    let none = BTreeSet::new();
    // Settle twice so the interlock contacts see the coils of the first pass.
    let first = settle(project, &netlist, &poles, &none, &none);
    let second = settle(project, &netlist, &poles, &none, &first.energized);
    device_states(project, &netlist, &second).motors[&h.motor_sym]
}

fn fixture() -> (Document, Hoist) {
    let mut doc = Document::default();
    let h = hoist(&mut doc).expect("fixture builds");
    (doc, h)
}

#[test]
fn the_pendant_picks_the_winding_and_the_direction() {
    let (doc, h) = fixture();
    assert_eq!(motor_after(&doc, &h, &[], false), MotorRun::Stopped);
    assert_eq!(
        motor_after(&doc, &h, &["S1"], false),
        MotorRun::Running {
            direction: Direction::Forward,
            speed: 0
        }
    );
    assert_eq!(
        motor_after(&doc, &h, &["S2"], false),
        MotorRun::Running {
            direction: Direction::Forward,
            speed: 1
        }
    );
    assert_eq!(
        motor_after(&doc, &h, &["S3"], false),
        MotorRun::Running {
            direction: Direction::Reverse,
            speed: 0
        }
    );
}

#[test]
fn the_upper_limit_cuts_coarse_up_only() {
    let (doc, h) = fixture();
    assert_eq!(motor_after(&doc, &h, &["S2"], true), MotorRun::Stopped);
    assert!(matches!(
        motor_after(&doc, &h, &["S1"], true),
        MotorRun::Running { speed: 0, .. }
    ));
    assert!(matches!(
        motor_after(&doc, &h, &["S3"], true),
        MotorRun::Running {
            direction: Direction::Reverse,
            ..
        }
    ));
}

#[test]
fn the_motor_carries_its_mechanical_load() {
    let (doc, h) = fixture();
    let d = doc.project().device(h.motor).unwrap();
    assert!(d.properties.contains_key("mech_mass_kg"));
}
