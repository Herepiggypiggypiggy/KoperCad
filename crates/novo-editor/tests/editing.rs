//! The WORKFLOW.md journeys the schematic editor owns (J2, J3, J4, J10),
//! driven headless through the editor the way the canvas drives it: pointer
//! positions, clicks and keys in, model edits out.

use novo_core::{AnyId, Document, Endpoint, PageId, Point, PotentialKind, SymbolRole};
use novo_editor::edit::{self, PlaceOptions};
use novo_editor::potential;
use novo_editor::tool::{Clicked, Tool};
use novo_editor::{Editor, EditorKey};

/// Pick radius in millimetres, as at a normal zoom.
const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup() -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    (doc, page, Editor::default())
}

fn tags(doc: &Document) -> Vec<String> {
    let mut t: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
    t.sort_by(|a, b| novo_core::natural_cmp(a, b));
    t
}

fn symbol_at(doc: &Document, page: PageId, origin: Point) -> novo_core::SymbolId {
    doc.project()
        .page_contents(page)
        .symbols
        .into_iter()
        .find(|&s| doc.project().symbol(s).unwrap().placement.origin == origin)
        .expect("a symbol at that origin")
}

fn pin(doc: &Document, sym: novo_core::SymbolId, name: &str) -> Endpoint {
    let s = doc.project().symbol(sym).unwrap();
    Endpoint::pin(sym, s.pin_index(name).unwrap())
}

fn pin_pos(doc: &Document, sym: novo_core::SymbolId, name: &str) -> Point {
    doc.project()
        .endpoint_position(pin(doc, sym, name))
        .unwrap()
}

fn click_pin(
    editor: &mut Editor,
    doc: &mut Document,
    page: PageId,
    sym: novo_core::SymbolId,
    name: &str,
) -> Clicked {
    let at = pin_pos(doc, sym, name);
    editor.click(doc, page, at, TOL)
}

fn connected(doc: &Document, a: Endpoint, b: Endpoint) -> bool {
    doc.project().netlist().connected(a, b)
}

#[test]
fn ten_contactors_take_ten_clicks_and_get_unique_tags() {
    let (mut doc, page, mut editor) = setup();
    // I, Enter on the top match: the search ranks the coil first for "con".
    let hit = &editor.catalog.search("con")[0];
    assert!(editor.begin_place(&hit.symbol.clone(), None));
    for i in 0..10 {
        let clicked = editor.click(&mut doc, page, p(40.0 + 24.0 * f64::from(i), 100.0), TOL);
        assert_eq!(clicked, Clicked::Edited);
    }
    assert_eq!(
        tags(&doc),
        [
            "-K1", "-K2", "-K3", "-K4", "-K5", "-K6", "-K7", "-K8", "-K9", "-K10"
        ]
    );
    // Each placement is one undo step, and the tool stays active.
    assert_eq!(doc.undo_depth(), 11);
    assert!(matches!(editor.tool(), Tool::Place(_)));
    doc.undo().unwrap();
    assert_eq!(tags(&doc).len(), 9);
    // The placed device is offered to the properties panel.
    assert!(editor.take_selection().is_some());
}

#[test]
fn placement_snaps_to_the_grid_and_rotates() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/contacts/no", None);
    editor.hover(&doc, page, p(41.3, 98.9), TOL);
    assert_eq!(editor.cursor(), Some(p(40.0, 100.0)));
    assert!(editor.key(&mut doc, EditorKey::Rotate, &[]));
    assert!(editor.key(&mut doc, EditorKey::Mirror, &[]));
    editor.click(&mut doc, page, p(41.3, 98.9), TOL);
    let s = doc.project().symbols().next().unwrap();
    assert_eq!(s.placement.origin, p(40.0, 100.0));
    assert_eq!(s.placement.rotation, novo_core::Rotation::R90);
    assert!(s.placement.mirrored);
}

