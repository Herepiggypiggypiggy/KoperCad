//! The handover contents page on the Gate 1 motor starter: wire lengths are
//! calculated or estimated, never exact, and the basis is named.

use novo_core::Document;
use novo_reports::handover::{HandoverInput, Truth, handover_summary, truth_rows};
use novo_tests::motor_starter;

#[test]
fn wire_lengths_name_their_basis_and_are_not_exact() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let rows = truth_rows(doc.project(), &HandoverInput::default());
    let wires = rows.iter().find(|r| r.item == "Wire lengths").unwrap();
    assert!(matches!(wires.truth, Truth::Calculated | Truth::Estimate));
    assert!(wires.detail.contains("By basis") || wires.detail.contains("by basis"));
    let issued = rows.iter().find(|r| r.item == "Issued state").unwrap();
    assert_eq!(issued.truth, Truth::Incomplete, "no revision issued yet");
}

#[test]
fn the_title_names_the_revision_state() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let out = handover_summary(doc.project(), &HandoverInput::default());
    assert!(out.title.contains("no revision issued"), "{}", out.title);
}
