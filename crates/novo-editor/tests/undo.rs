//! Ctrl+Z and Ctrl+Y across the editor's tools: each gesture is exactly one
//! undo step, undo restores the drawing and redo brings the gesture back
//! (WS-08 keyboard workflow; DESIGN §4).

use novo_core::{AnyId, Document, PageId, Point};
use novo_editor::tool::Clicked;
use novo_editor::{Editor, EditorKey};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup() -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(novo_editor::tool::ConnectMode::Ask);
    (doc, page, editor)
}

/// What a user would see change: symbols with their placement and role,
/// devices with their tags, wires with their ends.
fn drawing(doc: &Document) -> String {
    let project = doc.project();
    let mut symbols: Vec<String> = project
        .symbols()
        .map(|s| format!("{:?} {:?}", s.placement, s.role))
        .collect();
    symbols.sort();
    let mut tags: Vec<String> = project.devices().map(|d| d.tag.to_string()).collect();
    tags.sort();
    let mut wires: Vec<String> = project
        .wires()
        .map(|w| format!("{:?}->{:?}", w.from, w.to))
        .collect();
    wires.sort();
    format!("{symbols:?}\n{tags:?}\n{wires:?}")
}

/// Runs `gesture`, which must add exactly one undo step; checks that undo
/// restores the drawing from before it and redo the drawing it made.
fn one_undo_step(doc: &mut Document, gesture: impl FnOnce(&mut Document)) {
    let (before, depth) = (drawing(doc), doc.undo_depth());
    gesture(doc);
    let after = drawing(doc);
    assert_ne!(before, after, "the gesture changed nothing");
    assert_eq!(doc.undo_depth(), depth + 1, "one undo step");
    doc.undo().unwrap().expect("something to undo");
    assert_eq!(drawing(doc), before, "undo restores the drawing");
    doc.redo().unwrap().expect("something to redo");
    assert_eq!(drawing(doc), after, "redo brings the gesture back");
}

fn place(editor: &mut Editor, doc: &mut Document, page: PageId, id: &str, at: Point) {
    assert!(editor.begin_place(id, None));
    assert_eq!(editor.click(doc, page, at, TOL), Clicked::Edited);
    editor.leave_tool();
}

#[test]
fn placing_rotating_mirroring_and_deleting_are_one_step_each() {
    let (mut doc, page, mut editor) = setup();
    one_undo_step(&mut doc, |doc| {
        place(
            &mut editor,
            doc,
            page,
            "iec/protection/mcb_3p",
            p(40.0, 100.0),
        );
    });
    let sym = doc.project().symbols().next().unwrap().id;
    for key in [EditorKey::Rotate, EditorKey::Mirror] {
        one_undo_step(&mut doc, |doc| {
            assert!(editor.key(doc, key, &[AnyId::Symbol(sym)]));
        });
    }
    one_undo_step(&mut doc, |doc| {
        assert!(editor.key(doc, EditorKey::Delete, &[AnyId::Symbol(sym)]));
    });
}

#[test]
fn a_wire_and_a_wire_ended_with_q_are_one_step_each() {
    let (mut doc, page, mut editor) = setup();
    place(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 100.0),
    );
    place(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(160.0, 100.0),
    );
    let pins: Vec<(novo_core::SymbolId, Point)> = doc
        .project()
        .symbols()
        .map(|s| (s.id, s.pin_position(1).unwrap()))
        .collect();
    let (a, b) = (pins[0], pins[1]);
    let a_pin = doc.project().symbol(a.0).unwrap().pin_position(0).unwrap();

    // Click a pin, click another pin: one wire, one step.
    one_undo_step(&mut doc, |doc| {
        editor.begin_wire();
        editor.click(doc, page, a_pin, TOL);
        editor.click(doc, page, b.1, TOL);
        editor.leave_tool();
    });

    // Q ends the wire on the part under the pointer: still one step.
    let a_other = doc.project().symbol(a.0).unwrap().pin_position(2).unwrap();
    one_undo_step(&mut doc, |doc| {
        editor.begin_wire();
        editor.click(doc, page, a_other, TOL);
        editor.hover(doc, page, p(160.0, 100.0), TOL);
        assert!(editor.key(doc, EditorKey::EndOnPart, &[]));
        editor.leave_tool();
    });
}

#[test]
fn the_cable_line_edits_nothing_so_there_is_nothing_to_undo() {
    let (mut doc, page, mut editor) = setup();
    place(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 100.0),
    );
    let depth = doc.undo_depth();
    editor.begin_cable();
    editor.click(&mut doc, page, p(300.0, 20.0), TOL);
    editor.click(&mut doc, page, p(300.0, 40.0), TOL);
    editor.key(&mut doc, EditorKey::Finish, &[]);
    assert_eq!(doc.undo_depth(), depth, "the line only collects wires");
    assert!(
        editor.take_cable_request().is_none(),
        "and an empty line asks for no cable"
    );
}

#[test]
fn auto_numbered_feeds_undo_one_at_a_time() {
    let (mut doc, page, _) = setup();
    let lib = novo_symbols::Library::builtin().unwrap();
    let feed = lib
        .get(novo_editor::potential::FEED_SYMBOL)
        .unwrap()
        .clone();
    let names = |doc: &Document| {
        let mut n: Vec<String> = doc.project().potentials().map(|p| p.name.clone()).collect();
        n.sort();
        n
    };
    for n in 0..3 {
        let before = doc.undo_depth();
        doc.transact("feed", |tx| {
            novo_editor::edit::place(
                tx,
                page,
                &feed,
                novo_core::Placement::at(p(40.0 * f64::from(n), 40.0)),
                &novo_editor::edit::PlaceOptions::new(),
            )
        })
        .unwrap();
        assert_eq!(doc.undo_depth(), before + 1);
    }
    assert_eq!(names(&doc), ["L1", "L2", "L3"]);
    doc.undo().unwrap();
    assert_eq!(
        names(&doc),
        ["L1", "L2"],
        "the potential goes with its feed"
    );
    doc.redo().unwrap();
    assert_eq!(names(&doc), ["L1", "L2", "L3"]);
    // After undoing two, the next new feed takes the freed name again.
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(novo_editor::edit::next_feed_name(doc.project(), page), "L2");
}
