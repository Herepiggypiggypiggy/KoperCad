//! Wires that leave the enclosure are routed to its entry point; with a
//! stated cable run the length is exact on both sides (`docs/design/cabinet-entry.md`).

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, StructureId};
use novo_reports::{is_estimate, wire_cut_jobs};
use novo_routing::entry::set_entry_point;
use novo_tests::{MotorStarter, motor_starter};

/// X1 and F1 and K1 on a plate with a duct; the motor M1 is in +FLD.
fn cabinet() -> (Document, MotorStarter, StructureId) {
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
            tx.place_device(h.k1, cab, Point::new(400.0, 100.0), false, None)?;
            tx.place_device(h.x1, cab, Point::new(300.0, 400.0), false, None)?;
            let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
            Ok(cab)
        })
        .expect("layout");
    (doc, h, cab)
}

fn set_run(doc: &mut Document, h: &MotorStarter, metres: Option<f64>) {
    let cable = doc
        .project()
        .cable_of_wire(h.motor_wires[0])
        .expect("the fixture has the motor cable");
    doc.transact("Cable run", |tx| {
        tx.update::<novo_core::Cable>(cable, |c| c.length_m = metres)
    })
    .unwrap();
}

fn motor_job(doc: &Document) -> novo_reports::WireCutJob {
    wire_cut_jobs(doc.project())
        .into_iter()
        .find(|j| j.end_a.designation == "=A1+CAB-X1:1" && j.end_b.designation == "=A1+FLD-M1:U1")
        .expect("the X1 to M1 wire")
}

#[test]
fn a_field_wire_is_an_estimate_until_it_has_an_entry_and_a_cable_run() {
    let (mut doc, h, cab) = cabinet();
    set_run(&mut doc, &h, Some(12.0));
    // No entry point: as before.
    assert_eq!(motor_job(&doc).basis, "estimate");
    // An entry point but no stated run: still the whole-wire estimate.
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 600.0)))
    })
    .unwrap();
    set_run(&mut doc, &h, None);
    let unstated = motor_job(&doc);
    assert_eq!(unstated.basis, "estimate");
    assert!(is_estimate(&unstated.basis));
}

#[test]
fn with_both_the_length_is_the_route_to_the_entry_plus_the_run_plus_a_service_loop() {
    let (mut doc, h, cab) = cabinet();
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 600.0)))
    })
    .unwrap();
    set_run(&mut doc, &h, Some(10.0));
    let ten = motor_job(&doc);
    assert_eq!(ten.basis, "to entry + cable run");
    assert!(!is_estimate(&ten.basis));
    assert!(!ten.ducts.is_empty() || ten.length_mm > 10_000.0);
    set_run(&mut doc, &h, Some(20.0));
    let twenty = motor_job(&doc);
    assert_eq!(
        twenty.length_mm - ten.length_mm,
        10_000.0,
        "the run adds metre for metre"
    );
    // The inside part is routed: moving the entry changes it, not the run.
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(100.0, 600.0)))
    })
    .unwrap();
    let moved = motor_job(&doc);
    assert_ne!(moved.length_mm, twenty.length_mm);
    assert_eq!(moved.basis, "to entry + cable run");
}

#[test]
fn a_supply_feed_is_routed_to_the_entry_but_stays_an_estimate_without_a_stated_outside() {
    let (mut doc, _h, cab) = cabinet();
    doc.transact("Entry", |tx| {
        set_entry_point(tx, cab, Some(novo_routing::Point::new(400.0, 600.0)))
    })
    .unwrap();
    let feed: Vec<_> = wire_cut_jobs(doc.project())
        .into_iter()
        .filter(|j| {
            j.end_b.designation.ends_with("(feed)") || j.end_a.designation.ends_with("(feed)")
        })
        .collect();
    assert!(!feed.is_empty());
    assert!(feed.iter().all(|j| j.basis == "estimate"), "{feed:?}");
}
