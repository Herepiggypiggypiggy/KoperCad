//! The top bar's own pieces (lane V2a of the visual redesign): what the
//! segments show at each window width, and the page switcher's helpers.

use super::menu_bar::{ICON_ONLY_BELOW, TopBarFit};
use super::top_bar::cut;
use super::workspace::{SegmentNames, Workspace};

#[test]
fn the_segments_spend_names_by_window_width() {
    assert_eq!(TopBarFit::at(1920.0).segment_names, SegmentNames::All);
    assert_eq!(TopBarFit::at(1500.0).segment_names, SegmentNames::All);
    assert_eq!(TopBarFit::at(1366.0).segment_names, SegmentNames::All);
    assert_eq!(TopBarFit::at(1299.0).segment_names, SegmentNames::Current);
    assert_eq!(TopBarFit::at(1100.0).segment_names, SegmentNames::Current);
    assert_eq!(
        TopBarFit::at(ICON_ONLY_BELOW).segment_names,
        SegmentNames::Current
    );
    assert_eq!(
        TopBarFit::at(ICON_ONLY_BELOW - 1.0).segment_names,
        SegmentNames::None
    );
}

#[test]
fn the_electrical_workspaces_use_drawn_icons() {
    // 3D and Commissioning have no electrical symbol to draw; they keep the
    // general-purpose set.
    for workspace in [Workspace::Schematic, Workspace::Cabinet, Workspace::Reports] {
        assert!(
            matches!(workspace.glyph(), novo_theme::presentation::Glyph::Drawn(_)),
            "{workspace:?} has no drawn icon"
        );
    }
}

#[test]
fn long_page_names_are_cut_with_an_ellipsis() {
    assert_eq!(cut("Power", 44), "Power");
    let long = "x".repeat(60);
    let cut = cut(&long, 44);
    assert_eq!(cut.chars().count(), 44);
    assert!(cut.ends_with('…'));
}

#[test]
fn an_enabled_tile_is_clearly_brighter_than_a_disabled_one_in_both_themes() {
    fn luma(c: eframe::egui::Color32) -> f32 {
        0.2126 * f32::from(c.r()) + 0.7152 * f32::from(c.g()) + 0.0722 * f32::from(c.b())
    }
    for dark in [true, false] {
        let pal = novo_theme::Palette::for_mode(dark);
        let on = super::top_bar::tile_color(pal, true);
        let off = super::top_bar::tile_color(pal, false);
        assert!(
            (luma(on) - luma(off)).abs() >= 60.0,
            "dark={dark}: {on:?} vs {off:?}"
        );
        // Disabled stays readable: it is the palette's tertiary text, not a fade.
        assert_eq!(off, pal.text_tertiary);
    }
}
