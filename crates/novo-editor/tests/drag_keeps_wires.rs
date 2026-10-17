//! Dragging connected devices: the wires follow in one undo step and no wire
//! end is left open (release blocker, motor starter scenario A).

use novo_core::{AnyId, Document, Endpoint, PageId, Point, SymbolId, WireId};
use novo_editor::Editor;
use novo_editor::edit::open_end_count;
use novo_editor::tool::{Clicked, ConnectMode};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// Three contacts in a column, wired a-b and b-c.
fn column() -> (Document, PageId, Editor, [SymbolId; 3], [WireId; 2]) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Never);
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    for y in [40.0, 100.0, 160.0] {
        editor.click(&mut doc, page, p(40.0, y), TOL);
    }
    let at = |doc: &Document, y: f64| {
        *doc.project()
            .page_contents(page)
            .symbols
            .iter()
            .find(|&&s| doc.project().symbol(s).unwrap().placement.origin == p(40.0, y))
            .unwrap()
    };
    let s = [at(&doc, 40.0), at(&doc, 100.0), at(&doc, 160.0)];
    let pin = |doc: &Document, s, name: &str| {
        let sym = doc.project().symbol(s).unwrap();
        Endpoint::pin(s, sym.pin_index(name).unwrap())
    };
    for (from, to) in [(s[0], s[1]), (s[1], s[2])] {
        let (f, t) = (pin(&doc, from, "14"), pin(&doc, to, "13"));
        let pos = |doc: &Document, e: Endpoint| doc.project().endpoint_position(e).unwrap();
        editor.begin_wire();
        let (pf, pt) = (pos(&doc, f), pos(&doc, t));
        editor.hover(&doc, page, pf, TOL);
        editor.click(&mut doc, page, pf, TOL);
        editor.hover(&doc, page, pt, TOL);
        editor.click(&mut doc, page, pt, TOL);
        editor.leave_tool();
    }
    let wires: Vec<WireId> = doc.project().wires().map(|w| w.id).collect();
    assert_eq!(wires.len(), 2);
    (doc, page, editor, s, [wires[0], wires[1]])
}

/// Drags the selection by `delta` the way the canvas does: pick up at the
/// first symbol's origin, carrying as the view's default says.
fn drag(doc: &mut Document, page: PageId, editor: &mut Editor, ids: &[SymbolId], delta: Point) {
    let ids: Vec<AnyId> = ids.iter().map(|&s| AnyId::Symbol(s)).collect();
    let AnyId::Symbol(first) = ids[0] else {
        unreachable!()
    };
    let from = doc.project().symbol(first).unwrap().placement.origin;
    editor.hover(doc, page, from, TOL);
    assert!(editor.begin_move_with(&ids, true));
    editor.hover(doc, page, from + delta, TOL);
    assert_eq!(editor.click(doc, page, from + delta, TOL), Clicked::Edited);
}

fn endpoints(doc: &Document, w: WireId) -> (Endpoint, Endpoint) {
    let w = doc.project().wire(w).unwrap();
    (w.from, w.to)
}

#[test]
fn dragging_a_middle_device_keeps_both_wires_attached_in_one_undo_step() {
    let (mut doc, page, mut editor, s, w) = column();
    let before = [endpoints(&doc, w[0]), endpoints(&doc, w[1])];
    drag(&mut doc, page, &mut editor, &[s[1]], p(24.0, 0.0));
    assert_eq!(open_end_count(doc.project()), 0);
    assert_eq!([endpoints(&doc, w[0]), endpoints(&doc, w[1])], before);
    assert_eq!(doc.project().wires().count(), 2);
    let moved = doc.project().symbol(s[1]).unwrap().placement.origin;
    assert_eq!(moved, p(64.0, 100.0));
    // The wire ends sit on the moved pins.
    let to = doc
        .project()
        .endpoint_position(endpoints(&doc, w[0]).1)
        .unwrap();
    assert!(to.x > 60.0, "wire end did not follow: {to:?}");

    doc.undo().unwrap();
    assert_eq!(
        doc.project().symbol(s[1]).unwrap().placement.origin,
        p(40.0, 100.0)
    );
    assert_eq!([endpoints(&doc, w[0]), endpoints(&doc, w[1])], before);
    assert_eq!(open_end_count(doc.project()), 0);
}

#[test]
fn dragging_two_selected_devices_keeps_every_wire_attached() {
    let (mut doc, page, mut editor, s, w) = column();
    drag(&mut doc, page, &mut editor, &[s[0], s[1]], p(0.0, -20.0));
    assert_eq!(open_end_count(doc.project()), 0);
    assert_eq!(doc.project().wires().count(), 2);
    for wire in w {
        let (a, b) = endpoints(&doc, wire);
        assert!(matches!(a, Endpoint::Pin { .. }) && matches!(b, Endpoint::Pin { .. }));
    }
    doc.undo().unwrap();
    assert_eq!(open_end_count(doc.project()), 0);
    assert_eq!(
        doc.project().symbol(s[0]).unwrap().placement.origin,
        p(40.0, 40.0)
    );
}

#[cfg(feature = "egui")]
#[test]
fn a_view_drags_with_the_wires_unless_told_otherwise() {
    assert!(novo_editor::view::SchematicView::default().move_carries);
}
