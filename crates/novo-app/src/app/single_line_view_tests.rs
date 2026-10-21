//! The single-line diagram in the Reports workspace: reachable from the
//! Reports menu and the palette, drawn from the project, exported as a PDF
//! and added as a drawing page.

use novo_core::PageKind;
use novo_tests::distribution_board;

use super::gate1_tests::Drive;
use super::reports_view::{REPORT_COMMANDS, ReportFormat};
use super::workspace::Workspace;
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    distribution_board(d.app.backend.document_mut().unwrap()).unwrap();
    d.frame();
    (dir, d)
}

#[test]
fn the_command_is_in_the_reports_menu_and_the_palette() {
    assert!(novo_workflow::commands::is_known("report.singleline"));
    assert!(super::workbench::app_runs("report.singleline"));
    assert!(
        novo_workflow::menus::entries("Reports")
            .iter()
            .any(|(info, _)| info.id == "report.singleline"),
        "not in the Reports menu"
    );
    assert!(REPORT_COMMANDS.contains(&("report.singleline", "singleline")));
}

#[test]
fn the_command_opens_the_reports_workspace_on_the_diagram() {
    let (_dir, mut d) = open();
    d.app.wb.queued.push("report.singleline");
    d.frame();
    d.frame();
    assert_eq!(d.app.workspace(), Workspace::Reports);
    assert_eq!(d.app.report_selected().as_deref(), Some("singleline"));
    d.frame();
    // The drawing is what is shown first: one sheet for the board.
    let sheets = d.app.single_line_sheets();
    assert_eq!(sheets.len(), 1);
    assert!(sheets[0].texts.iter().any(|t| t.text == "-Q1  Breaker"));
    assert!(!d.app.single_line_view.schedule);
}

#[test]
fn the_schedule_tab_and_every_sheet_draw_without_trouble() {
    let (_dir, mut d) = open();
    d.app.show_report("singleline");
    d.frame();
    d.app.single_line_view.schedule = true;
    d.frame();
    d.app.single_line_view.schedule = false;
    d.app.single_line_view.sheet = 5; // out of range: clamped
    d.frame();
    assert_eq!(d.app.single_line_view.sheet, 0);
}

#[test]
fn an_empty_project_says_what_is_missing_instead_of_drawing_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.show_report("singleline");
    d.frame();
    d.frame();
    assert!(d.app.single_line_sheets().is_empty());
}

#[test]
fn the_view_follows_the_project() {
    let (_dir, mut d) = open();
    d.app.show_report("singleline");
    d.frame();
    let before = d.app.single_line_sheets();
    let m1 = d
        .app
        .document()
        .unwrap()
        .project()
        .devices()
        .find(|dev| dev.tag.to_string() == "=A1+FLD-M1")
        .unwrap()
        .id;
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Nameplate", |tx| {
            tx.set_property(m1, "rated_current_a", Some(9.0.into()))
        })
        .unwrap();
    d.frame();
    let after = d.app.single_line_sheets();
    assert_ne!(before, after);
}

#[test]
fn the_pdf_has_the_drawn_sheets_then_the_schedule() {
    let (_dir, mut d) = open();
    d.app.show_report("singleline");
    d.frame();
    let path = d
        .app
        .export_report("singleline", ReportFormat::Pdf)
        .expect("a PDF is written");
    let pdf = lopdf::Document::load(&path).unwrap();
    // One drawn sheet and one page of schedule.
    assert_eq!(pdf.get_pages().len(), 2);
    let excel = d
        .app
        .export_report("singleline", ReportFormat::Excel)
        .expect("an Excel file is written");
    assert!(excel.exists());
}

#[test]
fn add_as_page_puts_the_diagram_in_the_project_pdf() {
    let (_dir, mut d) = open();
    d.app.add_report_page("singleline");
    d.frame();
    let generated = d
        .app
        .document()
        .unwrap()
        .project()
        .pages()
        .filter(
            |p| matches!(&p.kind, PageKind::Generated { generator } if generator == "singleline"),
        )
        .count();
    assert_eq!(generated, 1);
    // A second time says it is already there and adds nothing.
    d.app.add_report_page("singleline");
    d.frame();
    let again = d
        .app
        .document()
        .unwrap()
        .project()
        .pages()
        .filter(
            |p| matches!(&p.kind, PageKind::Generated { generator } if generator == "singleline"),
        )
        .count();
    assert_eq!(again, 1);
}

/// Rewrites the fixture: `cargo test -p novo-app write_distribution_fixture -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/distribution-board.novo"]
fn write_distribution_fixture() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend.new_project("Distribution board (example)").unwrap();
    novo_tests::distribution_board(backend.document_mut().unwrap()).unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/distribution-board.novo");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let _ = std::fs::remove_file(&path);
    backend.save(&path).unwrap();
}

/// Rewrites the fixture: `cargo test -p novo-app write_diversity_fixture -- --ignored`.
#[test]
#[ignore = "writes qa/fixtures/distribution-board-diversity.novo"]
fn write_diversity_fixture() {
    use crate::backend::ProjectBackend;
    let mut backend = crate::backend::StorageBackend::default();
    backend
        .new_project("Distribution board with diversity (example)")
        .unwrap();
    let board = novo_tests::distribution_board(backend.document_mut().unwrap()).unwrap();
    backend
        .document_mut()
        .unwrap()
        .transact("Diversity", |tx| {
            tx.set_property(
                board.q1,
                novo_checks::single_line::DIVERSITY_FACTOR,
                Some(0.8.into()),
            )
        })
        .unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/distribution-board-diversity.novo");
    let _ = std::fs::remove_file(&path);
    backend.save(&path).unwrap();
}
