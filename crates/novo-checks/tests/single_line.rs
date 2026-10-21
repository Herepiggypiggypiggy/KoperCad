//! The single-line model: the tree derived from the netlist, the currents,
//! the cable runs and the figures on them.

use std::collections::BTreeMap;

use novo_checks::calc::Insulation;
use novo_checks::calc::cable::{Loaded, Method};
use novo_checks::calc::circuit::Status;
use novo_checks::calc::drop::{Run, Supply};
use novo_checks::calc::tables::TableSet;
use novo_checks::single_line::{Kind, Origin, SingleLine, single_line};
use novo_core::*;
use novo_tests::{Board, MotorStarter, distribution_board, motor_starter};

fn at(x: f64) -> Placement {
    Placement::at(Point::new(x, 0.0))
}

fn pins(names: &[&str]) -> Vec<Pin> {
    names
        .iter()
        .enumerate()
        .map(|(i, n)| Pin::new(*n, (i as f64) * 8.0, 0.0))
        .collect()
}

fn pole_pins() -> Vec<Pin> {
    pins(&["1", "2", "3", "4", "5", "6"])
}

const LOAD: [u16; 3] = [1, 3, 5];

fn board() -> (Document, Board) {
    let mut doc = Document::default();
    let board = distribution_board(&mut doc).unwrap();
    (doc, board)
}

fn build(doc: &Document) -> SingleLine {
    build_with(doc, &BTreeMap::new())
}

fn build_with(doc: &Document, proposed: &BTreeMap<WireId, f64>) -> SingleLine {
    single_line(
        doc.project(),
        &doc.project().netlist(),
        &TableSet::builtin(),
        proposed,
    )
}

fn node<'a>(
    sl: &'a SingleLine,
    project: &Project,
    device: DeviceId,
) -> &'a novo_checks::single_line::Node {
    let _ = project;
    sl.nodes
        .iter()
        .find(|n| n.device == Some(device))
        .unwrap_or_else(|| panic!("{device:?} is not on the diagram"))
}

fn tags(sl: &SingleLine, ids: &[usize]) -> Vec<String> {
    ids.iter().map(|i| sl.nodes[*i].tag.clone()).collect()
}

/// Voltage drop in percent of a copper run at the PVC operating
/// temperature, worked out here from the resistivity rather than the
/// library's `Run`: `100·√3·I·ρ·L / (S·U)`.
fn three_phase_drop(current: f64, length: f64, section: f64, voltage: f64) -> f64 {
    let rho_70 = 0.017_241 * (1.0 + 0.003_93 * (70.0 - 20.0));
    100.0 * 3.0_f64.sqrt() * current * rho_70 * length / section / voltage
}

#[test]
fn the_tree_follows_the_supply_through_protection_to_the_loads() {
    let (doc, b) = board();
    let sl = build(&doc);
    assert_eq!(sl.nodes[0].kind, Kind::Supply);
    let q1 = node(&sl, doc.project(), b.q1);
    assert_eq!(q1.parent, Some(0));
    assert_eq!(q1.kind, Kind::Breaker);
    assert_eq!(
        tags(&sl, &q1.children),
        ["=A1+CAB-Q2", "=A1+CAB-Q3", "=A1+CAB-Q4"]
    );
    let m1 = node(&sl, doc.project(), b.m1);
    assert_eq!(m1.parent, Some(node(&sl, doc.project(), b.q2).id));
    assert_eq!(m1.kind, Kind::Motor);
    // A motor behind a drive: the drive is a box with the motor under it.
    let drive = node(&sl, doc.project(), b.drive);
    assert_eq!(drive.kind, Kind::Drive);
    assert_eq!(drive.parent, Some(node(&sl, doc.project(), b.q3).id));
    let m2 = node(&sl, doc.project(), b.m2);
    assert_eq!(m2.parent, Some(drive.id));
    assert_eq!(
        node(&sl, doc.project(), b.heater).parent,
        Some(node(&sl, doc.project(), b.q4).id)
    );
    assert_eq!(sl.leaves(), 3);
    assert_eq!(sl.depth(), 4);
    assert!(sl.unfed.is_empty(), "{:?}", sl.unfed);
}

