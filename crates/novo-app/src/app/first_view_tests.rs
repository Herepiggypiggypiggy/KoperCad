//! The first view of a project: the sheet is in view, not a corner of it.

use eframe::egui;

use super::gate1_tests::Drive;
use crate::actions::Action;

#[test]
fn the_example_opens_with_the_whole_sheet_in_view_at_1366() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    for _ in 0..4 {
        d.frame_sized(egui::vec2(1366.0, 768.0), Vec::new());
    }
    let view = d.app.canvas.schematic().unwrap();
    let sheet = view.scene().unwrap().scene().bounds();
    let vp = &view.canvas.viewport;
    let (lo, hi) = (vp.world_to_screen(sheet.min), vp.world_to_screen(sheet.max));
    assert!(
        lo.x >= 0.0 && hi.x <= vp.size.x,
        "the sheet spans {}..{} of {}",
        lo.x,
        hi.x,
        vp.size.x
    );
    // The foot too: the title block and the revision row are in it.
    assert!(
        lo.y >= 0.0 && hi.y <= vp.size.y,
        "the sheet spans {}..{} of {} down",
        lo.y,
        hi.y,
        vp.size.y
    );
}

fn esc_key() -> egui::Event {
    egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: Some(egui::Key::Escape),
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }
}

#[test]
fn escape_closes_the_standards_check_and_the_calculations_windows() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.run_command(&ctx, "check.standards", None);
    d.app.run_command(&ctx, "check.calculations", None);
    for _ in 0..3 {
        d.frame();
    }
    assert!(d.app.standards_open && d.app.wb.calculations.is_some());
    // The one on top goes first, then the other: never ten presses.
    for _ in 0..2 {
        d.frame_with(vec![esc_key()]);
        d.frame();
    }
    assert!(!d.app.standards_open, "Standards check closes");
    assert!(d.app.wb.calculations.is_none(), "Calculations closes");
}

#[test]
fn escape_closes_three_stacked_windows_one_after_another() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.run_command(&ctx, "check.standards", None);
    d.frame();
    d.app.wb.assistant.open = true;
    for _ in 0..2 {
        d.frame();
    }
    d.app.run_command(&ctx, "check.calculations", None);
    for _ in 0..3 {
        d.frame();
    }
    assert!(d.app.standards_open && d.app.wb.assistant.open && d.app.wb.calculations.is_some());
    // The pointer rests over the drawing, which would take the key first.
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(1000.0, 650.0))]);
    for _ in 0..3 {
        d.frame_with(vec![esc_key()]);
        d.frame();
    }
    assert!(d.app.wb.calculations.is_none(), "Calculations closes");
    assert!(!d.app.wb.assistant.open, "Ask Claude closes");
    assert!(!d.app.standards_open, "Standards check closes");
}