#[test]
fn child_contacts_join_their_coil_with_the_next_numbers() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(40.0, 160.0), TOL);
    let hit = editor.catalog.search("K1 no")[0].clone();
    editor.begin_place(&hit.symbol, hit.parent.as_deref());
    editor.click(&mut doc, page, p(100.0, 60.0), TOL);
    editor.click(&mut doc, page, p(120.0, 60.0), TOL);
    let hit = editor.catalog.search("K1 nc")[0].clone();
    editor.begin_place(&hit.symbol, hit.parent.as_deref());
    editor.click(&mut doc, page, p(140.0, 60.0), TOL);

    assert_eq!(tags(&doc), ["-K1"]);
    let k1 = doc.project().devices().next().unwrap().id;
    let mut contacts: Vec<String> = doc
        .project()
        .symbols_of_device(k1)
        .iter()
        .filter(|s| matches!(s.role, SymbolRole::Device { main: false, .. }))
        .map(|s| {
            s.pins
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>()
                .join("-")
        })
        .collect();
    contacts.sort();
    assert_eq!(contacts, ["13-14", "23-24", "31-32"]);
}

#[test]
fn a_contact_without_a_parent_gets_its_own_device() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 60.0), TOL);
    assert_eq!(tags(&doc), ["-K1"]);
    let s = doc.project().symbols().next().unwrap();
    assert!(matches!(s.role, SymbolRole::Device { main: false, .. }));
}

/// Places the Gate 1 main circuit in one column: fuses, contactor,
/// overload and motor stacked 8 mm apart line up pin for pin.
fn main_circuit(doc: &mut Document, page: PageId, editor: &mut Editor, gap: f64) -> [Point; 4] {
    let origins = [
        p(60.0, 40.0),
        p(60.0, 56.0 + gap),
        p(60.0, 72.0 + 2.0 * gap),
        p(60.0, 88.0 + 3.0 * gap),
    ];
    for (symbol, at) in [
        "iec/protection/fuse_3p",
        "iec/contactors/main_3p",
        "iec/protection/overload_relay",
        "iec/motors/motor_3ph",
    ]
    .iter()
    .zip(origins)
    {
        editor.begin_place(symbol, None);
        assert_eq!(editor.click(doc, page, at, TOL), Clicked::Edited);
    }
    origins
}

#[test]
fn stacked_three_pole_devices_wire_themselves() {
    let (mut doc, page, mut editor) = setup();
    let [f, k, ol, m] = main_circuit(&mut doc, page, &mut editor, 16.0);
    let (f, k, ol, m) = (
        symbol_at(&doc, page, f),
        symbol_at(&doc, page, k),
        symbol_at(&doc, page, ol),
        symbol_at(&doc, page, m),
    );
    // Facing free pins on one line are wired straight: 9 wires, no clicks.
    assert_eq!(doc.project().wires().count(), 9);
    for (a, b) in [("2", "1"), ("4", "3"), ("6", "5")] {
        assert!(connected(&doc, pin(&doc, f, a), pin(&doc, k, b)));
        assert!(connected(&doc, pin(&doc, k, a), pin(&doc, ol, b)));
    }
    assert!(connected(&doc, pin(&doc, ol, "2"), pin(&doc, m, "U1")));
    assert!(connected(&doc, pin(&doc, ol, "6"), pin(&doc, m, "W1")));
    assert!(doc.project().wires().all(|w| w.route.is_empty()));
    assert_eq!(tags(&doc), ["-F1", "-F2", "-K1", "-M1"]);
}

