//! The crane project template (File > New from template): the structure
//! tree, the drawing pages, the self-filling cover pages and the Project
//! information fields that feed them.

use novo_core::{DeviceTag, Document, PageKind, Value};
use novo_reports::{FrontMatter, Report, front_keys};

use super::gate1_tests::Drive as Harness;
use super::template_crane::{CRANE_TEMPLATE_NAME, CraneOptions, Drive, build_crane_template_on};
use crate::actions::Action;

fn built_with(options: CraneOptions) -> Document {
    let mut doc = Document::default();
    build_crane_template_on(&mut doc, "2026-10-29", options).expect("the template builds");
    doc
}

fn built() -> Document {
    built_with(CraneOptions::default())
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
fn the_only_issues_are_the_open_points_the_circuits_name() {
    for options in [
        CraneOptions::default(),
        CraneOptions {
            hoist_units: 3,
            hoist_drive: Drive::Frequency,
            travel_drive: Drive::TwoSpeed,
        },
    ] {
        let doc = built_with(options);
        let other: Vec<_> = novo_checks::check(doc.project())
            .into_iter()
            .filter(|p| p.severity != novo_checks::Severity::Info)
            .filter(|p| p.kind != novo_checks::ProblemKind::UnconnectedPin)
            .map(|p| format!("{:?}: {}", p.severity, p.message))
            .collect();
        assert!(other.is_empty(), "{other:#?}");
    }
}

#[test]
fn the_hoist_unit_count_makes_the_cabinets_and_the_pages() {
    for units in 1..=4u8 {
        let doc = built_with(CraneOptions {
            hoist_units: units,
            ..CraneOptions::default()
        });
        let p = doc.project();
        for n in 1..=4u8 {
            let has = p
                .structure_by_tag(&DeviceTag::parse(&format!("=CRN+HU{n}+CTL")).unwrap())
                .is_some();
            assert_eq!(has, n <= units, "unit {n} of {units}");
        }
        let hoist_pages = p
            .pages()
            .filter(|pg| pg.title.starts_with("Hoist unit"))
            .count();
        assert_eq!(hoist_pages, usize::from(units));
        // Each unit's circuit is placed in its own control cabinet.
        for n in 1..=units {
            let t = DeviceTag::parse(&format!("=CRN+HU{n}+CTL")).unwrap();
            let in_cabinet = p
                .devices()
                .filter(|d| d.tag.function == t.function && d.tag.location == t.location)
                .count();
            assert!(in_cabinet > 10, "unit {n}: {in_cabinet} devices");
        }
    }
}

#[test]
fn the_drive_choice_picks_the_circuit() {
    let two_speed = |doc: &Document| {
        doc.project()
            .devices()
            .filter(|d| d.description.contains("two speeds"))
            .count()
    };
    let both_two = built_with(CraneOptions {
        hoist_drive: Drive::TwoSpeed,
        travel_drive: Drive::TwoSpeed,
        ..CraneOptions::default()
    });
    assert_eq!(two_speed(&both_two), 3, "hoist, bridge and crab motors");
    let hoist_only = built_with(CraneOptions {
        hoist_drive: Drive::TwoSpeed,
        travel_drive: Drive::Frequency,
        ..CraneOptions::default()
    });
    assert_eq!(two_speed(&hoist_only), 1);
    let none = built_with(CraneOptions {
        hoist_drive: Drive::Frequency,
        travel_drive: Drive::Frequency,
        ..CraneOptions::default()
    });
    assert_eq!(two_speed(&none), 0);
}

#[test]
fn the_structure_tree_is_crane_shaped() {
    let doc = built_with(CraneOptions {
        hoist_units: 2,
        ..CraneOptions::default()
    });
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
    assert_eq!(text(&doc, "general", "Drawn pages"), "5");
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
    assert_eq!(row[3], "1", "page 5");
    let plant = structure.iter().find(|r| r[0] == "=CRN").unwrap();
    assert_eq!(plant[2], "Plant");
    assert_eq!(plant[3], "5", "every page is under the plant");
}

#[test]
fn new_from_template_is_a_command_and_opens_an_unsaved_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Harness::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("file.new_crane"));
    assert!(d.app.is_enabled(Action::NewCraneProject));
    d.app.run_command(&ctx, "file.new_crane", None);
    d.frame();
    // The command asks first; Create makes the project.
    assert!(d.app.wb.crane_new.is_some(), "the options window opens");
    d.app.new_crane_project(CraneOptions::default());
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
