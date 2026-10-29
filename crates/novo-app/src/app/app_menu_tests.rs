//! The app menu (lane V2b of the visual redesign): what it lists, how it is
//! opened, searched and driven from the keyboard.

use std::collections::HashSet;

use eframe::egui;
use novo_workflow::menus::{self, MENUS};

use super::app_menu::{Entry, area_entries, search_entries};
use super::gate1_tests::Drive;

fn ids(entries: &[Entry]) -> Vec<&'static str> {
    entries
        .iter()
        .filter_map(|e| match e {
            Entry::Command { info, .. } => Some(info.id),
            _ => None,
        })
        .collect()
}

fn hidden_names() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.settings.menu_names = Some(false);
    d.frame();
    (dir, d)
}

#[test]
fn every_command_of_the_menu_bar_is_in_the_app_menu() {
    // With every group unfolded the app menu lists what the bar lists, nothing
    // less: the two read one registry.
    for menu in MENUS {
        let groups: HashSet<&'static str> = menus::drawn(menu)
            .into_iter()
            .filter_map(|i| match i {
                menus::MenuItem::Submenu { name, .. } => Some(name),
                _ => None,
            })
            .collect();
        let listed = ids(&area_entries(menu, &groups));
        for (info, _) in menus::entries(menu) {
            assert!(
                listed.contains(&info.id),
                "{} is missing in {menu}",
                info.id
            );
        }
    }
}

#[test]
fn a_submenu_is_a_folded_group_until_it_is_opened() {
    let folded = area_entries("View", &HashSet::new());
    assert!(folded.iter().any(|e| matches!(
        e,
        Entry::Group {
            name: "3D",
            open: false,
            ..
        }
    )));
    let open = area_entries("View", &HashSet::from(["3D"]));
    assert!(ids(&open).len() > ids(&folded).len());
}

#[test]
fn the_search_looks_across_all_areas_by_label_id_and_keyword() {
    let found = ids(&search_entries("save as"));
    assert!(found.contains(&"file.save_as"), "{found:?}");
    let found = ids(&search_entries("file.new"));
    assert!(found.contains(&"file.new"));
    // All words must be present.
    assert!(ids(&search_entries("save zzzzqq")).is_empty());
    assert!(!ids(&search_entries("")).is_empty());
}

#[test]
fn the_menu_words_are_shown_by_default_and_the_setting_hides_them() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.frame();
    assert!(d.app.settings.show_menu_names());
    assert!(d.app.menu_names_shown(), "wide window, default setting");
    d.app.settings.menu_names = Some(false);
    assert!(!d.app.menu_names_shown());
    d.app.settings.menu_names = None;
    assert!(
        d.app.menu_names_shown(),
        "no choice made: the default again"
    );
}

#[test]
fn f10_opens_the_app_menu_when_the_names_are_hidden() {
    let (_dir, mut d) = hidden_names();
    assert!(!d.app.wb.app_menu.was_open);
    d.key(egui::Key::F10, egui::Modifiers::NONE);
    d.frame();
    d.frame();
    assert!(egui::Popup::is_any_open(&d.ctx));
    assert!(d.app.wb.app_menu.was_open);
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    d.frame();
    assert!(!egui::Popup::is_any_open(&d.ctx));
}

#[test]
fn f10_with_the_names_shown_moves_to_the_bar_and_opens_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.frame();
    d.key(egui::Key::F10, egui::Modifiers::NONE);
    d.frame();
    assert!(!d.app.wb.app_menu.was_open);
}

#[test]
fn typing_then_enter_runs_the_first_match() {
    let (_dir, mut d) = hidden_names();
    d.key(egui::Key::F10, egui::Modifiers::NONE);
    d.frame();
    d.app.wb.app_menu.filter = "new project".to_owned();
    d.frame();
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    assert_eq!(
        d.app.settings.recent_actions.first().map(String::as_str),
        Some("file.new"),
        "Enter picked the first result"
    );
    assert!(!egui::Popup::is_any_open(&d.ctx), "and closed the menu");
}

/// A command that lives only in the palette is not in the menus, so the
/// menu search says so instead of listing it.
#[test]
fn a_palette_only_command_is_not_found_in_the_menus() {
    assert!(ids(&search_entries("icon gallery")).is_empty());
}

#[test]
fn alt_and_a_letter_pick_an_area_and_arrows_walk_the_list() {
    let (_dir, mut d) = hidden_names();
    d.key(egui::Key::F10, egui::Modifiers::NONE);
    d.frame();
    d.key(egui::Key::H, egui::Modifiers::ALT);
    assert_eq!(MENUS[d.app.wb.app_menu.area], "Help");
    assert!(d.app.wb.app_menu.in_rows);
    // Back to the list, then up through the areas.
    d.key(egui::Key::ArrowLeft, egui::Modifiers::NONE);
    assert!(!d.app.wb.app_menu.in_rows);
    d.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    assert_eq!(MENUS[d.app.wb.app_menu.area], "Simulate");
    d.key(egui::Key::ArrowRight, egui::Modifiers::NONE);
    assert!(d.app.wb.app_menu.in_rows);
    d.key(egui::Key::ArrowDown, egui::Modifiers::NONE);
    assert_eq!(d.app.wb.app_menu.row, 1);
}

#[test]
fn the_menu_draws_in_both_themes_at_a_small_and_a_large_window() {
    let (_dir, mut d) = hidden_names();
    d.key(egui::Key::F10, egui::Modifiers::NONE);
    for dark in [true, false] {
        novo_theme::set_theme(
            &d.ctx,
            if dark {
                novo_theme::ThemeChoice::Dark
            } else {
                novo_theme::ThemeChoice::Light
            },
        );
        for size in [egui::vec2(1366.0, 768.0), egui::vec2(1920.0, 1080.0)] {
            for area in [0, 3, 7] {
                d.app.wb.app_menu.area = area;
                d.frame_sized(size, vec![]);
                d.frame_sized(size, vec![]);
                assert!(
                    d.app.wb.app_menu.was_open,
                    "dark {dark} {size:?} area {area}"
                );
            }
        }
    }
}

/// The Help button of the bar (shown when the menu words are hidden) opens
/// the app menu at its Help area.
#[test]
fn the_help_button_opens_the_menu_at_the_help_area() {
    let (_dir, mut d) = hidden_names();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    d.frame();
    d.frame();
    assert!(!d.app.wb.app_menu.was_open);
    d.app.wb.app_menu.open_at = MENUS.iter().position(|m| *m == "Help");
    d.frame();
    d.frame();
    assert!(d.app.wb.app_menu.was_open, "the menu is open");
    assert_eq!(MENUS[d.app.wb.app_menu.area], "Help");
}
