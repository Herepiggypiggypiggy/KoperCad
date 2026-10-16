//! The calculation sheet (crate::calc_view) on the Gate 1 motor starter:
//! it names what is missing, then gives numbers and clauses once the
//! inputs are entered.

use novo_checks::calc::circuit::Status;
use novo_checks::calc::tables::TableSet;
use novo_checks::calc_view::{CalcSheet, StarterChoice, calc_sheet, clause};
use novo_core::*;
use novo_tests::{MotorStarter, motor_starter};

fn starter() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).unwrap();
    (doc, ms)
}

fn sheet(doc: &Document) -> CalcSheet {
    calc_sheet(
        doc.project(),
        &doc.project().netlist(),
        StarterChoice::default(),
        &TableSet::builtin(),
    )
}

fn feed(doc: &mut Document, ms: &MotorStarter, section: f64, length: f64) -> WireId {
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
        tx.update::<Wire>(wire, |w| w.cross_section_mm2 = Some(section))?;
        tx.set_property(wire, "length_m", Some(length.into()))?;
        tx.set_property(wire, "load_current_a", Some(12.0.into()))?;
        tx.set_property(ms.f1, "rated_current_a", Some(16.0.into()))?;
        tx.set_property(ms.f1, "breaking_capacity_ka", Some(50.0.into()))
    })
    .unwrap();
    wire
}

#[test]
fn without_a_supply_the_sheet_names_it_and_judges_nothing() {
    let (doc, _) = starter();
    let s = sheet(&doc);
    assert!(s.supply.is_none());
    assert!(
        s.missing
            .iter()
            .any(|m| m.contains("supply.short_circuit_ka"))
    );
    assert!(s.circuits.iter().all(|c| c.report.is_none()));
    assert!(s.planning_values);
}

#[test]
fn a_circuit_with_inputs_gets_rows_with_numbers_and_clauses() {
    let (mut doc, ms) = starter();
    feed(&mut doc, &ms, 2.5, 20.0);
    let s = sheet(&doc);
    assert!(s.supply.as_deref().is_some_and(|t| t.contains("10.0 kA")));
    let c = s
        .circuits
        .iter()
        .find(|c| c.device == ms.f1)
        .expect("the fuse is a circuit");
    let report = c.report.as_ref().expect("judged");
    let overload = report.rows.iter().find(|r| r.title == "Overload").unwrap();
    assert_eq!(overload.status, Status::Pass, "{}", overload.text);
    assert!(overload.limit.is_some());
    assert!(clause("Overload").contains("433"));
    assert!(clause("Disconnection").contains("411"));
    assert!(s.tables.contains("not checked against the standard"));
}

#[test]
fn a_failing_circuit_gets_a_smallest_passing_section() {
    let (mut doc, ms) = starter();
    feed(&mut doc, &ms, 1.5, 150.0);
    let s = sheet(&doc);
    let c = s.circuits.iter().find(|c| c.device == ms.f1).unwrap();
    assert!(c.report.as_ref().unwrap().has_failure());
    match c.smallest_section_mm2 {
        Some(Some(section)) => assert!(section > 1.5, "{section}"),
        other => panic!("expected a section, got {other:?}"),
    }
    assert!(s.failing() >= 1);
}

#[test]
fn a_missing_load_is_named_while_the_device_rating_stands_in() {
    let (mut doc, ms) = starter();
    let wire = feed(&mut doc, &ms, 2.5, 20.0);
    doc.transact("No load", |tx| {
        tx.set_property(wire, "load_current_a", None)
    })
    .unwrap();
    let s = sheet(&doc);
    let c = s.circuits.iter().find(|c| c.device == ms.f1).unwrap();
    assert!(c.missing.iter().any(|m| m.starts_with("load_current_a")));
    assert!(c.report.is_some());
}

#[test]
fn the_panel_rating_counts_components_and_knows_the_available_current() {
    let (mut doc, ms) = starter();
    feed(&mut doc, &ms, 2.5, 20.0);
    doc.transact("SCCR", |tx| {
        tx.set_property(ms.f1, "sccr_ka", Some(50.0.into()))
    })
    .unwrap();
    let s = sheet(&doc);
    assert!(s.panel.components >= 1);
    assert_eq!(s.panel.available_ka.map(|a| a.round()), Some(10.0));
    assert!(s.panel.result.panel_ka.is_some() || !s.panel.result.unknown.is_empty());
}

