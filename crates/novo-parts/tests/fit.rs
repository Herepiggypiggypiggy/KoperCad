//! Part suggestions rank by the kind of device, not only its letter code,
//! and the match percentage says something about each row.

use novo_core::{Document, Potential};
use novo_parts::{Catalog, Role, device_role, part_role, suggestions_for_device};
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter, Catalog) {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    (doc, gate, Catalog::starter())
}

fn roles(doc: &Document, catalog: &Catalog, device: novo_core::DeviceId) -> Vec<Option<Role>> {
    suggestions_for_device(catalog, doc.project(), device, "", 50)
        .iter()
        .map(|s| part_role(s.part))
        .collect()
}

#[test]
fn the_device_kind_comes_from_its_symbol() {
    let (doc, gate, _) = starter();
    let p = doc.project();
    assert_eq!(device_role(p, gate.k1), Some(Role::Contactor));
    assert_eq!(device_role(p, gate.f2), Some(Role::Overload));
    assert_eq!(device_role(p, gate.m1), Some(Role::Motor));
    assert_eq!(device_role(p, gate.s0), Some(Role::PushNc));
    assert_eq!(device_role(p, gate.s1), Some(Role::PushNo));
}

#[test]
fn an_overload_is_offered_overload_relays_only() {
    let (doc, gate, catalog) = starter();
    let kinds = roles(&doc, &catalog, gate.f2);
    assert!(!kinds.is_empty());
    assert!(
        kinds.iter().all(|r| *r == Some(Role::Overload)),
        "{kinds:?}"
    );
}

#[test]
fn a_breaker_is_not_offered_a_fuse_holder_or_an_overload() {
    let (doc, gate, catalog) = starter();
    let kinds = roles(&doc, &catalog, gate.f3);
    assert!(
        kinds
            .iter()
            .all(|r| matches!(r, Some(Role::Mcb | Role::Mpcb))),
        "{kinds:?}"
    );
    // An MCB comes before a motor protection breaker.
    assert_eq!(kinds[0], Some(Role::Mcb));
}

#[test]
fn a_contactor_gets_contactors_before_contactor_relays() {
    let (doc, gate, catalog) = starter();
    let kinds = roles(&doc, &catalog, gate.k1);
    assert_eq!(kinds[0], Some(Role::Contactor));
    // A contactor relay cannot carry a motor: it comes last.
    let relay = kinds.iter().position(|r| *r == Some(Role::Relay)).unwrap();
    assert_eq!(relay, kinds.len() - 1, "{kinds:?}");
}

#[test]
fn a_stop_button_lists_a_normally_closed_button_first() {
    let (doc, gate, catalog) = starter();
    let stop = suggestions_for_device(&catalog, doc.project(), gate.s0, "", 5);
    assert_eq!(part_role(stop[0].part), Some(Role::PushNc));
    assert!(stop.iter().all(|s| part_role(s.part) == Some(Role::PushNc)));
    let start = suggestions_for_device(&catalog, doc.project(), gate.s1, "", 5);
    assert!(
        start
            .iter()
            .all(|s| part_role(s.part) == Some(Role::PushNo))
    );
}

#[test]
fn the_coil_voltage_of_the_circuit_ranks_the_contactors() {
    let (mut doc, gate, catalog) = starter();
    let l = doc
        .project()
        .potentials()
        .find(|p| p.name == "L")
        .unwrap()
        .id;
    doc.transact("Set L to 230 V", |tx| {
        tx.update::<Potential>(l, |p| p.voltage = Some(230.0))?;
        Ok(())
    })
    .unwrap();
    let rows = suggestions_for_device(&catalog, doc.project(), gate.k1, "", 10);
    let top = &rows[0];
    assert_eq!(
        top.part.data.get("coil_voltage"),
        Some(&novo_core::Value::Text("230 V AC".into()))
    );
    // Every 24 V DC contactor ranks below every 230 V AC one.
    let worst_match = rows
        .iter()
        .filter(|r| r.part.description.contains("230 V AC"))
        .map(|r| r.fit.percent)
        .min()
        .unwrap();
    let best_miss = rows
        .iter()
        .filter(|r| r.part.description.contains("24 V DC"))
        .map(|r| r.fit.percent)
        .max()
        .unwrap();
    assert!(worst_match > best_miss, "{worst_match} vs {best_miss}");
    assert!(top.fit.reasons.iter().any(|r| r.contains("230 V AC")));
}

#[test]
fn the_match_differs_between_rows_and_is_not_the_data_quality() {
    let (doc, gate, catalog) = starter();
    // Searching shows parts of every kind for the letter, each with its own
    // match: the right kind scores above the wrong one.
    let rows = suggestions_for_device(&catalog, doc.project(), gate.f3, "f", 50);
    let best = rows.iter().map(|r| r.fit.percent).max().unwrap();
    let worst = rows.iter().map(|r| r.fit.percent).min().unwrap();
    assert!(best > worst, "{best} {worst}");
    for row in &rows {
        if part_role(row.part) == Some(Role::Mcb) {
            assert_eq!(row.fit.percent, best);
        } else if part_role(row.part) == Some(Role::FuseHolder) {
            assert!(row.fit.percent < 50);
        }
    }
}
