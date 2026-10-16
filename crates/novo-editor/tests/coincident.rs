//! A wire end and a pin (or junction) at the same position are one
//! connection, however the coincidence came about: drawn, placed, moved,
//! loaded or generated. No open-end flag stays behind.

use novo_core::{AnyId, Document, Endpoint, PageId, Point, SymbolId, WireId};
use novo_editor::Editor;
use novo_editor::edit::{self, Transform};
use novo_editor::geometry::Target;
use novo_editor::tool::{ConnectMode, Tool};

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup(auto_connect: bool) -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Never);
    editor.auto_connect = auto_connect;
    (doc, page, editor)
}

fn place(editor: &mut Editor, doc: &mut Document, page: PageId, at: Point) -> SymbolId {
    editor.begin_place("iec/contacts/no", None);
    editor.click(doc, page, at, TOL);
    editor.leave_tool();
    doc.project()
        .page_contents(page)
        .symbols
        .into_iter()
        .find(|&s| doc.project().symbol(s).unwrap().placement.origin == at)
        .expect("placed")
}

fn pin(doc: &Document, s: SymbolId, name: &str) -> (Endpoint, Point) {
    let sym = doc.project().symbol(s).unwrap();
    let e = Endpoint::pin(s, sym.pin_index(name).unwrap());
    (e, doc.project().endpoint_position(e).unwrap())
}

/// A wire from far above to an open end at `at`.
fn wire_to(doc: &mut Document, page: PageId, at: Point) -> WireId {
    doc.transact("wire", |tx| {
        tx.connect_on(
            page,
            Endpoint::Free {
                at: p(at.x, at.y - 30.0),
            },
            Endpoint::Free { at },
        )
    })
    .unwrap()
    .0
}

fn assert_linked(doc: &Document, w: WireId, to: Endpoint) {
    let wire = doc.project().wire(w).unwrap();
    assert_eq!(wire.to, to, "the wire end is on the pin");
}

#[test]
fn a_part_placed_onto_a_wire_end_links_with_and_without_auto_connect() {
    for auto in [true, false] {
        let (mut doc, page, mut editor) = setup(auto);
        // The pin of a part at (40, 40), found by placing one elsewhere first.
        let probe = place(&mut editor, &mut doc, page, p(200.0, 200.0));
        let (_, rel) = pin(&doc, probe, "13");
        let offset = p(rel.x - 200.0, rel.y - 200.0);
        let target = p(40.0 + offset.x, 40.0 + offset.y);
        let w = wire_to(&mut doc, page, target);
        assert_eq!(edit::open_end_count(doc.project()), 4 - 2, "two open ends");
        let s = place(&mut editor, &mut doc, page, p(40.0, 40.0));
        let (e, at) = pin(&doc, s, "13");
        assert_eq!(at, target);
        assert_linked(&doc, w, e);
        assert_eq!(edit::open_end_count(doc.project()), 1, "only the far end");
        // One undo step takes the placement and the link back.
        doc.undo().unwrap();
        assert!(matches!(
            doc.project().wire(w).unwrap().to,
            Endpoint::Free { .. }
        ));
    }
}

#[test]
fn moving_a_part_so_a_pin_lands_on_a_wire_end_links_it() {
    let (mut doc, page, mut editor) = setup(false);
    let s = place(&mut editor, &mut doc, page, p(40.0, 40.0));
    let (e, at) = pin(&doc, s, "13");
    let w = wire_to(&mut doc, page, p(at.x + 20.0, at.y));
    doc.transact("move", |tx| {
        edit::transform(tx, &[AnyId::Symbol(s)], Transform::Move(p(20.0, 0.0)))
    })
    .unwrap();
    assert_linked(&doc, w, e);
}

#[test]
fn turning_a_part_onto_a_wire_end_links_it_despite_rounding() {
    let (mut doc, page, mut editor) = setup(false);
    let s = place(&mut editor, &mut doc, page, p(40.0, 40.0));
    let (e, at) = pin(&doc, s, "13");
    // The wire ends where the pin will be after a quarter turn about its
    // own origin, plus float noise from the rotation.
    let origin = p(40.0, 40.0);
    let d = p(at.x - origin.x, at.y - origin.y);
    let turned = p(origin.x - d.y + 1e-12, origin.y + d.x - 1e-12);
    let w = wire_to(&mut doc, page, turned);
    doc.transact("turn", |tx| {
        edit::transform(tx, &[AnyId::Symbol(s)], Transform::Rotate(origin))
    })
    .unwrap();
    assert_linked(&doc, w, e);
}

