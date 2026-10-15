//! The DESIGN.md §6 journeys the schematic editor owns (place, connect,
//! rename, potential sources),
//! driven headless through the editor the way the canvas drives it: pointer
//! positions, clicks and keys in, model edits out.

use novo_core::{AnyId, Document, Endpoint, PageId, Point, PotentialKind, SymbolRole};
use novo_editor::edit::{self, PlaceOptions};
use novo_editor::geometry::Target;
use novo_editor::potential;
use novo_editor::tool::{Clicked, Tool, WiringMode};
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

#[test]
fn coil_k1_places_the_coil_of_the_existing_device() {
    let (mut doc, page, mut editor) = setup();
    // A contact placed first makes -K1 without a coil.
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 60.0), TOL);
    assert_eq!(tags(&doc), ["-K1"]);
    let k1 = doc.project().devices().next().unwrap().id;

    // `coil K1` gives -K1 its coil instead of making -K2.
    let hit = editor.catalog.search("coil K1")[0].clone();
    assert_eq!(hit.parent.as_deref(), Some("-K1"));
    editor.begin_place(&hit.symbol, hit.parent.as_deref());
    assert_eq!(
        editor.click(&mut doc, page, p(40.0, 160.0), TOL),
        Clicked::Edited
    );
    assert_eq!(tags(&doc), ["-K1"]);
    let mains = |doc: &Document| {
        doc.project()
            .symbols_of_device(k1)
            .iter()
            .filter(|s| matches!(s.role, SymbolRole::Device { main: true, .. }))
            .count()
    };
    assert_eq!(mains(&doc), 1);

    // A second coil for -K1 is refused with a notice; nothing changes.
    let depth = doc.undo_depth();
    assert_eq!(
        editor.click(&mut doc, page, p(80.0, 160.0), TOL),
        Clicked::Refused
    );
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(mains(&doc), 1);
    let notice = editor.notice().unwrap_or_default().to_string();
    assert!(
        notice.contains("-K1 already has its main function"),
        "{notice}"
    );

    // A tag that names no device yet places a new device with that tag.
    let hit = editor.catalog.search("coil K7")[0].clone();
    editor.begin_place(&hit.symbol, hit.parent.as_deref());
    editor.click(&mut doc, page, p(120.0, 160.0), TOL);
    assert_eq!(tags(&doc), ["-K1", "-K7"]);
}

#[test]
fn scripts_cannot_give_a_device_a_second_main_function() {
    let (mut doc, page, editor) = setup();
    let coil = editor.catalog.get("iec/coils/relay").unwrap().clone();
    let options = PlaceOptions {
        parent: Some("-K1".parse().unwrap()),
        ..PlaceOptions::new()
    };
    let place = |doc: &mut Document, x: f64| {
        doc.transact("Place", |tx| {
            edit::place(
                tx,
                page,
                &coil,
                novo_core::Placement::at(p(x, 40.0)),
                &options,
            )
        })
    };
    let (first, _) = place(&mut doc, 40.0).unwrap();
    let k1 = doc.project().devices().next().unwrap().id;
    assert_eq!(edit::main_symbol(doc.project(), k1), Some(first));
    let depth = doc.undo_depth();
    assert_eq!(
        place(&mut doc, 80.0).unwrap_err(),
        novo_core::ModelError::AlreadyExists(first.into())
    );
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.project().symbols().count(), 1);
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
        // Facing pins are proposed; Enter takes each.
        while editor.accept_proposal(doc) {}
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
    // Facing free pins on one line are proposed and accepted: 9 wires.
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
    // From the third contact's top pin onto the side of the vertical run
    // (arriving along the wire would be an overlap, which is refused).
    click_pin(&mut editor, &mut doc, page, c, "13");
    assert_eq!(
        editor.click(&mut doc, page, p(80.3, 64.0), TOL),
        Clicked::Edited
    );

    assert_eq!(doc.project().junctions().count(), 1);
    let j = doc.project().junctions().next().unwrap();
    assert_eq!(j.position, p(80.0, 64.0));
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
    // Repeat-last-tool brings the wire tool back.
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
        tx.update::<novo_core::Page>(page, |p| p.structure = "=A1+CAB1".parse().unwrap())
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
    let dropped = editor.click(&mut doc, page, p(120.0, 120.0), TOL);
    assert_eq!(dropped, Clicked::Edited, "{:?}", editor.notice());
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
    let menu = editor.potential_menu(&doc, &selection);
    assert_eq!(menu.carried, None);
    assert_eq!(
        menu.declare[0].label,
        "Declare source: 3~ 400 V (L1, L2, L3)"
    );
    let depth = doc.undo_depth();
    assert!(editor.assign_potentials(&mut doc, &menu.wires, &menu.declare[0]));
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
    let menu = editor.potential_menu(
        &doc,
        &ordered.iter().map(|&w| AnyId::Wire(w)).collect::<Vec<_>>(),
    );
    editor.assign_potentials(&mut doc, &ordered, &menu.declare[0]);

    // Now take the first phase's wire (whichever piece of it) and make it N.
    let f = symbol_at(&doc, page, p(40.0, 40.0));
    let first = doc.project().wires_at(pin(&doc, f, "2")).next().unwrap();
    // Its source sits on this wire, so the menu shows what it carries and
    // offers to change the source.
    let single = editor.potential_menu(&doc, &[AnyId::Wire(first)]);
    assert_eq!(single.carried.as_deref(), Some("Carries L1"));
    assert!(single.declare[0].label.starts_with("Change source: "));
    let n = single
        .declare
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
fn a_wire_to_a_pe_terminal_is_pe_by_rule_without_a_choice() {
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
    let menu = editor.potential_menu(&doc, &[AnyId::Wire(wire)]);
    assert_eq!(menu.carried.as_deref(), Some("Carries PE (by name)"));
    assert!(menu.declare.is_empty());
    // With the rule switched off it is an ordinary unassigned wire.
    editor.supply_names = false;
    let menu = editor.potential_menu(&doc, &[AnyId::Wire(wire)]);
    assert_eq!(menu.carried, None);
    assert_eq!(menu.declare[0].label, "Declare source: PE");
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
    // Quiet with nothing in hand.
    assert_eq!(editor.status(&doc, page), "");
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

/// Wires between facing pins are drawn only where the preview showed them.
fn wire_ends(doc: &Document) -> Vec<(Point, Point)> {
    let mut ends: Vec<(Point, Point)> = doc
        .project()
        .wires()
        .map(|w| {
            let a = doc.project().endpoint_position(w.from).unwrap();
            let b = doc.project().endpoint_position(w.to).unwrap();
            if (a.x, a.y) <= (b.x, b.y) {
                (a, b)
            } else {
                (b, a)
            }
        })
        .collect();
    ends.sort_by(|x, y| x.0.x.total_cmp(&y.0.x));
    ends
}

fn preview_ends(editor: &Editor, doc: &Document, page: PageId) -> Vec<(Point, Point)> {
    let mut ends: Vec<(Point, Point)> = editor
        .preview(doc, page)
        .proposals
        .into_iter()
        .map(|(a, b)| {
            if (a.x, a.y) <= (b.x, b.y) {
                (a, b)
            } else {
                (b, a)
            }
        })
        .collect();
    ends.sort_by(|x, y| x.0.x.total_cmp(&y.0.x));
    ends
}

fn proposal_ends(editor: &Editor) -> Vec<(Point, Point)> {
    let mut ends: Vec<(Point, Point)> = editor
        .proposals()
        .iter()
        .map(|f| {
            if (f.a.x, f.a.y) <= (f.b.x, f.b.y) {
                (f.a, f.b)
            } else {
                (f.b, f.a)
            }
        })
        .collect();
    ends.sort_by(|x, y| x.0.x.total_cmp(&y.0.x));
    ends
}

#[test]
fn facing_pins_are_proposed_as_previewed_and_enter_accepts_them() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.hover(&doc, page, p(40.0, 120.0), TOL);
    let shown = preview_ends(&editor, &doc, page);
    assert_eq!(shown.len(), 3);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    // Proposed, not connected.
    assert_eq!(doc.project().wires().count(), 0);
    assert_eq!(proposal_ends(&editor), shown);
    assert_eq!(editor.preview(&doc, page).current, Some(0));
    // Enter accepts one at a time, each its own undo step; Tab moves on.
    let depth = doc.undo_depth();
    assert!(editor.accept_proposal(&mut doc));
    assert!(editor.next_proposal());
    assert!(editor.accept_proposal(&mut doc));
    assert!(editor.accept_proposal(&mut doc));
    assert!(!editor.accept_proposal(&mut doc));
    assert_eq!(doc.undo_depth(), depth + 3);
    assert_eq!(wire_ends(&doc), shown);
}

