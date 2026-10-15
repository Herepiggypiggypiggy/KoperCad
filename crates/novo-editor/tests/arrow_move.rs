//! Arrow-key moves: the wires stay behind unless asked to come along.

use novo_core::{AnyId, Document, Endpoint, PageId, Point};
use novo_editor::Editor;
use novo_editor::tool::ConnectMode;

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn two_wired_contacts() -> (Document, PageId, Editor, AnyId, novo_core::WireId) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Never);
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 100.0), TOL);
    let symbols = doc.project().page_contents(page).symbols;
    let at = |o: Point| {
        *symbols
            .iter()
            .find(|&&s| doc.project().symbol(s).unwrap().placement.origin == o)
            .unwrap()
    };
    let (a, b) = (at(p(40.0, 40.0)), at(p(80.0, 100.0)));
    let pin = |s, name: &str| {
        let sym = doc.project().symbol(s).unwrap();
        Endpoint::pin(s, sym.pin_index(name).unwrap())
    };
    let (from, to) = (pin(a, "14"), pin(b, "13"));
    editor.begin_wire();
    let pos = |e: Endpoint| doc.project().endpoint_position(e).unwrap();
    let (pa, pb) = (pos(from), pos(to));
    editor.hover(&doc, page, pa, TOL);
    editor.click(&mut doc, page, pa, TOL);
    editor.hover(&doc, page, pb, TOL);
    editor.click(&mut doc, page, pb, TOL);
    editor.leave_tool();
    let w = doc.project().wires().next().unwrap().id;
    (doc, page, editor, AnyId::Symbol(b), w)
}

#[test]
fn a_nudge_leaves_the_wire_open_and_a_carrying_nudge_takes_it() {
    let (mut doc, _, mut editor, b, w) = two_wired_contacts();
    let was = doc.project().wire(w).unwrap().to;
    assert!(editor.nudge_selection(&mut doc, &[b], p(8.0, 0.0), false));
    assert!(matches!(
        doc.project().wire(w).unwrap().to,
        Endpoint::Free { .. }
    ));

    doc.undo().unwrap();
    assert_eq!(doc.project().wire(w).unwrap().to, was);
    assert!(editor.nudge_selection(&mut doc, &[b], p(8.0, 0.0), true));
    assert_eq!(doc.project().wire(w).unwrap().to, was);
}
