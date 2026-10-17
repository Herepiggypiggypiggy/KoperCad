use novo_commissioning::*;
use novo_core::Document;

fn rec(target: Target, check: CheckKind, state: CheckState, at: &str) -> CheckRecord {
    CheckRecord {
        target,
        check,
        state,
        by: "tv".into(),
        at: at.into(),
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
fn a_package_round_trips_and_rejects_other_json() {
    let (doc, m) = starter();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[0]);
    store
        .set_check(rec(
            wire,
            CheckKind::Continuity,
            CheckState::Passed,
            "2026-10-16T09:00:00Z",
        ))
        .unwrap();
    let json = FieldPackage::new(
        "Starter",
        "tv",
        "2026-10-16T10:00:00Z",
        &store,
        doc.project(),
    )
    .to_json();
    let back = FieldPackage::from_json(&json).unwrap();
    assert_eq!(back.store, store);
    assert_eq!(back.project, "Starter");
    assert_eq!(
        FieldPackage::from_json("{\"a\":1}").unwrap_err(),
        FieldError::NotAFieldFile
    );
    assert!(matches!(
        FieldPackage::from_json("{\"format\":\"kopercad-field\",\"version\":9}"),
        Err(FieldError::TooNew(9))
    ));
}

#[test]
fn merging_keeps_the_newer_decision_per_check() {
    let (doc, m) = starter();
    let project = doc.project();
    let w0 = Target::Wire(m.motor_wires[0]);
    let w1 = Target::Wire(m.motor_wires[1]);
    let mut desk = CommissioningStore::new();
    desk.set_check(rec(
        w0,
        CheckKind::Continuity,
        CheckState::Failed,
        "2026-10-16T09:00:00Z",
    ))
    .unwrap();
    desk.set_check(rec(
        w1,
        CheckKind::Continuity,
        CheckState::Passed,
        "2026-10-16T12:00:00Z",
    ))
    .unwrap();
    let mut field = CommissioningStore::new();
    // newer than the desk: wins
    field
        .set_check(rec(
            w0,
            CheckKind::Continuity,
            CheckState::Passed,
            "2026-10-16T10:00:00Z",
        ))
        .unwrap();
    // older than the desk: kept as it was
    field
        .set_check(rec(
            w1,
            CheckKind::Continuity,
            CheckState::Failed,
            "2026-10-16T08:00:00Z",
        ))
        .unwrap();
    // new check
    field
        .set_check(rec(
            w1,
            CheckKind::Labelled,
            CheckState::Passed,
            "2026-10-16T08:30:00Z",
        ))
        .unwrap();
    let package = FieldPackage::new("P", "tv", "2026-10-16T13:00:00Z", &field, project);
    let report = package.merge_into(&mut desk, project);
    assert_eq!(report.checks_updated, 1);
    assert_eq!(report.checks_added, 1);
    assert_eq!(report.checks_kept, 1);
    assert_eq!(desk.state(w0, CheckKind::Continuity), CheckState::Passed);
    assert_eq!(desk.state(w1, CheckKind::Continuity), CheckState::Passed);
    // merging the same package again changes nothing
    let again = package.merge_into(&mut desk, project);
    assert!(!again.changed());
}

#[test]
fn punch_items_from_two_devices_do_not_collide() {
    let (doc, m) = starter();
    let project = doc.project();
    let wire = Target::Wire(m.motor_wires[0]);
    let item = |title: &str, at: &str| PunchItem {
        id: 0,
        title: title.into(),
        note: String::new(),
        severity: Severity::Minor,
        state: PunchState::Open,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: at.into(),
        closed_by: None,
        closed_at: None,
        photos: Vec::new(),
        signature: None,
    };
    let mut desk = CommissioningStore::new();
    desk.add_punch(item("door seal", "2026-10-16T09:00:00Z"));
    let mut field = CommissioningStore::new();
    let seal = field.add_punch(item("door seal", "2026-10-16T09:00:00Z"));
    field.add_punch(item("loose ferrule", "2026-10-16T09:10:00Z"));
    field
        .set_punch_state(seal, PunchState::Verified, "ab", "2026-10-16T11:00:00Z")
        .unwrap();
    // a signature travels with the item
    let mut signed = field.punch(seal).unwrap().clone();
    signed.signature = Some(Signature {
        name: "A. Bakker".into(),
        at: "2026-10-16T11:00:00Z".into(),
        strokes: vec![vec![[0, 0], [500, 800]]],
    });
    let mut field2 = CommissioningStore::new();
    field2.add_punch(signed);
    let _ = wire;
    let package = FieldPackage::new("P", "ab", "2026-10-16T12:00:00Z", &field, project);
    let report = package.merge_into(&mut desk, project);
    assert_eq!((report.punch_added, report.punch_updated), (1, 1));
    assert_eq!(
        desk.punch_items().count(),
        2,
        "the shared item is not doubled"
    );
    assert_eq!(desk.open_punch_count(), 1);
    let json = FieldPackage::new("P", "ab", "t", &field2, project).to_json();
    let back = FieldPackage::from_json(&json).unwrap();
    assert_eq!(
        back.store
            .punch_items()
            .next()
            .unwrap()
            .signature
            .as_ref()
            .unwrap()
            .name,
        "A. Bakker"
    );
}

#[test]
fn decisions_about_deleted_objects_are_skipped() {
    let (doc, m) = starter();
    let other = Document::default();
    let mut field = CommissioningStore::new();
    field
        .set_check(rec(
            Target::Wire(m.motor_wires[0]),
            CheckKind::Continuity,
            CheckState::Passed,
            "2026-10-16T09:00:00Z",
        ))
        .unwrap();
    let mut desk = CommissioningStore::new();
    let report = FieldPackage::new("P", "tv", "t", &field, doc.project())
        .merge_into(&mut desk, other.project());
    assert_eq!(report.unknown_targets, 1);
    assert!(desk.is_empty());
}

#[test]
fn a_file_from_another_project_does_not_merge_onto_wrong_ids() {
    // The same ids exist in both projects, but the second one has other tags.
    let (doc_a, m) = starter();
    let (mut doc_b, _) = starter();
    let wire = m.motor_wires[0];
    doc_b
        .execute(novo_core::Command::SetWireNumber {
            wire,
            number: Some("ZZ99".into()),
        })
        .unwrap();
    let mut field = CommissioningStore::new();
    field
        .set_check(rec(
            Target::Wire(wire),
            CheckKind::Continuity,
            CheckState::Passed,
            "2026-10-16T09:00:00Z",
        ))
        .unwrap();
    let package = FieldPackage::new("A", "tv", "t", &field, doc_a.project());
    let mut desk = CommissioningStore::new();
    let report = package.merge_into(&mut desk, doc_b.project());
    assert_eq!(report.mismatched_targets, 1);
    assert!(desk.is_empty());
    // the same project still merges
    let ok = package.merge_into(&mut desk, doc_a.project());
    assert_eq!(ok.checks_added, 1);
}

#[test]
fn photos_travel_in_the_file_and_merge_by_name() {
    let (doc, _) = starter();
    let project = doc.project();
    let item = PunchItem {
        id: 0,
        title: "door seal".into(),
        note: String::new(),
        severity: Severity::Minor,
        state: PunchState::Open,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: "2026-10-16T09:00:00Z".into(),
        closed_by: None,
        closed_at: None,
        photos: Vec::new(),
        signature: None,
    };
    let picture: Vec<u8> = (0..=255u8).chain(0..=7).collect(); // not a multiple of 3
    let name = photo_name(&picture, "JPG");
    assert!(is_photo_name(&name));
    assert_eq!(name, photo_name(&picture, ".jpg"), "same bytes, same name");
    assert_ne!(name, photo_name(&picture[1..], "jpg"));
    assert_eq!(photo_name(&picture, "../../x"), photo_name(&picture, "jpg"));
    assert!(!is_photo_name("../evil.jpg") && !is_photo_name("a.jpg"));

    let mut field = CommissioningStore::new();
    let id = field.add_punch(item.clone());
    let photo = Photo {
        name: name.clone(),
        at: "2026-10-16T09:05:00Z".into(),
        note: String::new(),
    };
    assert!(field.add_photo(id, photo.clone()).unwrap());
    assert!(!field.add_photo(id, photo).unwrap(), "no duplicate");
    assert!(
        field
            .add_photo(
                99,
                Photo {
                    name: name.clone(),
                    at: String::new(),
                    note: String::new()
                }
            )
            .is_err()
    );

    let mut package = FieldPackage::new("P", "tab", "t", &field, project);
    package.attach_photos(|n| (n == name).then(|| picture.clone()));
    let back = FieldPackage::from_json(&package.to_json()).unwrap();
    assert_eq!(back.photo_files(), vec![(name.clone(), picture.clone())]);

    // the desk has the item without the photo: the merge adds it
    let mut desk = CommissioningStore::new();
    desk.add_punch(item);
    let report = back.merge_into(&mut desk, project);
    assert_eq!(report.photos_added, 1);
    assert_eq!(desk.punch_items().next().unwrap().photos.len(), 1);
    assert!(!back.merge_into(&mut desk, project).changed(), "idempotent");
}

#[test]
fn a_photo_name_that_could_escape_the_folder_is_not_read_from_a_file() {
    let (doc, _) = starter();
    let json = FieldPackage::new("P", "t", "t", &CommissioningStore::new(), doc.project())
        .to_json()
        .replace(
            "\"labels\"",
            "\"photos\": [{\"name\": \"../../x.jpg\", \"data\": \"AAAA\"}], \"labels\"",
        );
    let package = FieldPackage::from_json(&json).unwrap();
    assert!(package.photo_files().is_empty());
}

#[test]
fn a_picture_that_does_not_hash_to_its_name_is_dropped() {
    let (doc, _) = starter();
    let real = vec![1u8, 2, 3, 4, 5];
    let name = photo_name(&real, "jpg");
    let package = FieldPackage::new("A", "tv", "t", &CommissioningStore::new(), doc.project());
    let mut value: serde_json::Value = serde_json::from_str(&package.to_json()).unwrap();
    // "AAAA" decodes to three zero bytes, not the picture the name stands for
    value["photos"] = serde_json::json!([{"name": name, "data": "AAAA"}]);
    let back = FieldPackage::from_json(&value.to_string()).unwrap();
    assert!(
        back.photo_files().is_empty(),
        "wrong bytes under a real name"
    );

    // the same name with the right bytes is kept
    let mut good = package.clone();
    good.attach_photos(|_| None);
    let mut value: serde_json::Value = serde_json::from_str(&good.to_json()).unwrap();
    let data = {
        let mut p = FieldPackage::new("A", "tv", "t", &store_with_photo(&name), doc.project());
        p.attach_photos(|n| (n == name).then(|| real.clone()));
        serde_json::from_str::<serde_json::Value>(&p.to_json()).unwrap()["photos"].clone()
    };
    value["photos"] = data;
    let back = FieldPackage::from_json(&value.to_string()).unwrap();
    assert_eq!(back.photo_files(), vec![(name, real)]);
}

fn store_with_photo(name: &str) -> CommissioningStore {
    let mut store = CommissioningStore::new();
    let id = store.add_punch(PunchItem {
        id: 0,
        title: "x".into(),
        note: String::new(),
        severity: Severity::Minor,
        state: PunchState::Open,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: "t".into(),
        closed_by: None,
        closed_at: None,
        signature: None,
        photos: Vec::new(),
    });
    store
        .add_photo(
            id,
            Photo {
                name: name.into(),
                at: "t".into(),
                note: String::new(),
            },
        )
        .unwrap();
    store
}
