//! The continuity and insulation test sheet: order, expected values,
//! readings, misses as punch items, the dead wire cross-check.

use novo_commissioning::*;
use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, StructureId};
use novo_tests::motor_starter::{MotorStarter, motor_starter};

fn starter_in_cabinet() -> Document {
    let mut doc = Document::default();
    let h: MotorStarter = motor_starter(&mut doc).expect("fixture builds");
    let _: StructureId = doc
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
            let r1 = tx.add_rail(cab, 50.0, 100.0, 400.0, RailKind::DinRail)?;
            let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
            tx.place_device(h.f1, cab, Point::new(100.0, 100.0), false, Some((r1, 50.0)))?;
            tx.place_device(
                h.k1,
                cab,
                Point::new(200.0, 100.0),
                false,
                Some((r1, 150.0)),
            )?;
            tx.place_device(h.f2, cab, Point::new(80.0, 400.0), true, None)?;
            Ok(cab)
        })
        .expect("layout")
        .0;
    doc
}

fn sheet_of(doc: &Document, settings: TestSettings) -> TestSheet {
    let p = doc.project();
    TestSheet::of(p, &BuildPlan::of(p), &WirePlan::of(p), settings)
}

const AT: &str = "2026-10-27T08:00:00Z";

#[test]
fn every_drawn_wire_is_a_test_point_once_in_wiring_order() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    assert_eq!(sheet.points.len(), doc.project().wires().count());
    let seqs: Vec<usize> = sheet.points.iter().map(|p| p.seq).collect();
    assert_eq!(seqs, (1..=sheet.points.len()).collect::<Vec<_>>());
    let mut ids: Vec<_> = sheet.points.iter().map(|p| p.wire).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), sheet.points.len());
}

#[test]
fn the_order_follows_the_build_mode_wire_steps_then_the_rest() {
    let doc = starter_in_cabinet();
    let p = doc.project();
    let build = BuildPlan::of(p);
    let order = wiring_order(p, &build);
    // A wire named by two steps is tested once, at its first step.
    let mut stepped: Vec<novo_core::WireId> = Vec::new();
    for w in build.cabinets[0]
        .steps
        .iter()
        .filter(|s| s.phase == BuildPhase::Wires)
        .flat_map(|s| s.wires.iter().copied())
    {
        if !stepped.contains(&w) {
            stepped.push(w);
        }
    }
    assert!(!stepped.is_empty(), "fixture has cabinet wire steps");
    let head: Vec<_> = order.iter().take(stepped.len()).map(|(_, w)| *w).collect();
    assert_eq!(head, stepped);
    assert!(
        order[..stepped.len()]
            .iter()
            .all(|(c, _)| c.as_deref() == Some("+CAB"))
    );
}

#[test]
fn a_project_without_cabinet_still_lists_its_wires_without_a_cabinet() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let sheet = sheet_of(&doc, TestSettings::default());
    assert!(!sheet.points.is_empty());
    assert!(sheet.points.iter().all(|p| p.cabinet.is_none()));
}

#[test]
fn the_limit_is_calculated_from_length_and_section_with_margin_and_contact() {
    let s = TestSettings::default();
    let e = Expected::of(Circuit::Control, Some(2000.0), Some(1.5), &s);
    let want = 0.0178 * 2.0 / 1.5 * 1.25 + 0.05;
    assert!((e.max_ohm - want).abs() < 1e-12);
    assert_eq!(e.basis, Basis::Calculated);
    assert_eq!(e.continuity_text(), format!("<= {want:.2} ohm"));
}

#[test]
fn without_length_the_fallback_limit_says_it_is_assumed() {
    let s = TestSettings::default();
    let e = Expected::of(Circuit::Control, None, Some(1.5), &s);
    assert_eq!(e.basis, Basis::Fallback);
    assert_eq!(e.max_ohm, s.fallback_ohm);
    assert!(e.continuity_text().ends_with("(assumed)"));
}

#[test]
fn earth_wires_ask_no_insulation_reading_and_the_setting_turns_it_off() {
    let s = TestSettings::default();
    assert!(
        Expected::of(Circuit::Earth, Some(500.0), Some(2.5), &s)
            .insulation
            .is_none()
    );
    assert_eq!(
        Expected::of(Circuit::Power, Some(500.0), Some(2.5), &s).insulation_text(),
        "500 V >= 1 Mohm"
    );
    let off = TestSettings {
        insulation: false,
        ..s
    };
    assert!(
        Expected::of(Circuit::Power, Some(500.0), Some(2.5), &off)
            .insulation
            .is_none()
    );
}