#[test]
fn escape_or_clicking_elsewhere_leaves_proposals_unaccepted() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    assert_eq!(editor.proposals().len(), 3);
    // Escape's first rung drops the proposals and keeps the tool.
    assert!(editor.cancel_step());
    assert!(editor.proposals().is_empty());
    assert!(editor.tool_active());
    editor.click(&mut doc, page, p(40.0, 200.0), TOL);
    assert!(!editor.proposals().is_empty());
    editor.leave_tool();
    editor.click(&mut doc, page, p(150.0, 150.0), TOL);
    assert!(editor.proposals().is_empty());
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn moving_connects_nothing_the_preview_did_not_show() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(&mut doc, page, p(80.0, 120.0), TOL);
    editor.leave_tool();
    let k = AnyId::Symbol(symbol_at(&doc, page, p(80.0, 120.0)));
    // A move with no preview (arrow keys, scripts) proposes nothing.
    assert!(editor.move_selection(&mut doc, &[k], p(-40.0, 0.0)));
    assert!(editor.proposals().is_empty());
    assert!(editor.move_selection(&mut doc, &[k], p(40.0, 0.0)));
    // The move tool shows the proposals while dragging; the drop keeps them.
    editor.hover(&doc, page, p(80.0, 120.0), TOL);
    assert!(editor.begin_move(&[k]));
    editor.hover(&doc, page, p(40.0, 120.0), TOL);
    let shown = preview_ends(&editor, &doc, page);
    assert_eq!(shown.len(), 3);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    assert_eq!(doc.project().wires().count(), 0);
    assert_eq!(proposal_ends(&editor), shown);
}

#[test]
fn a_symbol_body_in_between_blocks_a_proposal() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    // A lamp off to the side whose body covers the column but whose pins
    // are not on it.
    editor.begin_place("iec/signalling/lamp", None);
    editor.click(&mut doc, page, p(48.0, 88.0), TOL);
    let lamp = symbol_at(&doc, page, p(48.0, 88.0));
    editor.begin_place("iec/contactors/main_3p", None);
    editor.hover(&doc, page, p(40.0, 140.0), TOL);
    let shown = preview_ends(&editor, &doc, page);
    let body = editor.body(doc.project().symbol(lamp).unwrap()).unwrap();
    for (a, b) in shown {
        let x = a.x;
        let crosses =
            x > body.0.x && x < body.1.x && a.y.min(b.y) < body.1.y && a.y.max(b.y) > body.0.y;
        assert!(!crosses, "a proposal runs through the lamp at x = {x}");
    }
}

#[test]
fn touching_pins_connect_on_drop() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    let f = symbol_at(&doc, page, p(40.0, 40.0));
    // Put the contactor's pin 1 exactly on the fuse's pin 2.
    let at = pin_pos(&doc, f, "2");
    editor.begin_place("iec/contactors/main_3p", None);
    editor.hover(&doc, page, p(40.0, 120.0), TOL);
    let ghost = editor.preview(&doc, page).symbols[0].clone();
    let i = ghost.pin_index("1").unwrap();
    let offset = at - ghost.pin_position(i).unwrap();
    editor.click(&mut doc, page, p(40.0 + offset.x, 120.0 + offset.y), TOL);
    let k = symbol_at(&doc, page, p(40.0 + offset.x, 120.0 + offset.y));
    assert!(connected(&doc, pin(&doc, f, "2"), pin(&doc, k, "1")));
    assert!(connected(&doc, pin(&doc, f, "6"), pin(&doc, k, "5")));
}

#[test]
fn a_pin_on_a_crossing_joins_only_the_wire_it_runs_along() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    // Two coils left and right, two coils above and below: a + of wires.
    editor.begin_place("iec/coils/relay", None);
    for (x, y) in [(0.0, 80.0), (160.0, 80.0)] {
        editor.click(&mut doc, page, p(x, y), TOL);
    }
    editor.leave_tool();
    let a = symbol_at(&doc, page, p(0.0, 80.0));
    let b = symbol_at(&doc, page, p(160.0, 80.0));
    editor.begin_wire();
    // A2 of the left coil to A2 of the right coil.
    click_pin(&mut editor, &mut doc, page, a, "A2");
    click_pin(&mut editor, &mut doc, page, b, "A2");
    editor.leave_tool();
    let horizontal = doc.project().wires().next().unwrap().id;
    let pts = edit_points(&doc, horizontal);
    // A vertical wire crossing it: between two terminals above and below.
    let cross_x = 80.0;
    let y = pts.iter().map(|p| p.y).fold(f64::NAN, f64::max);
    editor.auto_connect = false;
    editor.begin_place("iec/terminals/terminal", None);
    editor.click(&mut doc, page, p(cross_x, y - 40.0), TOL);
    editor.click(&mut doc, page, p(cross_x, y + 40.0), TOL);
    editor.leave_tool();
    let t1 = symbol_at(&doc, page, p(cross_x, y - 40.0));
    let t2 = symbol_at(&doc, page, p(cross_x, y + 40.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, t1, "2");
    click_pin(&mut editor, &mut doc, page, t2, "1");
    editor.leave_tool();
    assert_eq!(doc.project().wires().count(), 2);
    // A one-pin feed dropped right on the crossing joins one wire only.
    editor.auto_connect = true;
    let feed = doc.transact("feed", |tx| {
        let lib = novo_symbols::Library::builtin().unwrap();
        let sym = lib.get(potential::FEED_SYMBOL).unwrap().clone();
        edit::place(
            tx,
            page,
            &sym,
            novo_core::Placement::at(p(cross_x, y - 8.0)),
            &PlaceOptions::new(),
        )
    });
    assert!(feed.is_ok());
    // It split one wire (3 wires now), and the two nets stay apart.
    assert_eq!(doc.project().wires().count(), 3);
    assert!(!connected(&doc, pin(&doc, a, "A2"), pin(&doc, t1, "2")));
    assert!(!connected(&doc, pin(&doc, b, "A2"), pin(&doc, t2, "1")));
}