#[test]
fn wiring_three_phases_takes_one_plus_six_actions() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(&mut doc, page, p(60.0, 40.0), TOL);
    editor.begin_place("iec/protection/overload_relay", None);
    editor.click(&mut doc, page, p(100.0, 100.0), TOL);
    editor.leave_tool();
    let k = symbol_at(&doc, page, p(60.0, 40.0));
    let f = symbol_at(&doc, page, p(100.0, 100.0));
    let before = doc.undo_depth();

    // W, then two clicks per wire.
    assert!(editor.key(&mut doc, EditorKey::Wire, &[]));
    for (a, b) in [("2", "1"), ("4", "3"), ("6", "5")] {
        assert_eq!(click_pin(&mut editor, &mut doc, page, k, a), Clicked::Step);
        assert_eq!(
            click_pin(&mut editor, &mut doc, page, f, b),
            Clicked::Edited
        );
        assert!(connected(&doc, pin(&doc, k, a), pin(&doc, f, b)));
    }
    assert_eq!(doc.undo_depth(), before + 3);
    // Pin to pin between two downward/upward pins: a Z route, orthogonal.
    for w in doc.project().wires() {
        assert_eq!(w.route.len(), 2);
        let pts = novo_editor::geometry::wire_points(doc.project(), w.id).unwrap();
        for s in pts.windows(2) {
            assert!(
                s[0].x == s[1].x || s[0].y == s[1].y,
                "not orthogonal: {pts:?}"
            );
        }
    }
}

#[test]
fn ending_on_a_wire_makes_a_t_junction_without_asking() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 40.0), TOL);
    editor.click(&mut doc, page, p(120.0, 80.0), TOL);
    let a = symbol_at(&doc, page, p(40.0, 40.0));
    let b = symbol_at(&doc, page, p(80.0, 40.0));
    let c = symbol_at(&doc, page, p(120.0, 80.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    // A corner in empty space, then the other pin: 40,72 -> 80,72 -> 80,56.
    assert_eq!(
        editor.click(&mut doc, page, p(40.0, 72.0), TOL),
        Clicked::Step
    );
    assert_eq!(
        editor.click(&mut doc, page, p(80.0, 72.0), TOL),
        Clicked::Step
    );
    assert_eq!(
        click_pin(&mut editor, &mut doc, page, b, "14"),
        Clicked::Edited
    );
    // From the third contact's top pin onto the middle of that wire.
    click_pin(&mut editor, &mut doc, page, c, "13");
    assert_eq!(
        editor.click(&mut doc, page, p(60.0, 72.3), TOL),
        Clicked::Edited
    );

    assert_eq!(doc.project().junctions().count(), 1);
    let j = doc.project().junctions().next().unwrap();
    assert_eq!(j.position, p(60.0, 72.0));
    assert_eq!(doc.project().wires_at(Endpoint::junction(j.id)).count(), 3);
    assert!(connected(&doc, pin(&doc, a, "14"), pin(&doc, c, "13")));
    assert!(connected(&doc, pin(&doc, b, "14"), pin(&doc, c, "13")));
    // One undo removes the branch and the junction together.
    doc.undo().unwrap();
    assert_eq!(doc.project().junctions().count(), 0);
    assert_eq!(doc.project().wires().count(), 1);
}

#[test]
fn escape_mid_wire_drops_only_the_unfinished_wire() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 40.0), TOL);
    let a = symbol_at(&doc, page, p(40.0, 40.0));
    let b = symbol_at(&doc, page, p(80.0, 40.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    click_pin(&mut editor, &mut doc, page, b, "14");
    click_pin(&mut editor, &mut doc, page, a, "13");
    let depth = doc.undo_depth();
    // First rung: the wire in progress; second: the tool; then nothing.
    assert!(editor.cancel_step());
    assert!(matches!(editor.tool(), Tool::Wire(w) if w.start.is_none()));
    assert!(!editor.cancel_step());
    assert!(editor.leave_tool());
    assert!(!editor.leave_tool());
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.project().wires().count(), 1);
    // Enter brings the wire tool back.
    assert!(editor.key(&mut doc, EditorKey::Repeat, &[]));
    assert!(matches!(editor.tool(), Tool::Wire(_)));
}

