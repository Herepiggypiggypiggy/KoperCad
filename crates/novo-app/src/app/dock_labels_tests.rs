//! The tool dock names its buttons: a word beside each icon when the
//! drawing has room (Auto), always, or never (icons only, tooltips stay).

use eframe::egui;

use super::dock::LABELS_MIN_CANVAS;
use super::gate1_tests::Drive;
use crate::actions::Action;
use crate::settings::DockLabels;

/// A new project at this window size and label choice; returns the dock's
/// rect and the canvas rect. Always the dark theme: the resolved theme is a
/// process-wide flag (`crate::theme`), so a test that switched it would
/// race the others running beside it. The light theme is checked on the
/// widget alone in `the_labelled_button_is_wider_in_both_themes`.
fn shoot(size: egui::Vec2, labels: DockLabels) -> (egui::Rect, egui::Rect) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
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
        {
            let size = egui::vec2(w, h);
            let (dock, canvas) = shoot(size, DockLabels::Auto);
            let (icons_only, _) = shoot(size, DockLabels::Never);
            let labelled = canvas.width() >= LABELS_MIN_CANVAS;
            assert_eq!(
                dock.width() > icons_only.width() + 1.0,
                labelled,
                "{w}x{h}: canvas {} wide, dock {} vs {}",
                canvas.width(),
                dock.width(),
                icons_only.width()
            );
            assert!(
                dock.left() >= canvas.left() && dock.right() <= canvas.right(),
                "{w}x{h}: dock {dock:?} outside canvas {canvas:?}"
            );
        }
    }
}

#[test]
fn the_dock_label_choice_is_honoured_and_remembered() {
    let size = egui::vec2(1920.0, 1080.0);
    let (never_dock, _) = shoot(size, DockLabels::Never);
    let (always_dock, canvas) = shoot(size, DockLabels::Always);
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

#[test]
fn the_labelled_button_is_wider_in_both_themes() {
    use novo_theme::{Density, ThemeChoice, icons, widgets};
    for theme in [ThemeChoice::Dark, ThemeChoice::Light] {
        let ctx = egui::Context::default();
        novo_theme::apply_with(&ctx, theme, Density::Standard);
        let mut widths = [0.0_f32; 2];
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            widths[0] = widgets::tool_button(ui, icons::tool::UNDO, false, "Undo")
                .rect
                .width();
            widths[1] = widgets::tool_button_labeled(ui, icons::tool::UNDO, "Undo", false, "Undo")
                .rect
                .width();
        });
        output.textures_delta.clear();
        assert!(widths[1] > widths[0] + 10.0, "{theme:?}: {widths:?}");
    }
}
