use novo_commissioning::*;
use novo_core::Document;

fn rec(target: Target, check: CheckKind, state: CheckState) -> CheckRecord {
    CheckRecord {
        target,
        check,
        state,
        by: "tv".into(),
        at: "2026-10-15T10:00:00Z".into(),
        value: String::new(),
        note: String::new(),
    }
}

fn starter() -> (Document, novo_tests::motor_starter::MotorStarter) {
    let mut doc = Document::default();
    let m = novo_tests::motor_starter::motor_starter(&mut doc).unwrap();
    (doc, m)
}

#[test]
fn totals_follow_the_model() {
    let (doc, _) = starter();
    let project = doc.project();
    let all = targets(project);
    let expected: usize = all.iter().map(|t| t.target.checks().len()).sum();
    let p = progress(project, &CommissioningStore::new(), Scope::Project);
    assert_eq!(p.total, expected);
    assert_eq!(p.pending, expected);
    assert_eq!(p.passed, 0);
    // strips are checked through their terminals
    assert!(all.iter().all(|t| !matches!(t.target, Target::Device(d) if project.device(d).unwrap().kind == novo_core::DeviceKind::TerminalStrip)));
}

#[test]
fn passing_and_not_applicable_move_progress() {
    let (doc, m) = starter();
    let project = doc.project();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[0]);
    store
        .set_check(rec(wire, CheckKind::Continuity, CheckState::Passed))
        .unwrap();
    store
        .set_check(rec(wire, CheckKind::Labelled, CheckState::NotApplicable))
        .unwrap();
    let p = progress(project, &store, Scope::Project);
    assert_eq!(p.passed, 1);
    let blank = progress(project, &CommissioningStore::new(), Scope::Project);
    assert_eq!(p.total, blank.total - 1);
    let page_total: usize = progress_by_page(project, &store)
        .iter()
        .map(|(_, p)| p.passed)
        .sum();
    assert_eq!(page_total, 1);
}

#[test]
fn a_check_that_does_not_apply_is_refused() {
    let (_, m) = starter();
    let mut store = CommissioningStore::new();
    let err = store
        .set_check(rec(
            Target::Wire(m.motor_wires[0]),
            CheckKind::Torque,
            CheckState::Passed,
        ))
        .unwrap_err();
    assert!(matches!(err, CommissioningError::NotApplicable { .. }));
}

#[test]
fn a_failure_opens_one_punch_item_and_fixing_it_reopens_the_test() {
    let (_, m) = starter();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[1]);
    let id = store
        .set_check(rec(wire, CheckKind::Continuity, CheckState::Failed))
        .unwrap()
        .expect("punch item opened");
    // failing again does not duplicate it
    assert_eq!(
        store
            .set_check(rec(wire, CheckKind::Continuity, CheckState::Failed))
            .unwrap(),
        None
    );
    assert_eq!(store.open_punch_count(), 1);

    store
        .set_punch_state(id, PunchState::Fixed, "tv", "t1")
        .unwrap();
    assert_eq!(
        store.state(wire, CheckKind::Continuity),
        CheckState::Pending
    );
    assert_eq!(store.open_punch_count(), 1, "fixed waits for the re-test");

    store
        .set_punch_state(id, PunchState::Verified, "tv", "t2")
        .unwrap();
    let item = store.punch(id).unwrap();
    assert_eq!(item.closed_by.as_deref(), Some("tv"));
    assert_eq!(store.open_punch_count(), 0);
    assert!(
        store
            .set_punch_state(99, PunchState::Verified, "x", "y")
            .is_err()
    );
}

#[test]
fn the_store_round_trips_through_json() {
    let (_, m) = starter();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[0]);
    store
        .set_check(rec(wire, CheckKind::Continuity, CheckState::Failed))
        .unwrap();
    store
        .set_check(rec(wire, CheckKind::Labelled, CheckState::Passed))
        .unwrap();
    let json = serde_json::to_string(&store).unwrap();
    let mut back: CommissioningStore = serde_json::from_str(&json).unwrap();
    assert_eq!(back.records().count(), 2);
    assert_eq!(back.open_punch_count(), 1);
    let next = back.add_punch(PunchItem {
        id: 0,
        title: "door seal".into(),
        note: String::new(),
        severity: Severity::Minor,
        state: PunchState::Open,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: "t".into(),
        closed_by: None,
        closed_at: None,
    });
    assert_eq!(next, 2, "ids continue after a reload");
}
