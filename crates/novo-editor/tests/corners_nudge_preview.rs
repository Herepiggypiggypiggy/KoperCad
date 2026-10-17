//! Corners: the live preview over a pin is the route the click stores,
//! nudged clear of another wire, not the plain route that would overlap.

use novo_core::{Document, Endpoint, PageId, Point, SymbolId};
use novo_editor::tool::ConnectMode;
use novo_editor::{Editor, EditorKey, geometry};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn symbol_at(doc: &Document, page: PageId, origin: Point) -> SymbolId {
    doc.project()
        .page_contents(page)
        .symbols
        .into_iter()
        .find(|&s| doc.project().symbol(s).unwrap().placement.origin == origin)
        .expect("a symbol at that origin")
}

fn pin_pos(doc: &Document, sym: SymbolId, name: &str) -> Point {
    let s = doc.project().symbol(sym).unwrap();
    doc.project()
        .endpoint_position(Endpoint::pin(sym, s.pin_index(name).unwrap()))
        .unwrap()
}

fn scene(first_wire: bool) -> (Document, PageId, Editor, [SymbolId; 4], f64) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Ask);
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    for at in [p(40.0, 40.0), p(80.0, 40.0), p(60.0, 120.0), p(40.0, 120.0)] {
        editor.click(&mut doc, page, at, TOL);
    }
    editor.leave_tool();
    let [a, b, c, d] = [p(40.0, 40.0), p(80.0, 40.0), p(60.0, 120.0), p(40.0, 120.0)]
        .map(|at| symbol_at(&doc, page, at));
    // The first wire takes the halfway cross-over.
    if first_wire {
        editor.begin_wire();
        let at = pin_pos(&doc, a, "14");
        editor.click(&mut doc, page, at, TOL);
        let at = pin_pos(&doc, c, "13");
        editor.click(&mut doc, page, at, TOL);
    }
    let mid = (pin_pos(&doc, a, "14").y + pin_pos(&doc, c, "13").y) / 2.0;
    (doc, page, editor, [a, b, c, d], mid)
}

#[test]
fn the_preview_over_a_pin_shows_the_nudged_route_the_click_stores() {
    let (mut doc, page, mut editor, [_, b, _, d], mid) = scene(true);
    // The second wire, started at b and hovering over d: the preview.
    editor.begin_wire();
    let at = pin_pos(&doc, b, "14");
    editor.click(&mut doc, page, at, TOL);
    let end = pin_pos(&doc, d, "13");
    editor.hover(&doc, page, end, TOL);
    let preview = editor.preview(&doc, page).wire;
    assert!(preview.len() >= 3, "{preview:?}");
    assert!(
        preview
            .iter()
            .skip(1)
            .take(preview.len() - 2)
            .all(|q| (q.y - mid).abs() > 1.0),
        "the preview keeps off the first wire's cross-over: {preview:?}"
    );
    // The click stores exactly what was shown.
    editor.click(&mut doc, page, end, TOL);
    let second = doc.project().wires().last().unwrap().id;
    let stored = geometry::wire_points(doc.project(), second).unwrap();
    assert_eq!(preview, stored);
}

#[test]
fn a_flipped_elbow_with_the_pointer_a_little_off_the_pin_matches_the_stored_route() {
    let (mut doc, page, mut editor, [_, b, _, d], _) = scene(false);
    editor.begin_wire();
    let at = pin_pos(&doc, b, "14");
    editor.click(&mut doc, page, at, TOL);
    editor.key(&mut doc, EditorKey::Rotate, &[]);
    let end = pin_pos(&doc, d, "13");
    // Within the snap tolerance but not on the pin.
    editor.hover(&doc, page, p(end.x + 1.0, end.y - 1.0), TOL);
    let preview = editor.preview(&doc, page).wire;
    assert_eq!(preview.last(), Some(&end), "{preview:?}");
    editor.click(&mut doc, page, p(end.x + 1.0, end.y - 1.0), TOL);
    let second = doc.project().wires().last().unwrap().id;
    let stored = geometry::wire_points(doc.project(), second).unwrap();
    assert_eq!(preview, stored);
}
