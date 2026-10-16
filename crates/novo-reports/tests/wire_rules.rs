//! Wire production rules kept as project properties: service loop, end
//! allowances and estimate constants change the cut list, the wire list, the
//! wire cut job file and the routing, and nothing else.

use novo_core::{DeviceTag, Document, MountingPlate, Point, RailKind, Value, Wire};
use novo_reports::{CutList, Report, WireList, wire_cut_jobs};
use novo_routing::WireRules;
use novo_routing::project_rules::{ESTIMATE_MIN_MM, FERRULE_MM, FILL_LIMIT, SLACK_MM, set_rule};
use novo_tests::{MotorStarter, motor_starter};

fn cabinet() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture builds");
    doc.transact("Cabinet", |tx| {
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
        let duct = tx.add_rail(cab, 50.0, 250.0, 700.0, RailKind::Duct)?;
        tx.set_rail_size(duct, 700.0, 50.0, 60.0)?;
        Ok(())
    })
    .expect("layout");
    let wires: Vec<_> = doc.project().wires().map(|w| w.id).collect();
    doc.transact("Sizes", |tx| {
        for id in wires {
            tx.update::<Wire>(id, |w| {
                w.cross_section_mm2 = Some(1.5);
                w.color = Some("BK".into());
            })?;
        }
        Ok(())
    })
    .expect("sizes");
    (doc, h)
}

fn set(doc: &mut Document, key: &str, value: Option<f64>) {
    doc.transact("Wire rule", |tx| set_rule(tx, key, value))
        .expect("rule");
}

fn routed(doc: &Document) -> novo_reports::WireCutJob {
    wire_cut_jobs(doc.project())
        .into_iter()
        .find(|j| j.end_a.designation == "=A1+CAB-F1:2" && j.end_b.designation == "=A1+CAB-K1:1")
        .expect("F1 to K1")
}

#[test]
fn a_project_without_rules_cuts_with_the_defaults() {
    let (doc, _) = cabinet();
    assert_eq!(WireRules::of(doc.project()), WireRules::default());
    let job = routed(&doc);
    assert_eq!((job.length_mm, job.cut_mm), (670.0, 690.0));
}

#[test]
fn the_service_loop_changes_the_routed_length_at_both_ends() {
    let (mut doc, _) = cabinet();
    set(&mut doc, SLACK_MM, Some(80.0));
    let job = routed(&doc);
    // 30 mm more at each end than the default 50.
    assert_eq!(job.length_mm, 730.0);
    assert_eq!(job.cut_mm, 750.0);
    // The wire list shows the same length: one rule for both.
    let list = WireList.generate(doc.project());
    let section = list.sections.first().unwrap();
    let col = section.column("length_mm").unwrap();
    assert!(
        section
            .rows
            .iter()
            .any(|r| r.cells[col].to_string().trim_end_matches(".0") == "730"),
        "{:?}",
        section.text_rows()
    );
}

#[test]
fn the_end_allowance_is_the_rule_in_the_job_file_and_the_cut_list() {
    let (mut doc, _) = cabinet();
    set(&mut doc, FERRULE_MM, Some(12.0));
    assert_eq!(CutList::of(doc.project()).ferrule_mm, 12.0);
    let job = routed(&doc);
    assert_eq!(job.cut_mm, job.length_mm + 24.0);
    let cuts = CutList::default().generate(doc.project());
    let table = &cuts.sections[1];
    let ends = table.column("allowance_mm").unwrap();
    assert!(
        table
            .rows
            .iter()
            .all(|r| r.cells[ends].to_string().trim_end_matches(".0") == "24")
    );
}

#[test]
fn a_cut_list_with_its_own_allowances_keeps_them() {
    let (mut doc, _) = cabinet();
    set(&mut doc, FERRULE_MM, Some(12.0));
    let own = CutList {
        ferrule_mm: 15.0,
        ..CutList::default()
    };
    let out = own.generate(doc.project());
    let table = &out.sections[1];
    let ends = table.column("allowance_mm").unwrap();
    assert!(
        table
            .rows
            .iter()
            .all(|r| r.cells[ends].to_string().trim_end_matches(".0") == "30")
    );
}

#[test]
fn a_rule_back_at_its_default_leaves_no_property_behind() {
    let (mut doc, _) = cabinet();
    set(&mut doc, SLACK_MM, Some(80.0));
    assert!(doc.project().info().properties.contains_key(SLACK_MM));
    set(&mut doc, SLACK_MM, Some(50.0));
    assert!(!doc.project().info().properties.contains_key(SLACK_MM));
    set(&mut doc, SLACK_MM, Some(80.0));
    set(&mut doc, SLACK_MM, None);
    assert!(!doc.project().info().properties.contains_key(SLACK_MM));
}

#[test]
fn a_bad_value_is_refused_and_a_bad_file_value_reads_as_the_default() {
    let (mut doc, _) = cabinet();
    assert!(
        doc.transact("Wire rule", |tx| set_rule(tx, FILL_LIMIT, Some(2.0)))
            .is_err()
    );
    assert!(
        doc.transact("Wire rule", |tx| set_rule(tx, SLACK_MM, Some(f64::NAN)))
            .is_err()
    );
    assert!(
        doc.transact("Wire rule", |tx| set_rule(tx, "wire.nonsense", Some(1.0)))
            .is_err()
    );
    // A hand-edited file: text, a negative number.
    doc.transact("Edit", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(SLACK_MM.into(), Value::Text("lots".into()));
            i.properties.insert(FERRULE_MM.into(), Value::Float(-5.0));
        })
    })
    .unwrap();
    let rules = WireRules::of(doc.project());
    assert_eq!(rules.routing.slack_mm, 50.0);
    assert_eq!(rules.ferrule_mm, 10.0);
    assert_eq!(rules.ignored, vec![SLACK_MM, FERRULE_MM]);
    // Decimal comma text is read.
    doc.transact("Edit", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(SLACK_MM.into(), Value::Text("62,5".into()));
        })
    })
    .unwrap();
    assert_eq!(WireRules::of(doc.project()).routing.slack_mm, 62.5);
}

#[test]
fn the_estimate_constants_are_rules_for_unplaced_devices() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    // Nothing is placed on a plate: every length is an estimate.
    let before = wire_cut_jobs(doc.project());
    assert!(before.iter().all(|j| j.basis == "estimate"));
    set(&mut doc, SLACK_MM, Some(0.0));
    set(&mut doc, ESTIMATE_MIN_MM, Some(300.0));
    let after = wire_cut_jobs(doc.project());
    assert_eq!(before.len(), after.len());
    // Short schematic distances are lifted to the new minimum, with no loop.
    assert!(after.iter().all(|j| j.length_mm >= 300.0), "{after:?}");
    assert!(after.iter().any(|j| j.length_mm == 300.0), "{after:?}");
    assert!(before.iter().all(|j| j.length_mm >= 200.0));
}

#[test]
fn a_wire_takes_its_duct_diameter_from_its_cable_core_too() {
    use novo_routing::model::{wire_connections, wire_diameter_mm};
    let (doc, _) = cabinet();
    // 1.5 mm2 is a 3 mm wire, the figure every screen fills ducts with.
    let d = wire_diameter_mm(Some(1.5)).unwrap();
    assert!((d - 3.0).abs() < 0.05, "{d}");
    assert_eq!(wire_diameter_mm(Some(0.0)), None);
    assert_eq!(wire_diameter_mm(Some(f64::NAN)), None);
    assert_eq!(wire_diameter_mm(None), None);
    assert!(
        wire_connections(doc.project())
            .iter()
            .all(|c| c.diameter_mm == Some(d))
    );
}