fn edit_points(doc: &Document, wire: novo_core::WireId) -> Vec<Point> {
    novo_editor::geometry::wire_points(doc.project(), wire).unwrap()
}

#[test]
fn k1_no_finds_k1_in_the_pages_structure_and_refuses_a_missing_parent() {
    let (mut doc, page, mut editor) = setup();
    doc.transact("structure", |tx| {
        tx.update::<novo_core::Page>(page, |p| {
            p.structure = novo_core::DeviceTag::parse("=A1+CAB1").unwrap();
        })
    })
    .unwrap();
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contacts/no", Some("-K1"));
    assert_eq!(
        editor.click(&mut doc, page, p(80.0, 40.0), TOL),
        Clicked::Edited
    );
    assert_eq!(tags(&doc), ["=A1+CAB1-K1"]);
    editor.begin_place("iec/contacts/no", Some("-K7"));
    assert_eq!(
        editor.click(&mut doc, page, p(120.0, 40.0), TOL),
        Clicked::Refused
    );
    assert!(editor.notice().unwrap().contains("-K7"));
    assert_eq!(tags(&doc), ["=A1+CAB1-K1"]);
}

#[test]
fn deleting_two_branches_of_a_t_leaves_no_dot_on_a_stub() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/coils/relay", None);
    for (x, y) in [(40.0, 40.0), (40.0, 160.0), (120.0, 100.0)] {
        editor.click(&mut doc, page, p(x, y), TOL);
    }
    editor.leave_tool();
    let top = symbol_at(&doc, page, p(40.0, 40.0));
    let bottom = symbol_at(&doc, page, p(40.0, 160.0));
    let side = symbol_at(&doc, page, p(120.0, 100.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, top, "A2");
    click_pin(&mut editor, &mut doc, page, bottom, "A1");
    // T off the middle of that wire to the side coil.
    let mid = Point::new(pin_pos(&doc, top, "A2").x, 100.0);
    editor.click(&mut doc, page, mid, TOL);
    click_pin(&mut editor, &mut doc, page, side, "A1");
    editor.leave_tool();
    assert_eq!(doc.project().junctions().count(), 1);
    let j = Endpoint::junction(doc.project().junctions().next().unwrap().id);
    let branches: Vec<AnyId> = doc
        .project()
        .wires_at(j)
        .filter(|&w| {
            let wire = doc.project().wire(w).unwrap();
            !wire.ends().contains(&pin(&doc, top, "A2"))
        })
        .map(AnyId::Wire)
        .collect();
    assert_eq!(branches.len(), 2);
    assert!(editor.delete(&mut doc, &branches));
    assert_eq!(doc.project().junctions().count(), 0);
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn turning_a_moved_coil_connects_where_it_lands_not_where_it_was() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/terminals/terminal", None);
    editor.click(&mut doc, page, p(0.0, 100.0), TOL);
    editor.click(&mut doc, page, p(200.0, 100.0), TOL);
    editor.leave_tool();
    let t1 = symbol_at(&doc, page, p(0.0, 100.0));
    let t2 = symbol_at(&doc, page, p(200.0, 100.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, t1, "2");
    click_pin(&mut editor, &mut doc, page, t2, "1");
    editor.leave_tool();
    let before = doc.project().wires().count();
    let wire = doc.project().wires().next().unwrap().id;
    // The wire's horizontal run, and a grid point in the middle of it.
    let pts = edit_points(&doc, wire);
    let (a, b) = pts
        .windows(2)
        .map(|s| (s[0], s[1]))
        .filter(|(a, b)| (a.y - b.y).abs() < 1e-6)
        .max_by(|x, y| (x.1.x - x.0.x).abs().total_cmp(&(y.1.x - y.0.x).abs()))
        .unwrap();
    let wire_y = a.y;
    let mid_x = ((a.x + b.x) / 2.0 / 4.0).round() * 4.0;
    editor.auto_connect = true;
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(100.0, 300.0), TOL);
    editor.leave_tool();
    let coil = AnyId::Symbol(symbol_at(&doc, page, p(100.0, 300.0)));
    editor.hover(&doc, page, p(100.0, 300.0), TOL);
    assert!(editor.begin_move(&[coil]));
    assert!(editor.key(&mut doc, EditorKey::Rotate, &[coil]));
    // Unturned, the coil (vertical, pins above and below its origin) at
    // this x would sit on the horizontal wire's line with its pins off it;
    // drop it with its origin on the wire line, 4 mm off the wire's x span
    // so only the turn decides.
    editor.click(&mut doc, page, p(mid_x, wire_y), TOL);
    // Turned horizontal on the wire: inserted in series, so the wire
    // was split by the turned pins.
    assert_eq!(doc.project().wires().count(), before + 1);
}

/// The review's steps: a wire W from -K1:A2 to -K2:A1, then a second wire
/// from -K1:A2 to a point on W. The junction that splits W has two wires
/// back to -K1:A2 and one on to -K2.
fn loop_back_to_one_pin() -> (Document, PageId, Editor) {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/coils/relay", None);
    for (x, y) in [(40.0, 40.0), (40.0, 160.0)] {
        editor.click(&mut doc, page, p(x, y), TOL);
    }
    editor.leave_tool();
    let k1 = symbol_at(&doc, page, p(40.0, 40.0));
    let k2 = symbol_at(&doc, page, p(40.0, 160.0));
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, k1, "A2");
    click_pin(&mut editor, &mut doc, page, k2, "A1");
    click_pin(&mut editor, &mut doc, page, k1, "A2");
    let on_w = Point::new(pin_pos(&doc, k1, "A2").x, 100.0);
    assert_eq!(editor.click(&mut doc, page, on_w, TOL), Clicked::Edited);
    editor.leave_tool();
    assert_eq!(doc.project().junctions().count(), 1);
    assert_eq!(doc.project().wires().count(), 3);
    (doc, page, editor)
}

#[test]
fn deleting_the_far_symbol_of_a_loop_back_finishes_and_leaves_no_stub() {
    let (mut doc, page, mut editor) = loop_back_to_one_pin();
    let k1 = symbol_at(&doc, page, p(40.0, 40.0));
    let k2 = symbol_at(&doc, page, p(40.0, 160.0));
    assert!(editor.delete(&mut doc, &[AnyId::Symbol(k2)]));
    // The two wires left both run from -K1:A2 to a dot that reaches nothing
    // else: a dead loop, so it goes with its dot. -K1 stays.
    assert_eq!(doc.project().junctions().count(), 0);
    assert_eq!(doc.project().wires().count(), 0);
    assert!(doc.project().symbol(k1).is_some());
    assert_eq!(tags(&doc), ["-K1"]);
    // One undo step brings it all back.
    doc.undo().unwrap();
    assert_eq!(doc.project().junctions().count(), 1);
    assert_eq!(doc.project().wires().count(), 3);
}

