//! EMC findings as design-check problems, the waiver key and the family in
//! the standards document (novo-emc is tested on its own).

use novo_checks::standards::{self, document};
use novo_checks::{Problem, ProblemKind, Severity, check, check_open, waive};
use novo_core::*;

fn tag(t: &str) -> DeviceTag {
    DeviceTag::parse(t).unwrap()
}

/// A drive at the left and a motor at the right, an analog module and a
/// transmitter: both wires run the 760 mm of one duct.
fn cabinet() -> Project {
    let mut project = Project::new("EMC");
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            let cab = tx.add_structure(tag("+CAB"), "", true)?;
            tx.set_plate(
                cab,
                Some(MountingPlate {
                    width: 800.0,
                    height: 400.0,
                    margin: 10.0,
                }),
            )?;
            let duct = tx.add_rail(cab, 20.0, 300.0, 760.0, RailKind::Duct)?;
            tx.set_rail_size(duct, 760.0, 60.0, 60.0)?;
            let mut place = |t: &str, key: &str, x: f64| -> Result<SymbolId> {
                let d = tx.add_device(tag(t), "")?;
                tx.set_property(d, "width_mm", Some(Value::Float(60.0)))?;
                tx.set_property(d, "height_mm", Some(Value::Float(80.0)))?;
                tx.place_device(d, cab, Point::new(x, 190.0), false, None)?;
                tx.place_symbol(
                    page,
                    key,
                    Placement::at(Point::new(x, 10.0)),
                    vec![Pin::new("U", 0.0, 0.0), Pin::new("1", 8.0, 0.0)],
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )
            };
            let drive = place("-T1", "iec/drives/frequency_converter", 40.0)?;
            let motor = place("-M1", "iec/motors/motor_3ph", 700.0)?;
            let plc = place("-A1", "iec/plc/analog_input", 300.0)?;
            let sensor = place("-B1", "iec/sensors/transmitter", 640.0)?;
            tx.connect(Endpoint::pin(drive, 0), Endpoint::pin(motor, 0))?;
            tx.connect(Endpoint::pin(plc, 1), Endpoint::pin(sensor, 1))?;
            Ok(())
        })
        .unwrap();
    project
}

fn emc(problems: &[Problem]) -> Vec<&Problem> {
    problems
        .iter()
        .filter(|p| p.kind == ProblemKind::Emc)
        .collect()
}

#[test]
fn nothing_is_reported_until_the_project_asks_for_it() {
    let project = cabinet();
    assert!(emc(&check(&project)).is_empty());
}

#[test]
fn findings_arrive_as_problems_with_rule_ids_and_suggestions() {
    let mut project = cabinet();
    project
        .transact("on", |tx| novo_emc::project::set_enabled(tx, true))
        .unwrap();
    let problems = check(&project);
    let found = emc(&problems);
    assert!(!found.is_empty(), "{problems:#?}");
    let duct = found
        .iter()
        .find(|p| p.rule == Some("EMC-01"))
        .expect("shared duct");
    assert_eq!(duct.severity, Severity::Warning);
    assert!(
        duct.message.contains("same duct") && duct.message.contains("Suggestion:"),
        "{}",
        duct.message
    );
    assert!(duct.message.contains("(EMC-01, rule)"), "{}", duct.message);
    assert!(duct.at.iter().any(|id| matches!(id, AnyId::Wire(_))));
    // Every EMC problem names a rule of the catalogue.
    for p in &found {
        assert!(
            novo_emc::RULES.iter().any(|r| Some(r.id) == p.rule),
            "{p:?}"
        );
    }
}

#[test]
fn an_emc_finding_can_be_waived_with_a_reason_and_only_that_rule() {
    let mut project = cabinet();
    project
        .transact("on", |tx| novo_emc::project::set_enabled(tx, true))
        .unwrap();
    let problems = check(&project);
    let duct = emc(&problems)
        .into_iter()
        .find(|p| p.rule == Some("EMC-01"))
        .unwrap()
        .clone();
    assert_eq!(duct.waiver_key(), "check.emc.EMC-01");
    let other = emc(&problems)
        .into_iter()
        .filter(|p| p.rule != Some("EMC-01"))
        .count();
    project
        .transact("waive", |tx| {
            waive(tx, &duct, "separate duct is on the order").map(|_| ())
        })
        .unwrap();
    let open = check_open(&project);
    assert!(emc(&open).iter().all(|p| p.rule != Some("EMC-01")));
    assert_eq!(emc(&open).len(), other);
}

#[test]
fn the_standards_document_has_an_emc_row_that_is_honest_when_off_and_on() {
    let mut project = cabinet();
    let doc = |p: &Project| document::build(p, &standards::compliance(p));
    let row_text = |p: &Project| {
        doc(p)
            .design
            .iter()
            .find(|r| r.family.id == "emc")
            .expect("emc family")
            .clone()
    };
    let off = row_text(&project);
    assert_eq!(off.verdict, document::Verdict::NotApplicable, "{off:#?}");
    assert!(
        off.not_checkable
            .iter()
            .any(|t| t.contains("EMC check is off"))
    );
    project
        .transact("on", |tx| novo_emc::project::set_enabled(tx, true))
        .unwrap();
    let on = row_text(&project);
    assert!(on.warnings > 0, "{on:#?}");
    assert!(
        on.evidence.iter().any(|t| t.starts_with("Wire classes:")),
        "{:?}",
        on.evidence
    );
    assert!(
        on.not_checkable
            .iter()
            .any(|t| t.contains("Not checked: Radiated")),
        "{:?}",
        on.not_checkable
    );
    assert!(
        doc(&project)
            .limits
            .iter()
            .any(|t| t.contains("EMC results"))
    );
}
