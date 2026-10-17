use novo_commissioning::*;
use novo_core::{Command, Document};

fn starter() -> (Document, novo_tests::motor_starter::MotorStarter) {
    let mut doc = Document::default();
    let m = novo_tests::motor_starter::motor_starter(&mut doc).unwrap();
    (doc, m)
}

fn redline(target: Target, change: Change, at: &str) -> Redline {
    Redline {
        id: 0,
        target,
        label: "W1".into(),
        change,
        note: String::new(),
        by: "tv".into(),
        at: at.into(),
        state: RedlineState::Open,
    }
}

#[test]
fn a_change_must_fit_its_object() {
    let (_, m) = starter();
    let mut store = CommissioningStore::new();
    let wire = Target::Wire(m.motor_wires[0]);
    assert!(
        store
            .add_redline(redline(wire, Change::DeviceTag("-Q9".into()), "t"))
            .is_err()
    );
    let id = store
        .add_redline(redline(wire, Change::WireNumber("7".into()), "t"))
        .unwrap();
    assert_eq!(store.open_redline_count(), 1);
    store.set_redline_state(id, RedlineState::Applied).unwrap();
    assert_eq!(store.open_redline_count(), 0);
    let back = CommissioningStore::from_json(&store.to_json()).unwrap();
    assert_eq!(back, store);
}

#[test]
fn open_edits_become_commands_that_change_the_drawing() {
    let (mut doc, m) = starter();
    let mut store = CommissioningStore::new();
    store
        .add_redline(redline(
            Target::Wire(m.motor_wires[0]),
            Change::WireNumber("77".into()),
            "t1",
        ))
        .unwrap();
    store
        .add_redline(redline(
            Target::Device(m.f3),
            Change::DeviceTag("-F33".into()),
            "t2",
        ))
        .unwrap();
    store
        .add_redline(redline(Target::Device(m.f3), Change::Note, "t3"))
        .unwrap();
    let commands = apply_commands(store.redlines(), doc.project());
    assert_eq!(commands.len(), 2, "a note has nothing to apply");
    for (_, c) in commands {
        doc.execute(c.unwrap()).unwrap();
    }
    assert_eq!(
        doc.project()
            .wire(m.motor_wires[0])
            .unwrap()
            .number
            .as_deref(),
        Some("77")
    );
    assert_eq!(doc.project().device(m.f3).unwrap().tag.to_string(), "-F33");
}

#[test]
fn a_bad_tag_or_a_deleted_object_is_reported_not_skipped() {
    let (mut doc, m) = starter();
    let mut store = CommissioningStore::new();
    store
        .add_redline(redline(
            Target::Device(m.f3),
            Change::DeviceTag("???".into()),
            "t",
        ))
        .unwrap();
    store
        .add_redline(redline(
            Target::Wire(m.motor_wires[1]),
            Change::WireNumber("8".into()),
            "t",
        ))
        .unwrap();
    doc.execute(Command::Remove {
        id: m.motor_wires[1].into(),
    })
    .unwrap();
    let commands = apply_commands(store.redlines(), doc.project());
    assert_eq!(commands.len(), 2);
    assert!(commands.iter().all(|(_, c)| c.is_err()));
}

#[test]
fn the_revision_text_lists_every_redline() {
    let mut store = CommissioningStore::new();
    let (_, m) = starter();
    store
        .add_redline(redline(
            Target::Wire(m.motor_wires[0]),
            Change::WireNumber("77".into()),
            "t",
        ))
        .unwrap();
    let text = revision_description(store.redlines());
    assert_eq!(text, "As built: Wire W1 renumbered 77 (tv)");
}

