//! Open wire ends: pulling a cap, and resuming a wire from it (DESIGN.md §4).

use novo_core::{Document, Endpoint, PageId, Point, WireId};
use novo_editor::tool::{Clicked, ConnectMode, Tool};
use novo_editor::{Editor, EditorKey};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

/// A contact with a wire from its lower pin to an open end 40 mm right.
fn open_ended() -> (Document, PageId, Editor, WireId, Point) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Never);
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    let s = doc.project().page_contents(page).symbols[0];
    let sym = doc.project().symbol(s).unwrap();
    let pin = Endpoint::pin(s, sym.pin_index("14").unwrap());
    let from = doc.project().endpoint_position(pin).unwrap();
    editor.begin_wire();
    editor.hover(&doc, page, from, TOL);
    editor.click(&mut doc, page, from, TOL);
    let cap = p(from.x + 40.0, from.y);
    editor.hover(&doc, page, cap, TOL);
    assert_eq!(editor.click(&mut doc, page, cap, TOL), Clicked::Step);
    assert!(editor.key(&mut doc, EditorKey::FinishOpen, &[]));
    editor.leave_tool();
    let w = doc.project().wires().next().expect("the wire").id;
    assert!(matches!(
        doc.project().wire(w).unwrap().to,
        Endpoint::Free { .. }
    ));
    (doc, page, editor, w, cap)
}

#[test]
fn pulling_a_cap_moves_the_open_end_in_one_undo_step() {
    let (mut doc, page, mut editor, w, cap) = open_ended();
    doc.transact("number", |tx| {
        tx.update::<novo_core::Wire>(w, |x| x.number = Some("7".into()))
    })
    .unwrap();
    let depth = doc.undo_depth();
    editor.hover(&doc, page, cap, TOL);
    assert!(editor.begin_open_end(&doc, page, cap, TOL, true));
    let to = p(cap.x + 8.0, cap.y + 16.0);
    editor.hover(&doc, page, to, TOL);
    editor.click(&mut doc, page, to, TOL);
    assert_eq!(doc.undo_depth(), depth + 1);
    let wires: Vec<_> = doc.project().wires().collect();
    assert_eq!(wires.len(), 1);
    let moved = wires[0];
    assert_eq!(moved.to, Endpoint::Free { at: to });
    assert_eq!(moved.number.as_deref(), Some("7"), "the number is kept");
    doc.undo().unwrap();
    let back = doc.project().wires().next().unwrap();
    assert_eq!(back.to, Endpoint::Free { at: cap });
}

#[test]
fn pulling_a_cap_onto_a_pin_connects_it() {
    let (mut doc, page, mut editor, _, cap) = open_ended();
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(120.0, 100.0), TOL);
    editor.leave_tool();
    let b = doc
        .project()
        .page_contents(page)
        .symbols
        .into_iter()
        .find(|&s| doc.project().symbol(s).unwrap().placement.origin == p(120.0, 100.0))
        .unwrap();
    let sym = doc.project().symbol(b).unwrap();
    let pin = Endpoint::pin(b, sym.pin_index("13").unwrap());
    let at = doc.project().endpoint_position(pin).unwrap();
    assert!(editor.begin_open_end(&doc, page, cap, TOL, true));
    editor.hover(&doc, page, at, TOL);
    editor.click(&mut doc, page, at, TOL);
    let w = doc.project().wires().next().unwrap();
    assert_eq!(w.to, pin);
}

#[test]
fn a_cap_is_found_only_near_it_and_w_resumes_the_wire() {
    let (mut doc, page, mut editor, _, cap) = open_ended();
    assert!(!editor.begin_open_end(&doc, page, p(cap.x, cap.y + 20.0), TOL, true));
    assert!(
        editor
            .open_end_at(&doc, page, p(cap.x + 0.5, cap.y), TOL)
            .is_some()
    );
    // W on the cap resumes: the next click is a corner, a double click ends.
    editor.hover(&doc, page, cap, TOL);
    assert!(editor.key(&mut doc, EditorKey::Wire, &[]));
    assert!(matches!(editor.tool(), Tool::Wire(w) if w.replacing.is_some() && !w.pull));
    let next = p(cap.x, cap.y + 24.0);
    editor.hover(&doc, page, next, TOL);
    assert_eq!(editor.click(&mut doc, page, next, TOL), Clicked::Step);
    assert!(editor.key(&mut doc, EditorKey::FinishOpen, &[]));
    let wires: Vec<_> = doc.project().wires().collect();
    assert_eq!(wires.len(), 1, "the wire was extended, not duplicated");
    assert_eq!(wires[0].to, Endpoint::Free { at: next });
}

#[test]
fn a_pulled_wire_stays_on_its_cable_core() {
    let (mut doc, page, mut editor, w, cap) = open_ended();
    let cable = doc
        .transact("cable", |tx| {
            let cable = tx.add_cable(
                novo_core::DeviceTag::parse("-W1").unwrap(),
                vec![novo_core::CableCore::new("1")],
            )?;
            tx.assign_core(cable, 0, Some(w))?;
            Ok(cable)
        })
        .unwrap()
        .0;
    editor.hover(&doc, page, cap, TOL);
    assert!(editor.begin_open_end(&doc, page, cap, TOL, true));
    let to = p(cap.x + 8.0, cap.y + 16.0);
    editor.hover(&doc, page, to, TOL);
    editor.click(&mut doc, page, to, TOL);
    let new = doc.project().wires().next().unwrap().id;
    assert_eq!(doc.project().core_of_wire(new), Some((cable, 0)));
    doc.undo().unwrap();
    assert_eq!(doc.project().core_of_wire(w), Some((cable, 0)));
}
