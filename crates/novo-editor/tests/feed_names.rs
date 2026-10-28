//! A new potential feed takes the potential of what it is attached to and
//! never joins an unrelated conductor by its default name (cooling tower
//! report, finding 2). Driven through the editor, so the feed is placed,
//! wired by the facing-pin rule and renamed the way a person would see it.

use novo_core::{
    Document, Endpoint, PageId, Placement, Point, PotentialKind, Rotation, SymbolId, SymbolRole,
};
use novo_editor::Editor;
use novo_editor::EditorKey;
use novo_editor::edit::{self, PlaceOptions};
use novo_editor::potential::{FEED_PE_SYMBOL, FEED_SYMBOL};
use novo_editor::tool::ConnectMode;

const TOL: f64 = 1.5;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn setup() -> (Document, PageId, Editor) {
    let mut doc = Document::default();
    let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
    let mut editor = Editor::default();
    editor.set_connect_mode(ConnectMode::Draw);
    (doc, page, editor)
}

fn place(doc: &mut Document, page: PageId, key: &str, at: Point, turned: bool) -> SymbolId {
    let lib = novo_symbols::Library::builtin().unwrap();
    let sym = lib.get(key).unwrap_or_else(|| panic!("{key}")).clone();
    let placement = Placement {
        origin: at,
        rotation: if turned { Rotation::R180 } else { Rotation::R0 },
        mirrored: false,
    };
    doc.transact("place", |tx| {
        edit::place(tx, page, &sym, placement, &PlaceOptions::new())
    })
    .unwrap()
    .0
}

fn pin_pos(doc: &Document, sym: SymbolId, name: &str) -> Point {
    let s = doc.project().symbol(sym).unwrap();
    doc.project()
        .endpoint_position(Endpoint::pin(sym, s.pin_index(name).unwrap()))
        .unwrap()
}

/// Places an explicit-potential feed (the way "Declare source" does).
fn declared_feed(
    doc: &mut Document,
    page: PageId,
    at: Point,
    name: &str,
    kind: PotentialKind,
    volts: Option<f64>,
) -> SymbolId {
    let lib = novo_symbols::Library::builtin().unwrap();
    let sym = lib.get(FEED_SYMBOL).unwrap().clone();
    doc.transact("declare", |tx| {
        let potential = edit::find_or_add_potential(tx, name, kind, volts)?;
        let options = PlaceOptions {
            potential: Some(potential),
            ..PlaceOptions::new()
        };
        edit::place(tx, page, &sym, Placement::at(at), &options)
    })
    .unwrap()
    .0
}

fn carried(doc: &Document, feed: SymbolId) -> (String, PotentialKind) {
    let SymbolRole::PotentialFeed { potential } = doc.project().symbol(feed).unwrap().role else {
        panic!("not a feed")
    };
    let pot = doc.project().potential(potential).unwrap();
    (pot.name.clone(), pot.kind)
}

fn names(doc: &Document) -> Vec<String> {
    let mut n: Vec<String> = doc.project().potentials().map(|p| p.name.clone()).collect();
    n.sort();
    n
}

/// A feed standing under `pin` of `sym`, its own pin `gap` mm below it and
/// pointing up, placed with the editor (so the facing-pin rule draws the
/// wire). With `gap` 0 the two pins coincide.
fn feed_under(
    doc: &mut Document,
    editor: &mut Editor,
    page: PageId,
    sym: SymbolId,
    pin: &str,
    gap: f64,
) -> SymbolId {
    let at = pin_pos(doc, sym, pin);
    editor.begin_place(FEED_SYMBOL, None);
    assert!(editor.key(doc, EditorKey::Rotate, &[]));
    assert!(editor.key(doc, EditorKey::Rotate, &[]));
    editor.click(doc, page, p(at.x, at.y + gap + 8.0), TOL);
    editor.leave_tool();
    doc.project()
        .symbols()
        .filter(|s| matches!(s.role, SymbolRole::PotentialFeed { .. }))
        .map(|s| s.id)
        .max()
        .expect("the feed was placed")
}

fn joined(doc: &Document, a: SymbolId, b: SymbolId, pin: &str) -> bool {
    let s = doc.project().symbol(b).unwrap();
    doc.project().netlist().connected(
        Endpoint::pin(a, 0),
        Endpoint::pin(b, s.pin_index(pin).unwrap()),
    )
}

