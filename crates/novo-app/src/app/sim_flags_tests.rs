//! Problem triangles for the simulation windows (lane T10).

use super::Level;
use super::sim_flags::{
    CONTROLLER, DRIVE, HOIST, MOTOR_START, WINCH, failure_draft, finding_draft, motor_start_draft,
    motor_unmodelled_draft, note_drafts, pin_problem_draft, set_flags,
};
use crate::actions::Action;
use crate::app::gate1_tests::Drive;
use novo_flags::{Anchor, Flag, Life, Severity, Source};
use novo_sim::pins::PinProblem;

fn device_of(tag: &str) -> Option<String> {
    (tag == "-M1").then(|| "7".to_owned())
}

#[test]
fn a_left_out_device_is_a_yellow_flag_on_that_device() {
    let notes = vec![(Level::Warning, "-M1 is left out: no hook load".to_owned())];
    let d = note_drafts(DRIVE, &notes, &device_of);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].severity, Severity::Yellow);
    assert_eq!(d[0].title, "Left out of the run");
    assert_eq!(d[0].anchors, vec![Anchor::Device("7".into())]);
    assert!(d[0].actions.contains(&novo_flags::Action::Locate));
    assert_eq!(d[0].life, Life::Run(DRIVE.into()));
}

#[test]
fn a_note_without_a_device_sits_on_the_run() {
    let notes = vec![
        (
            Level::Warning,
            "No axis can run: see the notes above.".to_owned(),
        ),
        (Level::Error, "the step is not positive".to_owned()),
        (Level::Warning, "-M9 is left out: no ratio".to_owned()),
    ];
    let d = note_drafts(HOIST, &notes, &device_of);
    assert_eq!(d.len(), 3);
    assert!(
        d.iter()
            .all(|f| f.anchors == vec![Anchor::Job(HOIST.into())])
    );
    assert_eq!(d[0].title, "Nothing can run");
    assert_eq!(d[1].severity, Severity::Red);
    assert_eq!(d[1].title, "Run cannot start");
    assert!(!d[2].actions.contains(&novo_flags::Action::Locate));
}

#[test]
fn information_is_not_a_flag_and_titles_fit() {
    let notes = vec![
        (Level::Info, "-M1: fitted from the nameplate".to_owned()),
        (
            Level::Warning,
            "-M1: the brake feed device -K9 is not in the circuit".to_owned(),
        ),
        (Level::Error, "-M1: it broke".to_owned()),
    ];
    let d = note_drafts(DRIVE, &notes, &device_of);
    assert_eq!(d.len(), 2, "the information line is left out");
    assert_eq!(d[0].title, "Binding problem");
    assert_eq!(d[1].title, "Run problem");
    for f in &d {
        assert!(f.title.chars().count() <= 28, "{}", f.title);
    }
}

#[test]
fn two_reasons_for_one_device_are_two_flags_and_the_same_one_twice_is_one() {
    let a = (Level::Warning, "-M1 is left out: no ratio".to_owned());
    let b = (Level::Warning, "-M1 is left out: no radius".to_owned());
    let ids = |notes: &[(Level, String)]| -> Vec<_> {
        note_drafts(DRIVE, notes, &device_of)
            .into_iter()
            .map(|d| Flag::from_draft(d).id)
            .collect()
    };
    let both = ids(&[a.clone(), b]);
    assert_ne!(both[0], both[1]);
    assert_eq!(
        ids(std::slice::from_ref(&a)),
        ids(&[a]),
        "stable across re-runs"
    );
}

#[test]
fn the_same_note_in_two_windows_is_two_flags() {
    let note = vec![(Level::Warning, "-M1 is left out: no ratio".to_owned())];
    let a = Flag::from_draft(note_drafts(DRIVE, &note, &device_of).remove(0)).id;
    let b = Flag::from_draft(note_drafts(HOIST, &note, &device_of).remove(0)).id;
    assert_ne!(a, b);
}

#[test]
fn every_pin_problem_is_red_on_the_signal_row_with_a_short_title() {
    let all = [
        PinProblem::NotAPin("x".into()),
        PinProblem::NoDevice("-K9".into()),
        PinProblem::DuplicateDevice("-M1".into()),
        PinProblem::NoPin {
            device: "-M1".into(),
            pin: "9".into(),
        },
        PinProblem::NotSwitched {
            device: "-M1".into(),
            pin: "A1".into(),
        },
        PinProblem::Shared {
            device: "-M1".into(),
            pin: "13".into(),
            poles: 2,
        },
        PinProblem::NoPoles("-M1".into()),
    ];
    let mut rules = std::collections::BTreeSet::new();
    for p in &all {
        let d = pin_problem_draft("c1", "Hoist up", p, &device_of);
        assert_eq!(d.severity, Severity::Red);
        assert!(d.title.chars().count() <= 28, "{}", d.title);
        assert_eq!(d.anchors[0], Anchor::Control("signal:c1:Hoist up".into()));
        assert!(d.detail.0[0].contains(&p.to_string()));
        rules.insert(d.rule.clone());
    }
    assert_eq!(rules.len(), all.len(), "one rule per kind of problem");
    // A device the drawing has is a second anchor, with Locate.
    let on_device = pin_problem_draft("c1", "s", &all[2], &device_of);
    assert_eq!(on_device.anchors.len(), 2);
    assert!(on_device.actions.contains(&novo_flags::Action::Locate));
    let none = pin_problem_draft("c1", "s", &all[1], &device_of);
    assert_eq!(none.anchors.len(), 1);
}

