//! The Export and Import centres (lane V4): every command is a tile once,
//! search narrows the tiles, the windows open from File > Export..., File >
//! Import... and from the command field, and draw at both window sizes.

use eframe::egui;
use novo_workflow::centres::{self, Centre};

use super::centre::visible;
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
    let groups = visible(Centre::Export, "");
    assert_eq!(groups.len(), Centre::Export.sections().len());
    let tiles: usize = groups.iter().map(|(_, rows)| rows.len()).sum();
    assert_eq!(tiles, centres::export_ids().count());
    assert!(tiles >= 40, "{tiles} formats");
}

#[test]
fn a_search_matches_names_sentences_keywords_and_group_names() {
    let ids = |q: &str| -> Vec<&'static str> {
        visible(Centre::Export, q)
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
    assert!(d.app.wb.centre.is_none());
    d.app.run_command(&ctx, "file.export_centre", None);
    d.frame();
    assert!(d.app.wb.centre.is_some());
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
        assert!(d.app.wb.centre.is_some(), "open at {size:?}");
        // Every tile of the first group left an anchor for lessons.
        for id in Centre::Export.sections()[0].ids.iter().take(3) {
            assert!(
                d.app.learn_anchor(&format!("cmd.{id}")).is_some(),
                "{id} has no anchor at {size:?}"
            );
        }
        d.app.wb.centre = None;
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

#[test]
fn the_import_centre_lists_every_old_import_row_and_searches_them() {
    let groups = visible(Centre::Import, "");
    assert_eq!(groups.len(), Centre::Import.sections().len());
    let tiles: usize = groups.iter().map(|(_, rows)| rows.len()).sum();
    assert_eq!(tiles, Centre::Import.ids().count());
    let ids = |q: &str| -> Vec<&'static str> {
        visible(Centre::Import, q)
            .into_iter()
            .flat_map(|(_, rows)| rows.into_iter().map(|r| r.id))
            .collect()
    };
    assert!(ids("dwg").contains(&"file.import_dwg"));
    assert!(ids("eplan").contains(&"file.import_eplan"));
    assert!(ids("eplan").contains(&"parts.import_eplan"));
    assert!(ids("tags").contains(&"plc.import_tags"));
    assert!(!ids("tags").contains(&"file.import_dwg"));
    assert!(ids("zzzz").is_empty());
}

#[test]
fn the_import_command_opens_its_own_centre_and_file_has_one_import_row() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("file.import_centre"));
    d.app.run_command(&ctx, "file.import_centre", None);
    d.frame();
    assert_eq!(
        d.app.wb.centre.as_ref().map(|c| c.kind),
        Some(Centre::Import)
    );
    let file: Vec<_> = novo_workflow::menus::layout("File");
    let import_rows = file
        .iter()
        .filter(|i| match i {
            novo_workflow::menus::MenuItem::Command(c) => c.label.starts_with("Import"),
            novo_workflow::menus::MenuItem::Submenu { name, .. } => name.starts_with("Import"),
            _ => false,
        })
        .count();
    assert_eq!(import_rows, 1, "File holds one Import row");
}

#[test]
fn the_import_centre_draws_at_both_sizes_with_an_anchor_for_each_tile() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    let ctx = d.ctx.clone();
    for size in SIZES {
        d.app.run_command(&ctx, "file.import_centre", None);
        for _ in 0..3 {
            d.frame_sized(size, Vec::new());
        }
        for id in Centre::Import.sections()[0].ids {
            assert!(
                d.app.learn_anchor(&format!("cmd.{id}")).is_some(),
                "{id} has no anchor at {size:?}"
            );
        }
        d.app.wb.centre = None;
        d.frame_sized(size, Vec::new());
    }
    let fallback =
        super::learn_labels::anchor_fallback("cmd.file.import_dxf_background").expect("fallback");
    assert_eq!(fallback, "cmd.file.import_centre");
}

#[test]
fn the_start_centre_lists_templates_and_examples_and_file_has_one_row_for_them() {
    let groups = visible(Centre::Start, "");
    assert_eq!(groups.len(), 2);
    let ids = |q: &str| -> Vec<&'static str> {
        visible(Centre::Start, q)
            .into_iter()
            .flat_map(|(_, rows)| rows.into_iter().map(|r| r.id))
            .collect()
    };
    assert!(ids("crane").contains(&"file.new_crane"));
    assert!(ids("crane").contains(&"file.example_crane"));
    assert!(ids("emc").contains(&"file.example_emc"));
    assert!(!ids("pump").contains(&"file.new_crane"));
    assert!(ids("zzzz").is_empty());

    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("file.start_centre"));
    d.app.run_command(&ctx, "file.start_centre", None);
    d.frame();
    assert_eq!(
        d.app.wb.centre.as_ref().map(|c| c.kind),
        Some(Centre::Start)
    );
    let file = novo_workflow::menus::layout("File");
    let named = |what: &str| {
        file.iter()
            .filter(|i| match i {
                novo_workflow::menus::MenuItem::Command(c) => c.label.starts_with(what),
                novo_workflow::menus::MenuItem::Submenu { name, .. } => name.starts_with(what),
                _ => false,
            })
            .count()
    };
    assert_eq!(named("Templates and examples"), 1);
    assert_eq!(named("New from template"), 0);
    assert_eq!(named("Examples"), 0);
    let fallback =
        super::learn_labels::anchor_fallback("cmd.file.new_crane").expect("a tile has a fallback");
    assert_eq!(fallback, "cmd.file.start_centre");
}

#[test]
fn the_start_centre_draws_at_both_sizes_with_an_anchor_for_each_tile() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_project(dir.path());
    let ctx = d.ctx.clone();
    for size in SIZES {
        d.app.run_command(&ctx, "file.start_centre", None);
        for _ in 0..3 {
            d.frame_sized(size, Vec::new());
        }
        for id in Centre::Start.ids() {
            assert!(
                d.app.learn_anchor(&format!("cmd.{id}")).is_some(),
                "{id} has no anchor at {size:?}"
            );
        }
        d.app.wb.centre = None;
        d.frame_sized(size, Vec::new());
    }
}
