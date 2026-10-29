//! Every window of the program can be dragged and resized, stays on the
//! screen at 1366x768, remembers where it was left and comes back with
//! View > Reset panel layout. These tests drive the whole app; the helpers
//! themselves are tested in `panel.rs`.

use eframe::egui;

use super::gate1_tests::Drive;
use super::panel;
use crate::actions::Action;

const SIZE: egui::Vec2 = egui::vec2(1366.0, 768.0);

fn screen() -> egui::Rect {
    egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)
}

fn press(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn open_project() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    for _ in 0..3 {
        d.frame_sized(SIZE, vec![]);
    }
    (dir, d)
}

fn settle(d: &mut Drive) {
    for _ in 0..4 {
        d.frame_sized(SIZE, vec![]);
    }
}

/// Presses at `from`, drags in steps to `to` and lets go.
fn drag(d: &mut Drive, from: egui::Pos2, to: egui::Pos2) {
    d.frame_sized(SIZE, vec![egui::Event::PointerMoved(from)]);
    d.frame_sized(SIZE, vec![press(from, true)]);
    for step in 1..=5 {
        let at = from + (to - from) * (step as f32 / 5.0);
        d.frame_sized(SIZE, vec![egui::Event::PointerMoved(at)]);
    }
    d.frame_sized(SIZE, vec![press(to, false)]);
    settle(d);
}

fn rect_of(d: &Drive, id: egui::Id) -> Option<egui::Rect> {
    d.ctx.memory(|m| m.area_rect(id))
}

/// A window at the default size of the program (not pinned: the navigator
/// is a flyout below 1700 points).
#[test]
fn the_navigator_flyout_is_a_window_that_moves_resizes_and_resets() {
    let (_dir, mut d) = open_project();
    assert!(
        !d.app.outline_pinned(),
        "a 1366 window floats the navigator"
    );
    d.app.outline_open = true;
    settle(&mut d);
    let id = panel::id("outline_flyout");
    let home = rect_of(&d, id).expect("the navigator is shown");
    assert!(screen().contains_rect(home), "{home:?}");
    assert!(
        home.left() < 40.0 + super::rail::WIDTH,
        "it hangs from the left edge, beside the rail: {home:?}"
    );

    // Moved by its title bar.
    let grab = home.center_top() + egui::vec2(0.0, 10.0);
    drag(&mut d, grab, grab + egui::vec2(380.0, -8.0));
    assert!(d.app.outline_open, "dragging it does not close it");
    let moved = rect_of(&d, id).unwrap();
    assert!(
        (moved.min - home.min - egui::vec2(380.0, -8.0)).length() < 8.0,
        "{home:?} -> {moved:?}"
    );
    assert!(screen().contains_rect(moved), "{moved:?}");

    // Resized from its bottom right corner.
    let corner = moved.right_bottom() - egui::vec2(2.0, 2.0);
    drag(&mut d, corner, corner + egui::vec2(120.0, -150.0));
    let sized = rect_of(&d, id).unwrap();
    assert!(sized.width() > moved.width() + 80.0, "{moved:?} {sized:?}");
    assert!(
        sized.height() < moved.height() - 100.0,
        "{moved:?} {sized:?}"
    );

    // Back home.
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.reset_panels", None);
    settle(&mut d);
    let back = rect_of(&d, id).unwrap();
    assert!((back.min - home.min).length() < 4.0, "{home:?} {back:?}");
    assert!(
        (back.size() - home.size()).length() < 4.0,
        "{home:?} {back:?}"
    );
}

#[test]
fn the_pinned_navigator_column_is_dragged_wider() {
    let (_dir, mut d) = open_project();
    d.app.settings.outline_pinned = Some(true);
    settle(&mut d);
    let before = d.app.wb.outline_shown.expect("the column is shown");
    // Just inside the column's right edge, in the middle of the window.
    let rail = d.app.wb.rail_rect.map_or(0.0, |r| r.right());
    let edge = egui::pos2(rail + before - 2.0, 300.0);
    drag(&mut d, edge, edge + egui::vec2(110.0, 0.0));
    let after = d.app.wb.outline_shown.unwrap();
    assert!(after > before + 80.0, "{before} -> {after}");
    assert!(after <= 520.0, "capped: {after}");
}