#[test]
fn a_motor_starter_is_sized_from_the_nameplate_current() {
    let (mut doc, ms) = starter();
    feed(&mut doc, &ms, 2.5, 20.0);
    let wire = ms.motor_wires[0];
    doc.transact("Motor", |tx| {
        tx.set_property(ms.m1, "rated_current_a", Some(9.0.into()))?;
        tx.set_property(wire, "length_m", Some(15.0.into()))
    })
    .unwrap();
    let s = sheet(&doc);
    let m = s.starters.iter().find(|m| m.motor == ms.m1).expect("motor");
    assert!(m.missing.is_empty(), "{:?}", m.missing);
    let proposal = m
        .result
        .as_ref()
        .expect("sized")
        .as_ref()
        .expect("proposal");
    assert!((proposal.motor_current_a - 9.0).abs() < 1e-9);
    assert!(proposal.cable_mm2 >= 1.5);
}

#[test]
fn the_wire_panel_installation_key_is_read_as_the_installation_method() {
    let (mut doc, ms) = starter();
    let wire = feed(&mut doc, &ms, 1.5, 10.0);
    doc.transact("Method", |tx| {
        tx.set_property(wire, "installation_method", Some("C".into()))
    })
    .unwrap();
    let s = sheet(&doc);
    let c = s.circuits.iter().find(|c| c.device == ms.f1).unwrap();
    assert!(
        !c.missing
            .iter()
            .any(|m| m.starts_with("installation_method"))
    );
}

fn drop_percent(s: &CalcSheet, device: DeviceId) -> f64 {
    let c = s.circuits.iter().find(|c| c.device == device).unwrap();
    c.report
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .find(|r| r.title == "Voltage drop")
        .and_then(|r| r.value)
        .unwrap()
}

#[test]
fn a_stated_power_factor_changes_the_voltage_drop() {
    let (mut doc, ms) = starter();
    let wire = feed(&mut doc, &ms, 95.0, 100.0);
    let base = drop_percent(&sheet(&doc), ms.f1);
    doc.transact("pf", |tx| {
        tx.set_property(wire, "power_factor", Some(0.8.into()))
    })
    .unwrap();
    let s = sheet(&doc);
    assert!(drop_percent(&s, ms.f1) > base);
    let c = s.circuits.iter().find(|c| c.device == ms.f1).unwrap();
    let notes = &c.report.as_ref().unwrap().assumptions;
    assert!(!notes.iter().any(|n| n.contains("power factor 1 assumed")));
}

#[test]
fn the_starter_route_reads_ambient_and_grouping_like_the_circuit_rows() {
    let (mut doc, ms) = starter();
    feed(&mut doc, &ms, 2.5, 20.0);
    doc.transact("motor", |tx| {
        tx.set_property(ms.m1, "rated_current_a", Some(9.0.into()))?;
        tx.set_property(ms.motor_wires[0], "length_m", Some(15.0.into()))
    })
    .unwrap();
    let plain = sheet(&doc);
    let m = plain.starters.iter().find(|m| m.motor == ms.m1).unwrap();
    assert!(m.notes.iter().any(|n| n.contains("ambient")));
    doc.transact("hot", |tx| {
        tx.set_property(ms.motor_wires[0], "ambient_c", Some(55.0.into()))?;
        tx.set_property(ms.motor_wires[0], "grouped_circuits", Some(9.0.into()))
    })
    .unwrap();
    let hot = sheet(&doc);
    let m = hot.starters.iter().find(|m| m.motor == ms.m1).unwrap();
    assert!(!m.notes.iter().any(|n| n.contains("ambient")));
    let a = plain.starters.iter().find(|m| m.motor == ms.m1).unwrap();
    let cable = |e: &novo_checks::calc_view::StarterEntry| {
        e.result.as_ref().unwrap().as_ref().unwrap().cable_mm2
    };
    assert!(cable(m) >= cable(a));
}
