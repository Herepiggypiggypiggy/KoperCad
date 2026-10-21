//! Acceptance A-07: a project opened from its file, and Home, show the whole
//! page: frame and title block inside the canvas, clear of the tool dock.

use novo_canvas::Vec2;

use super::gate1_tests::Drive;
use crate::actions::Action;

/// Screen corners of the sheet on the shown page, and the canvas size.
fn sheet_on_screen(d: &mut Drive) -> ([Vec2; 2], Vec2) {
    let view = d.app.canvas.schematic().expect("a schematic canvas");
    let rect = view
        .scene()
        .and_then(|s| s.scene().sheet().map(|sheet| sheet.rect))
        .expect("the page has a sheet");
    let vp = &view.canvas.viewport;
    (
        [
            vp.world_to_screen(Vec2::new(rect.min.x, rect.min.y)),
            vp.world_to_screen(Vec2::new(rect.max.x, rect.max.y)),
        ],
        vp.size,
    )
}

fn assert_whole_sheet(d: &mut Drive, what: &str) {
    let ([lo, hi], size) = sheet_on_screen(d);
    // The dock floats over the bottom of the canvas: a fit keeps clear of it.
    let dock = 70.0;
    assert!(
        lo.x >= 0.0 && lo.y >= 0.0 && hi.x <= size.x && hi.y <= size.y - dock + 1.0,
        "{what}: sheet {lo:?}..{hi:?} in a canvas of {size:?}"
    );
    // And it is the sheet that fills the view, not a corner of a huge zoom.
    assert!(
        (hi.x - lo.x) > size.x * 0.5 || (hi.y - lo.y) > (size.y - dock) * 0.5,
        "{what}: sheet only {lo:?}..{hi:?} in {size:?}"
    );
}

#[test]
fn a_reopened_project_and_home_show_the_whole_sheet() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    for _ in 0..4 {
        d.frame();
    }
    let file = dir.path().join("Example.novo");
    assert!(d.app.save_to(&file));
    d.frame();
    d.app.run(&ctx, Action::CloseProject);
    d.frame();

    d.app.open_path(&file);
    for _ in 0..6 {
        d.frame();
    }
    assert_whole_sheet(&mut d, "after reopening");

    // Zoom in and away, then Home.
    d.app.canvas.schematic().unwrap().canvas.viewport.zoom *= 4.0;
    d.app.canvas.schematic().unwrap().canvas.viewport.center = Vec2::new(10.0, 10.0);
    d.frame();
    d.app.run(&ctx, Action::ZoomToFit);
    for _ in 0..6 {
        d.frame();
    }
    assert_whole_sheet(&mut d, "after Home");
}
