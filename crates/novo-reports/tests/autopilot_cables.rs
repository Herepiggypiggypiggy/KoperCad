//! Autopilot cable rows on the Gate 1 motor starter without its hand-made cable.

use std::collections::BTreeSet;

use novo_checks::calc::tables::TableSet;
use novo_core::{Command, Document};
use novo_reports::autopilot::{Change, plan_for};
use novo_tests::motor_starter;

fn without_cable() -> (Document, novo_tests::MotorStarter) {
    let mut doc = Document::default();
    let m = motor_starter(&mut doc).unwrap();
    doc.execute(Command::Remove { id: m.w1.into() }).unwrap();
    (doc, m)
}

#[test]
fn the_motor_wires_get_one_cable_row_that_applies_after_the_sizes() {
    let (mut doc, m) = without_cable();
    let plan = plan_for(doc.project(), &TableSet::builtin());
    let cables: Vec<_> = plan
        .proposals
        .iter()
        .filter(|p| matches!(p.change, Change::Cable(_)))
        .collect();
    assert_eq!(cables.len(), 1, "{:?}", plan.proposals);
    let row = cables[0];
    assert_eq!(row.wires, m.motor_wires);
    assert!(row.reason.contains("leave the cabinet"), "{}", row.reason);

    let depth = doc.undo_depth();
    let (counts, _) = doc
        .transact("Autopilot", |tx| plan.apply(tx, None))
        .unwrap();
    assert_eq!(counts.3, 1);
    assert_eq!(doc.undo_depth(), depth + 1, "one undo step");
    let cable = doc
        .project()
        .cable_of_wire(m.motor_wires[0])
        .expect("on a core");
    let on_cable: BTreeSet<_> = doc
        .project()
        .cable(cable)
        .unwrap()
        .cores
        .iter()
        .filter_map(|c| c.wire)
        .collect();
    assert_eq!(on_cable, m.motor_wires.iter().copied().collect());

    // Unticking the row leaves the wires without a cable.
    let (mut doc, m) = without_cable();
    let plan = plan_for(doc.project(), &TableSet::builtin());
    let keep: BTreeSet<usize> = plan
        .proposals
        .iter()
        .filter(|p| !matches!(p.change, Change::Cable(_)))
        .map(|p| p.id)
        .collect();
    doc.transact("Autopilot", |tx| plan.apply(tx, Some(&keep)))
        .unwrap();
    assert!(doc.project().cable_of_wire(m.motor_wires[0]).is_none());
}
