use novo_commissioning::*;
use novo_core::Document;

fn starter() -> (Document, novo_tests::motor_starter::MotorStarter) {
    let mut doc = Document::default();
    let m = novo_tests::motor_starter::motor_starter(&mut doc).unwrap();
    (doc, m)
}

fn pass(store: &mut CommissioningStore, target: Target) {
    for &check in target.checks() {
        store
            .set_check(CheckRecord {
                target,
                check,
                state: CheckState::Passed,
                by: "tv".into(),
                at: "2026-10-21T08:00:00Z".into(),
                value: String::new(),
                note: String::new(),
            })
            .unwrap();
    }
}

#[test]
fn every_drawn_wire_has_a_step_with_both_ends() {
    let (doc, _) = starter();
    let project = doc.project();
    let plan = WirePlan::of(project);
    assert_eq!(plan.len(), project.wires().count());
    let with_ends = project
        .wires()
        .filter_map(|w| plan.step(w.id))
        .filter(|s| !s.from.is_empty() && !s.to.is_empty())
        .count();
    assert!(with_ends > 0, "wires between pins name both ends");
}

#[test]
fn a_cable_core_is_told_apart_from_a_plain_wire() {
    let (doc, m) = starter();
    let project = doc.project();
    let plan = WirePlan::of(project);
    let core_wires: Vec<_> = m
        .motor_wires
        .iter()
        .filter_map(|&w| plan.step(w))
        .filter(|s| matches!(s.kind, WireKind::Core { .. }))
        .collect();
    assert!(!core_wires.is_empty(), "the motor cable carries cores");
    for s in core_wires {
        assert!(
            s.kind.short().starts_with("core ") && s.kind.short().contains("W1:"),
            "{}",
            s.kind.short()
        );
    }
    assert!(
        project
            .wires()
            .filter_map(|w| plan.step(w.id))
            .any(|s| s.kind == WireKind::Plain)
    );
}

#[test]
fn a_repeated_number_says_what_the_other_wire_is() {
    let (doc, _) = starter();
    let project = doc.project();
    let plan = WirePlan::of(project);
    for w in project.wires() {
        let step = plan.step(w.id).unwrap();
        if step.number.is_empty() {
            assert!(step.same_number.is_empty());
            continue;
        }
        let twins = project
            .wires()
            .filter(|o| o.id != w.id && o.number == w.number)
            .count();
        assert_eq!(step.same_number.len(), twins);
        assert_eq!(step.same_number_hint().is_empty(), twins == 0);
    }
}

#[test]
fn next_open_walks_pages_in_order_and_wraps() {
    let (doc, m) = starter();
    let project = doc.project();
    let mut store = CommissioningStore::new();
    let first = next_open(project, &store, None, None, WalkScope::Wires).unwrap();
    assert!(matches!(first.target, Target::Wire(_)));
    assert_eq!(first.pages.first(), Some(&m.main_page));
    // Deciding it moves the walk on, and the next one is another wire.
    pass(&mut store, first.target);
    let second = next_open(project, &store, Some(first.target), None, WalkScope::Wires).unwrap();
    assert_ne!(second.target, first.target);
    // Starting from the control page begins there.
    let on_control = next_open(
        project,
        &store,
        None,
        Some(m.control_page),
        WalkScope::Wires,
    )
    .unwrap();
    assert_eq!(on_control.pages.first(), Some(&m.control_page));
}

#[test]
fn next_open_skips_decided_and_ends_when_all_are_done() {
    let (doc, _) = starter();
    let project = doc.project();
    let mut store = CommissioningStore::new();
    for info in walk_order(project, WalkScope::Wires) {
        pass(&mut store, info.target);
    }
    assert!(next_open(project, &store, None, None, WalkScope::Wires).is_none());
    // Terminals and devices are still open in the full walk.
    let rest = next_open(project, &store, None, None, WalkScope::All).unwrap();
    assert!(!matches!(rest.target, Target::Wire(_)));
}

#[test]
fn next_open_after_the_last_open_target_wraps_to_an_earlier_one() {
    let (doc, _) = starter();
    let project = doc.project();
    let mut store = CommissioningStore::new();
    let order = walk_order(project, WalkScope::Wires);
    let (first, rest) = order.split_first().unwrap();
    for info in rest {
        pass(&mut store, info.target);
    }
    // Only the first is open; asking from the last wraps round to it.
    let last = order.last().unwrap().target;
    let next = next_open(project, &store, Some(last), None, WalkScope::Wires).unwrap();
    assert_eq!(next.target, first.target);
}

#[test]
fn place_order_groups_wires_by_the_device_they_start_on() {
    let (doc, _) = starter();
    let project = doc.project();
    let plan = WirePlan::of(project);
    let order = walk_order_with(project, &plan, WalkScope::Wires, WalkOrder::Place);
    let by_number = walk_order(project, WalkScope::Wires);
    assert_eq!(
        order.len(),
        by_number.len(),
        "the same wires, another order"
    );
    let devices: Vec<String> = order
        .iter()
        .filter_map(|i| match i.target {
            Target::Wire(w) => plan.step(w),
            _ => None,
        })
        .map(|s| {
            s.from
                .rsplit_once(':')
                .map_or(s.from.as_str(), |p| p.0)
                .to_owned()
        })
        .collect();
    // Once the walk leaves a device it does not come back to it.
    let mut seen = std::collections::HashSet::new();
    let mut last = None;
    for d in &devices {
        if Some(d) != last.as_ref() {
            assert!(seen.insert(d.clone()), "{d} was walked twice");
            last = Some(d.clone());
        }
    }
}