#[test]
fn judge_passes_at_the_limit_and_misses_open_high_and_low() {
    let e = Expected {
        max_ohm: 0.3,
        basis: Basis::Calculated,
        insulation: Some((500, 1.0)),
    };
    let v = |c, i| {
        judge(
            &e,
            &ReadingInput {
                continuity: c,
                insulation_mohm: i,
            },
        )
    };
    assert_eq!(
        v(Some(ContinuityInput::Ohm(0.3)), Some(1.0)).continuity,
        Some(Verdict::Pass)
    );
    assert_eq!(
        v(Some(ContinuityInput::Ohm(0.3)), Some(1.0)).insulation,
        Some(Verdict::Pass)
    );
    assert_eq!(
        v(Some(ContinuityInput::Ohm(0.31)), None).continuity,
        Some(Verdict::Miss)
    );
    assert_eq!(
        v(Some(ContinuityInput::Open), None).continuity,
        Some(Verdict::Miss)
    );
    assert_eq!(v(None, Some(0.9)).insulation, Some(Verdict::Miss));
    assert_eq!(v(None, None).continuity, None);
}

#[test]
fn a_pass_ticks_the_commissioning_continuity_check_with_the_value() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = &sheet.points[0];
    let mut store = CommissioningStore::new();
    let r = record_reading(
        &mut store,
        pt,
        ReadingInput {
            continuity: Some(ContinuityInput::Ohm(0.01)),
            insulation_mohm: None,
        },
        "Tadjiro",
        AT,
    );
    assert_eq!(r.verdicts.continuity, Some(Verdict::Pass));
    assert_eq!(r.punch, [None, None]);
    let rec = store
        .record(Target::Wire(pt.wire), CheckKind::Continuity)
        .unwrap();
    assert_eq!(rec.state, CheckState::Passed);
    assert!(rec.value.starts_with("0.01 ohm (max "));
    assert_eq!(store.open_punch_count(), 0);
}

#[test]
fn a_continuity_miss_opens_one_punch_item_and_feeds_commissioning_progress() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = &sheet.points[1];
    let mut store = CommissioningStore::new();
    let input = ReadingInput {
        continuity: Some(ContinuityInput::Open),
        insulation_mohm: None,
    };
    let r = record_reading(&mut store, pt, input, "T", AT);
    let id = r.punch[0].expect("punch opened");
    let item = store.punch(id).unwrap();
    assert_eq!(item.target, Some(Target::Wire(pt.wire)));
    assert_eq!(item.check, Some(CheckKind::Continuity));
    assert!(item.note.contains(&format!("#{}", pt.seq)));
    // Measuring it again while the item is open does not open a second one.
    let again = record_reading(&mut store, pt, input, "T", AT);
    assert_eq!(again.punch[0], None);
    assert_eq!(store.open_punch_count(), 1);
    assert_eq!(
        store.state(Target::Wire(pt.wire), CheckKind::Continuity),
        CheckState::Failed
    );
}

#[test]
fn an_insulation_miss_opens_a_blocking_punch_item_once() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = sheet
        .points
        .iter()
        .find(|p| p.expected.insulation.is_some())
        .expect("a power or control wire");
    let mut store = CommissioningStore::new();
    let input = ReadingInput {
        continuity: None,
        insulation_mohm: Some(0.2),
    };
    let r = record_reading(&mut store, pt, input, "T", AT);
    let item = store.punch(r.punch[1].expect("opened")).unwrap();
    assert_eq!(item.severity, Severity::Blocking);
    assert_eq!(item.check, None);
    assert!(item.note.contains("0.20 Mohm at 500 V"));
    assert_eq!(point_state(&store, pt), PointState::Miss);
    let again = record_reading(&mut store, pt, input, "T", AT);
    assert_eq!(again.punch[1], None);
    assert_eq!(store.open_punch_count(), 1);
}

#[test]
fn non_finite_input_is_not_a_reading() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = &sheet.points[0];
    let mut store = CommissioningStore::new();
    record_reading(
        &mut store,
        pt,
        ReadingInput {
            continuity: Some(ContinuityInput::Ohm(f64::NAN)),
            insulation_mohm: Some(f64::INFINITY),
        },
        "T",
        AT,
    );
    assert!(store.is_empty());
}

