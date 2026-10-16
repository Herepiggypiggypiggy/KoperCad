//! Commissioning mode in the app: the command opens the tab, decisions mark
//! the drawing and are kept in the project's commissioning document.

use novo_canvas::marks::MarkKind;
use novo_commissioning::{CheckKind, CheckState, CommissioningStore, Target, targets};

use super::OutlineMode;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn example() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    (dir, d)
}

#[test]
fn the_command_opens_the_tab_and_a_second_run_closes_it() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    assert!(d.app.wb.commissioning.is_none());
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    assert!(d.app.wb.commissioning.is_some());
    assert_eq!(d.app.outline_mode, OutlineMode::Commissioning);
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    assert!(d.app.wb.commissioning.is_none());
    assert_eq!(d.app.outline_mode, OutlineMode::Pages);
}

#[test]
fn decisions_mark_the_drawing_and_are_kept_in_the_project() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let page = d.app.current_page().expect("the example shows a page");
    let (wire, device) = {
        let project = d.doc().project();
        let all = targets(project);
        let on_page = |t: &&novo_commissioning::TargetInfo| t.pages.contains(&page);
        (
            all.iter()
                .filter(on_page)
                .find(|t| matches!(t.target, Target::Wire(_)))
                .expect("a wire on the page")
                .target,
            all.iter()
                .filter(on_page)
                .find(|t| matches!(t.target, Target::Device(_)))
                .expect("a device on the page")
                .target,
        )
    };
    {
        let mode = d.app.wb.commissioning.as_mut().unwrap();
        mode.who = "tv".into();
        mode.decide(wire, CheckKind::Continuity, CheckState::Failed);
        mode.decide(wire, CheckKind::Labelled, CheckState::Failed);
        for check in device.checks() {
            mode.decide(device, *check, CheckState::Passed);
        }
    }
    d.frame();
    d.frame();

    let marks = d
        .app
        .canvas
        .schematic_ref()
        .unwrap()
        .canvas
        .marks
        .list
        .clone();
    assert!(marks.iter().any(|m| m.kind == MarkKind::Error), "{marks:?}");
    assert!(marks.iter().any(|m| m.kind == MarkKind::Added), "{marks:?}");

    let json = d.app.backend.commissioning_json().expect("kept at once");
    let store = CommissioningStore::from_json(&json).unwrap();
    assert_eq!(store.state(wire, CheckKind::Continuity), CheckState::Failed);
    assert_eq!(
        store.open_punch_count(),
        2,
        "each failed check opened an item"
    );
    assert_eq!(store.record(wire, CheckKind::Continuity).unwrap().by, "tv");
}

#[test]
fn another_project_starts_without_the_data() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let page = d.app.current_page().unwrap();
    let wire = targets(d.doc().project())
        .into_iter()
        .find(|t| t.pages.contains(&page) && matches!(t.target, Target::Wire(_)))
        .unwrap()
        .target;
    d.app.wb.commissioning.as_mut().unwrap().decide(
        wire,
        CheckKind::Continuity,
        CheckState::Passed,
    );
    d.frame();
    assert!(!d.app.wb.commissioning.as_ref().unwrap().store.is_empty());
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.frame();
    let mode = d.app.wb.commissioning.as_ref();
    assert!(
        mode.is_none_or(|m| m.store.is_empty()),
        "the new project has none of the old project's data"
    );
    assert_eq!(d.app.backend.commissioning_json(), None);
}

#[test]
fn the_report_is_written_as_pdf_and_csv_and_the_command_is_known() {
    let (dir, mut d) = example();
    let ctx = d.ctx.clone();
    assert!(novo_workflow::commands::get("export.commissioning").is_some());
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let pdf = dir.path().join("Plant commissioning.pdf");
    d.app
        .write_commissioning_report(novo_pdf::ReportFileFormat::Pdf, &pdf)
        .expect("the PDF is written");
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    assert_eq!(d.app.wb.last_export.as_deref(), Some(pdf.as_path()));

    let csv = dir.path().join("Plant commissioning.csv");
    d.app
        .write_commissioning_report(novo_pdf::ReportFileFormat::Csv(Default::default()), &csv)
        .expect("the CSV is written");
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.contains("Progress per cabinet"), "{text}");
    assert!(
        text.contains("Pending"),
        "an untouched example is all pending"
    );
}

#[test]
fn a_file_that_is_not_ours_is_never_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Plant.novo");
    let first = super::commissioning::report_path(Some(&file), None, "Plant", "csv").unwrap();
    assert_eq!(first, dir.path().join("Plant commissioning.csv"));
    std::fs::write(&first, "mine").unwrap();
    let next = super::commissioning::report_path(Some(&file), None, "Plant", "csv").unwrap();
    assert_eq!(next, dir.path().join("Plant commissioning 2.csv"));
}

#[test]
fn the_drawing_cannot_be_edited_while_the_mode_is_showing() {
    use novo_core::Point;
    use novo_workflow::palette::Mode;
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    let count = |d: &Drive| d.doc().project().symbols().count();
    let before = count(&d);
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    assert!(d.app.commissioning_locks_edits());
    assert!(d.app.type_command(&ctx, Mode::Place, "mcb3"));
    d.click(Point::new(600.0, 400.0));
    d.frame();
    assert_eq!(count(&d), before, "the placed symbol was taken back");
    // Leaving the mode lets the same click edit again.
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    assert!(!d.app.commissioning_locks_edits());
    assert!(d.app.type_command(&ctx, Mode::Place, "mcb3"));
    d.click(Point::new(600.0, 400.0));
    d.frame();
    assert_eq!(count(&d), before + 1);
}
