//! The tool dock names its buttons: a word beside each icon when the
//! drawing has room (Auto), always, or never (icons only, tooltips stay).

use eframe::egui;

use super::dock::LABELS_MIN_CANVAS;
use super::gate1_tests::Drive;
use crate::actions::Action;
use crate::settings::DockLabels;

/// A new project at this window size, theme and label choice; returns the
/// dock's rect and the canvas rect.
fn shoot(size: egui::Vec2, theme: &str, labels: DockLabels) -> (egui::Rect, egui::Rect) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.settings.appearance_theme = theme.into();
    d.app.settings.set_dock_labels(labels);
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    let dock = ctx
        .memory(|m| m.area_rect(egui::Id::new("tool_dock")))
        .expect("the dock is shown");
    let (_, canvas) = d.app.wb.canvas_area.expect("a canvas");
    (dock, canvas)
}

#[test]
fn the_dock_has_words_where_there_is_room_and_always_fits() {
    for (w, h) in [(1280.0, 768.0), (1366.0, 768.0), (1920.0, 1080.0)] {
        for theme in ["dark", "light"] {
            let size = egui::vec2(w, h);
            let (dock, canvas) = shoot(size, theme, DockLabels::Auto);
            let (icons_only, _) = shoot(size, theme, DockLabels::Never);
            let labelled = canvas.width() >= LABELS_MIN_CANVAS;
            assert_eq!(
                dock.width() > icons_only.width() + 1.0,
                labelled,
                "{w}x{h} {theme}: canvas {} wide, dock {} vs {}",
                canvas.width(),
                dock.width(),
                icons_only.width()
            );
            assert!(
                dock.left() >= canvas.left() && dock.right() <= canvas.right(),
                "{w}x{h} {theme}: dock {dock:?} outside canvas {canvas:?}"
            );
        }
    }
}

#[test]
fn the_dock_label_choice_is_honoured_and_remembered() {
    let size = egui::vec2(1920.0, 1080.0);
    let (never_dock, _) = shoot(size, "dark", DockLabels::Never);
    let (always_dock, canvas) = shoot(size, "dark", DockLabels::Always);
    assert!(always_dock.width() > never_dock.width());
    assert!(always_dock.width() < canvas.width());

    let mut s = crate::settings::Settings::default();
    assert_eq!(s.dock_labels(), DockLabels::Auto);
    for choice in [DockLabels::Always, DockLabels::Never, DockLabels::Auto] {
        s.set_dock_labels(choice);
        let again: crate::settings::Settings = ron::from_str(&ron::to_string(&s).unwrap()).unwrap();
        assert_eq!(again.dock_labels(), choice);
    }
}
