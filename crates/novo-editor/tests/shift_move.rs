//! Shift+M: the move that takes the wires along and keeps doing so.

use novo_core::{AnyId, Document, PageId, Point};
use novo_editor::tool::{ConnectMode, Tool};
use novo_editor::{Editor, EditorKey};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup() -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Ask);
    (doc, page, editor)
}

#[test]
fn shift_m_keeps_carrying_when_the_per_frame_sync_says_otherwise() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    let a = doc
        .project()
        .page_contents(page)
        .symbols
        .into_iter()
        .next()
        .unwrap();
    editor.leave_tool();
    editor.hover(&doc, page, p(40.0, 40.0), TOL);
    assert!(editor.key(&mut doc, EditorKey::MoveWithWires, &[AnyId::Symbol(a)]));
    // Shift released: the view would sync carry from the default (off).
    editor.set_move_carry(false);
    assert!(matches!(editor.tool(), Tool::Move(m) if m.carry && m.pinned));
    // A plain M is not pinned and follows the sync.
    editor.leave_tool();
    assert!(editor.key(&mut doc, EditorKey::Move, &[AnyId::Symbol(a)]));
    editor.set_move_carry(true);
    assert!(matches!(editor.tool(), Tool::Move(m) if m.carry && !m.pinned));
}
