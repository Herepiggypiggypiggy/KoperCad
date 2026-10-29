//! The simulation status chip: what it says per state and that it is drawn
//! in the 3D view and the Cabinet with every panel closed.

use eframe::egui;

use super::gate1_tests::Drive;
use super::heat_status::SimStatus;
use super::sim_chip::{Ask, Tone, describe};
use super::view3d_tests::drive_with_cabinet;
use crate::actions::Action;

fn running(stopping: bool) -> SimStatus {
    SimStatus::Running {
        fraction: Some(0.4),
        label: "Coarse 3D · it 120 · 14 s".into(),
        stopping,
    }
}

#[test]
fn every_state_has_words_and_the_one_fitting_action() {
    let not_run = describe(&SimStatus::NotRun, false);
    assert_eq!(not_run.words, "Heat has not been run");
    assert_eq!(not_run.action, Some(("Run heat", Ask::Run)));

    let busy = describe(&running(false), false);
    assert_eq!(busy.words, "Heat is running");
    assert_eq!(busy.action, Some(("Cancel run", Ask::Cancel)));
    assert_eq!(busy.progress, Some(Some(0.4)));
    assert!(busy.detail.contains("it 120"));

    // After Cancel the run is ending: no second Cancel.
    let stopping = describe(&running(true), false);
    assert_eq!(stopping.words, "Heat is stopping");
    assert_eq!(stopping.action, None);

    let done = describe(
        &SimStatus::Done {
            label: "Coarse 3D · not validated".into(),
            converged: true,
        },
        false,
    );
    assert_eq!(done.words, "Heat is done");
    assert_eq!(done.action, Some(("Show heat", Ask::ShowHeat)));
    assert_eq!(done.tone, Tone::Good);
    // With the result on screen there is nothing to open.
    let shown = describe(
        &SimStatus::Done {
            label: "x".into(),
            converged: false,
        },
        true,
    );
    assert_eq!(shown.action, None);
    assert!(shown.detail.contains("not converged"));
    assert_eq!(shown.tone, Tone::Warn);

    let old = describe(&SimStatus::OutOfDate, true);
    assert_eq!(old.words, "Heat is out of date");
    assert_eq!(old.action, Some(("Run heat again", Ask::Run)));

    let failed = describe(&SimStatus::Failed("Cannot simulate: no size".into()), true);
    assert_eq!(failed.words, "Heat failed");
    assert!(failed.detail.contains("no size"));
    assert_eq!(failed.action, Some(("Run heat again", Ask::Run)));
    assert_eq!(failed.tone, Tone::Bad);
}

fn chip_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    let layer = egui::LayerId::new(egui::Order::Middle, super::panel::id("sim_status_chip"));
    ctx.memory(|m| m.areas().visible_last_frame(&layer))
        .then(|| ctx.memory(|m| m.area_rect(super::panel::id("sim_status_chip"))))
        .flatten()
}

/// Everything one frame paints as text.
fn painted_text(d: &mut Drive, size: egui::Vec2) -> String {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    let app = &mut d.app;
    let output = d.ctx.run_ui(input, |ui| app.show(ui));
    let mut text = String::new();
    for clipped in &output.shapes {
        collect_text(&clipped.shape, &mut text);
    }
    text
}

fn collect_text(shape: &egui::Shape, out: &mut String) {
    match shape {
        egui::Shape::Text(t) => {
            out.push_str(t.galley.text());
            out.push('\n');
        }
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| collect_text(s, out)),
        _ => {}
    }
}

#[test]
fn the_3d_view_shows_the_chip_with_heat_off_and_every_panel_closed() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d", None);
    assert!(!d.app.cabinet.heat.on);
    let size = egui::vec2(1366.0, 768.0);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    // The status is the first row of the toolbar, not a window of its own:
    // it takes no room from the panels under the toolbar.
    assert!(chip_rect(&ctx).is_none(), "3D has no separate chip card");
    let text = painted_text(&mut d, size);
    assert!(text.contains("Heat has not been run"), "{text}");
    assert!(text.contains("Run heat"), "{text}");
    let canvas = d.app.learn_anchor("workspace.view3d").expect("canvas");
    let toolbar = ctx
        .memory(|m| m.area_rect(super::panel::id("view3d_toolbar")))
        .expect("toolbar");
    assert!(
        toolbar.left() - canvas.left() < 30.0 && toolbar.top() - canvas.top() < 30.0,
        "{toolbar:?} {canvas:?}"
    );
    assert!(
        toolbar.bottom() - canvas.top() <= super::view3d_controls::BELOW,
        "the panels under the toolbar start at BELOW: {toolbar:?} {canvas:?}"
    );
}

#[test]
fn the_chip_follows_the_one_status_value_in_3d() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.3d", None);
    let size = egui::vec2(1366.0, 768.0);
    d.frame_sized(size, Vec::new());
    assert_eq!(d.app.cabinet.heat.sim_status(), SimStatus::NotRun);
    // A failure the Cabinet strip and the Heat panel show is the chip's too.
    d.app
        .cabinet
        .heat
        .fail("Cannot simulate: the enclosure size is not set".into());
    d.frame_sized(size, Vec::new());
    assert!(matches!(
        d.app.cabinet.heat.sim_status(),
        SimStatus::Failed(_)
    ));
    let text = painted_text(&mut d, size);
    assert!(text.contains("Heat failed"), "{text}");
}

#[test]
fn the_cabinet_shows_the_same_chip() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowCabinet);
    for _ in 0..4 {
        d.frame();
    }
    assert!(!d.app.cabinet.heat.on);
    assert!(
        chip_rect(&ctx).is_some(),
        "the chip is drawn in the Cabinet"
    );
}