#[test]
fn currents_are_stated_calculated_or_summed_and_say_which() {
    let (doc, b) = board();
    let sl = build(&doc);
    let p = doc.project();
    let m1 = node(&sl, p, b.m1);
    assert_eq!(m1.current_a.value, Some(12.0));
    assert_eq!(m1.current_a.origin, Origin::Stated);
    // 2.2 kW with the typical efficiency and power factor, flagged as such.
    let m2 = node(&sl, p, b.m2);
    assert_eq!(m2.current_a.origin, Origin::Calculated);
    assert!((m2.current_a.value.unwrap() - 4.58).abs() < 0.05);
    assert!(m2.current_basis.contains("typical"));
    // 6 kW three-phase heater: P / (√3 U).
    let heater = node(&sl, p, b.heater);
    assert!((heater.current_a.value.unwrap() - 6000.0 / (3.0_f64.sqrt() * 400.0)).abs() < 1e-9);
    // A drive takes what its motor takes; the breaker the sum of its branches.
    assert_eq!(node(&sl, p, b.drive).current_a.value, m2.current_a.value);
    let q1 = node(&sl, p, b.q1);
    let sum = 12.0 + m2.current_a.value.unwrap() + heater.current_a.value.unwrap();
    assert!((q1.current_a.value.unwrap() - sum).abs() < 1e-9);
    assert_eq!(q1.current_a.origin, Origin::Calculated);
    assert!(q1.current_basis.contains("no diversity"));
    assert!((sl.nodes[0].current_a.value.unwrap() - sum).abs() < 1e-9);
}

#[test]
fn the_drop_over_a_run_is_the_formula_and_adds_up_from_the_supply() {
    let (doc, b) = board();
    let sl = build(&doc);
    let p = doc.project();
    let m1 = node(&sl, p, b.m1);
    let run = m1.link.as_ref().unwrap();
    let expected = three_phase_drop(12.0, 30.0, 2.5, 400.0);
    assert!(
        (run.drop_percent.value.unwrap() - expected).abs() < 1e-6,
        "{:?} vs {expected}",
        run.drop_percent
    );
    // Σ from the supply: supply to Q1 (25 mm², 10 m), Q1 to Q2, Q2 to M1.
    let q1 = node(&sl, p, b.q1);
    let q2 = node(&sl, p, b.q2);
    let q1_current = q1.current_a.value.unwrap();
    let total = three_phase_drop(q1_current, 10.0, 25.0, 400.0)
        + three_phase_drop(12.0, 5.0, 16.0, 400.0)
        + expected;
    assert!(
        (run.drop_total_percent.value.unwrap() - total).abs() < 1e-6,
        "{:?} vs {total}",
        run.drop_total_percent
    );
    assert!(run.total_complete);
    assert_eq!(run.drop_percent.origin, Origin::Calculated);
    // The run into Q2 carries Q2's current, the sum of what is behind it.
    let into_q2 = q2.link.as_ref().unwrap();
    assert_eq!(into_q2.current_a.value, Some(12.0));
    assert_eq!(into_q2.segments.len(), 1);
    assert_eq!(into_q2.length_m.value, Some(5.0));
    assert_eq!(into_q2.section_mm2.value, Some(16.0));
    assert_eq!(into_q2.poles, 3);
    assert!(into_q2.three_phase);
}

#[test]
fn the_cable_rating_is_read_from_the_tables_for_the_weakest_wire() {
    let (doc, b) = board();
    let sl = build(&doc);
    let p = doc.project();
    let run = node(&sl, p, b.m2).link.as_ref().unwrap();
    let tables = TableSet::builtin();
    let iz = tables
        .ampacity(1.5, Insulation::Pvc, Method::C, Loaded::Three)
        .unwrap();
    assert_eq!(run.iz_a.value, Some(iz));
    assert_eq!(run.iz_a.origin, Origin::Calculated);
    assert_eq!(run.method, Method::C);
    // Method C is the default, not something the project said.
    assert_eq!(run.method_origin, Origin::Assumed);
    assert_eq!(run.power_factor.origin, Origin::Assumed);
    assert!(sl.planning_values);
    assert!(sl.tables.contains("not checked against the standard"));
}

#[test]
fn protection_above_a_cable_is_found_and_judged_against_its_rating() {
    let (mut doc, b) = board();
    // 1.5 mm² to a 25 A motor: the load is above what the cable carries, and
    // Q2 (16 A) is no better.
    let wires = b.wires["m1"];
    doc.transact("Thin cable", |tx| {
        for w in wires {
            tx.update::<Wire>(w, |w| w.cross_section_mm2 = Some(1.5))?;
        }
        tx.set_property(b.m1, "rated_current_a", Some(25.0.into()))
    })
    .unwrap();
    let sl = build(&doc);
    let p = doc.project();
    let run = node(&sl, p, b.m1).link.as_ref().unwrap();
    assert_eq!(run.status, Status::Fail);
    assert!(run.fail_codes.contains(&"Ib > Iz"), "{:?}", run.fail_codes);
    assert_eq!(run.protected_by.as_ref().map(|p| p.1), Some(16.0));
    assert!(sl.failing() >= 1);
    assert!(node(&sl, p, b.m1).fails());
    // The rest of the board is not blamed.
    assert!(!node(&sl, p, b.heater).fails());
}

