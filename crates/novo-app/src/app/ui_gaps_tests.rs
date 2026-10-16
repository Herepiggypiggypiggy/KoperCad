//! Free text, the net trace and the keymap file, through the whole window.

use eframe::egui;
use novo_core::{AnyId, Endpoint, Point, SymbolId, TextContent, WireId};

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

fn literal_texts(d: &Drive) -> Vec<String> {
    let mut found: Vec<String> = d
        .doc()
        .project()
        .texts()
        .filter(|t| t.attached_to.is_none())
        .filter_map(|t| match &t.content {
            TextContent::Literal { text } => Some(text.clone()),
            _ => None,
        })
        .collect();
    found.sort();
    found
}

fn select(d: &mut Drive, ids: &[AnyId]) {
    let view = d.app.canvas.schematic().expect("editor");
    view.canvas.selection.clear();
    view.canvas.selection.apply(
        ids.iter()
            .map(|&id| novo_canvas::model::item_id(id))
            .collect::<Vec<_>>(),
        novo_canvas::SelectOp::Replace,
    );
    d.frame();
}

/// Two parts joined by one wire; the wire is returned.
fn joined(d: &mut Drive) -> (SymbolId, SymbolId, WireId) {
    let a = d.place("mcb3", 80.0, 48.0);
    let b = d.place("coil", 200.0, 144.0);
    let (result, changes) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("join", |tx| {
            tx.connect(Endpoint::pin(a, 0), Endpoint::pin(b, 0))
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    d.frame();
    (a, b, result)
}

#[test]
fn draw_text_arms_the_tool_a_click_asks_for_words_and_enter_adds_one_undo_step() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "draw.text", None);
    d.frame();
    assert!(d.app.canvas.schematic().unwrap().editor.placing_text());
    let depth = d.doc().undo_depth();

    d.click(Point::new(120.0, 90.0));
    let view = d.app.canvas.schematic().unwrap();
    assert!(!view.editor.placing_text(), "one click per text");
    assert!(
        view.renaming(),
        "the words are asked for where the click was"
    );
    assert_eq!(d.doc().undo_depth(), depth, "nothing is added before Enter");

    d.app
        .canvas
        .schematic()
        .unwrap()
        .set_rename_text("Spare cable to the pump");
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(!d.app.canvas.schematic().unwrap().renaming());
    assert_eq!(
        literal_texts(&d),
        vec!["Spare cable to the pump".to_owned()]
    );
    assert_eq!(d.doc().undo_depth(), depth + 1);

    d.app.undo_redo(false);
    d.frame();
    assert!(literal_texts(&d).is_empty(), "undo takes the text away");
}

#[test]
fn escape_or_empty_words_add_no_text() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "draw.text", None);
    d.frame();
    d.app.canvas.schematic().unwrap().editor.cancel_step();
    d.frame();
    assert!(!d.app.canvas.schematic().unwrap().editor.placing_text());

    d.app.run_command(&ctx, "draw.text", None);
    d.click(Point::new(60.0, 60.0));
    d.app.canvas.schematic().unwrap().set_rename_text("   ");
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(literal_texts(&d).is_empty());
}

#[test]
fn f2_on_a_selected_free_text_edits_its_words() {
    let (_dir, mut d) = drive();
    let (result, _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("note", |tx| {
            let page = tx.project().pages().next().unwrap().id;
            tx.add_text(
                page,
                TextContent::Literal {
                    text: "Note".into(),
                },
                novo_core::TextPlacement {
                    offset: Point::new(50.0, 50.0),
                    ..Default::default()
                },
                None,
            )
        })
        .unwrap();
    d.frame();
    select(&mut d, &[AnyId::Text(result)]);
    // F2 reads the keyboard only while the drawing is hovered.
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(800.0, 450.0))]);
    d.key(egui::Key::F2, egui::Modifiers::NONE);
    let view = d.app.canvas.schematic().unwrap();
    assert!(view.renaming(), "F2 opens the text for editing");
    view.set_rename_text("Changed");
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(literal_texts(&d), vec!["Changed".to_owned()]);
}

#[test]
fn select_net_takes_the_wire_and_trace_lists_devices_and_pages() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    let (a, b, wire) = joined(&mut d);
    select(&mut d, &[AnyId::Symbol(a)]);

    d.app.run_command(&ctx, "net.select", None);
    d.frame();
    assert_eq!(d.app.selection(), vec![AnyId::Wire(wire)]);

    d.app.run_command(&ctx, "net.trace", None);
    d.frame();
    let trace = d.app.wb.net_trace.clone().expect("the trace window opens");
    assert_eq!(trace.nets.len(), 1);
    let net = &trace.nets[0];
    assert_eq!(net.wires, 1);
    assert_eq!(net.devices.len(), 2, "{:?}", net.devices);
    assert_eq!(net.pages.len(), 1);
    let on_net: Vec<SymbolId> = net.devices.iter().filter_map(|(_, p)| p.symbol).collect();
    assert!(on_net.contains(&a) && on_net.contains(&b));
}

#[test]
fn net_commands_without_a_selection_say_what_to_select() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    for id in ["net.select", "net.trace"] {
        d.app.run_command(&ctx, id, None);
        d.frame();
        assert!(d.app.wb.net_trace.is_none());
        assert!(
            d.app
                .messages()
                .last()
                .is_some_and(|m| m.text.contains("Select a wire")),
            "{id}"
        );
    }
}

#[test]
fn the_new_commands_are_run_by_the_app_and_shift_t_is_the_text_key() {
    for id in ["draw.text", "net.select", "net.trace", "help.keymap_file"] {
        assert!(super::workbench::app_runs(id), "{id}");
    }
    assert_eq!(
        novo_workflow::keymap::Keymap::workflow()
            .shortcut_text("draw.text")
            .as_deref(),
        Some("Shift+T")
    );
}

#[test]
fn shift_t_on_the_drawing_arms_the_text_tool() {
    let (_dir, mut d) = drive();
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(800.0, 450.0))]);
    d.key(egui::Key::T, egui::Modifiers::SHIFT);
    assert!(d.app.canvas.schematic().unwrap().editor.placing_text());
}