#[test]
fn dead_wire_cross_check_lists_unmeasured_wires_in_order_and_shrinks() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(
        &doc,
        TestSettings {
            insulation: false,
            ..Default::default()
        },
    );
    let mut store = CommissioningStore::new();
    assert_eq!(untested(&sheet, &store).len(), sheet.points.len());
    let ok = ReadingInput {
        continuity: Some(ContinuityInput::Ohm(0.0)),
        insulation_mohm: None,
    };
    record_reading(&mut store, &sheet.points[0], ok, "T", AT);
    let dead = untested(&sheet, &store);
    assert_eq!(dead.len(), sheet.points.len() - 1);
    assert_eq!(dead[0].seq, 2);
    let p = test_progress(&sheet, &store);
    assert_eq!(
        (p.complete, p.not_tested, p.miss),
        (1, sheet.points.len() - 1, 0)
    );
}

#[test]
fn a_pass_with_insulation_asked_but_not_read_is_only_part_done() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = sheet
        .points
        .iter()
        .find(|p| p.expected.insulation.is_some())
        .unwrap();
    let mut store = CommissioningStore::new();
    let c = ReadingInput {
        continuity: Some(ContinuityInput::Ohm(0.0)),
        insulation_mohm: None,
    };
    record_reading(&mut store, pt, c, "T", AT);
    assert_eq!(point_state(&store, pt), PointState::Passed);
    let i = ReadingInput {
        continuity: None,
        insulation_mohm: Some(50.0),
    };
    record_reading(&mut store, pt, i, "T", AT);
    assert_eq!(point_state(&store, pt), PointState::Complete);
}

#[test]
fn readings_and_settings_survive_the_project_file_and_old_files_still_read() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = sheet
        .points
        .iter()
        .find(|p| p.expected.insulation.is_some())
        .unwrap();
    let mut store = CommissioningStore::new();
    store.set_test_settings(TestSettings {
        insulation_volts: 250,
        ..Default::default()
    });
    let i = ReadingInput {
        continuity: None,
        insulation_mohm: Some(12.5),
    };
    record_reading(&mut store, pt, i, "T", AT);
    let back = CommissioningStore::from_json(&store.to_json()).unwrap();
    assert_eq!(back, store);
    assert_eq!(back.insulation(pt.wire).unwrap().mohm, 12.5);
    assert_eq!(back.test_settings().insulation_volts, 250);
    let old = CommissioningStore::from_json(r#"{"checks":[],"punch":[]}"#).unwrap();
    assert!(old.is_empty());
    assert_eq!(old.test_settings(), TestSettings::default());
}

#[test]
fn deleting_a_wire_drops_its_insulation_reading_on_prune() {
    let mut doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let pt = sheet
        .points
        .iter()
        .find(|p| p.expected.insulation.is_some())
        .unwrap()
        .clone();
    let mut store = CommissioningStore::new();
    let i = ReadingInput {
        continuity: None,
        insulation_mohm: Some(5.0),
    };
    record_reading(&mut store, &pt, i, "T", AT);
    doc.transact("Remove wire", |tx| tx.remove(pt.wire))
        .expect("removed");
    store.prune(doc.project());
    assert!(store.insulation(pt.wire).is_none());
}

#[test]
fn the_printed_sheet_has_expected_values_blank_boxes_the_disclaimer_and_a_dead_wire_list() {
    let doc = starter_in_cabinet();
    let sheet = sheet_of(&doc, TestSettings::default());
    let mut store = CommissioningStore::new();
    let out = test_sheet_report(&sheet, &store, 7);
    assert_eq!(out.report, "test-sheet");
    let first = &out.sections[0];
    assert!(first.title.contains("compliance test record"));
    let rows = first.text_rows();
    assert_eq!(rows.len(), sheet.points.len());
    assert!(rows[0][5].starts_with("<= "));
    assert_eq!(rows[0][6], "[ ]");
    assert_eq!(rows[0][9], "NOT TESTED");
    let dead = out.sections.last().unwrap();
    assert!(dead.title.starts_with("Dead wire cross-check"));
    assert_eq!(dead.text_rows().len(), sheet.points.len());

    let ok = ReadingInput {
        continuity: Some(ContinuityInput::Ohm(0.02)),
        insulation_mohm: Some(99.0),
    };
    record_reading(&mut store, &sheet.points[0], ok, "Tadjiro", AT);
    let out = test_sheet_report(&sheet, &store, 7);
    let rows = out.sections[0].text_rows();
    assert!(rows[0][6].starts_with("0.02 ohm"));
    assert_eq!(rows[0][8], "99.00");
    assert_eq!(rows[0][9], "OK");
    assert!(rows[0][10].starts_with("Tadjiro 10-27 08:00"));
    assert_eq!(
        out.sections.last().unwrap().text_rows().len(),
        sheet.points.len() - 1
    );
    // Same state, same sheet.
    assert_eq!(out, test_sheet_report(&sheet, &store, 7));
}