#[test]
fn about_and_preferences_are_windows_that_open_on_screen_and_move() {
    for (name, id) in [("about", "about"), ("preferences", "preferences")] {
        let (_dir, mut d) = open_project();
        match name {
            "about" => d.app.show_about = true,
            _ => d.app.wb.preferences = true,
        }
        settle(&mut d);
        let win = egui::Id::new(("modal", id));
        let home = rect_of(&d, win).unwrap_or_else(|| panic!("{name} is shown"));
        assert!(screen().contains_rect(home), "{name}: {home:?}");
        let grab = home.center_top() + egui::vec2(0.0, 10.0);
        drag(&mut d, grab, grab + egui::vec2(-200.0, 60.0));
        let moved = rect_of(&d, win).unwrap();
        assert!(
            (moved.min - home.min - egui::vec2(-200.0, 60.0)).length() < 8.0,
            "{name}: {home:?} -> {moved:?}"
        );
        assert!(screen().contains_rect(moved), "{name}: {moved:?}");
    }
}

fn frame_at(d: &mut Drive, t: f64, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        events,
        ..Default::default()
    };
    let app = &mut d.app;
    let mut output = d.ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}

/// Two quick clicks at `at`, starting at time `t`; returns the time after.
fn double_click(d: &mut Drive, at: egui::Pos2, t: f64) -> f64 {
    frame_at(d, t, vec![egui::Event::PointerMoved(at)]);
    for (i, dt) in [0.1, 0.15, 0.2, 0.25].into_iter().enumerate() {
        frame_at(d, t + dt, vec![press(at, i % 2 == 0)]);
    }
    for k in 0..4 {
        frame_at(d, t + 0.5 + k as f64 * 0.1, vec![]);
    }
    t + 5.0
}

fn near(a: egui::Rect, b: egui::Rect) -> bool {
    (a.min - b.min).length() < 3.0 && (a.size() - b.size()).length() < 3.0
}

#[test]
fn a_double_click_on_a_panel_title_maximises_it_and_again_restores_it() {
    let (_dir, mut d) = open_project();
    d.app.outline_open = true;
    settle(&mut d);
    let id = panel::id("outline_flyout");
    let home = rect_of(&d, id).expect("the navigator is shown");
    let (_, canvas) = d.app.wb.canvas_area.unwrap();

    let t = double_click(&mut d, home.center_top() + egui::vec2(0.0, 10.0), 100.0);
    let big = rect_of(&d, id).unwrap();
    assert!(
        big.width() > canvas.width() - 40.0 && big.height() > canvas.height() - 100.0,
        "fills the canvas: {home:?} -> {big:?} on {canvas:?}"
    );
    assert!(canvas.contains_rect(big), "{big:?} on {canvas:?}");
    assert!(d.app.outline_open, "a double click does not close it");

    // The title is at the top of the big window now.
    double_click(&mut d, big.center_top() + egui::vec2(0.0, 10.0), t);
    let back = rect_of(&d, id).unwrap();
    assert!(near(back, home), "{home:?} -> {back:?}");
}

#[test]
fn a_double_click_on_a_title_does_not_fold_the_panel_it_maximises() {
    let (_dir, mut d) = open_project();
    d.app.outline_open = true;
    settle(&mut d);
    let id = panel::id("outline_flyout");
    let home = rect_of(&d, id).unwrap();
    double_click(&mut d, home.center_top() + egui::vec2(0.0, 10.0), 100.0);
    let big = rect_of(&d, id).unwrap();
    // Folded, a panel is one title bar tall.
    assert!(big.height() > 200.0, "{big:?}");
}

#[test]
fn a_double_click_on_a_modal_title_maximises_it_and_again_restores_it() {
    let (_dir, mut d) = open_project();
    d.app.wb.preferences = true;
    settle(&mut d);
    let win = egui::Id::new(("modal", "preferences"));
    let home = rect_of(&d, win).expect("preferences is shown");
    let t = double_click(&mut d, home.center_top() + egui::vec2(0.0, 10.0), 100.0);
    let big = rect_of(&d, win).unwrap();
    assert!(
        big.width() > SIZE.x - 40.0 && big.height() > SIZE.y - 80.0,
        "fills the window: {home:?} -> {big:?}"
    );
    assert!(screen().contains_rect(big), "{big:?}");
    // The pointer ends outside the small window it restores to: that must
    // not count as a click on the ground behind it (it closes the window).
    double_click(&mut d, big.center_top() + egui::vec2(0.0, 10.0), t);
    let back = rect_of(&d, win).unwrap();
    assert!(near(back, home), "{home:?} -> {back:?}");
    assert!(d.app.wb.preferences, "the restored window is still open");
}

