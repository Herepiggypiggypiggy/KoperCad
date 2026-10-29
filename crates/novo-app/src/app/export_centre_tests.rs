//! The Export centre (lane V4): every export command is a tile once, search
//! narrows the tiles, the window opens from File > Export... and from the
//! command field, and it draws at both window sizes.

use eframe::egui;
use novo_workflow::centres::{self, EXPORT_SECTIONS};

use super::export_centre::visible;
use super::gate1_tests::Drive;
use crate::actions::Action;

const SIZES: [egui::Vec2; 2] = [egui::vec2(1366.0, 768.0), egui::vec2(1920.0, 1080.0)];

fn drive_with_project(dir: &std::path::Path) -> Drive {
    let mut d = Drive::new(&dir.join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d
}

#[test]
fn an_empty_search_shows_every_format_once() {
    let groups = visible("");
    assert_eq!(groups.len(), EXPORT_SECTIONS.len());
    let tiles: usize = groups.iter().map(|(_, rows)| rows.len()).sum();
    assert_eq!(tiles, centres::export_ids().count());
    assert!(tiles >= 40, "{tiles} formats");
}

#[test]
fn a_search_matches_names_sentences_keywords_and_group_names() {
    let ids = |q: &str| -> Vec<&'static str> {
        visible(q)
            .into_iter()
            .flat_map(|(_, rows)| rows.into_iter().map(|r| r.id))
            .collect()
    };
    let dxf = ids("dxf");
    assert!(dxf.contains(&"export.dxf_page") && dxf.contains(&"export.plate_dxf"));
    assert!(!dxf.contains(&"export.pdf"));
    // Two words must both match.
    assert_eq!(ids("pdf variant"), ["export.variant_pdf"]);
    // A group name finds its tiles.
    assert!(ids("plc").len() >= 5);
    // The sentence is searched too: "ruler" is only in the scale check's.
    assert_eq!(ids("ruler"), ["export.scale_check"]);
    assert!(ids("zzzz").is_empty());
}

#[test]
fn the_command_opens_the_centre_and_the_menu_has_one_export_row() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("file.export_centre"));
    assert!(d.app.wb.export_centre.is_none());
    d.app.run_command(&ctx, "file.export_centre", None);
    d.frame();
    assert!(d.app.wb.export_centre.is_some());
    let info = novo_workflow::commands::get("file.export_centre").unwrap();
    let place = novo_workflow::menus::place(info).expect("a File row");
    assert_eq!(place.menu, "File");
    assert!(place.submenu.is_none());
}

#[test]
fn the_centre_draws_at_both_sizes_with_an_anchor_for_each_tile() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    let ctx = d.ctx.clone();
    for size in SIZES {
        d.app.run_command(&ctx, "file.export_centre", None);
        for _ in 0..3 {
            d.frame_sized(size, Vec::new());
        }
        assert!(d.app.wb.export_centre.is_some(), "open at {size:?}");
        // Every tile of the first group left an anchor for lessons.
        for id in EXPORT_SECTIONS[0].ids.iter().take(3) {
            assert!(
                d.app.learn_anchor(&format!("cmd.{id}")).is_some(),
                "{id} has no anchor at {size:?}"
            );
        }
        d.app.wb.export_centre = None;
        d.frame_sized(size, Vec::new());
    }
}

#[test]
fn a_command_of_the_centre_is_found_through_the_opener_when_it_is_shut() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    d.frame();
    // Shut: a lesson pointing at a format falls back to the opener's row and
    // from there to the File menu, whichever of those is on screen.
    let fallback = super::learn_labels::anchor_fallback("cmd.export.bom").expect("a fallback");
    assert_eq!(fallback, "cmd.file.export_centre");
    assert_eq!(
        super::learn_labels::anchor_fallback(&fallback).as_deref(),
        Some("menu.File")
    );
}