#[test]
fn a_feed_under_a_24_volt_coil_is_0v_not_the_next_line() {
    let (mut doc, page, mut editor) = setup();
    let coil = place(&mut doc, page, "iec/coils/contactor", p(80.0, 80.0), false);
    let a1 = pin_pos(&doc, coil, "A1");
    declared_feed(
        &mut doc,
        page,
        p(a1.x, a1.y - 8.0),
        "24V",
        PotentialKind::DcPositive,
        Some(24.0),
    );
    // Mains is drawn elsewhere on the page: L1 is taken.
    declared_feed(
        &mut doc,
        page,
        p(0.0, 0.0),
        "L1",
        PotentialKind::Line,
        Some(400.0),
    );
    let feed = feed_under(&mut doc, &mut editor, page, coil, "A2", 16.0);
    assert!(joined(&doc, feed, coil, "A2"));
    assert_eq!(
        carried(&doc, feed),
        ("0V".to_owned(), PotentialKind::DcNegative)
    );
    // The name it was given first is gone again.
    assert_eq!(names(&doc), ["0V", "24V", "L1"]);
}

#[test]
fn a_feed_under_a_coil_on_a_line_is_the_neutral() {
    let (mut doc, page, mut editor) = setup();
    let coil = place(&mut doc, page, "iec/coils/contactor", p(80.0, 80.0), false);
    let a1 = pin_pos(&doc, coil, "A1");
    declared_feed(
        &mut doc,
        page,
        p(a1.x, a1.y - 8.0),
        "L",
        PotentialKind::Line,
        Some(230.0),
    );
    let feed = feed_under(&mut doc, &mut editor, page, coil, "A2", 16.0);
    assert_eq!(
        carried(&doc, feed),
        ("N".to_owned(), PotentialKind::Neutral)
    );
    assert_eq!(names(&doc), ["L", "N"]);
}

#[test]
fn a_feed_touching_a_pin_is_resolved_too() {
    let (mut doc, page, mut editor) = setup();
    let coil = place(&mut doc, page, "iec/coils/contactor", p(80.0, 80.0), false);
    let feed = feed_under(&mut doc, &mut editor, page, coil, "A2", 0.0);
    assert!(joined(&doc, feed, coil, "A2"), "touching pins join");
    assert_eq!(carried(&doc, feed).1, PotentialKind::Neutral);
}

#[test]
fn a_feed_on_a_pe_pin_is_pe_and_drawn_as_earth() {
    let (mut doc, page, mut editor) = setup();
    declared_feed(
        &mut doc,
        page,
        p(0.0, 0.0),
        "L1",
        PotentialKind::Line,
        Some(400.0),
    );
    let drive = place(
        &mut doc,
        page,
        "iec/drives/frequency_converter",
        p(80.0, 40.0),
        false,
    );
    let feed = feed_under(&mut doc, &mut editor, page, drive, "PE", 0.0);
    // Pins at the same place may need the dust to settle: join by drawing.
    assert_eq!(
        carried(&doc, feed),
        ("PE".to_owned(), PotentialKind::ProtectiveEarth)
    );
    assert_eq!(
        doc.project().symbol(feed).unwrap().symbol.0,
        FEED_PE_SYMBOL,
        "an earth feed is drawn with the earth symbol"
    );
    assert_eq!(names(&doc), ["L1", "PE"]);
}

#[test]
fn a_feed_on_a_net_that_carries_a_potential_takes_it() {
    let (mut doc, page, _) = setup();
    declared_feed(
        &mut doc,
        page,
        p(0.0, 0.0),
        "L1",
        PotentialKind::Line,
        Some(400.0),
    );
    let top = declared_feed(
        &mut doc,
        page,
        p(40.0, 0.0),
        "L2",
        PotentialKind::Line,
        Some(400.0),
    );
    let lamp = place(&mut doc, page, "iec/signalling/lamp", p(40.0, 80.0), false);
    let from = Endpoint::pin(top, 0);
    let x1 = pin_pos(&doc, lamp, "X1");
    let to = Endpoint::pin(
        lamp,
        doc.project().symbol(lamp).unwrap().pin_index("X1").unwrap(),
    );
    doc.transact("wire", |tx| tx.connect(from, to)).unwrap();
    // A new feed dropped on the middle of that wire is on the same conductor.
    let mid = (8.0 + x1.y) / 2.0;
    let mid = (mid / 4.0).round() * 4.0;
    let feed = place(&mut doc, page, FEED_SYMBOL, p(x1.x, mid - 8.0), false);
    assert_eq!(carried(&doc, feed).0, "L2");
    assert_eq!(names(&doc), ["L1", "L2"]);
}