#[test]
fn deleting_the_wire_on_to_the_far_symbol_of_a_loop_back_finishes() {
    let (mut doc, page, mut editor) = loop_back_to_one_pin();
    let k2 = symbol_at(&doc, page, p(40.0, 160.0));
    let w2 = doc.project().wires_at(pin(&doc, k2, "A1")).next().unwrap();
    assert!(editor.delete(&mut doc, &[AnyId::Wire(w2)]));
    assert_eq!(doc.project().junctions().count(), 0);
    assert_eq!(doc.project().wires().count(), 0);
    assert_eq!(tags(&doc), ["-K1", "-K2"]);
}

#[test]
fn deleting_tidies_only_the_junctions_it_touches() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/coils/relay", None);
    for (x, y) in [(40.0, 40.0), (200.0, 40.0)] {
        editor.click(&mut doc, page, p(x, y), TOL);
    }
    editor.leave_tool();
    let k1 = symbol_at(&doc, page, p(40.0, 40.0));
    let k2 = symbol_at(&doc, page, p(200.0, 40.0));
    // A wire from -K1:A2 to a lone dot, made by a script.
    let end = Point::new(pin_pos(&doc, k1, "A2").x, 100.0);
    let a2 = pin(&doc, k1, "A2");
    doc.transact("Stub", |tx| {
        let j = tx.add_junction(page, end)?;
        tx.connect(a2, Endpoint::junction(j))
    })
    .unwrap();
    assert!(editor.delete(&mut doc, &[AnyId::Symbol(k2)]));
    assert_eq!(doc.project().junctions().count(), 1);
    assert_eq!(doc.project().wires().count(), 1);
}

#[test]
fn turning_or_moving_the_selection_drops_the_proposals() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    assert_eq!(editor.proposals().len(), 3);
    let k = AnyId::Symbol(symbol_at(&doc, page, p(40.0, 120.0)));
    assert!(editor.rotate_selection(&mut doc, &[k]));
    assert!(editor.proposals().is_empty());
    assert!(!editor.accept_proposal(&mut doc));
    assert_eq!(doc.project().wires().count(), 0);
    // Leaving the tool drops them too.
    doc.undo().unwrap();
    editor.click(&mut doc, page, p(120.0, 120.0), TOL);
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(120.0, 40.0), TOL);
    assert!(!editor.proposals().is_empty());
    editor.leave_tool();
    assert!(editor.proposals().is_empty());
}

#[test]
fn a_proposal_whose_pin_moved_is_dropped_on_enter() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    assert_eq!(editor.proposals().len(), 3);
    // Something other than the editor (a script) moves the contactor.
    let k = symbol_at(&doc, page, p(40.0, 120.0));
    doc.transact("Script move", |tx| {
        edit::transform(tx, &[AnyId::Symbol(k)], edit::Transform::Move(p(8.0, 0.0)))
    })
    .unwrap();
    let depth = doc.undo_depth();
    // Enter draws nothing the preview did not show; the proposal goes.
    assert!(!editor.accept_proposal(&mut doc));
    assert_eq!(editor.proposals().len(), 2);
    assert_eq!(doc.project().wires().count(), 0);
    assert_eq!(doc.undo_depth(), depth);
}

#[test]
fn a_declined_proposal_does_not_return() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/protection/fuse_3p", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contactors/main_3p", None);
    editor.click(&mut doc, page, p(40.0, 120.0), TOL);
    assert_eq!(editor.proposals().len(), 3);
    let declined = editor.proposals()[editor.highlighted().unwrap()];
    let depth = doc.undo_depth();
    assert!(editor.decline_proposal(&mut doc));
    // One undo step, no wire, the other two still proposed.
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.project().wires().count(), 0);
    assert_eq!(editor.proposals().len(), 2);
    // Proposing the same symbols again (after a move, say) leaves it out,
    // from either side.
    let k = symbol_at(&doc, page, p(40.0, 120.0));
    let f = symbol_at(&doc, page, p(40.0, 40.0));
    for s in [k, f] {
        let sym = doc.project().symbol(s).unwrap().clone();
        let again = edit::facing_wires(doc.project(), &[sym], &|s| editor.body(s));
        assert_eq!(again.len(), 2);
        assert!(again.iter().all(|w| {
            let pair = [w.from, w.to];
            !(pair.contains(&declined.from) && pair.contains(&declined.to))
        }));
    }
    // Undo forgets the decline.
    doc.undo().unwrap();
    let sym = doc.project().symbol(k).unwrap().clone();
    assert_eq!(
        edit::facing_wires(doc.project(), &[sym], &|s| editor.body(s)).len(),
        3
    );
}

#[test]
fn renaming_a_page_refuses_a_dot_without_an_undo_step() {
    let (mut doc, page, mut editor) = setup();
    assert!(!editor.rename_page(&mut doc, page, "1.1"));
    assert_eq!(
        editor.notice(),
        Some("a dot is reserved for page.column references")
    );
    assert_eq!(doc.undo_label(), Some("Add page"));
    assert!(editor.rename_page(&mut doc, page, "1a"));
    assert_eq!(editor.notice(), None);
    assert_eq!(doc.project().page(page).unwrap().name, "1a");
    assert_eq!(doc.undo_label(), Some("Rename page"));
}

/// Two contacts above, two below, so pin-to-pin wires route as Z shapes with
/// their cross-over halfway: `top` pins 14 at x = 40 and 80, `low` pins 13 at
/// x = 60 and 40.
fn two_rows() -> (Document, PageId, [novo_core::SymbolId; 4]) {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    for at in [p(40.0, 40.0), p(80.0, 40.0), p(60.0, 120.0), p(40.0, 120.0)] {
        editor.click(&mut doc, page, at, TOL);
    }
    let s = |x: f64, y: f64| symbol_at(&doc, page, p(x, y));
    let syms = [s(40.0, 40.0), s(80.0, 40.0), s(60.0, 120.0), s(40.0, 120.0)];
    (doc, page, syms)
}

fn target(
    project: &novo_core::Project,
    sym: novo_core::SymbolId,
    name: &str,
) -> novo_editor::geometry::Target {
    let s = project.symbol(sym).unwrap();
    let end = Endpoint::pin(sym, s.pin_index(name).unwrap());
    novo_editor::geometry::Target::Endpoint(end, project.endpoint_position(end).unwrap())
}

fn page_has_no_overlaps(doc: &Document, page: PageId) -> bool {
    novo_editor::overlap::page_overlaps(doc.project(), page).is_empty()
}

#[test]
fn a_router_leg_that_would_overlap_is_moved_a_step() {
    let (mut doc, page, [a, b, c, d]) = two_rows();
    doc.transact("w1", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), a, "14"),
            &[],
            target(tx.project(), c, "13"),
        )
    })
    .unwrap();
    doc.transact("w2", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), b, "14"),
            &[],
            target(tx.project(), d, "13"),
        )
    })
    .unwrap();
    assert_eq!(doc.project().wires().count(), 2);
    // The second wire's cross-over left the halfway line the first one uses.
    let mid = (pin_pos(&doc, a, "14").y + pin_pos(&doc, c, "13").y) / 2.0;
    let second = doc.project().wires().last().unwrap().id;
    let pts = novo_editor::geometry::wire_points(doc.project(), second).unwrap();
    assert!(
        pts.iter()
            .skip(1)
            .take(pts.len() - 2)
            .all(|q| (q.y - mid).abs() > 1.0)
    );
    assert!(page_has_no_overlaps(&doc, page));
}

