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
