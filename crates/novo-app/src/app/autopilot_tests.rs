//! Autopilot and the Ready to build meter on the shipped example.

use std::collections::BTreeSet;

use super::autopilot::build;
use super::gate1_tests::Drive;
use crate::actions::Action;
use novo_reports::autopilot::Basis;

fn example() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    (dir, d)
}

#[test]
fn the_example_goes_from_unsized_wires_to_none_in_one_undo_step() {
    let (_dir, mut d) = example();
    let before = novo_reports::unset_wires(d.doc().project());
    assert!(!before.no_size.is_empty(), "the example starts unsized");
    let (plan, ready_before) = build(d.doc().project(), false);

    assert!(plan.unresolved.is_empty(), "{:?}", plan.unresolved);
    assert!(plan.proposals.iter().all(|p| !p.reason.is_empty()));
    // Every size is defensible: calculated from a rating, or a rule. None is a blind minimum.
    for p in plan.proposals.iter().filter(|p| p.change.kind() == "Size") {
        assert_ne!(p.basis, Basis::Assumed, "{}: {}", p.label, p.reason);
    }
    let sized: BTreeSet<_> = plan
        .proposals
        .iter()
        .filter(|p| p.change.kind() == "Size")
        .flat_map(|p| p.wires.iter().copied())
        .collect();
    assert_eq!(sized, before.no_size.iter().copied().collect());

    let depth = d.doc().undo_depth();
    d.app.apply_autopilot(&plan, None);
    d.frame();
    let after = novo_reports::unset_wires(d.doc().project());
    assert!(after.no_size.is_empty(), "{:?}", after.no_size);
    assert!(after.no_colour.is_empty(), "{:?}", after.no_colour);
    assert_eq!(d.doc().undo_depth(), depth + 1, "one undo step");
    let (_, ready_after) = build(d.doc().project(), false);
    assert!(ready_after.percent() > ready_before.percent());

    d.app.backend.document_mut().unwrap().undo().unwrap();
    let undone = novo_reports::unset_wires(d.doc().project());
    assert_eq!(undone.no_size.len(), before.no_size.len());
}

#[test]
fn the_motor_circuit_gets_the_sizes_and_colours_a_designer_would_check() {
    let (_dir, d) = example();
    let (plan, _) = build(d.doc().project(), false);
    let project = d.doc().project();
    let find = |kind: &str, label: &str| {
        plan.proposals
            .iter()
            .find(|p| p.change.kind() == kind && p.label.contains(label))
            .unwrap_or_else(|| panic!("no {kind} for {label}"))
    };
    // Protective earth: green-yellow, as large as the largest power wire.
    assert_eq!(find("Colour", "PE (feed)").change.value(), "GNYE");
    assert_eq!(find("Size", "PE (feed)").change.value(), "1.5 mm²");
    // Neutral: light blue.
    assert_eq!(find("Colour", "N (feed)").change.value(), "BU");
    // Power wires black, control wires red (230 V AC control, IEC 60204-1).
    assert_eq!(find("Colour", "L1 (feed)").change.value(), "BK");
    assert_eq!(find("Colour", "F3:2").change.value(), "RD");
    // The 6 A control fuse: 1 mm², from the small-section planning cap.
    let control = find("Size", "F3:2");
    assert_eq!(control.change.value(), "1 mm²");
    assert!(
        control.reason.contains("F3 (6 A) protects it"),
        "{}",
        control.reason
    );
    assert_eq!(control.basis, Basis::Planning);
    // The unrated fuse F1: the rated devices on the circuit set the current.
    let power = find("Size", "L1 (feed)");
    assert!(
        power.reason.contains("rated devices on its circuit"),
        "{}",
        power.reason
    );
    let _ = project;
}

#[test]
fn accepting_one_row_changes_only_its_wires() {
    let (_dir, mut d) = example();
    let (plan, _) = build(d.doc().project(), false);
    let one = plan
        .proposals
        .iter()
        .find(|p| p.change.kind() == "Colour")
        .unwrap();
    let wires = one.wires.clone();
    let ids: BTreeSet<usize> = [one.id].into();
    d.app.apply_autopilot(&plan, Some(&ids));
    d.frame();
    let unset = novo_reports::unset_wires(d.doc().project());
    assert!(wires.iter().all(|w| !unset.no_colour.contains(w)));
    assert!(!unset.no_colour.is_empty(), "the other rows are untouched");
    assert!(!unset.no_size.is_empty());
}

#[test]
fn a_changed_drawing_refuses_the_old_review_list() {
    let (_dir, mut d) = example();
    let (plan, _) = build(d.doc().project(), false);
    let wire = d.doc().project().wires().next().unwrap().id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("edit", |tx| {
            tx.update::<novo_core::Wire>(wire, |w| w.cross_section_mm2 = Some(2.5))
        })
        .unwrap();
    let depth = d.doc().undo_depth();
    d.app.apply_autopilot(&plan, None);
    assert_eq!(
        d.doc().undo_depth(),
        depth,
        "nothing written from a stale list"
    );
}

