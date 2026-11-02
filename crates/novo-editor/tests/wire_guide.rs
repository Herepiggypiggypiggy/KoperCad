//! The wire tool tells where it is: a step for the on-canvas guide and a ring
//! on every pin that can still take a wire.

use novo_core::{Document, Endpoint, PageId, Point};
use novo_editor::Editor;
use novo_editor::tool::{ConnectMode, free_pins};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup() -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Never);
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.leave_tool();
    (doc, page, editor)
}

#[test]
fn the_guide_follows_the_three_steps_of_a_wire() {
    let (mut doc, page, mut editor) = setup();
    assert_eq!(editor.wire_step(), None);
    editor.begin_wire();
    assert_eq!(editor.wire_step(), Some(0));
    let s = doc.project().page_contents(page).symbols[0];
    let (i14, i13) = {
        let sym = doc.project().symbol(s).unwrap();
        (sym.pin_index("14").unwrap(), sym.pin_index("13").unwrap())
    };
    let from = doc
        .project()
        .endpoint_position(Endpoint::pin(s, i14))
        .unwrap();
    editor.hover(&doc, page, from, TOL);
    editor.click(&mut doc, page, from, TOL);
    // In open space the next click turns the wire.
    editor.hover(&doc, page, p(from.x + 30.0, from.y + 30.0), TOL);
    assert_eq!(editor.wire_step(), Some(1));
    // On the other pin it finishes.
    let other = doc
        .project()
        .endpoint_position(Endpoint::pin(s, i13))
        .unwrap();
    editor.hover(&doc, page, other, TOL);
    assert_eq!(editor.wire_step(), Some(2));
}

#[test]
fn every_pin_without_a_wire_gets_a_ring_while_wiring() {
    let (doc, page, mut editor) = setup();
    assert_eq!(free_pins(doc.project(), page).len(), 2);
    assert!(editor.preview(&doc, page).free_pins.is_empty());
    editor.begin_wire();
    assert_eq!(editor.preview(&doc, page).free_pins.len(), 2);
}
