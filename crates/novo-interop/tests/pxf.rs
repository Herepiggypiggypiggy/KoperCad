//! The PXF import plan on the synthetic fixture: preview, apply, undo.

use novo_core::{DeviceTag, Document};
use novo_interop::eplan::pxf::{PxfOptions, plan_pxf, read_pxf_str};

const SMALL: &str = include_str!("fixtures/pxf/synthetic-small.epj");

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

#[test]
fn the_plan_previews_then_applies_as_one_undo_step() {
    let file = read_pxf_str(SMALL).unwrap();
    let mut doc = Document::default();
    let plan = plan_pxf(doc.project(), &file, &PxfOptions::default());
    let r = plan.report();
    assert_eq!(r.count_of("pages (empty)"), 2);
    assert_eq!(r.count_of("terminal strips"), 1);
    assert_eq!(r.count_of("terminals"), 1);
    assert_eq!(r.count_of("cables"), 1);
    assert_eq!(r.count_of("cable cores"), 1);
    assert!(r.text().contains("symbol drawings"));
    assert!(r.text().contains("interruption points"));
    assert!(!plan.is_empty());

    let before = doc.project().devices().count();
    let n = plan.apply(&mut doc).unwrap();
    assert!(n > 0);
    let p = doc.project();
    assert_eq!(p.pages().count(), 2);
    assert_eq!(p.structures().count(), 2); // =A1 and +CAB1
    assert_eq!(p.cables().count(), 1);
    let q1 = p.devices_by_tag(&tag("=A1+CAB1-Q1"));
    assert_eq!(q1.len(), 1);
    let q1 = p.device(q1[0]).unwrap();
    assert_eq!(q1.description, "Breaker 3P");
    assert_eq!(q1.parts.len(), 1);
    let part = p.part(q1.parts[0]).unwrap();
    assert_eq!(part.manufacturer, "DEMO");
    assert_eq!(part.order_number, "DEMO.BRK3P16");
    assert_eq!(part.type_number, "BRK3P16");
    assert!(p.devices_by_tag(&tag("=A1+CAB1-X1")).len() == 1);
    assert!(p.devices_by_tag(&tag("=A1+CAB1-M1.2")).len() == 1);

    assert!(doc.undo().unwrap().is_some());
    assert_eq!(doc.project().devices().count(), before);
    assert_eq!(doc.project().pages().count(), 0);
}

#[test]
fn a_second_import_adds_nothing_and_a_stale_plan_is_refused() {
    let file = read_pxf_str(SMALL).unwrap();
    let mut doc = Document::default();
    let plan = plan_pxf(doc.project(), &file, &PxfOptions::default());
    plan.apply(&mut doc).unwrap();
    assert!(
        plan.apply(&mut doc).is_err(),
        "the project changed since the plan"
    );

    let again = plan_pxf(doc.project(), &file, &PxfOptions::default());
    assert!(again.is_empty());
    assert!(again.existing.contains(&"=A1+CAB1-Q1".to_string()));
    assert_eq!(again.apply(&mut doc).unwrap(), 0);
}