#[test]
fn a_clicked_corner_that_would_overlap_is_refused_and_nothing_changes() {
    let (mut doc, page, [a, b, c, d]) = two_rows();
    let mid = (pin_pos(&doc, a, "14").y + pin_pos(&doc, c, "13").y) / 2.0;
    doc.transact("w1", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), a, "14"),
            &[],
            target(tx.project(), c, "13"),
        )
    })
    .unwrap();
    let depth = doc.undo_depth();
    let result = doc.transact("w2", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), b, "14"),
            &[p(50.0, mid)],
            target(tx.project(), d, "13"),
        )
    });
    assert!(result.is_err());
    assert_eq!(doc.undo_depth(), depth);
    assert_eq!(doc.project().wires().count(), 1);
}

#[test]
fn crossing_wires_are_not_an_overlap() {
    let (mut doc, page, [a, b, c, d]) = two_rows();
    // a to d and b to c cross between the rows.
    doc.transact("w1", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), a, "14"),
            &[],
            target(tx.project(), d, "13"),
        )
    })
    .unwrap();
    doc.transact("w2", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), b, "14"),
            &[],
            target(tx.project(), c, "13"),
        )
    })
    .unwrap();
    assert_eq!(doc.project().wires().count(), 2);
    assert!(page_has_no_overlaps(&doc, page));
}

#[test]
fn wires_from_one_pin_may_share_only_the_stretch_before_they_part() {
    let (mut doc, page, [a, _b, c, d]) = two_rows();
    let mid = (pin_pos(&doc, a, "14").y + pin_pos(&doc, c, "13").y) / 2.0;
    // A leaves a.14, goes down, left, down to the middle line and across it.
    doc.transact("A", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), a, "14"),
            &[p(40.0, 70.0), p(30.0, 70.0), p(30.0, mid), p(100.0, mid)],
            target(tx.project(), d, "13"),
        )
    })
    .unwrap();
    // B starts at the same pin and its default Z route would run along A's
    // horizontal at `mid` after the two have parted: it must be moved clear.
    doc.transact("B", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), a, "14"),
            &[],
            target(tx.project(), c, "13"),
        )
    })
    .unwrap();
    let overlaps = novo_editor::overlap::page_overlaps(doc.project(), page);
    let pin = pin_pos(&doc, a, "14");
    // Only the stretch that starts at the shared pin may be shared.
    assert!(
        overlaps
            .iter()
            .all(|(_, o)| o.from.approx_eq(pin, 1e-6) || o.to.approx_eq(pin, 1e-6)),
        "{overlaps:?}"
    );
}

// Wiring modes (DESIGN.md §4): Corners and Lines.

/// Three contacts: a and b side by side, c lower right. Auto-connect off.
fn three_contacts() -> (Document, PageId, Editor, [novo_core::SymbolId; 3]) {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 40.0), TOL);
    editor.click(&mut doc, page, p(120.0, 80.0), TOL);
    editor.leave_tool();
    let a = symbol_at(&doc, page, p(40.0, 40.0));
    let b = symbol_at(&doc, page, p(80.0, 40.0));
    let c = symbol_at(&doc, page, p(120.0, 80.0));
    (doc, page, editor, [a, b, c])
}

fn only_wire_route(doc: &Document) -> Vec<Point> {
    doc.project().wires().next().unwrap().route.clone()
}

#[test]
fn corners_is_the_default_and_the_toggle_keeps_the_points() {
    let (mut doc, page, mut editor, [a, b, _]) = three_contacts();
    assert_eq!(editor.wire_mode(), WiringMode::Corners);
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    // A corner in Corners, then the mode changes mid-wire.
    editor.click(&mut doc, page, p(40.0, 72.0), TOL);
    editor.key(&mut doc, EditorKey::WireMode, &[]);
    assert_eq!(editor.wire_mode(), WiringMode::Lines);
    let Tool::Wire(w) = editor.tool() else {
        panic!("still wiring")
    };
    assert_eq!(w.corners, vec![p(40.0, 72.0)]);
    // The next click follows Lines: locked to the axis, so 80,72.
    assert_eq!(
        editor.click(&mut doc, page, p(80.0, 73.0), TOL),
        Clicked::Step
    );
    assert_eq!(
        click_pin(&mut editor, &mut doc, page, b, "14"),
        Clicked::Edited
    );
    assert_eq!(only_wire_route(&doc), vec![p(40.0, 72.0), p(80.0, 72.0)]);
    // Switching back changes no wire.
    editor.toggle_wire_mode();
    assert_eq!(only_wire_route(&doc), vec![p(40.0, 72.0), p(80.0, 72.0)]);
}

#[test]
fn lines_stores_what_was_drawn_and_corners_lets_the_router_join_the_points() {
    for mode in [WiringMode::Corners, WiringMode::Lines] {
        let (mut doc, page, mut editor, [a, _, c]) = three_contacts();
        editor.set_wire_mode(mode);
        editor.begin_wire();
        click_pin(&mut editor, &mut doc, page, a, "14");
        assert_eq!(
            click_pin(&mut editor, &mut doc, page, c, "13"),
            Clicked::Edited
        );
        let route = only_wire_route(&doc);
        let (from, to) = (pin_pos(&doc, a, "14"), pin_pos(&doc, c, "13"));
        match mode {
            // Down, across, down: the router's Z.
            WiringMode::Corners => assert_eq!(route.len(), 2),
            // Along the longer axis first, then one right angle onto the pin.
            WiringMode::Lines => assert_eq!(route, vec![p(to.x, from.y)]),
        }
        // Either way the model is the same kind of wire: connected, one step.
        assert!(connected(&doc, pin(&doc, a, "14"), pin(&doc, c, "13")));
        doc.undo().unwrap();
        assert_eq!(doc.project().wires().count(), 0);
    }
}

#[test]
fn a_line_over_another_wire_is_refused_and_changes_nothing() {
    let (mut doc, page, mut editor, [a, b, _]) = three_contacts();
    editor.set_wire_mode(WiringMode::Lines);
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    editor.click(&mut doc, page, p(40.0, 72.0), TOL);
    editor.click(&mut doc, page, p(80.0, 72.0), TOL);
    click_pin(&mut editor, &mut doc, page, b, "14");
    assert_eq!(doc.project().wires().count(), 1);
    // Start on the wire, then draw along it.
    editor.click(&mut doc, page, p(60.0, 72.0), TOL);
    let before = doc.undo_label().map(str::to_owned);
    assert_eq!(
        editor.click(&mut doc, page, p(100.0, 75.0), TOL),
        Clicked::Refused
    );
    assert_eq!(editor.notice(), Some("Overlaps another wire"));
    assert_eq!(doc.undo_label().map(str::to_owned), before);
    assert_eq!(doc.project().wires().count(), 1);
    // The preview keeps the wire up to the last legal point and marks the
    // clash.
    editor.hover(&doc, page, p(100.0, 75.0), TOL);
    let preview = editor.preview(&doc, page);
    assert!(!preview.blocked.is_empty());
    assert_eq!(preview.wire, vec![p(60.0, 72.0)]);
    assert!(
        editor
            .status(&doc, page)
            .starts_with("Overlaps another wire")
    );
    // Crossing it is fine: a line from above straight down through the wire.
    editor.cancel_step();
    editor.click(&mut doc, page, p(60.0, 72.0), TOL);
    assert_eq!(
        editor.click(&mut doc, page, p(60.0, 90.0), TOL),
        Clicked::Step
    );
}

