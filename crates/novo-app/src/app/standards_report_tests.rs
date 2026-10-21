//! The standards report command: no standard selected says so, a selected
//! set writes a PDF and a web page beside the project.

use super::gate1_tests::Drive;
use crate::actions::Action;
use novo_checks::standards::{self, StandardSet};

fn example() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    (dir, d)
}

#[test]
fn the_command_is_known_and_writes_the_design_checks_without_a_standard() {
    let (dir, mut d) = example();
    assert!(novo_workflow::commands::get("export.standards_report").is_some());
    let html = dir.path().join("Plant standards.html");
    d.app
        .write_standards_report(super::standards_report::StandardsFormat::Html, &html)
        .expect("written with no standard selected");
    let text = std::fs::read_to_string(html).unwrap();
    assert!(text.contains("Design checks"));
    assert!(text.contains("No standard is selected"));
    assert!(text.contains("unverified"));
}

#[test]
fn the_report_lists_every_check_family_with_its_basis_and_never_claims_compliance() {
    let (dir, mut d) = example();
    assert!(d.app.edit_project("Select standards", |tx| {
        standards::select(tx, &[StandardSet::Iec60204])
    }));
    let html = dir.path().join("Plant standards.html");
    d.app
        .write_standards_report(super::standards_report::StandardsFormat::Html, &html)
        .unwrap();
    let text = std::fs::read_to_string(&html).unwrap();
    for f in novo_checks::standards::design::FAMILIES {
        assert!(text.contains(f.title), "{} missing", f.title);
        assert!(
            text.contains(f.assumptions.split('.').next().unwrap()),
            "{}",
            f.id
        );
    }
    // No Heat result is held in this session: the report says so instead of
    // saying the cabinet is fine.
    assert!(text.contains("No Heat result is held"));
    assert!(!text.to_lowercase().contains("project complies"));
    // A pass on planning values or a model carries the caveat in its word.
    assert!(text.contains("Met (estimate)"));
    assert!(!text.contains("can be trusted"));
    let project = d.app.backend.document().unwrap().project();
    let doc = standards::document::build(project, &standards::compliance(project));
    let out = super::standards_report::report_output(&doc);
    for id in ["limits", "design_summary", "design_detail"] {
        assert!(out.section(id).is_some(), "{id}");
    }
    let heat = doc.design.iter().find(|r| r.family.id == "heat").unwrap();
    assert_ne!(heat.verdict, standards::document::Verdict::Pass);
    let pdf = dir.path().join("Plant standards.pdf");
    d.app
        .write_standards_report(super::standards_report::StandardsFormat::Pdf, &pdf)
        .expect("pdf");
}

#[test]
fn a_selected_standard_gives_a_pdf_and_a_web_page() {
    let (dir, mut d) = example();
    assert!(d.app.edit_project("Select standards", |tx| {
        standards::select(tx, &[StandardSet::Iec60204, StandardSet::Ul508a])
    }));
    let pdf = dir.path().join("Plant standards.pdf");
    d.app
        .write_standards_report(super::standards_report::StandardsFormat::Pdf, &pdf)
        .expect("pdf written");
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    let html = dir.path().join("Plant standards.html");
    d.app
        .write_standards_report(super::standards_report::StandardsFormat::Html, &html)
        .expect("html written");
    let text = std::fs::read_to_string(html).unwrap();
    assert!(text.contains("IEC 60204-1"));
    assert!(text.contains("UL 508A"));
    assert!(text.contains("does not certify"));
    let xlsx = dir.path().join("Plant standards.xlsx");
    d.app
        .write_standards_report(super::standards_report::StandardsFormat::Excel, &xlsx)
        .expect("xlsx written");
    assert!(std::fs::read(&xlsx).unwrap().starts_with(b"PK"));
}

#[test]
fn the_notice_has_its_own_section_and_no_cell_is_cut() {
    let (_dir, mut d) = example();
    assert!(d.app.edit_project("Select standards", |tx| {
        standards::select(tx, &StandardSet::ALL)
    }));
    let project = d.app.backend.document().unwrap().project();
    let doc = standards::document::build(project, &standards::compliance(project));
    let report = super::standards_report::report_output(&doc);
    let notice = report.section("notice").expect("notice section");
    assert_eq!(notice.columns.len(), 1);
    assert!(notice.rows.len() >= 2);
    let planning = report
        .sections
        .iter()
        .flat_map(|s| &s.rows)
        .any(|r| r.cells[0].to_string() == "planning value");
    assert!(
        !planning
            || report
                .sections
                .iter()
                .flat_map(|s| &s.rows)
                .all(|r| r.cells[0].to_string() != "planning")
    );
}

#[test]
fn simulate_commands_live_in_the_simulate_menu_and_leave_cabinet_for_the_schematic() {
    use novo_workflow::menus;
    let ids: Vec<_> = menus::entries("Simulate")
        .iter()
        .map(|(c, _)| c.id)
        .collect();
    for id in [
        "sim.live",
        "sim.plc_program",
        "simulate.motor-start",
        "sim.faults",
        "sim.add_fault",
        "view.protection",
    ] {
        assert!(ids.contains(&id), "{id} is not in Simulate");
    }
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    d.app
        .switch_workspace(crate::app::workspace::Workspace::Cabinet);
    d.app.run_command(&ctx, "sim.live", None);
    d.frame();
    assert_eq!(
        d.app.workspace(),
        crate::app::workspace::Workspace::Schematic
    );
    assert!(d.app.wb.live.on);
}
