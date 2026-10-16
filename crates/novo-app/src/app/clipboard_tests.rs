//! Copy, cut and paste through the whole window, headless.

use eframe::egui;
use novo_core::{AnyId, Point, SymbolId};

use super::gate1_tests::Drive;
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

fn tags(d: &Drive) -> Vec<String> {
    let mut t: Vec<String> = d
        .doc()
        .project()
        .devices()
        .map(|dev| dev.tag.to_string())
        .collect();
    t.sort();
    t
}

fn pasting(d: &mut Drive) -> bool {
    d.app.canvas.schematic().is_some_and(|v| v.editor.pasting())
}

#[test]
fn ctrl_c_then_ctrl_v_follows_the_pointer_and_a_click_drops_it_in_one_undo_step() {
    let (_dir, mut d) = drive();
    let a = d.place("mcb3", 80.0, 48.0);
    let b = d.place("coil", 200.0, 144.0);
    select(&mut d, &[a, b]);
    d.frame_with(vec![egui::Event::Copy]);
    let clip = d.app.wb.clip.last.clone().expect("copied");
    assert_eq!(clip.len(), 2);
    let depth = d.doc().undo_depth();

    d.frame_with(vec![egui::Event::Paste(clip.to_text())]);
    assert!(pasting(&mut d), "the paste is on the pointer");
    assert_eq!(d.doc().undo_depth(), depth, "nothing changed yet");

    d.click(Point::new(80.0, 300.0));
    assert!(!pasting(&mut d));
    assert_eq!(d.doc().undo_depth(), depth + 1, "one undo step");
    let all = tags(&d);
    let mut unique = all.clone();
    unique.dedup();
    assert_eq!(all, unique, "no tag is used twice: {all:?}");
    assert_eq!(all.len(), 4);
    assert_eq!(d.app.selection().len(), 2, "the paste is selected");

    d.app.undo_redo(false);
    d.frame();
    assert_eq!(tags(&d).len(), 2);
}

#[test]
fn escape_cancels_a_paste_in_hand() {
    let (_dir, mut d) = drive();
    let a = d.place("mcb3", 80.0, 48.0);
    select(&mut d, &[a]);
    d.frame_with(vec![egui::Event::Copy]);
    d.app.run_command(&d.ctx.clone(), "edit.paste", None);
    d.frame();
    assert!(pasting(&mut d));
    d.app.canvas.schematic().unwrap().editor.cancel_step();
    d.frame();
    assert!(!pasting(&mut d));
    assert!(d.app.wb.clip.pending.is_none());
    assert_eq!(tags(&d).len(), 1);
}

#[test]
fn cut_removes_the_selection_and_paste_in_place_brings_it_back_with_its_tags() {
    let (_dir, mut d) = drive();
    let a = d.place("mcb3", 80.0, 48.0);
    select(&mut d, &[a]);
    let before = tags(&d);
    d.frame_with(vec![egui::Event::Cut]);
    assert_eq!(d.doc().project().symbols().count(), 0);
    assert!(tags(&d).is_empty());
    d.app
        .run_command(&d.ctx.clone(), "edit.paste_in_place", None);
    d.frame();
    assert_eq!(tags(&d), before, "a cut moves, so the tags stay");
    let origin = d.doc().project().symbols().next().unwrap().placement.origin;
    assert_eq!(origin, Point::new(80.0, 48.0));
}

#[test]
fn text_that_is_not_a_copy_is_not_pasted() {
    let (_dir, mut d) = drive();
    d.place("mcb3", 80.0, 48.0);
    d.frame_with(vec![egui::Event::Paste("hello".into())]);
    assert!(!pasting(&mut d));
    assert_eq!(tags(&d).len(), 1);
}

#[test]
fn shift_o_toggles_the_override_marks() {
    let (_dir, mut d) = drive();
    assert!(d.app.wb.override_marks);
    d.app
        .run_command(&d.ctx.clone(), "view.override_marks", None);
    assert!(!d.app.wb.override_marks);
}

#[test]
fn the_edit_commands_are_run_by_the_app() {
    for id in [
        "edit.copy",
        "edit.cut",
        "edit.paste",
        "edit.paste_keep_tags",
        "edit.paste_in_place",
        "view.override_marks",
    ] {
        assert!(super::workbench::app_runs(id), "{id}");
    }
}
