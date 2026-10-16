//! The Languages window and the report language: found through the File
//! menu and the palette, saved in the project, one undo step, used by the
//! PDF options.

use novo_reports::i18n::{self, ProjectTable};
use novo_tests::motor_starter;

use super::gate1_tests::Drive;
use super::languages::{Source, missing_as_text, rows};
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    motor_starter(d.app.backend.document_mut().unwrap()).unwrap();
    d.frame();
    (dir, d)
}

#[test]
fn the_command_is_in_the_file_menu_and_opens_the_window() {
    assert!(novo_workflow::commands::is_known("project.languages"));
    assert!(super::workbench::app_runs("project.languages"));
    assert!(
        novo_workflow::menus::entries("File")
            .iter()
            .any(|(info, _)| info.id == "project.languages")
    );
    let (_dir, mut d) = open();
    d.app.wb.queued.push("project.languages");
    d.frame();
    assert!(d.app.wb.languages.is_some());
    d.app.wb.queued.push("project.languages");
    d.frame();
    assert!(d.app.wb.languages.is_none());
}

#[test]
fn choosing_a_language_is_saved_in_the_project_and_undone_in_one_step() {
    let (_dir, mut d) = open();
    assert!(d.app.report_translator().is_english());
    d.app.set_report_language("nl");
    let project = d.app.backend.document().unwrap().project().clone();
    assert_eq!(i18n::language_of(&project), "nl");
    assert_eq!(d.app.report_translator().tr("Wire list"), "Draadlijst");
    d.app.backend.document_mut().unwrap().undo().unwrap();
    assert!(d.app.report_translator().is_english());
}

#[test]
fn rows_list_missing_texts_and_project_entries_win() {
    let (_dir, mut d) = open();
    let mut table = ProjectTable::default();
    table.put_language(i18n::Catalog::new("de", "Deutsch"));
    table.set("de", "Wire list", "Drahtliste");
    d.app.set_translation_table("Add language", &table);
    d.app.set_report_language("de");
    let project = d.app.backend.document().unwrap().project().clone();
    let tr = d.app.report_translator();
    let list = rows(&project, &tr);
    let wire = list.iter().find(|r| r.english == "Wire list").unwrap();
    assert_eq!(
        (wire.source, wire.text.as_str()),
        (Source::Project, "Drahtliste")
    );
    let device = list.iter().find(|r| r.english == "Device list").unwrap();
    assert_eq!(device.source, Source::Missing);
    assert!(missing_as_text(&list).contains("Device list\t"));

    // Dutch has nothing missing in this project.
    d.app.set_report_language("nl");
    let project = d.app.backend.document().unwrap().project().clone();
    let list = rows(&project, &d.app.report_translator());
    assert!(
        list.iter().all(|r| r.source != Source::Missing),
        "{}",
        missing_as_text(&list)
    );
}