#[test]
fn backspace_takes_back_the_last_line_then_the_start() {
    let (mut doc, page, mut editor, [a, _, _]) = three_contacts();
    editor.set_wire_mode(WiringMode::Lines);
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    editor.click(&mut doc, page, p(40.0, 72.0), TOL);
    editor.click(&mut doc, page, p(80.0, 72.0), TOL);
    let corners = |e: &Editor| match e.tool() {
        Tool::Wire(w) => (w.start.is_some(), w.corners.len()),
        _ => panic!("not wiring"),
    };
    assert_eq!(corners(&editor), (true, 2));
    assert!(editor.key(&mut doc, EditorKey::Back, &[]));
    assert_eq!(corners(&editor), (true, 1));
    assert!(editor.key(&mut doc, EditorKey::Back, &[]));
    assert!(editor.key(&mut doc, EditorKey::Back, &[]));
    assert_eq!(corners(&editor), (false, 0));
    // Nothing to take back now; the model was never touched.
    assert!(!editor.key(&mut doc, EditorKey::Back, &[]));
    assert_eq!(doc.project().wires().count(), 0);
}

#[test]
fn r_flips_the_elbow_in_corners_and_does_nothing_in_lines() {
    let (mut doc, page, mut editor, [a, _, _]) = three_contacts();
    editor.begin_wire();
    let from = click_pin(&mut editor, &mut doc, page, a, "14");
    assert_eq!(from, Clicked::Step);
    let start = pin_pos(&doc, a, "14");
    // Without R the elbow runs along the longer move first (horizontal).
    // On the 2.5 mm grid, so the click is not snapped.
    let to = p(start.x + 60.0, start.y + 25.0 - 1.0);
    editor.key(&mut doc, EditorKey::Rotate, &[]);
    editor.click(&mut doc, page, to, TOL);
    let Tool::Wire(w) = editor.tool() else {
        panic!("still wiring")
    };
    // Flipped: down first, then across, then the clicked corner.
    assert_eq!(w.corners, vec![p(start.x, to.y), to]);
    assert!(!w.flip);
    // In Lines, R is consumed and flips nothing.
    editor.set_wire_mode(WiringMode::Lines);
    assert!(editor.key(&mut doc, EditorKey::Rotate, &[]));
    let Tool::Wire(w) = editor.tool() else {
        panic!("still wiring")
    };
    assert!(!w.flip);
}

#[test]
fn enter_finishes_on_a_last_point_that_lies_on_a_pin() {
    let (mut doc, page, mut editor, [a, _, c]) = three_contacts();
    editor.set_wire_mode(WiringMode::Lines);
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    let (from, to) = (pin_pos(&doc, a, "14"), pin_pos(&doc, c, "13"));
    // Along y to above the third contact, then down with the pointer well
    // to the side of the pin: the vertical lock lands exactly on it.
    editor.click(&mut doc, page, p(to.x, from.y + 1.0), TOL);
    editor.click(&mut doc, page, p(to.x + 5.0, to.y), TOL);
    assert_eq!(doc.project().wires().count(), 0);
    assert!(editor.key(&mut doc, EditorKey::Finish, &[]));
    assert_eq!(doc.project().wires().count(), 1);
    assert!(connected(&doc, pin(&doc, a, "14"), pin(&doc, c, "13")));
    assert_eq!(only_wire_route(&doc), vec![p(to.x, from.y)]);
    // And Enter with nothing under the last point explains itself.
    click_pin(&mut editor, &mut doc, page, a, "14");
    editor.click(&mut doc, page, p(from.x, from.y + 15.0), TOL);
    assert!(editor.key(&mut doc, EditorKey::Finish, &[]));
    assert_eq!(
        editor.notice(),
        Some("The last point is not on a pin or a wire")
    );
}

#[test]
fn a_refusal_comes_back_as_data_naming_the_other_wire_and_the_stretch() {
    let (mut doc, page, [a, b, c, d]) = two_rows();
    let mid = (pin_pos(&doc, a, "14").y + pin_pos(&doc, c, "13").y) / 2.0;
    let (w1, _) = doc
        .transact("w1", |tx| {
            edit::draw_wire(
                tx,
                page,
                target(tx.project(), a, "14"),
                &[],
                target(tx.project(), c, "13"),
            )
        })
        .unwrap();
    let refusal = edit::check_draw(
        doc.project(),
        page,
        target(doc.project(), b, "14"),
        &[p(50.0, mid)],
        target(doc.project(), d, "13"),
    )
    .unwrap_err();
    assert_eq!(refusal.other, w1);
    assert!((refusal.from.y - mid).abs() < 1e-6 && (refusal.to.y - mid).abs() < 1e-6);
    assert!(refusal.from.distance(refusal.to) > 1.0);
    // Without the clicked corner the router can clear it, so no refusal.
    assert!(
        edit::check_draw(
            doc.project(),
            page,
            target(doc.project(), b, "14"),
            &[],
            target(doc.project(), d, "13"),
        )
        .is_ok()
    );
}

/// Two straight vertical wires, one at x = 40 and one at x = 80 that a move
/// of its two contacts by -40 would lay over the first.
fn two_columns() -> (Document, PageId, Vec<AnyId>) {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/contacts/no", None);
    for at in [p(40.0, 40.0), p(40.0, 120.0), p(80.0, 40.0), p(80.0, 120.0)] {
        editor.click(&mut doc, page, at, TOL);
    }
    let s = |x: f64, y: f64| symbol_at(&doc, page, p(x, y));
    let (a, e, g, f) = (s(40.0, 40.0), s(40.0, 120.0), s(80.0, 40.0), s(80.0, 120.0));
    for (top, low) in [(a, e), (g, f)] {
        doc.transact("wire", |tx| {
            edit::draw_wire(
                tx,
                page,
                target(tx.project(), top, "14"),
                &[],
                target(tx.project(), low, "13"),
            )
        })
        .unwrap();
    }
    (doc, page, vec![g.into(), f.into()])
}

#[test]
fn an_interactive_move_onto_another_wire_is_refused_whole() {
    let (mut doc, page, moving) = two_columns();
    let depth = doc.undo_depth();
    let result = doc.transact("Move", |tx| {
        edit::transform_interactive(tx, &moving, edit::Transform::Move(p(-40.0, 0.0)))
    });
    assert!(result.is_err());
    assert_eq!(doc.undo_depth(), depth);
    assert!(page_has_no_overlaps(&doc, page));
}

#[test]
fn a_move_that_cannot_ask_is_accepted_and_the_overlap_is_reportable() {
    let (mut doc, page, moving) = two_columns();
    doc.transact("Move", |tx| {
        edit::transform(tx, &moving, edit::Transform::Move(p(-40.0, 0.0)))
    })
    .unwrap();
    assert!(!page_has_no_overlaps(&doc, page));
}

