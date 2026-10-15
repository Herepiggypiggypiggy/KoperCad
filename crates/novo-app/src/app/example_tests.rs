//! The example project: Open example on the start panel and in the command
//! field (docs/ONBOARDING-DESIGN.md §1).

use novo_core::{DeviceTag, PageKind};

use super::example::EXAMPLE_NAME;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let d = Drive::new(&dir.path().join("untitled"));
    (dir, d)
}

fn tags(d: &Drive) -> Vec<String> {
    let mut tags: Vec<String> = d
        .doc()
        .project()
        .devices()
        .map(|dev| dev.tag.to_string())
        .collect();
    tags.sort();
    tags
}

#[test]
fn open_example_works_with_no_project_and_shows_the_motor_starter() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    assert!(d.app.document().is_none());
    assert!(d.app.is_enabled(Action::OpenExample));
    d.app.run(&ctx, Action::OpenExample);
    d.frame();

    let project = d.doc().project();
    assert_eq!(project.info().name, EXAMPLE_NAME);
    for tag in [
        "=A1+CAB-K1",
        "=A1+CAB-F2",
        "=A1+CAB-X1",
        "=A1+FLD-M1",
        "=A1+CAB-S0",
        "=A1+CAB-S1",
    ] {
        let tag = DeviceTag::parse(tag).unwrap();
        assert_eq!(project.devices_by_tag(&tag).len(), 1, "{tag}");
    }
    let drawn = project
        .pages_in_order()
        .into_iter()
        .filter(|p| !matches!(p.kind, PageKind::Generated { .. }))
        .count();
    assert_eq!(drawn, 2, "main and control circuit");
    assert!(
        project.pages().count() > drawn,
        "generated pages come with it"
    );
    assert!(d.app.current_page().is_some());
}

#[test]
fn the_example_has_no_error_level_problem() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let errors: Vec<_> = novo_checks::check(d.doc().project())
        .into_iter()
        .filter(|p| p.severity == novo_checks::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn opening_the_example_twice_gives_the_same_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    let first = tags(&d);
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert_eq!(tags(&d), first);
}

#[test]
fn closing_the_example_untouched_leaves_nothing_under_unsaved() {
    let (dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(!d.app.is_dirty(), "marked saved once drawn");
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    assert!(d.app.backend.unsaved_projects().is_empty());
    let left = std::fs::read_dir(dir.path().join("untitled"))
        .map(|rd| rd.count())
        .unwrap_or(0);
    assert_eq!(left, 0, "the temporary file is gone");
}

#[test]
fn an_edited_example_is_kept_like_any_unsaved_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.place("mcb3", 120.0, 48.0);
    d.frame();
    assert!(d.app.is_dirty());
    d.app.run(&ctx, Action::CloseProject);
    d.frame();
    assert_eq!(d.app.backend.unsaved_projects().len(), 1);
}

#[test]
fn the_example_replaces_an_open_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert_eq!(d.doc().project().info().name, EXAMPLE_NAME);
    assert_eq!(tags(&d).len(), 8, "eight devices, the strip included");
}
