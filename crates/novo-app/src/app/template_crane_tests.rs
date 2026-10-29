//! The crane project template (File > New from template): the structure
//! tree, the drawing pages, the self-filling cover pages and the Project
//! information fields that feed them.

use novo_core::{DeviceTag, Document, PageKind, Value};
use novo_reports::{FrontMatter, Report, front_keys};

use super::gate1_tests::Drive;
use super::template_crane::{CRANE_TEMPLATE_NAME, build_crane_template_on};
use crate::actions::Action;

fn built() -> Document {
    let mut doc = Document::default();
    build_crane_template_on(&mut doc, "2026-10-29").expect("the template builds");
    doc
}

fn text(doc: &Document, sheet: &str, field: &str) -> String {
    let out = FrontMatter.generate(doc.project());
    out.section(sheet)
        .unwrap()
        .text_rows()
        .into_iter()
        .find(|r| r[0] == field)
        .unwrap_or_else(|| panic!("no {field} on {sheet}"))[1]
        .clone()
}

#[test]
fn the_template_opens_without_a_warning_or_an_error() {
    let doc = built();
    let bad: Vec<_> = novo_checks::check(doc.project())
        .into_iter()
        .filter(|p| p.severity != novo_checks::Severity::Info)
        .map(|p| format!("{:?}: {}", p.severity, p.message))
        .collect();
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn the_structure_tree_is_crane_shaped() {
    let doc = built();
    let p = doc.project();
    let node = |t: &str| {
        p.structure_by_tag(&DeviceTag::parse(t).unwrap())
            .unwrap_or_else(|| panic!("no node {t}"))
    };
    for t in [
        "=CRN",
        "=CRN+GIR",
        "=CRN+HU1",
        "=CRN+HU2",
        "=CRN+OPS",
        "=CRN+FLD+LSB",
        "=CRN+FLD+LCH",
        "=CRN+FLD+MB",
        "=CRN+FLD+MH",
    ] {
        assert!(!node(t).enclosure, "{t} is a place, not a box");
    }
    for t in [
        "=CRN+GIR+DST",
        "=CRN+GIR+MCC",
        "=CRN+HU1+CTL",
        "=CRN+HU2+REL",
        "=CRN+OPS+PLG",
    ] {
        assert!(node(t).enclosure, "{t} is a cabinet");
    }
    assert_eq!(
        p.structures().count(),
        p.structures()
            .map(|s| s.tag.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        "every tag once"
    );
}

#[test]
fn the_cover_pages_come_first_and_are_five_sheets() {
    let doc = built();
    let order = doc.project().pages_in_order();
    let first = order[0];
    assert!(matches!(&first.kind, PageKind::Generated { generator } if generator == "front"));
    assert_eq!(first.name, "00");
    // Then the drawing, filed under the cabinets.
    assert_eq!(order[1].name, "1");
    assert_eq!(order[1].structure.to_string(), "=CRN+GIR+DST");
    assert_eq!(FrontMatter.generate(doc.project()).sections.len(), 5);
}

#[test]
fn the_cover_pages_fill_themselves_from_the_project() {
    let mut doc = built();
    assert_eq!(text(&doc, "title", "Issue date"), "2026-10-29");
    assert_eq!(text(&doc, "title", "Customer"), "-", "a blank field shows");
    assert_eq!(text(&doc, "general", "Supply voltage"), "400 V");
    assert_eq!(text(&doc, "general", "Drawn pages"), "6");
    assert_eq!(text(&doc, "revisions", "-"), "");

    doc.transact("Fill in", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(front_keys::CUSTOMER.into(), Value::Text("Acme".into()));
            i.properties
                .insert(front_keys::CRANE_SWL_T.into(), Value::Text("12.5".into()));
        })
    })
    .unwrap();
    assert_eq!(text(&doc, "title", "Customer"), "Acme");
    assert_eq!(text(&doc, "project", "Safe working load"), "12.5 t");

    // The structure overview counts pages per cabinet.
    let out = FrontMatter.generate(doc.project());
    let structure = out.section("structure").unwrap().text_rows();
    let row = structure.iter().find(|r| r[0] == "=CRN+HU1+CTL").unwrap();
    assert_eq!(row[2], "Enclosure");
    assert_eq!(row[3], "2", "pages 5 and 6");
    let plant = structure.iter().find(|r| r[0] == "=CRN").unwrap();
    assert_eq!(plant[2], "Plant");
    assert_eq!(plant[3], "6", "every page is under the plant");
}

#[test]
fn new_from_template_is_a_command_and_opens_an_unsaved_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("file.new_crane"));
    assert!(d.app.is_enabled(Action::NewCraneProject));
    d.app.run_command(&ctx, "file.new_crane", None);
    d.frame();
    let doc = d.doc();
    assert_eq!(doc.project().info().name, CRANE_TEMPLATE_NAME);
    assert!(doc.project().structures().count() > 20);
    assert!(
        d.app.document().is_some_and(|doc| doc.is_dirty()),
        "a template is a new project, not an example"
    );
    // The Project information fields reach the cover pages.
    let mut fields = d.app.project_info_fields();
    fields[0] = "Acme".into();
    d.app.set_project_info(&fields);
    assert_eq!(text(d.doc(), "title", "Customer"), "Acme");
    d.app.backend.document_mut().unwrap().undo().unwrap();
    assert_eq!(text(d.doc(), "title", "Customer"), "-", "one undo step");
}