#[test]
fn moving_one_end_reroutes_clear_or_is_refused_when_its_pin_lies_on_another_wire() {
    // The lower contact moves away: the re-routed wire is clear.
    let (mut doc, page, moving) = two_columns();
    let lower = vec![moving[1]];
    doc.transact("Move", |tx| {
        edit::transform_interactive(tx, &lower, edit::Transform::Move(p(40.0, 0.0)))
    })
    .unwrap();
    assert!(page_has_no_overlaps(&doc, page));
    // Moved onto the other wire's line, its last stretch runs along that wire
    // whatever the cross-over does, so an interactive move is refused.
    let (mut doc, _, moving) = two_columns();
    let lower = vec![moving[1]];
    let depth = doc.undo_depth();
    let result = doc.transact("Move", |tx| {
        edit::transform_interactive(tx, &lower, edit::Transform::Move(p(-40.0, 0.0)))
    });
    assert!(result.is_err());
    assert_eq!(doc.undo_depth(), depth);
}

#[test]
fn a_refused_final_click_keeps_the_wire_in_progress() {
    let (mut doc, page, mut editor, [a, b, c]) = three_contacts();
    editor.set_wire_mode(WiringMode::Lines);
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    editor.click(&mut doc, page, p(40.0, 72.0), TOL);
    editor.click(&mut doc, page, p(80.0, 72.0), TOL);
    click_pin(&mut editor, &mut doc, page, b, "14");
    assert_eq!(doc.project().wires().count(), 1);
    // A second wire from the third contact: down the side, then onto the
    // first wire's horizontal run, which it would then lie along.
    let top = pin_pos(&doc, c, "13");
    click_pin(&mut editor, &mut doc, page, c, "13");
    editor.click(&mut doc, page, p(top.x, 72.0), TOL);
    let depth = doc.undo_depth();
    assert_eq!(
        editor.click(&mut doc, page, p(60.0, 72.0), TOL),
        Clicked::Refused
    );
    assert_eq!(editor.notice(), Some("Overlaps another wire"));
    // Nothing was stored and the wire in hand still has its start and point.
    assert_eq!(doc.undo_depth(), depth);
    let Tool::Wire(w) = editor.tool() else {
        panic!("still wiring")
    };
    assert!(w.start.is_some());
    assert_eq!(w.corners, vec![p(top.x, 72.0)]);
    // Back up one point and finish somewhere clear.
    editor.key(&mut doc, EditorKey::Back, &[]);
    assert_eq!(
        editor.click(&mut doc, page, p(60.0, 90.0), TOL),
        Clicked::Step
    );
}

#[test]
fn a_line_cannot_run_back_over_the_wire_being_drawn() {
    let (mut doc, page, mut editor, [a, _, _]) = three_contacts();
    editor.set_wire_mode(WiringMode::Lines);
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    editor.click(&mut doc, page, p(40.0, 72.0), TOL);
    editor.click(&mut doc, page, p(80.0, 72.0), TOL);
    // Back along the leg just drawn.
    assert_eq!(
        editor.click(&mut doc, page, p(60.0, 72.0), TOL),
        Clicked::Refused
    );
    assert_eq!(
        editor.notice(),
        Some("A line cannot run back over the wire")
    );
    let Tool::Wire(w) = editor.tool() else {
        panic!("still wiring")
    };
    assert_eq!(w.corners.len(), 2);
    // The preview marks it and keeps the wire up to the last legal point.
    editor.hover(&doc, page, p(60.0, 72.0), TOL);
    let preview = editor.preview(&doc, page);
    assert!(!preview.blocked.is_empty());
    assert_eq!(preview.wire.last(), Some(&p(80.0, 72.0)));
}

#[test]
fn a_mode_switch_from_a_diagonal_corners_point_still_ends_orthogonal_on_a_pin() {
    let (mut doc, page, mut editor, [a, b, _]) = three_contacts();
    editor.begin_wire();
    click_pin(&mut editor, &mut doc, page, a, "14");
    // A free Corners point off both axes of the pin, then Lines.
    editor.click(&mut doc, page, p(60.0, 80.0), TOL);
    editor.key(&mut doc, EditorKey::WireMode, &[]);
    assert_eq!(
        click_pin(&mut editor, &mut doc, page, b, "14"),
        Clicked::Edited
    );
    let wire = doc.project().wires().next().unwrap();
    let mut points = vec![pin_pos(&doc, a, "14")];
    points.extend(wire.route.iter().copied());
    points.push(pin_pos(&doc, b, "14"));
    assert!(
        points
            .windows(2)
            .all(|s| { (s[0].x - s[1].x).abs() < 1e-9 || (s[0].y - s[1].y).abs() < 1e-9 })
    );
    assert!(points.contains(&p(60.0, 80.0)));
}

#[test]
fn dropping_a_moved_selection_onto_another_wire_is_refused_in_the_editor() {
    let (mut doc, page, moving) = two_columns();
    let mut editor = Editor::default();
    editor.hover(&doc, page, p(80.0, 40.0), TOL);
    assert!(editor.begin_move(&moving));
    let depth = doc.undo_depth();
    assert_eq!(
        editor.click(&mut doc, page, p(40.0, 40.0), TOL),
        Clicked::Refused
    );
    assert!(editor.notice().is_some());
    assert_eq!(doc.undo_depth(), depth);
    assert!(page_has_no_overlaps(&doc, page));
}

#[test]
fn connecting_a_pin_onto_the_side_of_another_wire_is_allowed() {
    let (mut doc, page, [a, b, c, _d]) = two_rows();
    let w1 = doc
        .transact("w1", |tx| {
            edit::draw_wire(
                tx,
                page,
                target(tx.project(), a, "14"),
                &[],
                target(tx.project(), c, "13"),
            )
        })
        .unwrap()
        .0;
    let pts = novo_editor::geometry::wire_points(doc.project(), w1).unwrap();
    // The middle of the first vertical stretch of the first wire.
    let (p0, p1) = (pts[0], pts[1]);
    let at = p((p0.x + p1.x) / 2.0, (p0.y + p1.y) / 2.0);
    let start = target(doc.project(), b, "14");
    let end = novo_editor::geometry::Target::Wire(w1, at);
    let checked = edit::check_draw(doc.project(), page, start, &[], end);
    assert!(checked.is_ok(), "{checked:?} pts {pts:?} at {at:?}");
    doc.transact("w2", |tx| edit::draw_wire(tx, page, start, &[], end))
        .unwrap();
    assert!(page_has_no_overlaps(&doc, page));
}

#[test]
fn a_pin_beside_a_horizontal_stretch_ends_on_its_side() {
    let (mut doc, page, [a, b, c, _d]) = two_rows();
    let w1 = doc
        .transact("w1", |tx| {
            edit::draw_wire(
                tx,
                page,
                target(tx.project(), a, "14"),
                &[],
                target(tx.project(), c, "13"),
            )
        })
        .unwrap()
        .0;
    // The horizontal stretch of w1 is y = 88 from x = 40 to 60; b is to its
    // right, so a route arriving along it would share its length.
    let end = novo_editor::geometry::Target::Wire(w1, p(50.0, 88.0));
    let start = target(doc.project(), b, "14");
    assert!(edit::check_draw(doc.project(), page, start, &[], end).is_ok());
    doc.transact("w2", |tx| edit::draw_wire(tx, page, start, &[], end))
        .unwrap();
    assert!(page_has_no_overlaps(&doc, page));
}