#[test]
fn redlines_travel_in_a_field_file_and_the_further_state_wins() {
    let (doc, m) = starter();
    let target = Target::Wire(m.motor_wires[0]);
    let mut field = CommissioningStore::new();
    let id = field
        .add_redline(redline(target, Change::WireNumber("5".into()), "t"))
        .unwrap();
    let package = FieldPackage::new("A", "tv", "t", &field, doc.project());
    let mut desk = CommissioningStore::new();
    let first = package.merge_into(&mut desk, doc.project());
    assert_eq!(first.redlines_added, 1);
    // merging again changes nothing
    assert!(!package.merge_into(&mut desk, doc.project()).changed());
    // the desk applies it; a later copy still open does not reopen it
    desk.set_redline_state(1, RedlineState::Applied).unwrap();
    assert!(!package.merge_into(&mut desk, doc.project()).changed());
    assert_eq!(desk.redline(1).unwrap().state, RedlineState::Applied);
    // the other way: the field applied or rejected it
    field.set_redline_state(id, RedlineState::Rejected).unwrap();
    let package = FieldPackage::new("A", "tv", "t", &field, doc.project());
    let mut other = CommissioningStore::new();
    package.merge_into(&mut other, doc.project());
    let open = FieldPackage::new(
        "A",
        "tv",
        "t",
        &{
            let mut s = CommissioningStore::new();
            s.add_redline(redline(target, Change::WireNumber("5".into()), "t"))
                .unwrap();
            s
        },
        doc.project(),
    );
    let report = package.merge_into(&mut desk.clone(), doc.project());
    assert_eq!(report.redlines_updated, 0, "applied stays applied");
    let mut fresh = CommissioningStore::new();
    open.merge_into(&mut fresh, doc.project());
    let report = package.merge_into(&mut fresh, doc.project());
    assert_eq!(report.redlines_updated, 1);
    assert_eq!(fresh.redline(1).unwrap().state, RedlineState::Rejected);
}

/// A document with a strip -X1 holding terminals "1" and "2".
fn strip_with_two_terminals() -> (
    Document,
    novo_core::DeviceId,
    novo_core::TerminalId,
    novo_core::TerminalId,
) {
    let mut doc = Document::default();
    let strip = doc
        .execute(Command::AddDevice {
            tag: "-X1".parse().unwrap(),
            description: String::new(),
            kind: novo_core::DeviceKind::TerminalStrip,
        })
        .unwrap()
        .created[0];
    let strip = match strip {
        novo_core::AnyId::Device(d) => d,
        other => panic!("{other:?}"),
    };
    let mut ids = Vec::new();
    for name in ["1", "2"] {
        let out = doc
            .execute(Command::AddTerminal {
                strip,
                name: name.into(),
            })
            .unwrap();
        match out.created[0] {
            novo_core::AnyId::Terminal(t) => ids.push(t),
            other => panic!("{other:?}"),
        }
    }
    (doc, strip, ids[0], ids[1])
}

#[test]
fn a_terminal_rename_changes_its_designation_in_one_command() {
    let (mut doc, strip, one, _) = strip_with_two_terminals();
    let mut store = CommissioningStore::new();
    store
        .add_redline(redline(
            Target::Terminal(one),
            Change::TerminalName("11".into()),
            "t",
        ))
        .unwrap();
    // only a terminal can be renamed to a designation
    assert!(
        store
            .add_redline(redline(
                Target::Device(strip),
                Change::TerminalName("9".into()),
                "t"
            ))
            .is_err()
    );
    let commands = apply_commands(store.redlines(), doc.project());
    assert_eq!(commands.len(), 1);
    doc.execute(commands.into_iter().next().unwrap().1.unwrap())
        .unwrap();
    assert_eq!(doc.project().terminal(one).unwrap().name, "11");
    let text = revision_description(store.redlines());
    assert!(text.contains("renamed 11"), "{text}");
}

#[test]
fn a_rename_to_a_name_the_strip_has_or_to_nothing_is_reported() {
    let (doc, _, one, _) = strip_with_two_terminals();
    let mut store = CommissioningStore::new();
    for name in ["2", "  "] {
        store
            .add_redline(redline(
                Target::Terminal(one),
                Change::TerminalName(name.into()),
                name,
            ))
            .unwrap();
    }
    let commands = apply_commands(store.redlines(), doc.project());
    assert_eq!(commands.len(), 2);
    assert!(commands.iter().all(|(_, c)| c.is_err()));
    let reasons: Vec<String> = commands.into_iter().map(|(_, c)| c.unwrap_err()).collect();
    assert!(reasons[0].contains("already has"), "{reasons:?}");
    assert!(reasons[1].contains("designation"), "{reasons:?}");
}
