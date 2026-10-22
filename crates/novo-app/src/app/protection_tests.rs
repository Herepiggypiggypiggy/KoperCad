//! The Protection window: the command, what it reads from the project, and
//! that it draws.

use novo_checks::protection::sccr::Verdict;
use novo_core::{Device, Value};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let d = Drive::new(&dir.path().join("untitled"));
    (dir, d)
}

#[test]
fn protection_needs_a_project_and_opens_a_window() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    assert!(!d.app.is_enabled(Action::Protection));
    assert!(Action::from_id("view.protection").is_some());
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.is_enabled(Action::Protection));
    d.app.run(&ctx, Action::Protection);
    d.frame();
    assert!(d.app.protection.open);
    // The example's catalogue parts carry ratings, so there is something to plot.
    let (_, study) = d.app.protection.study.as_ref().expect("study read");
    assert!(!study.devices.is_empty());
}

#[test]
fn protection_reads_the_devices_and_draws_every_tab() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let ids: Vec<(String, novo_core::DeviceId)> = d
        .doc()
        .project()
        .devices()
        .map(|dev| (dev.tag.to_string(), dev.id))
        .collect();
    let find = |t: &str| ids.iter().find(|(tag, _)| tag == t).unwrap().1;
    let (f1, f2) = (find("=A1+CAB-F1"), find("=A1+CAB-F2"));
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Ratings", |tx| {
            for (device, rated) in [(f1, 25.0), (f2, 6.3)] {
                tx.update::<Device>(device, |dev| {
                    dev.properties
                        .insert("rated_current_a".into(), Value::Float(rated));
                    dev.properties
                        .insert("breaking_capacity_ka".into(), Value::Float(50.0));
                })?;
            }
            tx.set_info(|i| {
                i.properties
                    .insert("supply.short_circuit_ka".into(), Value::Float(10.0));
            })
        })
        .unwrap();
    d.app.run(&ctx, Action::Protection);
    d.frame();
    let (_, study) = d.app.protection.study.as_ref().expect("study");
    // -F1 and -F2 as set here, and -F3 from its catalogue rating.
    assert_eq!(study.devices.len(), 3);
    assert_eq!(d.app.protection.shown.len(), 3);
    let results = d.app.protection.results.as_ref().expect("analysed");
    assert_eq!(results.chain.pairs.len(), 2);
    // Fuse holder default 10 kA, relay and breaker defaults 5 kA: below
    // the 10 kA available.
    assert_eq!(results.rating.verdict, Verdict::Insufficient);
    // Every tab draws without panicking.
    for tab in [
        super::protection::Tab::Selectivity,
        super::protection::Tab::Rating,
    ] {
        d.app.protection.tab = tab;
        d.frame();
    }
    d.app.protection.tab = super::protection::Tab::Curves;
    d.frame();
}

fn first_device(d: &Drive) -> novo_core::DeviceId {
    d.doc().project().devices().next().unwrap().id
}

fn supply_property(d: &Drive) -> Option<Value> {
    d.doc()
        .project()
        .info()
        .properties
        .get("supply.short_circuit_ka")
        .cloned()
}

#[test]
fn the_fault_current_typed_in_the_window_is_saved_with_the_project() {
    use super::protection::Edit;
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    // The example states a 6 kA supply.
    assert_eq!(supply_property(&d), Some(Value::Float(6.0)));
    // The box is being dragged: the value is live but not yet saved.
    d.app.protection.set_supply_for_test(Some(12.5));
    assert_eq!(supply_property(&d), Some(Value::Float(6.0)));
    assert!(d.app.apply_protection_edit(Edit::Supply(Some(12.5))));
    assert_eq!(supply_property(&d), Some(Value::Float(12.5)));
    d.app.run(&ctx, Action::Protection);
    d.frame();
    let (_, study) = d.app.protection.study.as_ref().expect("study");
    assert_eq!(study.supply_ka, Some(12.5), "shown again from the project");
    // One undo step, and zero clears it.
    d.app.run(&ctx, Action::Undo);
    assert_eq!(supply_property(&d), Some(Value::Float(6.0)));
    assert!(d.app.apply_protection_edit(Edit::Supply(Some(8.0))));
    assert!(d.app.apply_protection_edit(Edit::Supply(None)));
    assert_eq!(supply_property(&d), None);
}

#[test]
fn limiter_credit_entries_are_checked_and_saved_on_the_device() {
    use super::protection::{Edit, limiter_value};
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let dev = first_device(&d);
    let table = "10000:5000:5e4; 100000:20000:4e5".to_string();
    assert!(d.app.apply_protection_edit(Edit::Device(
        dev,
        novo_checks::protection::model::LET_THROUGH,
        table.clone()
    )));
    assert!(d.app.apply_protection_edit(Edit::Flag(
        dev,
        novo_checks::protection::model::CURRENT_LIMITING,
        true
    )));
    let props = &d.doc().project().device(dev).unwrap().properties;
    assert_eq!(props.get("let_through"), Some(&Value::Text(table)));
    assert_eq!(props.get("current_limiting"), Some(&Value::Bool(true)));
    // A bad table or number is refused and nothing is written.
    assert!(
        !d.app
            .apply_protection_edit(Edit::Device(dev, "let_through", "1000:800".into()))
    );
    assert!(limiter_value("withstand_i2t", "-3").is_err());
    assert!(limiter_value("withstand_i2t", "abc").is_err());
    assert_eq!(limiter_value("withstand_i2t", "").unwrap(), None);
    assert_eq!(
        limiter_value("withstand_peak_ka", "2,5").unwrap(),
        Some(Value::Float(2.5))
    );
    // The Panel rating tab draws the table.
    d.app.run(&ctx, Action::Protection);
    d.app.protection.tab = super::protection::Tab::Rating;
    d.frame();
}