#[test]
fn the_motor_start_result_is_red_when_it_stalls_yellow_when_slow_and_quiet_otherwise() {
    let stalled = motor_start_draft("-M1", Some("7".into()), true, None).unwrap();
    assert_eq!(stalled.severity, Severity::Red);
    assert_eq!(stalled.anchors, vec![Anchor::Device("7".into())]);
    let slow = motor_start_draft("-M1", None, false, Some(40.0)).unwrap();
    assert_eq!(slow.severity, Severity::Yellow);
    assert_eq!(slow.anchors, vec![Anchor::Job(MOTOR_START.into())]);
    assert!(slow.detail.0[0].contains("40 s"));
    assert!(motor_start_draft("-M1", None, false, None).is_none());
    let unmodelled = motor_unmodelled_draft("-M1", None, Some("no power"));
    assert!(unmodelled.unchecked, "could not be judged: dashed");
    assert_eq!(unmodelled.severity, Severity::Yellow);
}

#[test]
fn scenario_findings_and_failures_sit_on_the_run() {
    let red = finding_draft(WINCH, true, "the brake did not hold");
    let yellow = finding_draft(WINCH, false, "the rope went slack");
    assert_eq!(red.severity, Severity::Red);
    assert_eq!(yellow.severity, Severity::Yellow);
    assert_eq!(failure_draft(WINCH, "bad number").title, "Run failed");
    assert_eq!(red.anchors, vec![Anchor::Job(WINCH.into())]);
}

#[test]
fn setting_a_windows_flags_replaces_only_that_window_and_does_not_pulse_again() {
    let mut ui = super::flags::FlagUi::default();
    let notes = vec![(Level::Warning, "-M1 is left out: no ratio".to_owned())];
    set_flags(&mut ui, DRIVE, note_drafts(DRIVE, &notes, &device_of));
    set_flags(&mut ui, HOIST, note_drafts(HOIST, &notes, &device_of));
    assert_eq!(ui.store.counts().total(), 2);
    let fresh = ui.store.take_fresh();
    assert_eq!(fresh.len(), 2);
    // Same drafts again: nothing new.
    set_flags(&mut ui, DRIVE, note_drafts(DRIVE, &notes, &device_of));
    assert!(ui.store.take_fresh().is_empty());
    // Clearing one window leaves the other.
    set_flags(&mut ui, DRIVE, Vec::new());
    assert_eq!(ui.store.counts().total(), 1);
    set_flags(&mut ui, CONTROLLER, Vec::new());
    assert_eq!(ui.store.counts().total(), 1);
    assert!(ui.store.list(false).iter().all(|f| f.source == Source::Sim));
}

#[test]
fn starting_a_run_with_nothing_to_run_raises_a_flag_the_window_lists() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.start_drive_run();
    d.app.start_hoist_run();
    let drive = d.app.sim_flags_of(DRIVE);
    assert_eq!(drive.len(), 1, "{drive:?}");
    assert_eq!(drive[0].title, "Nothing can run");
    assert_eq!(d.app.sim_flags_of(HOIST).len(), 1);
    // The window draws them and a frame passes.
    d.app.wb.drive_run.open = true;
    d.app.wb.hoist.open = true;
    d.frame();
    d.frame();
    // The same facts a second time are not a second flag.
    d.app.start_drive_run();
    assert_eq!(d.app.sim_flags_of(DRIVE).len(), 1);
    // One count: the two windows are two flags.
    assert_eq!(d.app.wb.flags.store.counts().total(), 2);
}

/// How many places in a file still say a problem in words (toast level,
/// banner, error or warning colour). The number only goes down; a lane that
/// migrates a site lowers it here, in this area's own file
/// (docs/design/problem-triangles.md, section 12). Lane T10 took it from 98
/// to 82 in these files.
fn census(source: &str) -> usize {
    source
        .lines()
        .filter(|l| {
            !l.trim_start().starts_with("//")
                && (l.contains("Level::Error")
                    || l.contains("Level::Warning")
                    || l.contains("banner(")
                    || [
                        "palette.error",
                        "palette.warning",
                        "pal.error",
                        "pal.warning",
                        "p.error",
                        "p.warning",
                        "ui_palette().error",
                        "ui_palette().warning",
                    ]
                    .iter()
                    .any(|k| l.contains(k)))
        })
        .count()
}

#[test]
fn problem_census_sim() {
    // (file, ceiling). Test modules at the end of a file are not counted.
    let files: [(&str, &str, usize); 12] = [
        ("simulate", include_str!("simulate.rs"), 1),
        ("faults", include_str!("faults.rs"), 19),
        ("drive_run", include_str!("drive_run.rs"), 11),
        ("hoist_run", include_str!("hoist_run.rs"), 6),
        ("machine", include_str!("machine.rs"), 4),
        ("live", include_str!("live.rs"), 8),
        ("live_anim", include_str!("live_anim.rs"), 4),
        ("motor_freq", include_str!("motor_freq.rs"), 3),
        ("winch_scenarios", include_str!("winch_scenarios.rs"), 2),
        ("sim_chip", include_str!("sim_chip.rs"), 2),
        ("controller_card", include_str!("controller_card.rs"), 12),
        (
            "controller_import",
            include_str!("controller_import.rs"),
            10,
        ),
    ];
    for (name, source, ceiling) in files {
        let production = source.split("#[cfg(test)]\nmod ").next().unwrap_or(source);
        let n = census(production);
        assert!(
            n <= ceiling,
            "{name}.rs says {n} problems in words, the ceiling is {ceiling}: use a flag \
             (sim_flags.rs) or lower the ceiling when you remove one"
        );
    }
}
