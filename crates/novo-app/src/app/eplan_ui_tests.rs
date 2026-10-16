//! The EPLAN interop and PLC dialogs: commands open them, the engine does
//! the work (parts import preview, export options, card layout dialog,
//! PLC overview).

use novo_parts::Catalog;
use novo_plc::{ChannelType, Space, cards};

use super::eplan_export::EplanExport;
use super::eplan_import::languages_in;
use super::gate1_tests::Drive;
use super::plc_card::overview;
use crate::actions::Action;

fn open() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

const PARTS_XML: &str = r#"<?xml version="1.0"?>
<parts>
  <part ORDERNR="2002-1201" MANUFACTURER="WAGO" DESCR1="Terminal block"/>
  <part ORDERNR="3RT2015" MANUFACTURER="Siemens" DESCR1="Contactor"/>
  <part MANUFACTURER="Acme" DESCR1="No number"/>
</parts>"#;

#[test]
fn the_new_commands_are_run_by_the_app() {
    for id in ["plc.overview", "parts.import_eplan", "export.eplan"] {
        assert!(super::workbench::app_runs(id), "{id}");
    }
}

#[test]
fn an_eplan_parts_export_is_previewed_then_added_to_the_catalog() {
    let (dir, mut d) = open();
    let xml = dir.path().join("parts.xml");
    std::fs::write(&xml, PARTS_XML).unwrap();
    let cat = dir.path().join("catalog.json");
    assert!(d.app.import_eplan_parts_from(&xml, Some(cat.clone())));
    let preview = d.app.wb.eplan_parts.as_ref().expect("a preview opens");
    assert_eq!(preview.parts().len(), 2);
    assert_eq!(preview.counts(), (2, 0, 0));
    // Nothing is saved before the person adds.
    assert!(!cat.exists());
    d.frame();
    assert!(d.app.confirm_eplan_parts());
    let saved = Catalog::load(&cat).unwrap();
    assert_eq!(saved.parts().len(), 2);
    assert!(d.app.wb.eplan_parts.is_none());
    // The same file again: everything is already there as it is.
    assert!(d.app.import_eplan_parts_from(&xml, Some(cat)));
    assert_eq!(d.app.wb.eplan_parts.as_ref().unwrap().counts(), (0, 0, 2));
}

#[test]
fn the_delimited_eplan_export_is_read_and_rejects_are_counted() {
    let (dir, mut d) = open();
    let csv = dir.path().join("parts.csv");
    std::fs::write(
        &csv,
        "Part number;Manufacturer;Description 1\nA-1;Acme;Relay\n;Acme;Nothing\n",
    )
    .unwrap();
    assert!(
        d.app
            .import_eplan_parts_from(&csv, Some(dir.path().join("c.json")))
    );
    let p = d.app.wb.eplan_parts.as_ref().unwrap();
    assert_eq!(p.parts().len(), 1);
    assert_eq!(p.rejected(), 1);
}

#[test]
fn a_file_with_no_parts_says_so_and_opens_nothing() {
    let (dir, mut d) = open();
    let f = dir.path().join("empty.xml");
    std::fs::write(&f, "<root><a/></root>").unwrap();
    assert!(
        !d.app
            .import_eplan_parts_from(&f, Some(dir.path().join("c.json")))
    );
    assert!(d.app.wb.eplan_parts.is_none());
    assert!(d.app.messages().last().unwrap().text.contains("No parts"));
}

#[test]
fn languages_are_found_in_multi_language_texts() {
    let text = "<x>en_US@Page;de_DE@Seite;</x> <y>nl_NL@Pagina;</y> a@b";
    assert_eq!(languages_in(text), ["de_DE", "en_US", "nl_NL"]);
    assert!(languages_in("<plain/>").is_empty());
}

#[test]
fn the_export_command_opens_the_options_and_remembers_them() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "export.eplan", None);
    d.frame();
    let form = d.app.wb.eplan_export.as_mut().expect("options open");
    form.lang = "nl_NL".into();
    d.app.wb.last_export = None;
    let written = d.app.confirm_eplan_export();
    assert_eq!(
        d.app.wb.eplan_export_last.as_ref().map(|f| f.lang.as_str()),
        Some("nl_NL")
    );
    assert_eq!(
        EplanExport {
            lang: "nl_NL".into(),
            ..EplanExport::default()
        }
        .options()
        .lang,
        "nl_NL"
    );
    // A project with no file exports to Documents, which a test box may lack.
    if let Some(path) = written {
        assert!(path.extension().is_some_and(|e| e == "epj"));
        let _ = std::fs::remove_file(path);
    }
    // The next time it opens on the same choices.
    d.app.run_command(&ctx, "export.eplan", None);
    assert_eq!(d.app.wb.eplan_export.as_ref().unwrap().lang, "nl_NL");
}

#[test]
fn the_card_dialog_adds_a_card_with_a_chosen_start_and_refuses_an_overlap() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "plc.add_card", None);
    d.frame();
    let form = d.app.wb.plc_card.as_mut().expect("the dialog opens");
    form.counts = [8, 4, 0, 0, 0];
    form.start_input = "10".into();
    let depth = d.doc().undo_depth();
    assert!(d.app.confirm_plc_card());
    assert_eq!(d.doc().undo_depth(), depth + 1);
    let all = cards(d.doc().project());
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].start[Space::Input as usize], Some(10));
    assert_eq!(all[0].start[Space::Output as usize], Some(0));
    assert_eq!(all[0].channels().count(ChannelType::DigitalInput), 8);

    // A second card on top of the first is refused and the dialog stays.
    d.app.run_command(&ctx, "plc.add_card", None);
    let form = d.app.wb.plc_card.as_mut().unwrap();
    form.counts = [8, 0, 0, 0, 0];
    form.start_input = "10".into();
    assert!(!d.app.confirm_plc_card());
    let problem = d.app.wb.plc_card.as_ref().unwrap().problem.clone();
    assert!(problem.unwrap().contains("overlap"));
    assert_eq!(cards(d.doc().project()).len(), 1);

    // Left empty, the start follows the first card.
    d.app.wb.plc_card.as_mut().unwrap().start_input.clear();
    assert!(d.app.confirm_plc_card());
    assert_eq!(cards(d.doc().project()).len(), 2);
}

#[test]
fn the_command_field_form_still_adds_in_one_step() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "plc.add_card", Some("DI4 DQ4"));
    assert!(d.app.wb.plc_card.is_none());
    assert_eq!(cards(d.doc().project()).len(), 1);
}

#[test]
fn the_overview_lists_cards_channels_and_addresses() {
    let (_dir, mut d) = open();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "plc.add_card", Some("DI4 DQ2"));
    d.app.run_command(&ctx, "plc.overview", None);
    assert!(d.app.wb.plc_overview);
    d.frame();
    let library = novo_symbols::Library::shared(novo_symbols::Pack::Iec);
    let (rows, issues) = overview(d.doc().project(), library);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].layout, "DI4 DQ2");
    assert_eq!(rows[0].input.as_deref(), Some("%IB0 to 0"));
    assert_eq!(rows[0].output.as_deref(), Some("%QB0 to 0"));
    assert_eq!(rows[0].channels.len(), 6);
    assert!(rows[0].channels[0].0.contains("DI 0"));
    assert!(issues.is_empty(), "{issues:?}");
}
