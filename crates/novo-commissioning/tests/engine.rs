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
    assert_eq!(
        p.total, blank.total,
        "not applicable still counts, as in the report"
    );
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
    let json = store.to_json();
    let mut back = CommissioningStore::from_json(&json).unwrap();
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
        photos: Vec::new(),
        signature: None,
    });
    assert_eq!(next, 2, "ids continue after a reload");
}

#[test]
fn the_report_input_lists_every_check_and_punch_item() {
    let (doc, m) = starter();
    let project = doc.project();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[0]);
    let mut r = rec(wire, CheckKind::Continuity, CheckState::Failed);
    r.value = "open".into();
    store.set_check(r).unwrap();
    store
        .set_check(rec(
            Target::Device(m.k1),
            CheckKind::Tested,
            CheckState::Passed,
        ))
        .unwrap();
    let input = to_report_input(project, &store, Vec::new());
    let expected: usize = targets(project)
        .iter()
        .map(|t| t.target.checks().len())
        .sum();
    assert_eq!(input.checks.len(), expected);
    assert_eq!(input.project_name, project.info().name);
    let failed: Vec<_> = input
        .checks
        .iter()
        .filter(|c| c.state == novo_reports::commissioning::CheckState::Failed)
        .collect();
    assert_eq!(failed.len(), 1);
    assert_eq!(failed[0].what, "Continuity");
    assert_eq!(failed[0].note, "open");
    assert_eq!(failed[0].by, "tv");
    assert_eq!(input.punch.len(), 1);
    assert_eq!(input.punch[0].tag, failed[0].tag);
    let output = novo_reports::commissioning::commissioning_report(&input);
    assert!(output.row_count() > 0);
}

#[test]
fn not_applicable_counts_as_done_and_an_empty_scope_is_zero() {
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
    assert_eq!((p.passed, p.not_applicable), (1, 1));
    assert_eq!(p.done(), 2);
    assert_eq!(
        p.total,
        progress(project, &CommissioningStore::new(), Scope::Project).total
    );
    assert_eq!(Progress::default().percent(), 0.0);
}

#[test]
fn prune_drops_decisions_about_deleted_objects() {
    let (mut doc, m) = starter();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[0]);
    store
        .set_check(rec(wire, CheckKind::Continuity, CheckState::Passed))
        .unwrap();
    store.prune(doc.project());
    assert_eq!(store.records().count(), 1);
    doc.transact("Delete", |tx| tx.remove(m.motor_wires[0]))
        .unwrap();
    store.prune(doc.project());
    assert_eq!(store.records().count(), 0);
}

#[test]
fn signing_off_verifies_with_the_signer_and_reopening_drops_the_signature() {
    let mut store = CommissioningStore::new();
    let id = store.add_punch(PunchItem {
        id: 0,
        title: "Loose lug".into(),
        note: String::new(),
        severity: Severity::Major,
        state: PunchState::Fixed,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: "2026-10-17T08:00:00Z".into(),
        closed_by: None,
        closed_at: None,
        signature: None,
        photos: Vec::new(),
    });
    let unsigned = Signature::from_strokes("  ", "t", vec![]);
    assert!(store.sign_off(id, unsigned).is_err());
    assert_eq!(store.punch(id).unwrap().state, PunchState::Fixed);
    let sig = Signature::from_strokes(
        "A. Boss",
        "2026-10-17T09:00:00Z",
        vec![vec![[0, 0], [10, 10]]],
    );
    store.sign_off(id, sig).unwrap();
    let item = store.punch(id).unwrap();
    assert_eq!(item.state, PunchState::Verified);
    assert_eq!(item.closed_by.as_deref(), Some("A. Boss"));
    assert_eq!(item.signature.as_ref().unwrap().strokes.len(), 1);
    store
        .set_punch_state(id, PunchState::Open, "tv", "t")
        .unwrap();
    assert!(store.punch(id).unwrap().signature.is_none());
}

#[test]
fn strokes_are_clamped_and_capped() {
    let long: Vec<[u16; 2]> = (0..5000).map(|i| [i, 2000]).collect();
    let sig = Signature::from_strokes("x", "t", vec![vec![[5, 5]], long.clone(), long]);
    let points: usize = sig.strokes.iter().map(Vec::len).sum();
    assert_eq!(
        points,
        Signature::MAX_POINTS,
        "one-point stroke dropped, total capped"
    );
    assert!(
        sig.strokes
            .iter()
            .flatten()
            .all(|p| p[0] <= 1000 && p[1] <= 1000)
    );
}
