//! Fault injection and walk-throughs on the Gate 1 motor starter (lane F1): what the drawn
//! circuit does when one thing in it breaks.

use novo_core::Document;
use novo_sim::inject::Defect;
use novo_sim::walkthrough::{Action, Check, MotorIs, NetIs, Verdict, Walkthrough, run};
use novo_tests::motor_starter;

fn press(device: &str, operated: bool) -> Action {
    Action::Operate {
        device: device.into(),
        pin: None,
        operated,
    }
}

fn motor(is: MotorIs) -> Action {
    Action::Expect {
        check: Check::Motor {
            device: "-M1".into(),
            is,
        },
    }
}

fn coil(on: bool) -> Action {
    Action::Expect {
        check: Check::Coil {
            device: "-K1".into(),
            on,
        },
    }
}

fn starter() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    doc
}

#[test]
fn the_healthy_starter_starts_holds_and_stops() {
    let doc = starter();
    let w = Walkthrough::new("healthy")
        .at(0.5, motor(MotorIs::Stopped))
        .at(1.0, press("-S1", true))
        .at(1.2, press("-S1", false))
        .at(1.5, coil(true))
        .at(1.5, motor(MotorIs::Running))
        .at(2.0, press("-S0", true))
        .at(2.2, press("-S0", false))
        .at(2.5, coil(false))
        .at(2.5, motor(MotorIs::Stopped));
    let report = run(doc.project(), &w);
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
    assert!(report.ok());
    assert!(report.end_differences.is_empty());
}

#[test]
fn a_welded_main_contact_keeps_the_motor_running_after_stop() {
    let doc = starter();
    let weld = Defect::ContactWelded {
        device: "-K1".into(),
        pin: None,
        mirror: false,
    };
    let w = Walkthrough::new("weld")
        .at(0.5, press("-S1", true))
        .at(0.7, press("-S1", false))
        .at(1.0, Action::Inject { with: weld })
        .at(2.0, press("-S0", true))
        .at(2.2, press("-S0", false))
        .at(2.5, coil(false))
        .at(2.5, motor(MotorIs::Stopped));
    let report = run(doc.project(), &w);
    let last = report.steps.last().unwrap();
    assert_eq!(last.verdict, Verdict::Fail, "{report:?}");
    assert!(last.detail.contains("running"), "{}", last.detail);
    assert!(
        report
            .end_differences
            .iter()
            .any(|d| d.contains("-M1 is running")),
        "{:?}",
        report.end_differences
    );
    assert!(report.to_markdown().contains("FAIL"));
}

#[test]
fn an_open_coil_never_pulls_in() {
    let doc = starter();
    let w = Walkthrough::new("open coil")
        .at(
            0.1,
            Action::Inject {
                with: Defect::CoilOpen {
                    device: "-K1".into(),
                },
            },
        )
        .at(0.5, press("-S1", true))
        .at(1.0, coil(false))
        .at(1.0, motor(MotorIs::Stopped));
    let report = run(doc.project(), &w);
    assert!(report.ok(), "{report:?}");
    // The start button was pressed but the healthy plant would have started.
    assert!(
        report
            .steps
            .iter()
            .any(|s| s.appeared.iter().any(|a| a.contains("-K1 is off")))
    );
}

#[test]
fn a_tripped_overload_drops_the_motor_and_repair_brings_it_back() {
    let doc = starter();
    let trip = Defect::Tripped {
        device: "-F2".into(),
    };
    let w = Walkthrough::new("overload")
        .at(0.2, press("-S1", true))
        .at(0.4, press("-S1", false))
        .at(0.8, motor(MotorIs::Running))
        .at(1.0, Action::Inject { with: trip.clone() })
        .at(1.5, motor(MotorIs::Stopped))
        .at(2.0, Action::Repair { with: trip })
        .at(2.5, motor(MotorIs::Stopped))
        .at(3.0, press("-S1", true))
        .at(3.2, press("-S1", false))
        .at(3.6, motor(MotorIs::Running));
    let report = run(doc.project(), &w);
    assert!(report.ok(), "{report:#?}");
}

#[test]
fn a_lost_phase_leaves_the_motor_humming() {
    let doc = starter();
    let w = Walkthrough::new("phase")
        .at(0.2, press("-S1", true))
        .at(0.4, press("-S1", false))
        .at(
            1.0,
            Action::Inject {
                with: Defect::PhaseLost {
                    potential: "L2".into(),
                },
            },
        )
        .at(1.5, motor(MotorIs::PhaseLoss));
    let report = run(doc.project(), &w);
    assert!(report.ok(), "{report:#?}");
}

#[test]
fn a_conductor_broken_in_the_stop_circuit_is_found_by_its_effect() {
    let doc = starter();
    let w = Walkthrough::new("stop chain broken")
        .at(
            0.1,
            Action::Inject {
                with: Defect::ConductorOpen {
                    at: "-S0:12".into(),
                },
            },
        )
        .at(0.5, press("-S1", true))
        .at(0.7, press("-S1", false))
        .at(1.0, coil(false));
    let report = run(doc.project(), &w);
    assert!(report.ok(), "{report:#?}");
}

#[test]
fn an_earth_fault_on_a_live_conductor_is_a_short() {
    let doc = starter();
    let w = Walkthrough::new("earth")
        .at(
            0.1,
            Action::Inject {
                with: Defect::EarthFault { at: "-F1:2".into() },
            },
        )
        .at(
            0.5,
            Action::Expect {
                check: Check::Net {
                    at: "-F1:2".into(),
                    is: NetIs::Shorted,
                },
            },
        );
    let report = run(doc.project(), &w);
    assert!(report.ok(), "{report:#?}");
    assert!(
        report.steps[0]
            .appeared
            .iter()
            .any(|a| a.contains("short between")),
        "{:?}",
        report.steps[0]
    );
}