#[test]
fn a_wire_never_starts_on_nothing() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_wire();
    assert_eq!(
        editor.click(&mut doc, page, p(10.0, 10.0), TOL),
        Clicked::Refused
    );
    assert!(editor.notice().is_some());
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn a_contact_dropped_on_a_wire_is_inserted_in_series() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 20.0), TOL);
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    let top = symbol_at(&doc, page, p(40.0, 20.0));
    let coil = symbol_at(&doc, page, p(40.0, 120.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, top, "14");
    click_pin(&mut editor, &mut doc, page, coil, "A1");
    assert_eq!(doc.project().wires().count(), 1);

    editor.auto_connect = true;
    let hit = editor.catalog.search("pbnc")[0].clone();
    editor.begin_place(&hit.symbol, None);
    editor.click(&mut doc, page, p(40.0, 64.0), TOL);
    let stop = symbol_at(&doc, page, p(40.0, 64.0));
    let s = doc.project().symbol(stop).unwrap();
    let (s_top, s_bottom) = (Endpoint::pin(stop, 0), Endpoint::pin(stop, 1));
    assert_eq!(s.pins.len(), 2);
    assert_eq!(doc.project().wires().count(), 2);
    assert!(connected(&doc, pin(&doc, top, "14"), s_top));
    assert!(connected(&doc, s_bottom, pin(&doc, coil, "A1")));
    assert!(!connected(
        &doc,
        pin(&doc, top, "14"),
        pin(&doc, coil, "A1")
    ));
}

#[test]
fn deleting_a_device_removes_its_wires_and_tidies_junctions() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    for (x, y) in [(40.0, 40.0), (80.0, 100.0), (120.0, 40.0)] {
        editor.click(&mut doc, page, p(x, y), TOL);
    }
    let a = symbol_at(&doc, page, p(40.0, 40.0));
    let b = symbol_at(&doc, page, p(80.0, 100.0));
    let c = symbol_at(&doc, page, p(120.0, 40.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    click_pin(&mut editor, &mut doc, page, c, "14");
    click_pin(&mut editor, &mut doc, page, b, "13");
    editor.click(&mut doc, page, p(80.0, 56.0), TOL);
    assert_eq!(doc.project().junctions().count(), 1);
    editor.leave_tool();

    assert!(editor.key(&mut doc, EditorKey::Delete, &[AnyId::Symbol(b)]));
    // The branch went with the contact, and the junction joined the rest.
    assert_eq!(doc.project().junctions().count(), 0);
    assert_eq!(doc.project().wires().count(), 1);
    assert!(connected(&doc, pin(&doc, a, "14"), pin(&doc, c, "14")));
    // Its device went too, since nothing else draws it.
    assert_eq!(tags(&doc), ["-K1", "-K3"]);
    doc.undo().unwrap();
    assert_eq!(tags(&doc).len(), 3);
}

#[test]
fn rename_keeps_structure_moves_contacts_and_refuses_duplicates() {
    let (mut doc, page, mut editor) = setup();
    doc.transact("Structure", |tx| {
        tx.set_property(page, "structure", Some("=A1+CAB1".into()))
    })
    .unwrap();
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 40.0), TOL);
    editor.begin_place("iec/contacts/no", Some("=A1+CAB1-K1"));
    editor.click(&mut doc, page, p(120.0, 40.0), TOL);
    assert_eq!(tags(&doc), ["=A1+CAB1-K1", "=A1+CAB1-K2"]);
    let k1 = doc
        .project()
        .devices_by_tag(&"=A1+CAB1-K1".parse().unwrap())[0];

    assert!(
        editor
            .rename(&mut doc, k1, "-K2")
            .unwrap_err()
            .contains("already used")
    );
    assert!(editor.check_rename(&doc, k1, "-K 2!").is_err());
    let depth = doc.undo_depth();
    editor.rename(&mut doc, k1, "-K10").unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(tags(&doc), ["=A1+CAB1-K2", "=A1+CAB1-K10"]);
    // Its contact follows: it points at the device, not at the old text.
    let contact = symbol_at(&doc, page, p(120.0, 40.0));
    assert_eq!(doc.project().symbol(contact).unwrap().device(), Some(k1));
}