#[test]
fn a_feed_attached_to_nothing_gets_a_name_no_potential_has() {
    let (mut doc, page, _) = setup();
    for (i, n) in ["L1", "L2", "L3"].iter().enumerate() {
        declared_feed(
            &mut doc,
            page,
            p(40.0 * i as f64, 0.0),
            n,
            PotentialKind::Line,
            Some(400.0),
        );
    }
    let other = doc.transact("page", |tx| tx.add_page("2")).unwrap().0;
    // Page 2 has no feeds, but L1 is the mains conductor of page 1.
    let lib = novo_symbols::Library::builtin().unwrap();
    let sym = lib.get(FEED_SYMBOL).unwrap().clone();
    let feed = doc
        .transact("feed", |tx| {
            edit::place(
                tx,
                other,
                &sym,
                Placement::at(p(0.0, 0.0)),
                &PlaceOptions::new(),
            )
        })
        .unwrap()
        .0;
    assert_eq!(carried(&doc, feed).0, "L4");
}

#[test]
fn a_supply_output_gets_24v_and_0v() {
    let (mut doc, page, mut editor) = setup();
    let psu = place(
        &mut doc,
        page,
        "iec/supplies/power_supply_dc",
        p(80.0, 40.0),
        false,
    );
    let plus = feed_under(&mut doc, &mut editor, page, psu, "L+", 16.0);
    let zero = feed_under(&mut doc, &mut editor, page, psu, "M", 16.0);
    assert_eq!(
        carried(&doc, plus),
        ("24V".to_owned(), PotentialKind::DcPositive)
    );
    assert_eq!(
        carried(&doc, zero),
        ("0V".to_owned(), PotentialKind::DcNegative)
    );
}

#[test]
fn a_name_a_person_gave_is_never_changed_by_the_attachment() {
    let (mut doc, page, _) = setup();
    let coil = place(&mut doc, page, "iec/coils/contactor", p(80.0, 80.0), false);
    let feed = place(&mut doc, page, FEED_SYMBOL, p(240.0, 240.0), true);
    assert!(!joined(&doc, feed, coil, "A2"), "far away: still alone");
    assert!(carried(&doc, feed).0.starts_with('L'));
    // The person names it L2 by hand.
    doc.transact("rename", |tx| {
        edit::rename_label(tx, feed, "L2").map_err(|reason| novo_core::ModelError::Refused {
            what: "rename".into(),
            reason,
        })
    })
    .unwrap();
    // Moving it onto the coil afterwards does not turn it into a neutral.
    let (a2, now) = (pin_pos(&doc, coil, "A2"), pin_pos(&doc, feed, "1"));
    doc.transact("move", |tx| {
        edit::transform(
            tx,
            &[novo_core::AnyId::Symbol(feed)],
            edit::Transform::Move(p(a2.x - now.x, a2.y - now.y)),
        )?;
        edit::settle(tx, &[feed])
    })
    .unwrap();
    assert!(joined(&doc, feed, coil, "A2"));
    assert_eq!(carried(&doc, feed).0, "L2");
}

/// The same move of a feed nobody named: it becomes what it lands on.
#[test]
fn moving_a_provisional_feed_onto_a_coil_return_resolves_it() {
    let (mut doc, page, _) = setup();
    let coil = place(&mut doc, page, "iec/coils/contactor", p(80.0, 80.0), false);
    let feed = place(&mut doc, page, FEED_SYMBOL, p(240.0, 240.0), true);
    assert_eq!(carried(&doc, feed).0, "L1");
    let (a2, now) = (pin_pos(&doc, coil, "A2"), pin_pos(&doc, feed, "1"));
    doc.transact("move", |tx| {
        edit::transform(
            tx,
            &[novo_core::AnyId::Symbol(feed)],
            edit::Transform::Move(p(a2.x - now.x, a2.y - now.y)),
        )?;
        edit::settle(tx, &[feed])
    })
    .unwrap();
    assert_eq!(
        carried(&doc, feed),
        ("N".to_owned(), PotentialKind::Neutral)
    );
    assert_eq!(names(&doc), ["N"]);
}

/// Finding 12: a feed dropped exactly on a pin that already has a wire
/// stayed "not connected" until it was dragged away and back.
#[test]
fn a_feed_on_a_pin_that_is_already_wired_joins_it() {
    let (mut doc, page, mut editor) = setup();
    let lamp = place(&mut doc, page, "iec/signalling/lamp", p(80.0, 80.0), false);
    let x1 = pin_pos(&doc, lamp, "X1");
    let end = doc
        .transact("end", |tx| tx.add_junction(page, p(x1.x, x1.y - 40.0)))
        .unwrap()
        .0;
    let pin = doc.project().symbol(lamp).unwrap().pin_index("X1").unwrap();
    doc.transact("wire", |tx| {
        tx.connect(Endpoint::pin(lamp, pin), Endpoint::junction(end))
    })
    .unwrap();
    let feed = place(&mut doc, page, FEED_SYMBOL, p(x1.x, x1.y - 8.0), false);
    let _ = &mut editor;
    assert!(joined(&doc, feed, lamp, "X1"));
}