#[test]
fn a_wire_drawn_to_an_open_end_on_a_pin_links() {
    let (mut doc, page, mut editor) = setup(false);
    let s = place(&mut editor, &mut doc, page, p(40.0, 40.0));
    let (e, at) = pin(&doc, s, "13");
    let other = place(&mut editor, &mut doc, page, p(40.0, 100.0));
    let (from, from_at) = pin(&doc, other, "13");
    let start = Target::Endpoint(from, from_at);
    let end = Target::Endpoint(Endpoint::Free { at }, at);
    let w = doc
        .transact("draw", |tx| edit::draw_wire(tx, page, start, &[], end))
        .unwrap()
        .0;
    assert_linked(&doc, w, e);
    assert!(matches!(editor.tool(), Tool::Select));
}

#[test]
fn an_open_end_on_a_junction_links() {
    let (mut doc, page, _) = setup(false);
    let j = doc
        .transact("j", |tx| tx.add_junction(page, p(10.0, 10.0)))
        .unwrap()
        .0;
    let w = wire_to(&mut doc, page, p(10.0, 10.0));
    let n = doc
        .transact("heal", |tx| edit::join_coincident(tx, page))
        .unwrap()
        .0;
    assert_eq!(n, 1);
    assert_linked(&doc, w, Endpoint::Junction { junction: j });
}

#[test]
fn a_loaded_project_is_healed_without_an_undo_step() {
    let (mut doc, page, mut editor) = setup(false);
    let s = place(&mut editor, &mut doc, page, p(40.0, 40.0));
    let (e, at) = pin(&doc, s, "13");
    let w = wire_to(&mut doc, page, at);
    assert!(matches!(
        doc.project().wire(w).unwrap().to,
        Endpoint::Free { .. }
    ));
    let mut project = doc.into_project();
    assert_eq!(edit::heal_project(&mut project).unwrap(), 1);
    assert_eq!(project.wire(w).unwrap().to, e);
    // Healing twice finds nothing.
    assert_eq!(edit::heal_project(&mut project).unwrap(), 0);
    // And the pin is on a net with the wire.
    let nets = project.netlist();
    assert!(nets.net_of(e).is_some());
    assert_eq!(nets.net_of(e), nets.net_of_wire(project.wire(w).unwrap()));
}

#[test]
fn near_but_not_on_stays_open_and_a_far_cap_stays_a_cap() {
    let (mut doc, page, mut editor) = setup(false);
    let s = place(&mut editor, &mut doc, page, p(40.0, 40.0));
    let (_, at) = pin(&doc, s, "13");
    // 0.1 mm off is a different point: the open-end flag is truthful.
    let w = wire_to(&mut doc, page, p(at.x + 0.1, at.y));
    let mut project = doc.into_project();
    assert_eq!(edit::heal_project(&mut project).unwrap(), 0);
    assert!(matches!(project.wire(w).unwrap().to, Endpoint::Free { .. }));
}

#[test]
fn the_pointer_sticks_to_the_grid_along_a_wire_even_near_its_ends() {
    let (mut doc, page, mut editor) = setup(false);
    let g = editor.grid;
    // A horizontal wire from x = 0.3 g to 6.2 g on a grid line.
    let (a, b) = (p(0.3 * g, 4.0 * g), p(6.2 * g, 4.0 * g));
    doc.transact("wire", |tx| {
        tx.connect_on(page, Endpoint::Free { at: a }, Endpoint::Free { at: b })
    })
    .unwrap();
    editor.begin_wire();
    for x in [0.5, 0.9, 1.2, 3.4, 5.6, 5.9] {
        let raw = p(x * g, 4.0 * g + 0.2);
        editor.hover(&doc, page, raw, g * 0.6);
        let Some(Target::Wire(_, at)) = editor.target() else {
            panic!("the wire is under the pointer at {x}");
        };
        let cells = at.x / g;
        assert!(
            (cells - cells.round()).abs() < 1e-9,
            "x = {x} g glides to {cells} g"
        );
        assert_eq!(at.y, a.y);
    }
}
