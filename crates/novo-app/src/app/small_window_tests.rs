//! What the app lays out in the smallest windows: nothing may reach beyond
//! the window (a status bar or pill half off screen is unreadable).

use super::gate1_tests::Drive;
use super::workspace::Workspace;
use crate::actions::Action;
use eframe::egui;

/// One frame; returns the area the frame used.
fn frame(d: &mut Drive, w: f32, h: f32) -> egui::Rect {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(w, h),
        )),
        ..Default::default()
    };
    let app = &mut d.app;
    let mut used = egui::Rect::NOTHING;
    let mut out = d.ctx.run_ui(input, |ui| {
        app.show(ui);
        used = ui.ctx().globally_used_rect();
    });
    out.textures_delta.clear();
    used
}

#[test]
fn the_layout_stays_inside_a_683_by_384_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    for workspace in [
        Workspace::Schematic,
        Workspace::Cabinet,
        Workspace::View3d,
        Workspace::Reports,
        Workspace::Commissioning,
    ] {
        d.app.switch_workspace(workspace);
        let mut used = egui::Rect::NOTHING;
        for _ in 0..3 {
            used = frame(&mut d, 683.0, 384.0);
        }
        assert!(
            used.right() <= 683.5 && used.bottom() <= 384.5,
            "{workspace:?}: the layout reaches {used:?}"
        );
    }
}

#[test]
fn preferences_fits_a_683_by_384_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.app.wb.preferences = true;
    for _ in 0..4 {
        frame(&mut d, 683.0, 384.0);
    }
    let ids = d.ctx.memory(|m| m.areas().visible_layer_ids());
    let layer = ids
        .into_iter()
        .find(|l| format!("{l:?}").contains("preferences"))
        .expect("the dialog is shown");
    let rect = d
        .ctx
        .memory(|m| m.area_rect(layer.id))
        .expect("the dialog has a rect");
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(683.0, 384.0));
    assert!(screen.contains_rect(rect), "{rect:?}");
}

#[test]
fn preferences_stays_usable_after_the_window_shrinks() {
    // A change of Interface size shrinks the screen in points at once.
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.app.wb.preferences = true;
    for _ in 0..3 {
        frame(&mut d, 1366.0, 768.0);
    }
    for _ in 0..6 {
        frame(&mut d, 683.0, 384.0);
    }
    let ids = d.ctx.memory(|m| m.areas().visible_layer_ids());
    let layer = ids
        .into_iter()
        .find(|l| format!("{l:?}").contains("preferences"))
        .expect("the dialog is shown");
    let rect = d.ctx.memory(|m| m.area_rect(layer.id)).unwrap();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(683.0, 384.0));
    assert!(screen.contains_rect(rect), "{rect:?} outside {screen:?}");
}

#[test]
fn the_start_screen_scrolls_when_the_recent_list_does_not_fit() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.settings.recent_projects = (0..10)
        .map(|n| std::path::PathBuf::from(format!("/work/project-{n}.novo")))
        .collect();
    let mut used = egui::Rect::NOTHING;
    for _ in 0..4 {
        used = frame(&mut d, 683.0, 384.0);
    }
    assert!(
        used.right() <= 683.5 && used.bottom() <= 384.5,
        "the start screen reaches {used:?}"
    );
}

fn learn_rect(d: &Drive) -> egui::Rect {
    let ids = d.ctx.memory(|m| m.areas().visible_layer_ids());
    let layer = ids
        .into_iter()
        .find(|l| format!("{l:?}").contains("learn_window"))
        .expect("the Learn window is shown");
    d.ctx.memory(|m| m.area_rect(layer.id)).unwrap()
}

#[test]
fn the_learn_window_regrows_after_the_app_window_shrinks_and_grows() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.app.learn.window = true;
    for _ in 0..4 {
        frame(&mut d, 1366.0, 768.0);
    }
    let before = learn_rect(&d);
    for _ in 0..6 {
        frame(&mut d, 683.0, 384.0);
    }
    for _ in 0..6 {
        frame(&mut d, 1366.0, 768.0);
    }
    let after = learn_rect(&d);
    assert!(
        after.width() >= before.width() * 0.9 && after.height() >= before.height() * 0.9,
        "the Learn window stayed {after:?}, it was {before:?}"
    );
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1366.0, 768.0));
    assert!(screen.contains_rect(after), "{after:?} outside {screen:?}");
}