#[test]
fn a_protective_device_above_the_cable_rating_leaves_it_unprotected() {
    let (mut doc, b) = board();
    // Q2 is 16 A over a 1.5 mm² cable (Iz well under 16 A in these tables
    // is not guaranteed, so make the device clearly too large).
    let wires = b.wires["m1"];
    doc.transact("Big breaker", |tx| {
        for w in wires {
            tx.update::<Wire>(w, |w| w.cross_section_mm2 = Some(1.5))?;
        }
        tx.set_property(b.q2, "rated_current_a", Some(63.0.into()))
    })
    .unwrap();
    let sl = build(&doc);
    let run = node(&sl, doc.project(), b.m1).link.as_ref().unwrap();
    assert!(run.fail_codes.contains(&"In > Iz"), "{:?}", run.fail_codes);
}

#[test]
fn a_long_run_fails_the_drop_limit_and_the_limit_is_named() {
    let (mut doc, b) = board();
    let wires = b.wires["m2"];
    doc.transact("Long cable", |tx| {
        for w in wires {
            tx.set_property(w, "length_m", Some(400.0.into()))?;
        }
        Ok(())
    })
    .unwrap();
    let sl = build(&doc);
    let run = node(&sl, doc.project(), b.m2).link.as_ref().unwrap();
    assert!(run.drop_total_percent.value.unwrap() > sl.drop_limit_percent);
    assert!(run.fail_codes.contains(&"ΔU > limit"));
    assert!(run.reasons.iter().any(|r| r.contains("above the limit")));
    // The limit is the default until the project says otherwise.
    assert!(!sl.drop_limit_stated);
    doc.transact("Limit", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert("check.voltage_drop_pct".into(), 20.0.into());
        })
    })
    .unwrap();
    let sl = build(&doc);
    assert!(sl.drop_limit_stated);
    assert_eq!(sl.drop_limit_percent, 20.0);
    let run = node(&sl, doc.project(), b.m2).link.as_ref().unwrap();
    assert!(!run.fail_codes.contains(&"ΔU > limit"));
}

#[test]
fn missing_data_stays_missing_and_is_never_a_pass() {
    let (mut doc, b) = board();
    let wires = b.wires["m1"];
    doc.transact("No length", |tx| {
        for w in wires {
            tx.set_property(w, "length_m", None)?;
        }
        Ok(())
    })
    .unwrap();
    let sl = build(&doc);
    let run = node(&sl, doc.project(), b.m1).link.as_ref().unwrap();
    assert!(run.drop_percent.is_missing());
    assert!(!run.complete);
    assert!(!run.total_complete);
    assert_eq!(run.status, Status::Unknown);
    // The runs above are still complete.
    let q2 = node(&sl, doc.project(), b.q2).link.as_ref().unwrap();
    assert!(q2.complete && q2.total_complete);
    // The runs below it inherit the gap in their total.
    assert!(run.reasons.iter().any(|r| r.contains("no length")));
}

#[test]
fn a_proposed_section_is_used_and_marked_as_proposed() {
    let (mut doc, b) = board();
    let wires = b.wires["m1"];
    doc.transact("No section", |tx| {
        for w in wires {
            tx.update::<Wire>(w, |w| w.cross_section_mm2 = None)?;
        }
        Ok(())
    })
    .unwrap();
    let plain = build(&doc);
    let run = node(&plain, doc.project(), b.m1).link.as_ref().unwrap();
    assert!(run.section_mm2.is_missing());
    assert!(run.iz_a.is_missing());

    let proposed: BTreeMap<WireId, f64> = wires.iter().map(|w| (*w, 4.0)).collect();
    let sl = build_with(&doc, &proposed);
    let run = node(&sl, doc.project(), b.m1).link.as_ref().unwrap();
    assert_eq!(run.section_mm2.value, Some(4.0));
    assert_eq!(run.section_mm2.origin, Origin::Proposed);
    assert!(run.iz_a.value.is_some());
    assert!(
        run.segments
            .iter()
            .all(|s| s.section_mm2.origin == Origin::Proposed)
    );
    let expected = three_phase_drop(12.0, 30.0, 4.0, 400.0);
    assert!((run.drop_percent.value.unwrap() - expected).abs() < 1e-6);
}

#[test]
fn the_supply_states_where_its_fault_level_comes_from() {
    let (mut doc, _) = board();
    let sl = build(&doc);
    assert_eq!(sl.supply_ik_ka.origin, Origin::Stated);
    assert!((sl.supply_ik_ka.value.unwrap() - 10.0).abs() < 0.1);
    assert!(
        sl.supply_text
            .as_deref()
            .is_some_and(|t| t.contains("400 V"))
    );
    doc.transact("No supply", |tx| {
        tx.set_info(|i| {
            i.properties.remove("supply.short_circuit_ka");
        })
    })
    .unwrap();
    let sl = build(&doc);
    assert!(sl.supply_ik_ka.is_missing());
    assert!(
        sl.missing
            .iter()
            .any(|m| m.contains("supply.short_circuit_ka"))
    );
}