#[test]
fn a_defect_that_names_nothing_is_reported_not_ignored() {
    let doc = starter();
    let w = Walkthrough::new("typo").at(
        0.1,
        Action::Inject {
            with: Defect::CoilOpen {
                device: "-K99".into(),
            },
        },
    );
    let report = run(doc.project(), &w);
    assert!(!report.ok());
    assert!(report.problems[0].contains("-K99"), "{:?}", report.problems);
}

#[test]
fn a_walkthrough_round_trips_through_json() {
    let w = Walkthrough::new("round trip")
        .noted(1.0, press("-S1", true), "start")
        .at(
            2.0,
            Action::Inject {
                with: Defect::ShortTo {
                    at: "-K1:A1".into(),
                    potential: "N".into(),
                },
            },
        )
        .at(3.0, motor(MotorIs::Forward));
    let back = Walkthrough::from_json(&w.to_json()).unwrap();
    assert_eq!(back, w);
    assert!(Walkthrough::from_json("{").is_err());
    let newer = w.to_json().replace("\"format\": 1", "\"format\": 99");
    assert!(Walkthrough::from_json(&newer).is_err());
}

#[test]
fn the_project_is_not_changed_by_a_run() {
    let doc = starter();
    let rev = doc.project().revision();
    let w = Walkthrough::new("cut").at(
        0.1,
        Action::Inject {
            with: Defect::ConductorOpen {
                at: "-S0:12".into(),
            },
        },
    );
    run(doc.project(), &w);
    assert_eq!(doc.project().revision(), rev);
}

fn receiver(pin: &str, operated: bool) -> Action {
    Action::Operate {
        device: "-A1".into(),
        pin: Some(pin.into()),
        operated,
    }
}

#[test]
fn a_welded_up_contactor_and_a_down_command_short_the_reversing_bus() {
    use novo_tests::radio_hoist;
    let mut doc = Document::default();
    let h = radio_hoist(&mut doc).expect("fixture builds");
    let _ = h;
    // The receiver's main and E-stop chain contacts are closed by the radio; output 1 is "up
    // slow", output 3 "down slow".
    let healthy = Walkthrough::new("up then down")
        .at(0.1, receiver("M1", true))
        .at(0.1, receiver("E1", true))
        .at(0.5, receiver("1", true))
        .at(1.0, coil_of("-K1", true))
        .at(1.0, coil_of("-K2", false))
        .at(1.5, receiver("3", true))
        .at(2.0, coil_of("-K2", false));
    let report = run(doc.project(), &healthy);
    assert!(report.ok(), "{report:#?}");

    // The up contactor's make contacts weld. A plain auxiliary block does not notice: its break
    // contact closes as drawn, so the down command pulls -K2 in onto the welded phase sequence.
    let weld = |mirror| Defect::ContactWelded {
        device: "-K1".into(),
        pin: None,
        mirror,
    };
    let script = |mirror| {
        Walkthrough::new("welded up")
            .at(0.1, receiver("M1", true))
            .at(0.1, receiver("E1", true))
            .at(0.5, Action::Inject { with: weld(mirror) })
            .at(1.0, motor(MotorIs::Forward))
            .at(1.5, receiver("3", true))
            .at(2.0, coil_of("-K2", false))
    };
    let plain = run(doc.project(), &script(false));
    assert!(!plain.ok(), "-K2 must pull in: {plain:#?}");
    assert!(
        plain
            .end_differences
            .iter()
            .any(|d| d.contains("short between")),
        "{:?}",
        plain.end_differences
    );
    // With positively guided break contacts the welded armature keeps the interlock open.
    let guided = run(doc.project(), &script(true));
    assert!(guided.ok(), "{guided:#?}");
}

fn coil_of(device: &str, on: bool) -> Action {
    Action::Expect {
        check: Check::Coil {
            device: device.into(),
            on,
        },
    }
}

#[test]
fn breaking_a_conductor_while_running_drops_the_motor() {
    let doc = starter();
    let w = Walkthrough::new("break while running")
        .at(0.2, press("-S1", true))
        .at(0.4, press("-S1", false))
        .at(0.8, motor(MotorIs::Running))
        .at(
            1.0,
            Action::Inject {
                with: Defect::ConductorOpen {
                    at: "-S0:12".into(),
                },
            },
        )
        .at(1.5, motor(MotorIs::Stopped))
        .at(1.5, coil(false));
    let report = run(doc.project(), &w);
    assert!(report.ok(), "{report:#?}");
}

#[test]
fn naming_a_pin_that_is_not_a_contact_is_reported() {
    use novo_tests::radio_hoist;
    let mut doc = Document::default();
    radio_hoist(&mut doc).expect("fixture builds");
    // Find the break-contact pin of -K2 that guards -K1 would need drawing knowledge; here the
    // check is only that naming a pin that is not a switched contact says so.
    let w = Walkthrough::new("bad pin").at(
        0.1,
        Action::Inject {
            with: Defect::ContactWelded {
                device: "-K2".into(),
                pin: Some("A1".into()),
                mirror: false,
            },
        },
    );
    let report = run(doc.project(), &w);
    assert!(!report.problems.is_empty());
}