#[test]
fn a_double_click_on_a_dialog_title_maximises_it_and_again_restores_it() {
    let (_dir, mut d) = open_project();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "export.pdf_options", None);
    settle(&mut d);
    let win = egui::Id::new(Some("Export PDF"));
    let home = rect_of(&d, win).expect("the dialog is shown");
    let t = double_click(&mut d, home.center_top() + egui::vec2(0.0, 10.0), 100.0);
    let big = rect_of(&d, win).unwrap();
    assert!(
        big.width() > SIZE.x - 40.0 && big.height() > SIZE.y - 80.0,
        "fills the window: {home:?} -> {big:?}"
    );
    assert!(screen().contains_rect(big), "{big:?}");
    double_click(&mut d, big.center_top() + egui::vec2(0.0, 10.0), t);
    let back = rect_of(&d, win).unwrap();
    assert!(near(back, home), "{home:?} -> {back:?}");
}

#[test]
fn reset_panel_layout_forgets_a_maximised_window() {
    let (_dir, mut d) = open_project();
    d.app.outline_open = true;
    settle(&mut d);
    let id = panel::id("outline_flyout");
    let home = rect_of(&d, id).unwrap();
    double_click(&mut d, home.center_top() + egui::vec2(0.0, 10.0), 100.0);
    assert!(rect_of(&d, id).unwrap().width() > home.width() + 100.0);
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.reset_panels", None);
    settle(&mut d);
    let back = rect_of(&d, id).unwrap();
    assert!(near(back, home), "{home:?} -> {back:?}");
}

#[test]
fn a_double_click_on_the_edge_of_the_pinned_navigator_gives_it_its_width_back() {
    let (_dir, mut d) = open_project();
    d.app.settings.outline_pinned = Some(true);
    settle(&mut d);
    let home = d.app.wb.outline_shown.expect("the column is shown");
    // The column sits right of the tool rail.
    let rail = d.app.wb.rail_rect.map_or(0.0, |r| r.right());
    let edge = egui::pos2(rail + home - 2.0, 300.0);
    drag(&mut d, edge, edge + egui::vec2(120.0, 0.0));
    let wide = d.app.wb.outline_shown.unwrap();
    assert!(wide > home + 80.0, "{home} -> {wide}");
    let edge = egui::pos2(rail + wide - 2.0, 300.0);
    double_click(&mut d, edge, 100.0);
    let back = d.app.wb.outline_shown.unwrap();
    assert!((back - home).abs() < 3.0, "{home} -> {wide} -> {back}");
}

fn panel_rect(d: &Drive, name: &str) -> Option<egui::Rect> {
    egui::containers::panel::PanelState::load(&d.ctx, egui::Id::new(name)).map(|s| s.outer_rect)
}

#[test]
fn the_pinned_pages_column_moves_to_the_right_side_and_back() {
    let (_dir, mut d) = open_project();
    d.app.settings.outline_pinned = Some(true);
    settle(&mut d);
    let left = panel_rect(&d, "outline").expect("the column is shown at the left");
    assert!(left.left() < 80.0, "{left:?}");
    super::panel_dock::set_navigator_right(&d.ctx, true);
    settle(&mut d);
    let right = panel_rect(&d, "outline_right").expect("the column is shown at the right");
    assert!(right.right() > SIZE.x - 2.0, "{right:?}");
    assert!(d.app.wb.outline_shown.is_some_and(|w| w > 100.0));
    // Reset panel layout brings it home.
    panel::reset_layout(&d.ctx);
    settle(&mut d);
    assert!(!super::panel_dock::navigator_right(&d.ctx));
    assert!(panel_rect(&d, "outline").is_some());
}

#[test]
fn the_pages_column_docks_as_a_pane_of_a_strip_with_other_panels() {
    let (_dir, mut d) = open_project();
    d.app.settings.outline_pinned = Some(true);
    settle(&mut d);
    assert!(panel_rect(&d, "outline").is_some());
    // Docked: no column, a pane in the left strip, the canvas to its right.
    super::panel_dock::set(
        &d.ctx,
        super::panel_dock::PAGES,
        Some(super::panel_dock::Slot::Left),
    );
    settle(&mut d);
    let strip = panel_rect(&d, "dock_left").expect("the left strip");
    let pane = rect_of(&d, egui::Id::new("pages_pane").with("docked")).expect("the pages pane");
    assert!(
        strip.width() > 200.0 && pane.width() > 200.0,
        "{strip:?} {pane:?}"
    );
    assert!(pane.height() > 300.0, "{pane:?}");
    // The header buttons send it to the right strip, and back to the column.
    super::panel_dock::set(
        &d.ctx,
        super::panel_dock::PAGES,
        Some(super::panel_dock::Slot::Right),
    );
    settle(&mut d);
    let strip = panel_rect(&d, "dock_right").expect("the right strip");
    assert!(strip.right() > SIZE.x - 2.0, "{strip:?}");
    super::panel_dock::set(&d.ctx, super::panel_dock::PAGES, None);
    settle(&mut d);
    assert!(
        d.app.wb.outline_shown.is_some_and(|w| w > 100.0),
        "back in its column"
    );
    // Reset panel layout floats a docked Pages again.
    super::panel_dock::set(
        &d.ctx,
        super::panel_dock::PAGES,
        Some(super::panel_dock::Slot::Bottom),
    );
    settle(&mut d);
    assert_eq!(
        super::panel_dock::pages_docked(&d.ctx),
        Some(super::panel_dock::Slot::Bottom)
    );
    panel::reset_layout(&d.ctx);
    settle(&mut d);
    assert_eq!(super::panel_dock::pages_docked(&d.ctx), None);
}