#[test]
fn unnumbered_wires_get_numbers_from_the_scheme_with_a_reason() {
    let (_dir, mut d) = example();
    let wires: Vec<_> = d.doc().project().wires().map(|w| w.id).collect();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("clear", |tx| {
            for id in wires {
                tx.update::<novo_core::Wire>(id, |w| w.number = None)?;
            }
            Ok(())
        })
        .unwrap();
    let (plan, _) = build(d.doc().project(), false);
    let numbers: Vec<_> = plan
        .proposals
        .iter()
        .filter(|p| p.change.kind() == "Number")
        .collect();
    assert!(!numbers.is_empty());
    assert!(
        numbers
            .iter()
            .all(|p| p.reason.contains("scheme") || p.reason.contains("net"))
    );
    d.app.apply_autopilot(&plan, None);
    d.frame();
    assert!(
        d.doc().project().wires().all(|w| w.number.is_some()),
        "every wire is numbered"
    );
}

#[test]
fn the_command_opens_the_window_and_the_bench_names_it() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "wires.autopilot", None);
    d.frame();
    assert!(d.app.wb.autopilot.is_some());
    let p = d.doc().project();
    let jobs = novo_reports::wire_cut_jobs_with(p, &novo_reports::FerruleStandard::of(p));
    let gaps = super::bench::readiness(&jobs, true);
    assert!(
        gaps.iter()
            .any(|g| g.fix == Some(("Autopilot", "wires.autopilot"))),
        "{gaps:?}"
    );
}

#[test]
fn assumed_rows_start_unticked_and_stay_out_of_accept_all() {
    use novo_reports::autopilot::{Autopilot, Change, Proposal};
    let mut plan = Autopilot::default();
    let row = |basis| Proposal {
        id: 0,
        wires: Vec::new(),
        label: "wire".into(),
        change: Change::Size(1.5),
        reason: "r".into(),
        basis,
    };
    plan.extend([row(Basis::Planning), row(Basis::Assumed), row(Basis::Rule)]);
    let ticked = super::autopilot::default_ticked(&plan);
    assert_eq!(ticked, BTreeSet::from([0, 2]));
}

#[test]
fn earth_and_neutral_reasons_say_which_circuit_they_follow() {
    let (_dir, d) = example();
    let (plan, _) = build(d.doc().project(), false);
    let followers: Vec<_> = plan
        .proposals
        .iter()
        .filter(|p| p.change.kind() == "Size")
        .filter(|p| p.reason.starts_with("protective") || p.reason.starts_with("the neutral"))
        .collect();
    assert!(!followers.is_empty(), "the example has PE or neutral wires");
    for p in &followers {
        assert!(
            p.reason.contains("on the same device") || p.reason.contains("none on the same device"),
            "{}: {}",
            p.label,
            p.reason
        );
        // A size that follows another proposal is never presented as a plain rule.
        if p.reason.contains("itself a proposal") {
            assert_eq!(p.basis, Basis::Planning, "{}", p.reason);
        }
    }
    for p in &followers {
        eprintln!("{}: {} [{}]", p.label, p.reason, p.basis.label());
    }
}

#[test]
fn cable_rows_group_the_wires_that_leave_together_and_apply_last() {
    let (_dir, mut d) = example();
    let (plan, _) = build(d.doc().project(), false);
    let cables: Vec<_> = plan
        .proposals
        .iter()
        .filter(|p| p.change.kind() == "Cable")
        .collect();
    eprintln!("cable rows: {}", cables.len());
    for c in &cables {
        eprintln!("{}: {} -> {}", c.label, c.change.value(), c.reason);
    }
    let before = d.doc().project().cables().count();
    d.app.apply_autopilot(&plan, None);
    d.frame();
    let made = d.doc().project().cables().count() - before;
    assert_eq!(made, cables.len(), "every cable row creates its cable");
}

#[test]
fn estimated_lengths_are_reported_as_left_to_the_designer() {
    let (_dir, d) = example();
    let (plan, _) = build(d.doc().project(), false);
    if !plan.left_to_you.is_empty() {
        assert!(
            plan.left_to_you[0].contains("estimates"),
            "{:?}",
            plan.left_to_you
        );
    }
}

#[test]
fn estimates_in_the_example_are_explained_by_cause() {
    let (_dir, d) = example();
    let (plan, _) = build(d.doc().project(), false);
    let text = plan.left_to_you.join("\n");
    // The motor cable states its length, so its four cores are exact; the six
    // supply leads come in from outside and still wait for an entry and a
    // stated length outside it.
    assert!(!text.contains("another location"), "{text}");
    assert!(
        text.contains("6 wire lengths are estimates: the wire comes in from outside"),
        "{text}"
    );
    assert!(!text.contains("not placed"), "{text}");
}
