//! The Reports hub: the live tables and the Excel and CSV exports.

use novo_core::AnyId;

use super::gate1_tests::Drive;
use super::reports_hub::{Format, place_of};
use crate::actions::Action;

fn example() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    (dir, d)
}

#[test]
fn the_hub_is_a_third_view_next_to_schematic_and_cabinet() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    assert!(d.app.is_enabled(Action::ShowReports));
    d.app.run(&ctx, Action::ShowReports);
    d.frame();
    assert!(d.app.hub.open && !d.app.cabinet_open);

    d.app.run(&ctx, Action::ShowCabinet);
    assert!(!d.app.hub.open && d.app.cabinet_open);
    d.app.run(&ctx, Action::ShowReports);
    assert!(d.app.hub.open && !d.app.cabinet_open);

    // Picking a page leaves it, as it leaves the Cabinet.
    let page = d.app.current_page().unwrap();
    d.app.show_page(page);
    assert!(!d.app.hub.open);
}

#[test]
fn the_palette_opens_the_hub_and_knows_the_export_commands() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.reports", None);
    d.frame();
    assert!(d.app.hub.open);
    assert!(novo_workflow::commands::get("view.reports").is_some());
    assert!(super::workbench::app_runs("view.reports"));
    assert!(super::workbench::app_runs("export.reports"));
}

#[test]
fn the_hub_draws_every_report_without_a_project_problem() {
    let (_dir, mut d) = example();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::ShowReports);
    d.frame();
    // Every report becomes the shown one in turn.
    let ids: Vec<String> = {
        let project = d.app.backend.document().unwrap().project();
        d.app
            .wb
            .reports
            .live(project)
            .ids()
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    };
    assert!(ids.len() >= 6, "{ids:?}");
    for id in ids {
        d.app.hub.selected = id;
        d.frame();
    }
    d.app.hub.filter = "K1".into();
    d.frame();
}

#[test]
fn excel_and_csv_exports_write_files() {
    let (dir, mut d) = example();
    let xlsx = dir.path().join("all.xlsx");
    assert_eq!(
        d.app.export_reports_to(None, Format::Xlsx, &xlsx),
        Some(xlsx.clone())
    );
    let bytes = std::fs::read(&xlsx).unwrap();
    assert!(bytes.starts_with(b"PK"), "a zip container");

    let csv = dir.path().join("bom.csv");
    assert!(
        d.app
            .export_reports_to(Some("bom"), Format::Csv, &csv)
            .is_some()
    );
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.starts_with('\u{feff}'));
    assert!(text.contains(','), "comma separated by default");

    d.app.settings.csv_semicolon = true;
    let semi = dir.path().join("bom2.csv");
    d.app.export_reports_to(Some("bom"), Format::Csv, &semi);
    let text = std::fs::read_to_string(&semi).unwrap();
    assert!(text.lines().next().unwrap().contains(';'));
}

#[test]
fn csv_needs_one_report_and_an_unknown_report_exports_nothing() {
    let (dir, mut d) = example();
    let csv = dir.path().join("all.csv");
    assert!(d.app.export_reports_to(None, Format::Csv, &csv).is_none());
    assert!(!csv.exists());
    assert!(
        d.app
            .export_reports_to(Some("nope"), Format::Xlsx, &dir.path().join("x.xlsx"))
            .is_none()
    );
}

#[test]
fn a_row_goes_to_its_symbol() {
    let (_dir, d) = example();
    let project = d.doc().project();
    let symbol = project.symbols().next().expect("the example draws symbols");
    let place = place_of(project, AnyId::Symbol(symbol.id)).unwrap();
    assert_eq!(place.page, symbol.page);
    assert_eq!(place.symbol, Some(symbol.id));
}

#[test]
fn a_report_without_a_page_can_be_added_as_one() {
    let (_dir, mut d) = example();
    let before = d.doc().project().pages().count();
    d.app.add_report_page("cutlist", "Cut list");
    let project = d.doc().project();
    assert_eq!(project.pages().count(), before + 1);
    let page = project
        .pages()
        .find(|p| matches!(&p.kind, novo_core::PageKind::Generated { generator } if generator == "cutlist"))
        .expect("added");
    assert!(page.name.parse::<u64>().unwrap() % 10 == 0);
    // One undo takes it back.
    d.app.undo_redo(false);
    assert_eq!(d.doc().project().pages().count(), before);
}

#[test]
fn an_export_never_replaces_a_project_file() {
    let (dir, mut d) = example();
    // The open project's own file, by name and by content.
    let project = dir.path().join("plant.novo");
    assert!(d.app.save_to(&project));
    let before = std::fs::read(&project).unwrap();
    assert!(before.starts_with(b"SQLite format 3"));
    assert!(
        d.app
            .export_reports_to(None, Format::Xlsx, &project)
            .is_none()
    );
    assert!(
        d.app
            .export_reports_to(Some("bom"), Format::Csv, &project)
            .is_none()
    );
    assert_eq!(std::fs::read(&project).unwrap(), before);

    // A database under another name is refused by its content.
    let other = dir.path().join("copy.bin");
    std::fs::write(&other, &before).unwrap();
    assert!(
        d.app
            .export_reports_to(None, Format::Xlsx, &other)
            .is_none()
    );
    assert_eq!(std::fs::read(&other).unwrap(), before);
}
