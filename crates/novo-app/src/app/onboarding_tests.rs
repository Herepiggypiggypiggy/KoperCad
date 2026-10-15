//! The fast-keys walk-through (docs/ONBOARDING-DESIGN.md §2 and §5).

use novo_core::DeviceTag;

use super::gate1_tests::Drive;
use super::onboarding::{PRACTICE_NAME, STEPS};
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let d = Drive::new(&dir.path().join("untitled"));
    (dir, d)
}

#[test]
fn every_step_names_a_command_with_a_default_key() {
    let (_dir, d) = drive();
    for step in &STEPS {
        assert!(
            d.app.wb.keymap.shortcut_text(step.command).is_some(),
            "{} has no key",
            step.command
        );
        assert!(step.text.contains("{key}"), "{}", step.id);
    }
    let ids: std::collections::HashSet<_> = STEPS.iter().map(|s| s.id).collect();
    assert_eq!(ids.len(), STEPS.len());
}

#[test]
fn the_first_run_points_at_the_walkthrough_until_it_starts() {
    let (_dir, mut d) = drive();
    assert!(d.app.first_run());
    assert!(!d.app.walkthrough_on());
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::TryFastKeys);
    d.frame();
    assert!(!d.app.first_run());
    assert!(d.app.walkthrough_on());
}

#[test]
fn the_practice_project_has_a_motor_and_three_terminals_and_is_not_dirty() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::TryFastKeys);
    d.frame();
    let project = d.doc().project();
    assert_eq!(project.info().name, PRACTICE_NAME);
    assert_eq!(project.devices_by_tag(&DeviceTag::product("M1")).len(), 1);
    assert_eq!(project.terminals().count(), 3);
    assert!(!d.app.is_dirty());
}

#[test]
fn steps_tick_from_use_and_never_edit_the_project() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::TryFastKeys);
    d.frame();
    let revision = d.doc().project().revision();
    d.app.run_command(&ctx, "tool.connect", None);
    d.frame();
    d.app.run_command(&ctx, "tool.cable", None);
    d.frame();
    d.app.run_command(&ctx, "edit.duplicate", None);
    d.app.run(&ctx, Action::CommandPalette);
    d.app.run_command(&ctx, "sim.live", None);
    d.frame();
    let o = &d.app.settings().onboarding;
    for id in ["wire", "cable", "duplicate", "commands", "live"] {
        assert!(o.is_ticked(id), "{id}");
    }
    assert!(!o.is_ticked("place"));
    assert_eq!(d.doc().project().revision(), revision);
}

#[test]
fn skip_stops_the_ticking_and_show_walkthrough_starts_over() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::TryFastKeys);
    d.frame();
    d.app.tour_tick("wire");
    d.app.settings.onboarding.dismissed = true;
    assert!(!d.app.walkthrough_on());
    d.app.tour_tick("cable");
    assert!(!d.app.settings().onboarding.is_ticked("cable"));
    d.app.run(&ctx, Action::ShowWalkthrough);
    assert!(d.app.walkthrough_on());
    assert!(d.app.settings().onboarding.ticked.is_empty());
}

#[test]
fn show_walkthrough_works_with_no_project_and_shows_when_one_opens() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowWalkthrough);
    assert!(!d.app.walkthrough_on(), "no project, no card");
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    assert!(d.app.walkthrough_on());
}
