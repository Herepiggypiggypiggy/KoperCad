//! The calculation checks (crate::calc_checks) on the Gate 1 motor starter
//! with numbers added to it: each kind is flagged once when its numbers are
//! wrong, and the clean starter with the same numbers right is quiet.

use novo_checks::{ProblemKind, Severity, check};
use novo_core::*;
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).unwrap();
    (doc, ms)
}

fn of_kind(doc: &Document, kind: ProblemKind) -> Vec<novo_checks::Problem> {
    check(doc.project())
        .into_iter()
        .filter(|p| p.kind == kind)
        .collect()
}

#[test]
fn no_numbers_no_calc_problems() {
    let (doc, _) = starter();
    for kind in [
        ProblemKind::VoltageDrop,
        ProblemKind::ShortCircuit,
        ProblemKind::DisconnectionTime,
        ProblemKind::ThermalWithstand,
        ProblemKind::MotorStarter,
        ProblemKind::ProtectiveConductor,
    ] {
        assert!(of_kind(&doc, kind).is_empty(), "{kind:?}");
    }
}

#[test]
fn long_thin_motor_wire_drops_too_much() {
    let (mut doc, ms) = starter();
    doc.transact("Numbers", |tx| {
        for id in [ms.l1, ms.l2, ms.l3] {
            tx.update::<Potential>(id, |p| p.voltage = Some(400.0))?;
        }
        tx.set_property(ms.motor_wires[0], "length_m", Some(100.0.into()))?;
        tx.set_property(ms.motor_wires[0], "load_current_a", Some(16.0.into()))
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::VoltageDrop);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert!(found[0].message.contains("9.5"), "{}", found[0].message);
    assert_eq!(found[0].at, vec![AnyId::Wire(ms.motor_wires[0])]);

    // Half the length is fine.
    doc.transact("Shorter", |tx| {
        tx.set_property(ms.motor_wires[0], "length_m", Some(50.0.into()))
    })
    .unwrap();
    assert!(of_kind(&doc, ProblemKind::VoltageDrop).is_empty());
}

#[test]
fn device_below_the_supply_short_circuit_current() {
    let (mut doc, ms) = starter();
    doc.transact("Numbers", |tx| {
        tx.set_info(|info| {
            info.properties
                .insert("supply.short_circuit_ka".into(), 10.0.into());
        })?;
        tx.set_property(ms.f1, "breaking_capacity_ka", Some(6.0.into()))?;
        tx.set_property(ms.k1, "breaking_capacity_ka", Some(10.0.into()))
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::ShortCircuit);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Error);
    assert!(found[0].at.contains(&AnyId::Symbol(ms.f1_sym)));
    assert!(found[0].message.contains("10 kA") && found[0].message.contains("6 kA"));
}

#[test]
fn overload_relay_that_cannot_be_set_to_the_motor() {
    let (mut doc, ms) = starter();
    doc.transact("Numbers", |tx| {
        tx.set_property(ms.m1, "rated_current_a", Some(9.0.into()))?;
        tx.set_property(ms.f2, "setting_min_a", Some(4.0.into()))?;
        tx.set_property(ms.f2, "setting_max_a", Some(6.3.into()))
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::MotorStarter);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Error);
    assert!(
        found[0].message.contains("4 to 6.3 A"),
        "{}",
        found[0].message
    );
    assert!(found[0].at.contains(&AnyId::Symbol(ms.m1_sym)));
    assert!(found[0].at.contains(&AnyId::Symbol(ms.f2_main)));

    // A matching range clears it.
    doc.transact("Right relay", |tx| {
        tx.set_property(ms.f2, "setting_min_a", Some(6.0.into()))?;
        tx.set_property(ms.f2, "setting_max_a", Some(10.0.into()))
    })
    .unwrap();
    assert!(of_kind(&doc, ProblemKind::MotorStarter).is_empty());
}

#[test]
fn contactor_and_fuse_are_found_upstream_of_the_motor() {
    let (mut doc, ms) = starter();
    doc.transact("Numbers", |tx| {
        tx.set_property(ms.m1, "rated_current_a", Some(9.0.into()))?;
        tx.set_property(ms.k1, "ac3_current_a", Some(7.0.into()))?;
        tx.set_property(ms.f1, "rated_current_a", Some(32.0.into()))?;
        tx.set_property(ms.k1, "max_scpd_a", Some(25.0.into()))
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::MotorStarter);
    let messages: Vec<&str> = found.iter().map(|p| p.message.as_str()).collect();
    assert!(
        messages.iter().any(|m| m.contains("contactor")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("coordinated up to 25 A")),
        "{messages:?}"
    );
}

#[test]
fn thin_protective_core_in_the_motor_cable() {
    let (mut doc, ms) = starter();
    doc.transact("Numbers", |tx| {
        tx.update::<Cable>(ms.w1, |c| {
            for core in &mut c.cores {
                core.cross_section_mm2 = Some(if core.name == "GNYE" { 0.75 } else { 2.5 });
            }
        })
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::ProtectiveConductor);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].at, vec![AnyId::Cable(ms.w1)]);
}

#[test]
fn let_through_energy_above_what_the_wire_withstands() {
    let (mut doc, ms) = starter();
    let wire = doc
        .project()
        .wires_at(Endpoint::pin(ms.f1_sym, 1))
        .next()
        .expect("wire on the fuse's load side");
    doc.transact("Numbers", |tx| {
        tx.update::<Wire>(wire, |w| w.cross_section_mm2 = Some(1.5))?;
        tx.set_property(ms.f1, "let_through_i2t", Some(100_000.0.into()))
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::ThermalWithstand);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].at.contains(&AnyId::Wire(wire)));
    // 1.5 mm² PVC withstands 115²·1.5² = 29 756 A²s.
    assert!(
        found[0].message.contains("29756.25"),
        "{}",
        found[0].message
    );
}

#[test]
fn long_run_cannot_trip_its_breaker_on_an_earth_fault() {
    let (mut doc, ms) = starter();
    let wire = doc
        .project()
        .wires_at(Endpoint::pin(ms.f1_sym, 1))
        .next()
        .expect("wire on the fuse's load side");
    doc.transact("Numbers", |tx| {
        tx.set_info(|info| {
            info.properties
                .insert("supply.short_circuit_ka".into(), 10.0.into());
        })?;
        tx.update::<Wire>(wire, |w| w.cross_section_mm2 = Some(1.5))?;
        tx.set_property(wire, "length_m", Some(150.0.into()))?;
        tx.set_property(ms.f1, "rated_current_a", Some(16.0.into()))?;
        tx.set_property(ms.f1, "trip_curve", Some("C".into()))
    })
    .unwrap();
    let found = of_kind(&doc, ProblemKind::DisconnectionTime);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(found[0].at.contains(&AnyId::Wire(wire)));

    doc.transact("Short run", |tx| {
        tx.set_property(wire, "length_m", Some(10.0.into()))
    })
    .unwrap();
    assert!(of_kind(&doc, ProblemKind::DisconnectionTime).is_empty());
}