#[test]
fn move_and_rotate_keep_wires_connected_and_orthogonal() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 100.0), TOL);
    let a = symbol_at(&doc, page, p(40.0, 40.0));
    let b = symbol_at(&doc, page, p(80.0, 100.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    click_pin(&mut editor, &mut doc, page, b, "13");
    editor.leave_tool();

    // M picks the contact up, the pointer carries it, a click drops it.
    editor.hover(&doc, page, p(80.0, 100.0), TOL);
    assert!(editor.key(&mut doc, EditorKey::Move, &[AnyId::Symbol(b)]));
    editor.hover(&doc, page, p(120.0, 120.0), TOL);
    let preview = editor.preview(&doc, page);
    assert_eq!(preview.symbols[0].placement.origin, p(120.0, 120.0));
    assert_eq!(
        editor.click(&mut doc, page, p(120.0, 120.0), TOL),
        Clicked::Edited
    );
    assert_eq!(
        doc.project().symbol(b).unwrap().placement.origin,
        p(120.0, 120.0)
    );

    assert!(editor.key(&mut doc, EditorKey::Rotate, &[AnyId::Symbol(b)]));
    assert_eq!(
        doc.project().symbol(b).unwrap().placement.rotation,
        novo_core::Rotation::R90
    );
    let w = doc.project().wires().next().unwrap().id;
    let pts = novo_editor::geometry::wire_points(doc.project(), w).unwrap();
    assert_eq!(pts[0], pin_pos(&doc, a, "14"));
    assert_eq!(*pts.last().unwrap(), pin_pos(&doc, b, "13"));
    for s in pts.windows(2) {
        assert!(
            s[0].x == s[1].x || s[0].y == s[1].y,
            "not orthogonal: {pts:?}"
        );
    }
}

/// Three horizontal wires from a three-pole source to a three-pole load.
fn three_wires(doc: &mut Document, page: PageId, editor: &mut Editor) -> Vec<novo_core::WireId> {
    editor.auto_connect = false;
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(doc, page, p(40.0, 120.0), TOL);
    let f = symbol_at(doc, page, p(40.0, 40.0));
    let k = symbol_at(doc, page, p(40.0, 120.0));
    editor.begin_wire();
    for (a, b) in [("2", "1"), ("4", "3"), ("6", "5")] {
        editor.click(doc, page, pin_pos(doc, f, a), TOL);
        editor.click(doc, page, pin_pos(doc, k, b), TOL);
    }
    editor.leave_tool();
    doc.project().wires().map(|w| w.id).collect()
}

#[test]
fn three_wires_suggest_three_phase_and_get_l1_l2_l3_in_page_order() {
    let (mut doc, page, mut editor) = setup();
    let mut wires = three_wires(&mut doc, page, &mut editor);
    wires.reverse(); // selection order must not matter
    let selection: Vec<AnyId> = wires.iter().map(|&w| AnyId::Wire(w)).collect();
    let (ordered, suggestions) = editor.potential_suggestions(&doc, &selection);
    assert_eq!(suggestions[0].label, "Three phase 400 V (L1, L2, L3)");
    let depth = doc.undo_depth();
    assert!(editor.assign_potentials(&mut doc, &ordered, &suggestions[0]));
    assert_eq!(doc.undo_depth(), depth + 1);

    let f = symbol_at(&doc, page, p(40.0, 40.0));
    let netlist = doc.project().netlist();
    let name = |e: Endpoint| {
        let pot = netlist.potential_of(e).expect("a potential");
        doc.project().potential(pot).unwrap().name.clone()
    };
    assert_eq!(name(pin(&doc, f, "2")), "L1");
    assert_eq!(name(pin(&doc, f, "4")), "L2");
    assert_eq!(name(pin(&doc, f, "6")), "L3");
    let l1 = doc.project().potentials().find(|p| p.name == "L1").unwrap();
    assert_eq!(l1.kind, PotentialKind::Line);
    assert_eq!(l1.voltage, Some(400.0));
    // Still three nets, each still one wire end to end electrically.
    let k = symbol_at(&doc, page, p(40.0, 120.0));
    assert!(connected(&doc, pin(&doc, f, "2"), pin(&doc, k, "1")));
    assert!(
        doc.project()
            .netlist()
            .potential_conflicts()
            .next()
            .is_none()
    );
}

#[test]
fn changing_a_wires_potential_changes_its_whole_net() {
    let (mut doc, page, mut editor) = setup();
    let wires = three_wires(&mut doc, page, &mut editor);
    let ordered = potential::page_order(doc.project(), &wires);
    let (_, suggestions) = editor.potential_suggestions(
        &doc,
        &ordered.iter().map(|&w| AnyId::Wire(w)).collect::<Vec<_>>(),
    );
    editor.assign_potentials(&mut doc, &ordered, &suggestions[0]);

    // Now take the first phase's wire (whichever piece of it) and make it N.
    let f = symbol_at(&doc, page, p(40.0, 40.0));
    let first = doc.project().wires_at(pin(&doc, f, "2")).next().unwrap();
    let (_, single) = editor.potential_suggestions(&doc, &[AnyId::Wire(first)]);
    let n = single
        .iter()
        .find(|s| s.specs[0].name == "N")
        .unwrap()
        .clone();
    assert!(editor.assign_potentials(&mut doc, &[first], &n));
    let netlist = doc.project().netlist();
    let k = symbol_at(&doc, page, p(40.0, 120.0));
    let pot = netlist.potential_of(pin(&doc, k, "1")).unwrap();
    assert_eq!(doc.project().potential(pot).unwrap().name, "N");
    // The old L1 feed was replaced, not stacked: no conflict.
    assert!(netlist.potential_conflicts().next().is_none());
}

#[test]
fn a_wire_to_a_pe_terminal_suggests_pe_first() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/terminals/terminal_pe", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/motors/motor_3ph", None);
    editor.click(&mut doc, page, p(60.0, 100.0), TOL);
    let x = symbol_at(&doc, page, p(40.0, 40.0));
    let m = symbol_at(&doc, page, p(60.0, 100.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, x, "2");
    click_pin(&mut editor, &mut doc, page, m, "PE");
    let wire = doc.project().wires().next().unwrap().id;
    let designation = doc
        .project()
        .pin_designation(pin(&doc, x, "2").as_pin().unwrap())
        .unwrap();
    assert_eq!(designation, "-X1:PE");
    let (_, suggestions) = editor.potential_suggestions(&doc, &[AnyId::Wire(wire)]);
    assert_eq!(suggestions[0].specs[0].name, "PE");
    assert!(suggestions[0].label.contains("from its connection"));
}

