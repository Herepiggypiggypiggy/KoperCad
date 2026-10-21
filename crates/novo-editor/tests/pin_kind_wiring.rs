//! Auto-wiring joins pins by what they are, not by where they sit: a coil
//! never gets a power wire, a motor never gets wired to a coil, and a pair
//! that is only unusual is asked about, not drawn.

use novo_core::{Document, PageId, Point};
use novo_editor::Editor;
use novo_editor::tool::ConnectMode;

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup(mode: ConnectMode) -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(mode);
    (doc, page, editor)
}

fn put(editor: &mut Editor, doc: &mut Document, page: PageId, id: &str, at: Point) {
    assert!(editor.begin_place(id, None), "{id}");
    editor.click(doc, page, at, TOL);
}

#[test]
fn a_motor_starter_wires_the_main_circuit_pole_by_pole() {
    let (mut doc, page, mut editor) = setup(ConnectMode::Draw);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 40.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/contactors/main_3p",
        p(40.0, 80.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/overload_relay",
        p(40.0, 120.0),
    );
    let depth = doc.undo_depth();
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/motors/motor_3ph",
        p(40.0, 160.0),
    );
    // Three poles Q1-K1, three K1-F, three F-M; the PE pin faces nothing.
    assert_eq!(doc.project().wires().count(), 9);
    // The motor and its three wires are one undo step.
    assert_eq!(doc.undo_depth(), depth + 1);
    assert!(editor.wiring_note().unwrap().contains("Connected 3"));
    doc.undo().unwrap();
    assert_eq!(doc.project().wires().count(), 6);
}

#[test]
fn a_motor_below_a_coil_is_not_wired_to_it() {
    let (mut doc, page, mut editor) = setup(ConnectMode::Draw);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_1p",
        p(40.0, 40.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/coils/contactor",
        p(40.0, 80.0),
    );
    // A one-pole breaker can feed a coil: that wire is drawn.
    assert_eq!(doc.project().wires().count(), 1);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/motors/motor_3ph",
        p(40.0, 120.0),
    );
    // The coil's A2 faces the motor's U1, and they are different circuits.
    assert_eq!(doc.project().wires().count(), 1);
    let notice = editor.wiring_note().expect("it says why");
    assert!(notice.contains("Not wired"), "{notice}");
    assert!(
        notice.contains("coil") || notice.contains("control"),
        "{notice}"
    );
    assert!(notice.contains("power"), "{notice}");
}

#[test]
fn a_coil_below_a_three_pole_breaker_is_not_wired() {
    let (mut doc, page, mut editor) = setup(ConnectMode::Draw);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 40.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/coils/contactor",
        p(40.0, 80.0),
    );
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn a_refused_pair_is_not_proposed_in_ask_mode_either() {
    let (mut doc, page, mut editor) = setup(ConnectMode::Ask);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/coils/contactor",
        p(40.0, 40.0),
    );
    assert!(editor.begin_place("iec/motors/motor_3ph", None));
    editor.click(&mut doc, page, p(40.0, 80.0), TOL);
    assert!(editor.proposals().is_empty());
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn crossed_poles_are_asked_about_not_drawn() {
    // The contactor sits one pole to the right: breaker pole 2 would go to
    // contactor pole 1's neighbour.
    let (mut doc, page, mut editor) = setup(ConnectMode::Draw);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 40.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/contactors/main_3p",
        p(48.0, 80.0),
    );
    assert_eq!(doc.project().wires().count(), 0);
    assert!(editor.wiring_note().unwrap().contains("crosses poles"));

    let (mut doc, page, mut editor) = setup(ConnectMode::Ask);
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 40.0),
    );
    assert!(editor.begin_place("iec/contactors/main_3p", None));
    editor.click(&mut doc, page, p(48.0, 80.0), TOL);
    // In Ask mode the unusual pairs are proposed, with the reason.
    assert_eq!(editor.proposals().len(), 2);
    assert!(
        editor
            .wiring_note()
            .unwrap()
            .contains("Check before accepting")
    );
}

#[test]
fn a_reversing_starter_wires_each_contactor_to_its_own_coil_circuit() {
    let (mut doc, page, mut editor) = setup(ConnectMode::Draw);
    // Main circuit: Q1 feeds K1 forward (left) and K2 reverse (right); the
    // coils hang under their own control breaker and never touch the mains.
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_3p",
        p(40.0, 40.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/contactors/main_3p",
        p(40.0, 80.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/contactors/main_3p",
        p(120.0, 80.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/protection/mcb_1p",
        p(200.0, 40.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/coils/contactor",
        p(200.0, 80.0),
    );
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/coils/contactor",
        p(240.0, 80.0),
    );
    // Q1-K1 three poles, F-coil one; K2 and the second coil face nothing.
    assert_eq!(doc.project().wires().count(), 4);
    // Under the first coil goes the motor, as in the audit: nothing.
    put(
        &mut editor,
        &mut doc,
        page,
        "iec/motors/motor_3ph",
        p(200.0, 120.0),
    );
    assert_eq!(doc.project().wires().count(), 4);
}
