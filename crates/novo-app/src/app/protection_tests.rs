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
    // The example has no ratings: the window explains instead of plotting.
    let (_, study) = d.app.protection.study.as_ref().expect("study read");
    assert!(study.devices.is_empty());
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
    assert_eq!(study.devices.len(), 2);
    assert_eq!(d.app.protection.shown.len(), 2);
    let results = d.app.protection.results.as_ref().expect("analysed");
    assert_eq!(results.chain.pairs.len(), 1);
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
