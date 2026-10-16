//! SAFE-2: safety functions bound to project devices raise Problems when
//! the achieved PL is below the required one, and say what is unbound.

use novo_checks::safety::{
    B10D_CYCLES, Basis, FUNCTIONS_KEY, PartRef, Role, SafetyFunction, encode, functions, rate,
};
use novo_checks::standards::set_info_property;
use novo_checks::{ProblemKind, check};
use novo_core::*;

fn project_with_devices() -> Document {
    let mut doc = Document::default();
    doc.transact("Devices", |tx| {
        let tag = |t: &str| DeviceTag::parse(t).unwrap();
        let s1 = tx.add_device(tag("-S1"), "Emergency stop")?;
        tx.set_property(s1, B10D_CYCLES, Some(Value::Float(1.0e6)))?;
        let k1 = tx.add_device(tag("-K1"), "Contactor")?;
        tx.set_property(k1, B10D_CYCLES, Some(Value::Float(2.0e6)))?;
        tx.add_device(tag("-K9"), "Relay without data")?;
        Ok(())
    })
    .unwrap();
    doc
}

fn store(doc: &mut Document, list: &[SafetyFunction]) {
    doc.transact("Safety functions", |tx| {
        set_info_property(tx, FUNCTIONS_KEY, encode(list))
    })
    .unwrap();
}

fn bound(f: &mut SafetyFunction, role: Role, tag: &str) {
    let block = f.blocks.iter_mut().find(|b| b.role == role).unwrap();
    block.channel1 = vec![PartRef {
        name: tag.into(),
        device: Some(tag.into()),
        basis: Basis::B10d,
        value: None,
    }];
}

fn kinds(doc: &Document) -> Vec<ProblemKind> {
    check(doc.project()).into_iter().map(|p| p.kind).collect()
}

#[test]
fn a_project_without_functions_is_silent() {
    let doc = project_with_devices();
    assert!(!kinds(&doc).iter().any(|k| matches!(
        k,
        ProblemKind::SafetyLevelNotMet | ProblemKind::SafetyBinding
    )));
}

#[test]
fn functions_round_trip_through_the_project() {
    let mut doc = project_with_devices();
    let f = SafetyFunction::new("Guard door");
    store(&mut doc, std::slice::from_ref(&f));
    assert_eq!(functions(doc.project()), vec![f]);
    store(&mut doc, &[]);
    assert!(functions(doc.project()).is_empty());
}

#[test]
fn a_missing_device_and_missing_data_are_reported_as_binding_problems() {
    let mut doc = project_with_devices();
    let mut f = SafetyFunction::new("E-stop");
    bound(&mut f, Role::Sensor, "-S77");
    bound(&mut f, Role::Logic, "-K9");
    bound(&mut f, Role::Actuator, "-K1");
    store(&mut doc, &[f.clone()]);
    let binding: Vec<String> = check(doc.project())
        .into_iter()
        .filter(|p| p.kind == ProblemKind::SafetyBinding)
        .map(|p| p.message)
        .collect();
    assert!(binding.iter().any(|m| m.contains("-S77")), "{binding:?}");
    assert!(
        binding.iter().any(|m| m.contains("no B10d or MTTFd")),
        "{binding:?}"
    );
    // Not rated while parts are unresolved, so no level problem yet.
    assert!(rate(doc.project(), &f).total.is_none());
    assert!(!kinds(&doc).contains(&ProblemKind::SafetyLevelNotMet));
}

#[test]
fn a_weak_function_fails_its_requirement_and_marks_the_bound_devices() {
    let mut doc = project_with_devices();
    let mut f = SafetyFunction::new("E-stop");
    f.severity = "S2".into();
    f.frequency = "F2".into();
    f.avoidance = "P2".into();
    for b in &mut f.blocks {
        b.category = "B".into();
        b.dc_percent = 0.0;
        b.ccf = 0;
    }
    bound(&mut f, Role::Sensor, "-S1");
    bound(&mut f, Role::Actuator, "-K1");
    f.blocks[1].channel1 = vec![PartRef {
        name: "Relay".into(),
        device: None,
        basis: Basis::Mttfd,
        value: Some(10.0),
    }];
    store(&mut doc, &[f.clone()]);
    let rating = rate(doc.project(), &f);
    assert_eq!(rating.met(), Some(false));
    let problems = check(doc.project());
    let p = problems
        .iter()
        .find(|p| p.kind == ProblemKind::SafetyLevelNotMet)
        .expect("level problem");
    assert!(p.message.contains("requires PL e"), "{}", p.message);
    assert_eq!(p.at.len(), 2, "both bound devices are marked");
}

#[test]
fn a_strong_function_meets_a_modest_requirement() {
    let mut doc = project_with_devices();
    let mut f = SafetyFunction::new("Guard");
    f.severity = "S1".into();
    f.frequency = "F1".into();
    f.avoidance = "P1".into();
    bound(&mut f, Role::Sensor, "-S1");
    bound(&mut f, Role::Actuator, "-K1");
    for b in &mut f.blocks {
        b.category = "3".into();
        b.dc_percent = 99.0;
        b.ccf = 70;
    }
    f.blocks[1].channel1 = vec![PartRef {
        name: "Relay".into(),
        device: None,
        basis: Basis::Mttfd,
        value: Some(100.0),
    }];
    store(&mut doc, &[f.clone()]);
    assert_eq!(rate(doc.project(), &f).met(), Some(true));
    assert!(!kinds(&doc).contains(&ProblemKind::SafetyLevelNotMet));
}

#[test]
fn an_own_value_not_typed_yet_is_unrated_not_the_best_class() {
    let mut doc = project_with_devices();
    let mut f = SafetyFunction::new("Guard");
    bound(&mut f, Role::Sensor, "-S1");
    bound(&mut f, Role::Actuator, "-K1");
    // Chosen "own value" on a part whose device does have data: still unrated.
    f.blocks[0].channel1[0].value = Some(0.0);
    f.blocks[1].channel1 = vec![PartRef {
        name: "Relay".into(),
        device: None,
        basis: Basis::Mttfd,
        value: Some(100.0),
    }];
    store(&mut doc, &[f.clone()]);
    let rating = rate(doc.project(), &f);
    assert!(rating.total.is_none());
    let messages: Vec<String> = check(doc.project())
        .into_iter()
        .filter(|p| p.kind == ProblemKind::SafetyBinding)
        .map(|p| p.message)
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("not entered yet")),
        "{messages:?}"
    );
}

#[test]
fn a_tag_on_two_devices_says_so() {
    let mut doc = project_with_devices();
    doc.transact("Duplicate", |tx| {
        tx.add_device(DeviceTag::parse("-S1").unwrap(), "Second S1")?;
        Ok(())
    })
    .unwrap();
    let mut f = SafetyFunction::new("Guard");
    bound(&mut f, Role::Sensor, "-S1");
    store(&mut doc, &[f]);
    let messages: Vec<String> = check(doc.project())
        .into_iter()
        .filter(|p| p.kind == ProblemKind::SafetyBinding)
        .map(|p| p.message)
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("matches 2 devices")),
        "{messages:?}"
    );
}
