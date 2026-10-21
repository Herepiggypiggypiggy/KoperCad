//! Autopilot proposes the cabinet entry point when supply leads come in from
//! outside and the plate has none; the row is assumed, so never ticked by default.

use std::collections::BTreeSet;

use novo_checks::calc::tables::TableSet;
use novo_core::{DeviceTag, Document, MountingPlate, Point, StructureId};
use novo_reports::autopilot::{Basis, Change, plan_for};
use novo_routing::entry::entry_point;
use novo_tests::motor_starter;

fn cabinet() -> (Document, StructureId) {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    let (cab, _) = doc
        .transact("Cabinet", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB")?, "Cabinet", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 600.0,
                    margin: 10.0,
                }),
            )?;
            tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, None)?;
            Ok(cab)
        })
        .expect("layout");
    (doc, cab)
}

#[test]
fn a_plate_with_supply_leads_and_no_entry_gets_one_assumed_row_at_the_bottom_middle() {
    let (mut doc, cab) = cabinet();
    let plan = plan_for(doc.project(), &TableSet::builtin());
    let rows: Vec<_> = plan
        .proposals
        .iter()
        .filter(|p| matches!(p.change, Change::EntryPoint { .. }))
        .collect();
    assert_eq!(rows.len(), 1, "{:?}", plan.proposals);
    let row = rows[0];
    assert_eq!(
        row.basis,
        Basis::Assumed,
        "where the gland is, only the designer knows"
    );
    assert_eq!(
        row.change,
        Change::EntryPoint {
            plate: cab,
            x: 400.0,
            y: 600.0
        }
    );
    // Accepting only that row sets the point, in one undo step.
    let only: BTreeSet<usize> = [row.id].into();
    let depth = doc.undo_depth();
    let (counts, _) = doc
        .transact("Autopilot", |tx| plan.apply(tx, Some(&only)))
        .unwrap();
    assert_eq!(counts.4, 1);
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(
        entry_point(doc.project(), cab),
        Some(novo_routing::Point::new(400.0, 600.0))
    );
    // With an entry point there is nothing left to propose for it.
    let again = plan_for(doc.project(), &TableSet::builtin());
    assert!(
        !again
            .proposals
            .iter()
            .any(|p| matches!(p.change, Change::EntryPoint { .. }))
    );
}
