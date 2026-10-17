//! The shared property sheet of a selection, through the whole window.

use novo_core::{AnyId, SymbolId};
use novo_parts::{PropertySheet, RowKey, RowValue};

use super::gate1_tests::Drive;
use super::selection_sheet::shown_text;
use crate::actions::Action;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

fn select(d: &mut Drive, symbols: &[SymbolId]) {
    let view = d.app.canvas.schematic().expect("editor");
    view.canvas.selection.clear();
    view.canvas.selection.apply(
        symbols
            .iter()
            .map(|&s| novo_canvas::model::item_id(AnyId::Symbol(s)))
            .collect::<Vec<_>>(),
        novo_canvas::SelectOp::Replace,
    );
    d.frame();
}

fn descriptions(d: &Drive) -> Vec<String> {
    d.doc()
        .project()
        .devices()
        .map(|dev| dev.description.clone())
        .collect()
}

/// Two devices with different descriptions, both selected.
fn two_selected() -> (tempfile::TempDir, Drive) {
    let (dir, mut d) = drive();
    let a = d.place("mcb3", 80.0, 48.0);
    let b = d.place("coil", 200.0, 144.0);
    let ids: Vec<_> = d.doc().project().devices().map(|dev| dev.id).collect();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Describe", |tx| {
            tx.update::<novo_core::Device>(ids[0], |dev| dev.description = "One".into())?;
            tx.update::<novo_core::Device>(ids[1], |dev| dev.description = "Two".into())
        })
        .unwrap();
    select(&mut d, &[a, b]);
    (dir, d)
}

#[test]
fn several_devices_share_one_sheet_that_marks_the_fields_that_differ() {
    let (_dir, mut d) = two_selected();
    assert_eq!(d.app.multi_selected_devices().len(), 2);
    // The inspector draws the sheet without trouble.
    d.frame();
    let selection = d.app.selection();
    let sheet = PropertySheet::build(d.doc().project(), &selection);
    assert_eq!(sheet.title(), "2 devices");
    let key = RowKey::Field("description".into());
    let row = sheet.row(&key).unwrap();
    assert_eq!(row.value, RowValue::Mixed(2));
    // A field that differs shows no text (the hint says "multiple values");
    // text typed into it is shown as typed.
    assert_eq!(shown_text(row, None), "");
    assert_eq!(shown_text(row, Some(&"Typed".to_owned())), "Typed");
}

#[test]
fn typing_a_value_sets_it_on_every_selected_device_in_one_undo_step() {
    let (_dir, mut d) = two_selected();
    let before = descriptions(&d);
    assert_ne!(before[0], before[1]);
    let depth = d.doc().undo_depth();
    d.app
        .set_selection_field(&RowKey::Field("description".into()), "Pump");
    assert_eq!(descriptions(&d), ["Pump", "Pump"]);
    assert_eq!(d.doc().undo_depth(), depth + 1);

    // The sheet now shows the shared value.
    let selection = d.app.selection();
    let sheet = PropertySheet::build(d.doc().project(), &selection);
    let row = sheet.row(&RowKey::Field("description".into())).unwrap();
    assert_eq!(shown_text(row, None), "Pump");

    d.app.backend.document_mut().unwrap().undo().unwrap();
    assert_eq!(descriptions(&d), before);
}

#[test]
fn a_property_is_added_to_and_removed_from_the_whole_selection() {
    let (_dir, mut d) = two_selected();
    let depth = d.doc().undo_depth();
    d.app
        .set_selection_field(&RowKey::Property("Rated power kW".into()), "4,5");
    assert_eq!(d.doc().undo_depth(), depth + 1);
    assert!(
        d.doc()
            .project()
            .devices()
            .all(|dev| dev.properties.contains_key("rated_power_kw"))
    );
    d.app.remove_selection_property("rated_power_kw");
    assert!(
        d.doc()
            .project()
            .devices()
            .all(|dev| !dev.properties.contains_key("rated_power_kw"))
    );
}

#[test]
fn one_part_is_assigned_to_the_whole_selection_in_one_undo_step() {
    let (_dir, mut d) = two_selected();
    assert!(d.app.wb.part_catalog.ensure().is_none());
    let key = d.app.wb.part_catalog.get().unwrap().parts()[0].key();
    let depth = d.doc().undo_depth();
    d.app.assign_selection_part(&key);
    assert_eq!(d.doc().undo_depth(), depth + 1);
    assert!(d.doc().project().devices().all(|dev| dev.parts.len() == 1));
    d.app.backend.document_mut().unwrap().undo().unwrap();
    assert!(d.doc().project().devices().all(|dev| dev.parts.is_empty()));
}

#[test]
fn the_command_opens_the_window_for_one_device_and_defers_to_the_inspector_for_many() {
    let (_dir, mut d) = two_selected();
    // Nothing selected: it only says so.
    select(&mut d, &[]);
    d.app.wb.queued.push("edit.shared_properties");
    d.frame();
    assert!(!d.app.wb.selection_sheet.window);

    // One device: the window opens and edits that device.
    let first = d.doc().project().symbols().next().unwrap().id;
    select(&mut d, &[first]);
    d.app.wb.queued.push("edit.shared_properties");
    d.frame();
    assert!(d.app.wb.selection_sheet.window);
    d.app
        .set_selection_field(&RowKey::Field("description".into()), "Solo");
    assert_eq!(descriptions(&d).iter().filter(|t| *t == "Solo").count(), 1);

    // Several devices: the Inspector has the sheet, the window stays shut.
    d.app.wb.selection_sheet.window = false;
    let all: Vec<SymbolId> = d.doc().project().symbols().map(|s| s.id).collect();
    select(&mut d, &all);
    d.app.wb.queued.push("edit.shared_properties");
    d.frame();
    assert!(!d.app.wb.selection_sheet.window);
}
