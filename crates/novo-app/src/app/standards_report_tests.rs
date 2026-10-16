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
fn the_command_is_known_and_needs_a_selected_standard() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    assert!(novo_workflow::commands::get("export.standards_report").is_some());
    d.app.run_command(&ctx, "export.standards_report", None);
    d.frame();
    assert!(d.app.wb.last_export.is_none());
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
