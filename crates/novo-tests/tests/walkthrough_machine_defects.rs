//! Machine-level defects in walk-throughs (lane F1b): a worn brake, an open phase and a sagging
//! supply on the two-speed hoist fixture, checked against where the hook goes.

use novo_core::{Document, Project};
use novo_machine::cosim::StepConfig;
use novo_machine::hoist_loop::{HoistLoop, HoistModule};
use novo_machine::spec;
use novo_sim::machine::{ConditionName, HoistDefect, HookIs, PhaseName, RectifierDefect};
use novo_sim::walkthrough::{Action, Check, Verdict, Walkthrough, run, run_with};
use novo_tests::hoist::hoist;

fn modules(project: &Project) -> Vec<HoistModule> {
    let mut out = Vec::new();
    for device in project.devices() {
        if !device.properties.contains_key(spec::HOIST_KEY) {
            continue;
        }
        let tag = device.tag.to_string();
        let get = |key: &str| device.properties.get(key).map(|v| v.to_string());
        if let Some(m) = spec::hoist_from_properties(&tag, &tag, &get).expect("hoist data") {
            out.push(m);
        }
    }
    out
}

fn fixture() -> Document {
    let mut doc = Document::default();
    hoist(&mut doc).expect("fixture builds");
    doc
}

fn make(project: &Project) -> impl Fn() -> HoistLoop + '_ {
    move || {
        let mut lp = HoistLoop::new(StepConfig::default()).unwrap();
        lp.hoists = modules(project);
        lp
    }
}

fn id(project: &Project) -> String {
    modules(project)[0].id.clone()
}

fn press(device: &str, operated: bool) -> Action {
    Action::Operate {
        device: device.into(),
        pin: None,
        operated,
    }
}

fn hook(h: &str, is: HookIs) -> Action {
    Action::Expect {
        check: Check::Hook {
            hoist: h.into(),
            is,
        },
    }
}

fn machine(h: &str, with: HoistDefect) -> Action {
    Action::Machine {
        hoist: h.into(),
        with,
    }
}

/// Lift for three seconds, let go, wait.
fn lift(w: Walkthrough) -> Walkthrough {
    w.at(1.0, press("-S1", true)).at(4.0, press("-S1", false))
}

#[test]
fn a_healthy_hoist_lifts_and_holds_the_load() {
    let doc = fixture();
    let p = doc.project();
    let h = id(p);
    let w = lift(Walkthrough::new("healthy"))
        .at(3.9, hook(&h, HookIs::Moving))
        .at(3.9, hook(&h, HookIs::Above { m: 0.2 }))
        .at(8.0, hook(&h, HookIs::Standing))
        .at(8.0, hook(&h, HookIs::Clear));
    let report = run_with(p, &w, Some(&make(p)));
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
    assert!(
        report.end_differences.is_empty(),
        "{:?}",
        report.end_differences
    );
}

#[test]
fn a_worn_brake_lets_the_load_sink_and_the_hoist_says_so() {
    let doc = fixture();
    let p = doc.project();
    let h = id(p);
    let w = lift(Walkthrough::new("worn brake"))
        .at(0.5, machine(&h, HoistDefect::BrakeLining { share: 0.05 }))
        .at(12.0, hook(&h, HookIs::Moving))
        .at(
            12.0,
            hook(
                &h,
                HookIs::Reporting {
                    condition: ConditionName::BrakeSlipping,
                },
            ),
        )
        .at(12.0, hook(&h, HookIs::Below { m: 0.0 }));
    let report = run_with(p, &w, Some(&make(p)));
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
    assert!(
        report.end_differences.iter().any(|d| d.contains("lower")),
        "{:?}",
        report.end_differences
    );
}

#[test]
fn a_repaired_brake_holds_again() {
    let doc = fixture();
    let p = doc.project();
    let h = id(p);
    let w = lift(Walkthrough::new("repair"))
        .at(0.5, machine(&h, HoistDefect::BrakeLining { share: 0.05 }))
        .at(6.0, Action::MachineRepair { hoist: h.clone() })
        .at(20.0, hook(&h, HookIs::Standing));
    let report = run_with(p, &w, Some(&make(p)));
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
}

#[test]
fn an_open_brake_coil_and_a_lost_phase_are_reported() {
    let doc = fixture();
    let p = doc.project();
    let h = id(p);
    let w = Walkthrough::new("faults")
        .at(
            0.1,
            machine(&h, HoistDefect::Rectifier(RectifierDefect::OpenOutput)),
        )
        .at(1.0, press("-S1", true))
        .at(
            5.0,
            hook(
                &h,
                HookIs::Reporting {
                    condition: ConditionName::FedBrakeClosed,
                },
            ),
        )
        .at(5.0, hook(&h, HookIs::Below { m: 0.05 }));
    let report = run_with(p, &w, Some(&make(p)));
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
    let w = Walkthrough::new("phase")
        .at(
            0.1,
            machine(
                &h,
                HoistDefect::LostPhase {
                    phase: PhaseName::L2,
                },
            ),
        )
        .at(1.0, press("-S1", true))
        .at(
            4.0,
            hook(
                &h,
                HookIs::Reporting {
                    condition: ConditionName::PhaseLost,
                },
            ),
        );
    let report = run_with(p, &w, Some(&make(p)));
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
}

#[test]
fn the_upper_limit_in_the_drawing_still_stops_the_faulted_hoist() {
    let doc = fixture();
    let p = doc.project();
    let h = id(p);
    // Coarse up (S2) for a long time: the drawn limit at 3 m cuts the coarse contactor.
    let w = Walkthrough::new("limit")
        .at(0.5, machine(&h, HoistDefect::Supply { ratio: 0.9 }))
        .at(1.0, press("-S2", true))
        .at(30.0, hook(&h, HookIs::Above { m: 2.9 }))
        .at(30.0, hook(&h, HookIs::Below { m: 3.6 }));
    let report = run_with(p, &w, Some(&make(p)));
    for s in &report.steps {
        assert_ne!(s.verdict, Verdict::Fail, "{}: {}", s.label, s.detail);
    }
}

#[test]
fn machine_steps_fail_plainly_without_a_hoist_and_json_round_trips() {
    let doc = fixture();
    let p = doc.project();
    let w = Walkthrough::new("no hoists")
        .at(1.0, machine("x", HoistDefect::Supply { ratio: 0.8 }))
        .at(2.0, hook("x", HookIs::Standing));
    let report = run(p, &w);
    assert_eq!(report.failed, 2);
    assert!(report.steps[0].detail.contains("no hoist"));
    let json = w.to_json();
    let back = Walkthrough::from_json(&json).expect("reads back");
    assert_eq!(back, w);
}
