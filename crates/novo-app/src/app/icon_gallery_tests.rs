//! The icon gallery (lane V0 of the visual redesign): opens from the command
//! field, draws both tabs, and lists every command with its home.

use novo_workflow::commands::COMMANDS;

use super::gate1_tests::Drive;
use super::icon_gallery::{home_text, matching};

#[test]
fn the_command_opens_and_closes_the_gallery() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    assert!(!d.app.wb.icon_gallery.open);
    d.app.wb.queued.push("view.icon_gallery");
    d.frame();
    d.frame();
    assert!(d.app.wb.icon_gallery.open);
    d.app.wb.queued.push("view.icon_gallery");
    d.frame();
    assert!(!d.app.wb.icon_gallery.open);
}

#[test]
fn the_command_runs_in_the_app() {
    assert!(super::workbench::app_runs("view.icon_gallery"));
}

#[test]
fn the_open_gallery_draws_in_both_themes_and_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.queued.push("view.icon_gallery");
    for dark in [true, false] {
        novo_theme::set_theme(
            &d.ctx,
            if dark {
                novo_theme::ThemeChoice::Dark
            } else {
                novo_theme::ThemeChoice::Light
            },
        );
        for tab in 0..2 {
            d.app.wb.icon_gallery.set_tab(tab);
            d.frame();
            d.frame();
            assert!(d.app.wb.icon_gallery.open, "dark {dark}, tab {tab}");
        }
    }
}

#[test]
fn the_filter_finds_commands_by_id_label_short_label_and_home() {
    assert_eq!(matching("").len(), COMMANDS.len());
    assert!(
        matching("icon gallery")
            .iter()
            .any(|c| c.id == "view.icon_gallery")
    );
    assert!(matching("export.pdf").iter().any(|c| c.id == "export.pdf"));
    assert!(
        matching("palette only")
            .iter()
            .any(|c| c.id == "edit.escape")
    );
    assert!(matching("zzzz-no-such").is_empty());
}

#[test]
fn every_command_shows_a_home_in_the_table() {
    for c in COMMANDS {
        assert_ne!(home_text(c.id), "no home", "{}", c.id);
    }
    assert_eq!(home_text("file.save"), "File");
    assert_eq!(home_text("view.icon_gallery"), "palette only");
}

#[test]
fn add_on_icons_and_commands_show_in_the_gallery_and_leave_when_unloaded() {
    use novo_theme::{drawn, presentation};
    let _tables = super::addons_look::TABLES
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let icon = drawn::register(drawn::IconSpec {
        name: "pt-gallery.pendant".into(),
        title: "Pendant".into(),
        shapes: vec![drawn::ShapeSpec::Rect {
            x: 6.0,
            y: 3.0,
            w: 12.0,
            h: 18.0,
            r: 2.0,
        }],
    })
    .unwrap();
    presentation::register(
        "addon.pt-gallery.panel",
        presentation::Glyph::Drawn(icon),
        "Pendant",
    )
    .unwrap();

    let rows = super::icon_gallery::rows_for("addon.pt-gallery");
    assert_eq!(rows.len(), 1);
    assert!(rows[0].addon);

    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.wb.queued.push("view.icon_gallery");
    for tab in 0..2 {
        d.app.wb.icon_gallery.set_tab(tab);
        d.frame();
        d.frame();
    }

    presentation::clear();
    drawn::clear();
    assert!(super::icon_gallery::rows_for("addon.pt-gallery").is_empty());
}