#[test]
fn a_protective_device_carries_the_calculations_verdict() {
    let (mut doc, b) = board();
    doc.transact("Ratings", |tx| {
        tx.set_property(b.q2, "breaking_capacity_ka", Some(6.0.into()))?;
        tx.set_property(b.q2, "trip_curve", Some("C".into()))
    })
    .unwrap();
    let sl = build(&doc);
    let q2 = node(&sl, doc.project(), b.q2);
    assert_eq!(q2.breaking_ka.value, Some(6.0));
    assert_eq!(q2.characteristic, Some(("C".to_owned(), Origin::Stated)));
    let verdict = q2
        .verdict
        .as_ref()
        .expect("judged by the Calculations sheet");
    assert!(verdict.ik_max_ka.is_some());
    // A fuse without a stated class is shown as the usual gG, marked assumed.
    let (doc, ms): (Document, MotorStarter) = {
        let mut d = Document::default();
        let ms = motor_starter(&mut d).unwrap();
        (d, ms)
    };
    let sl = build(&doc);
    let f1 = node(&sl, doc.project(), ms.f1);
    assert_eq!(f1.kind, Kind::Fuse);
    assert_eq!(f1.characteristic, Some(("gG".to_owned(), Origin::Assumed)));
}

#[test]
fn a_device_no_supply_reaches_is_listed_and_not_drawn() {
    let (mut doc, _) = board();
    let stray = doc
        .transact("Stray", |tx| {
            let page = tx.add_page("2")?;
            let d = tx.add_device(DeviceTag::parse("=A1+CAB-Q9")?, "Loose breaker")?;
            let s = tx.place_symbol(
                page,
                "iec/protection/mcb_3p",
                at(0.0),
                pole_pins(),
                SymbolRole::Device {
                    device: d,
                    main: true,
                },
            )?;
            tx.update::<SymbolInstance>(s, |s| {
                s.pass_through = vec![vec![0, 1], vec![2, 3], vec![4, 5]]
            })?;
            // A motor on its load side makes the nets power nets.
            let m = tx.add_device(DeviceTag::parse("=A1+FLD-M9")?, "Loose motor")?;
            let ms = tx.place_symbol(
                page,
                "iec/motors/motor_3ph",
                at(40.0),
                pins(&["U1", "V1", "W1"]),
                SymbolRole::Device {
                    device: m,
                    main: true,
                },
            )?;
            for (i, load) in LOAD.iter().enumerate() {
                tx.connect(Endpoint::pin(s, *load), Endpoint::pin(ms, i as u16))?;
            }
            Ok(d)
        })
        .unwrap()
        .0;
    let sl = build(&doc);
    assert!(sl.nodes.iter().all(|n| n.device != Some(stray)));
    assert!(sl.unfed.iter().any(|t| t == "=A1+CAB-Q9"), "{:?}", sl.unfed);
    assert!(sl.notes.iter().any(|n| n.contains("no supply reaches")));
}

#[test]
fn the_overload_relay_of_a_motor_starter_sits_on_the_motor_run() {
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).unwrap();
    let sl = build(&doc);
    let p = doc.project();
    // Supply, F1 fuses, K1 contactor, M1: the overload relay is drawn with
    // bridges, so it is on the run rather than a box of its own.
    assert_eq!(sl.nodes.len(), 4);
    assert_eq!(node(&sl, p, ms.f1).parent, Some(0));
    assert_eq!(node(&sl, p, ms.k1).parent, Some(node(&sl, p, ms.f1).id));
    let m1 = node(&sl, p, ms.m1);
    assert_eq!(m1.parent, Some(node(&sl, p, ms.k1).id));
    let run = m1.link.as_ref().unwrap();
    assert!(
        run.inline.iter().any(|i| i.contains("F2")),
        "{:?}",
        run.inline
    );
    // The run goes through the terminal strip to the motor cable.
    assert!(
        run.segments
            .iter()
            .any(|s| s.cable.as_deref() == Some("=A1-W1"))
    );
    // 2.2 kW, nothing stated: the current is calculated, and so is everything above it.
    assert_eq!(m1.current_a.origin, Origin::Calculated);
}

#[test]
fn a_project_without_a_power_distribution_has_an_empty_diagram() {
    let doc = Document::default();
    let sl = build(&doc);
    assert!(sl.is_empty());
    assert_eq!(sl.nodes.len(), 1);
    assert_eq!(sl.leaves(), 1);
}

#[test]
fn building_twice_gives_the_same_diagram() {
    let (doc, _) = board();
    assert_eq!(build(&doc), build(&doc));
    let _ = (Run::new(1.0, 1.0, 1.0, Supply::ThreePhase, 400.0), 0);
}