#[test]
fn a_refusal_names_the_other_wire_and_says_why() {
    let (mut doc, page, [a, _b, c, d]) = two_rows();
    doc.transact("w1", |tx| {
        edit::draw_wire(
            tx,
            page,
            target(tx.project(), a, "14"),
            &[],
            target(tx.project(), c, "13"),
        )
    })
    .unwrap();
    // A clicked corner on the first wire's vertical run makes the new wire
    // run along it.
    let err = doc
        .transact("w2", |tx| {
            edit::draw_wire(
                tx,
                page,
                target(tx.project(), d, "13"),
                &[p(40.0, 72.0)],
                target(tx.project(), c, "14"),
            )
        })
        .unwrap_err()
        .to_string();
    assert!(err.contains("would run along the wire from"), "{err}");
    assert!(err.contains("not share its length"), "{err}");
    assert!(!err.contains("out of range"), "{err}");
}

#[test]
fn renaming_a_contact_to_its_coils_tag_links_it_and_two_coils_are_refused() {
    let (mut doc, page, mut editor) = setup();
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.click(&mut doc, page, p(80.0, 40.0), TOL);
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(120.0, 40.0), TOL);
    editor.leave_tool();
    assert_eq!(tags(&doc).len(), 3);
    let contact = symbol_at(&doc, page, p(120.0, 40.0));
    let own = doc.project().symbol(contact).unwrap().device().unwrap();
    let k1 = doc.project().devices_by_tag(&"-K1".parse().unwrap())[0];
    let k2 = doc.project().devices_by_tag(&"-K2".parse().unwrap())[0];

    // Two coils with one tag stay refused, in plain words.
    let reason = editor.rename(&mut doc, k2, "-K1").unwrap_err();
    assert!(reason.contains("already used") && reason.contains("coil"));
    assert_eq!(tags(&doc).len(), 3);

    // The contact takes its coil's device: one undo step, no confirm.
    assert!(editor.check_rename(&doc, own, "-K1").is_ok());
    let depth = doc.undo_depth();
    editor.rename(&mut doc, own, "-K1").unwrap();
    assert_eq!(doc.undo_depth(), depth + 1);
    assert_eq!(doc.project().symbol(contact).unwrap().device(), Some(k1));
    assert_eq!(tags(&doc), ["-K1", "-K2"]);
    assert_eq!(editor.notice(), Some("Linked to -K1"));
    doc.undo().unwrap();
    assert_eq!(tags(&doc).len(), 3);
}

#[test]
fn a_click_on_a_pin_starts_a_wire_with_no_tool_switch() {
    let (mut doc, page, mut editor, [a, b, _]) = three_contacts();
    assert!(!editor.tool_active());
    let pa = pin_pos(&doc, a, "14");
    let pb = pin_pos(&doc, b, "14");
    assert_eq!(editor.click(&mut doc, page, pa, TOL), Clicked::Step);
    let Tool::Wire(w) = editor.tool() else {
        panic!("wiring")
    };
    assert!(w.start.is_some() && !w.drag);
    // The next click on a pin finishes it.
    assert_eq!(editor.click(&mut doc, page, pb, TOL), Clicked::Edited);
    assert_eq!(doc.project().wires().count(), 1);
    // A click on empty canvas starts nothing.
    editor.leave_tool();
    assert_eq!(
        editor.click(&mut doc, page, p(300.0, 300.0), TOL),
        Clicked::Unhandled
    );
}

#[test]
fn a_double_click_on_nothing_ends_the_wire_as_an_open_end_in_both_modes() {
    for lines in [false, true] {
        let (mut doc, page, mut editor, [a, _, _]) = three_contacts();
        if lines {
            editor.toggle_wire_mode();
        }
        editor.begin_wire();
        let pa = pin_pos(&doc, a, "14");
        click_pin(&mut editor, &mut doc, page, a, "14");
        // The first click of the double click is the last corner.
        let end = p(pa.x, pa.y + 40.0);
        editor.click(&mut doc, page, end, TOL);
        assert!(editor.key(&mut doc, EditorKey::FinishOpen, &[]));
        let wire = doc.project().wires().next().expect("a wire is stored");
        assert!(
            matches!(wire.to, Endpoint::Free { at } if at == end),
            "{:?}",
            wire.to
        );
        assert!(wire.route.is_empty(), "no extra corner: {:?}", wire.route);
        assert!(
            !editor.tool_active() || matches!(editor.tool(), Tool::Wire(w) if w.start.is_none())
        );
    }
}

#[test]
fn a_double_click_on_empty_canvas_starts_a_wire_open_at_that_end() {
    for lines in [false, true] {
        let (mut doc, page, mut editor, [a, _, _]) = three_contacts();
        if lines {
            editor.toggle_wire_mode();
        }
        editor.begin_wire();
        let at = p(200.0, 200.0);
        // The first click of the double click starts nothing.
        editor.click(&mut doc, page, at, TOL);
        assert!(matches!(editor.tool(), Tool::Wire(w) if w.start.is_none()));
        assert!(editor.key(&mut doc, EditorKey::StartFree, &[]));
        let Tool::Wire(w) = editor.tool() else {
            panic!("wiring")
        };
        assert!(matches!(
            w.start,
            Some(Target::Endpoint(Endpoint::Free { .. }, _))
        ));
        // It goes on like any wire and ends on a pin.
        assert_eq!(
            click_pin(&mut editor, &mut doc, page, a, "14"),
            Clicked::Edited
        );
        let wire = doc.project().wires().next().expect("a wire is stored");
        assert!(
            matches!(wire.from, Endpoint::Free { at: f } if f == at),
            "{:?}",
            wire.from
        );
    }
}

#[test]
fn carrying_a_contact_over_a_coil_says_it_will_link() {
    let (mut doc, page, mut editor) = setup();
    editor.auto_connect = false;
    editor.begin_place("iec/coils/relay", None);
    editor.click(&mut doc, page, p(40.0, 40.0), TOL);
    editor.begin_place("iec/contacts/no", None);
    editor.click(&mut doc, page, p(120.0, 40.0), TOL);
    editor.leave_tool();
    let coil = symbol_at(&doc, page, p(40.0, 40.0));
    let contact = symbol_at(&doc, page, p(120.0, 40.0));
    editor.hover(&doc, page, p(120.0, 40.0), TOL);
    assert!(editor.begin_move(&[contact.into()]));
    let before = editor.status(&doc, page);
    editor.hint_link(doc.project(), &[contact.into()], Some(coil.into()));
    let status = editor.status(&doc, page);
    assert!(status.starts_with("Link to "), "{status}");
    assert_ne!(status, before);
    // Over anything else the ordinary hint returns.
    editor.hint_link(doc.project(), &[contact.into()], Some(contact.into()));
    assert_eq!(editor.status(&doc, page), before);
}