/// The rule for new code: no window that is neither movable nor resizable.
/// Every floating panel goes through `panel.rs`; the few raw `egui::Area`s
/// that remain are not windows (hover tips, toasts, the radial wheel,
/// markers drawn on the sheet) and are listed here with the reason.
#[test]
fn no_fixed_window_is_built_outside_the_panel_helpers() {
    const AREAS: &[(&str, &str)] = &[
        ("hud", "cursor readout, not interactive"),
        ("\"docked\"", "a pane of a dock slot, drawn by panel_dock"),
        (
            "rail_flyout_card",
            "flyout card of the tool rail, closes on Esc or a click elsewhere",
        ),
        ("toasts", "messages that fade away"),
        ("refusal_card_id()", "a refusal that fades away"),
        ("empty_sheet_ghost", "a hint drawn on the empty sheet"),
        ("lesson_note", "lesson note beside a highlighted control"),
        ("lesson_demo_keys", "lesson demo key caps"),
        ("view3d_wire_hover", "hover tip by the pointer"),
        ("view3d_device_tip", "hover tip by the pointer"),
        ("view3d.drag_gaps", "tip by the pointer while dragging"),
        ("view3d.move_warning", "tip by the pointer while dragging"),
        ("occurrences", "pick list that closes on a click elsewhere"),
        ("cabinet_plate_menu", "context menu"),
        ("place_wheel_hidden", "the radial wheel"),
        ("place_wheel_catch", "the radial wheel"),
        (
            "command_field",
            "the command palette and the tool's text field",
        ),
        ("build_mode", "Build mode fills the canvas"),
        ("live_wire_tip", "hover tip on the sheet"),
        (
            "defect_banner",
            "strip over the sheet while a defect is injected, not interactive",
        ),
        ("live_input", "badge drawn on a symbol"),
        ("live_load", "badge drawn on a symbol"),
        ("live_pole", "badge drawn on a symbol"),
        ("live_coil", "badge drawn on a symbol"),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                files.push(path);
            }
        }
    }
    let mut found = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        // The helper itself, and test code that builds windows to drive.
        if name == "panel.rs" || name.ends_with("_tests.rs") || name == "panel_windows_tests.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // Code from an inline `#[cfg(test)] mod x {` on is test code (a
        // `#[cfg(test)] mod x;` line, as in app.rs, only registers a file).
        let lines: Vec<&str> = text.lines().collect();
        let end = (0..lines.len())
            .find(|&i| {
                lines[i].trim() == "#[cfg(test)]"
                    && lines.get(i + 1).is_some_and(|n| {
                        n.trim_start().starts_with("mod ") && n.trim_end().ends_with('{')
                    })
            })
            .unwrap_or(lines.len());
        let code = lines[..end].join("\n");
        let code = code.as_str();
        for needle in ["egui::Modal::new(", "egui::Window::new("] {
            if code.contains(needle) {
                found.push(format!("{name}: {needle}"));
            }
        }
        for (at, _) in code.match_indices("egui::Area::new(") {
            let near = &code[at..(at + 260).min(code.len())];
            let near_before = &code[at.saturating_sub(160)..at];
            let allowed = AREAS
                .iter()
                .any(|(id, _)| near.contains(id) || near_before.contains(id));
            if !allowed {
                found.push(format!("{name}: an egui::Area that is not on the list"));
            }
        }
    }
    assert!(
        found.is_empty(),
        "use panel::Panel, panel::Float, panel::dialog or panel::modal, or list the area in this test with the reason it is not a window:\n{}",
        found.join("\n")
    );
}
