//! The table import dialog: the command opens it, the mapping can be
//! changed, nothing reaches the catalog before Add, and a corrected mapping
//! is remembered for the next file with the same columns.

use novo_parts::{Catalog, RowStatus, Target};

use super::gate1_tests::Drive;
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

const TABLE: &str = "Hersteller;Artikel;Bezeichnung;Nennstrom [A];Verlustleistung (W);Preis;Notiz\n\
Example Maker;EX-1;Schütz;9;1,2;12,50 €;a\n\
Example Maker;EX-2;Schütz gross;12;;18,00 €;b\n\
Example Maker;;Ohne Nummer;1;;;c\n";

#[test]
fn the_command_is_run_by_the_app_and_in_the_menu() {
    assert!(super::workbench::app_runs("parts.import_table"));
    assert!(
        novo_workflow::menus::entries("File")
            .iter()
            .any(|(info, _)| info.id == "parts.import_table")
    );
}

#[test]
fn a_table_is_mapped_previewed_and_added_only_on_confirm() {
    let (dir, mut d) = open();
    let file = dir.path().join("liste.csv");
    std::fs::write(&file, TABLE.as_bytes()).unwrap();
    let cat = dir.path().join("catalog.json");
    let saved = dir.path().join("maps.json");
    assert!(
        d.app
            .import_parts_table_from(&file, Some(cat.clone()), Some(saved.clone()))
    );
    // Both tabs draw.
    d.frame();
    d.app.wb.parts_table.as_mut().unwrap().refresh();
    let import = d.app.wb.parts_table.as_ref().unwrap();
    assert_eq!(import.preview.counts().new, 2);
    assert_eq!(import.preview.counts().skipped, 1);
    assert!(!cat.exists(), "nothing is saved before Add");
    // "Notiz" is not known: kept as data, the person turns it off.
    let notiz = import
        .mapping
        .columns
        .iter()
        .position(|c| c.header == "Notiz")
        .unwrap();
    assert!(matches!(
        import.mapping.columns[notiz].target,
        Target::Custom(_)
    ));
    d.app
        .wb
        .parts_table
        .as_mut()
        .unwrap()
        .set_target(notiz, Target::Ignore);
    d.app
        .wb
        .parts_table
        .as_mut()
        .unwrap()
        .mapping
        .default_manufacturer = "Example Maker".into();
    d.app.wb.parts_table.as_mut().unwrap().refresh();
    let p = &d.app.wb.parts_table.as_ref().unwrap().preview;
    assert!(p.parts.iter().all(|x| !x.data.contains_key("notiz")));
    assert_eq!(p.rows[2].status, RowStatus::Skipped);
    d.frame();
    assert!(d.app.confirm_parts_table());
    let catalog = Catalog::load(&cat).unwrap();
    assert_eq!(catalog.len(), 2);
    assert!(dir.path().join("parts-table-import-report.txt").exists());
    // The same layout again: the corrected mapping comes back.
    assert!(d.app.import_parts_table_from(&file, Some(cat), Some(saved)));
    let again = d.app.wb.parts_table.as_ref().unwrap();
    assert!(again.mapping.columns[notiz].target == Target::Ignore);
    assert_eq!(again.preview.counts().unchanged, 2);
    assert!(!again.preview.can_import());
}

#[test]
fn old_excel_and_unknown_files_are_refused_in_words() {
    let (dir, mut d) = open();
    for (name, word) in [("list.xls", "old Excel"), ("list.pdf", "not a table file")] {
        let f = dir.path().join(name);
        std::fs::write(&f, b"x").unwrap();
        assert!(!d.app.import_parts_table_from(&f, None, None));
        assert!(d.app.wb.parts_table.is_none());
        assert!(d.app.messages().last().unwrap().text.contains(word));
    }
}

#[test]
fn a_file_over_the_size_cap_is_refused_before_it_is_read() {
    let (dir, mut d) = open();
    let big = dir.path().join("big.csv");
    let f = std::fs::File::create(&big).unwrap();
    f.set_len(novo_parts::MAX_FILE_BYTES + 1).unwrap();
    assert!(!d.app.import_parts_table_from(&big, None, None));
    assert!(d.app.wb.parts_table.is_none());
    assert!(
        d.app
            .messages()
            .last()
            .unwrap()
            .text
            .contains("larger than")
    );
}

#[test]
fn an_old_error_toast_is_cleared_when_the_dialog_opens() {
    let (dir, mut d) = open();
    d.app
        .message(crate::app::Level::Error, "Parts import: Not a directory");
    assert!(d.app.messages.iter().any(|m| !m.dismissed));
    let file = dir.path().join("liste.csv");
    std::fs::write(&file, TABLE.as_bytes()).unwrap();
    assert!(
        d.app
            .import_parts_table_from(&file, Some(dir.path().join("c.json")), None)
    );
    assert!(d.app.messages.iter().all(|m| m.dismissed));
    assert!(d.app.wb.parts_table.is_some());
}