#[test]
fn terminals_number_themselves_on_the_strip_in_use() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/terminals/terminal", None);
    for x in [40.0, 48.0, 56.0] {
        editor.click(&mut doc, page, p(x, 40.0), TOL);
    }
    editor.begin_place("iec/terminals/terminal_pe", None);
    editor.click(&mut doc, page, p(64.0, 40.0), TOL);
    let names: Vec<String> = doc
        .project()
        .terminals()
        .map(|t| doc.project().terminal_designation(t.id).unwrap())
        .collect();
    assert_eq!(names, ["-X1:1", "-X1:2", "-X1:3", "-X1:PE"]);
    assert_eq!(tags(&doc), ["-X1"]);
}

#[test]
fn status_bar_says_what_the_next_click_does() {
    let (mut doc, page, mut editor) = setup();
    assert!(editor.status(&doc, page).contains("I insert"));
    editor.begin_place("iec/coils/relay", None);
    assert!(editor.status(&doc, page).contains("as -K1"));
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    assert!(editor.status(&doc, page).contains("as -K2"));
    editor.begin_wire();
    assert!(editor.status(&doc, page).contains("start a wire"));
}

#[test]
fn scripts_can_place_without_auto_connect() {
    let (mut doc, page, editor) = setup();
    let symbol = editor.catalog.get("iec/contacts/no").unwrap().clone();
    let (id, _) = doc
        .transact("Script", |tx| {
            edit::place(
                tx,
                page,
                &symbol,
                novo_core::Placement::at(p(40.0, 40.0)),
                &PlaceOptions::default(),
            )
        })
        .unwrap();
    assert!(doc.project().symbol(id).is_some());
}
