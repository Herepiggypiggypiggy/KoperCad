//! A row drag that stops short shows where and why (a red box on the
//! position that would overlap another device).

use novo_core::{Document, PageId, Point};
use novo_editor::Editor;

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup() -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.auto_connect = false;
    (doc, page, editor)
}

#[test]
fn a_blocked_row_position_is_marked_and_a_free_row_has_no_mark() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(120.0, 40.0), TOL);
    editor.leave_tool();
    editor.begin_place("iec/contacts/no", None);
    editor.hover(&doc, page, p(40.0, 40.0), TOL);
    assert!(editor.begin_row());
    // Dragged left, away from the device: nothing blocks it.
    editor.row_follow(p(-40.0, 40.0), 8.0);
    assert!(editor.preview(&doc, page).refused.is_none());
    // Dragged right through the device at x = 120: the row stops short and
    // the position it left out is boxed.
    editor.row_follow(p(300.0, 40.0), 8.0);
    let (lo, hi) = editor
        .preview(&doc, page)
        .refused
        .expect("the stopped position is marked");
    assert!(lo.x < 120.0 + 20.0 && hi.x > 120.0 - 20.0, "{lo:?} {hi:?}");
    assert!(lo.x < hi.x && lo.y < hi.y);
}
