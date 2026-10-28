//! The cooling tower report's shorts (finding 3) end to end: a drawing of an
//! incomer and a drive on two pages, with the two mistakes of the report,
//! checked. The same project is the QA fixture
//! `qa/fixtures/cooling-tower-shorts.novo` (`cargo test -p novo-tests
//! --test short_checks write_fixture -- --ignored` writes it).

use novo_checks::{Problem, ProblemKind, Severity, check};
use novo_core::{Document, Endpoint, PageId, Placement, Point, PotentialKind, Project, SymbolId};
use novo_editor::edit::{self, PlaceOptions};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

struct Drawing {
    f4: SymbolId,
    t1: SymbolId,
    page_drive: PageId,
}

fn place(
    doc: &mut Document,
    page: PageId,
    key: &str,
    at: Point,
    options: PlaceOptions,
) -> SymbolId {
    let lib = novo_symbols::Library::builtin().unwrap();
    let sym = lib.get(key).unwrap_or_else(|| panic!("{key}")).clone();
    doc.transact("place", |tx| {
        edit::place(tx, page, &sym, Placement::at(at), &options)
    })
    .unwrap()
    .0
}

fn ip(doc: &mut Document, page: PageId, at: Point, name: &str) -> SymbolId {
    let options = PlaceOptions {
        interruption: Some(name.to_owned()),
        ..PlaceOptions::new()
    };
    place(
        doc,
        page,
        "iec/connectivity/interruption_point",
        at,
        options,
    )
}

fn pin(doc: &Document, s: SymbolId, name: &str) -> (Endpoint, Point) {
    let sym = doc.project().symbol(s).unwrap();
    let i = sym.pin_index(name).unwrap_or_else(|| panic!("pin {name}"));
    let e = Endpoint::pin(s, i);
    (e, doc.project().endpoint_position(e).unwrap())
}

fn wire(doc: &mut Document, a: Endpoint, b: Endpoint) {
    doc.transact("wire", |tx| tx.connect(a, b)).unwrap();
}

/// Page 2: mains L1..L3 through the group breaker -F4 to interruption
/// points. Page 1a: the drive -T1 fed from the same points. The drawing is
/// right; `shorts` then makes the two mistakes of the report.
fn draw(doc: &mut Document) -> Drawing {
    let page_incomer = doc.transact("page", |tx| tx.add_page("2")).unwrap().0;
    let page_drive = doc.transact("page", |tx| tx.add_page("1a")).unwrap().0;
    let f4 = place(
        doc,
        page_incomer,
        "iec/protection/mcb_3p",
        p(40.0, 40.0),
        PlaceOptions::new(),
    );
    // Mains declared the way "Declare source" does.
    for (name, top) in [("1", "L1"), ("3", "L2"), ("5", "L3")] {
        let (_, at) = pin(doc, f4, name);
        let potential = doc
            .transact("potential", |tx| {
                edit::find_or_add_potential(tx, top, PotentialKind::Line, Some(400.0))
            })
            .unwrap()
            .0;
        let options = PlaceOptions {
            potential: Some(potential),
            ..PlaceOptions::new()
        };
        place(
            doc,
            page_incomer,
            "iec/connectivity/potential_feed",
            p(at.x, at.y - 8.0),
            options,
        );
    }
    // Interruption points under the outputs, placed left to right.
    for (name, ip_name) in [("2", "IP1"), ("4", "IP2"), ("6", "IP3")] {
        let (e, at) = pin(doc, f4, name);
        let ip = ip(doc, page_incomer, p(at.x, at.y + 24.0), ip_name);
        wire(doc, e, pin(doc, ip, "1").0);
    }
    let t1 = place(
        doc,
        page_drive,
        "iec/drives/frequency_converter",
        p(40.0, 80.0),
        PlaceOptions::new(),
    );
    for (name, ip_name) in [("L1", "IP1"), ("L2", "IP2"), ("L3", "IP3")] {
        let (e, at) = pin(doc, t1, name);
        let ip = ip(doc, page_drive, p(at.x, at.y - 24.0), ip_name);
        wire(doc, e, pin(doc, ip, "1").0);
    }
    Drawing { f4, t1, page_drive }
}

fn shorts(project: &Project) -> Vec<Problem> {
    check(project)
        .into_iter()
        .filter(|p| p.kind == ProblemKind::PotentialConflict)
        .collect()
}

#[test]
fn the_drawing_as_it_should_be_has_no_shorts() {
    let mut doc = Document::default();
    draw(&mut doc);
    assert!(
        shorts(doc.project()).is_empty(),
        "{:#?}",
        shorts(doc.project())
    );
}

/// A feed named L1 on the drive's PE pin, which is what the report found
/// when a new feed took the next free line name.
fn l1_on_the_pe_pin(doc: &mut Document, d: &Drawing) -> SymbolId {
    let (_, pe) = pin(doc, d.t1, "PE");
    let l1 = doc
        .transact("potential", |tx| {
            edit::find_or_add_potential(tx, "L1", PotentialKind::Line, Some(400.0))
        })
        .unwrap()
        .0;
    let options = PlaceOptions {
        potential: Some(l1),
        ..PlaceOptions::new()
    };
    place(
        doc,
        d.page_drive,
        "iec/connectivity/potential_feed",
        p(pe.x, pe.y - 8.0),
        options,
    )
}

#[test]
fn pe_on_a_phase_is_found() {
    let mut doc = Document::default();
    let d = draw(&mut doc);
    assert!(shorts(doc.project()).is_empty());
    l1_on_the_pe_pin(&mut doc, &d);
    let found = shorts(doc.project());
    assert_eq!(found.len(), 1, "{found:#?}");
    assert_eq!(found[0].severity, Severity::Error);
    assert!(
        found[0].message.starts_with("Protective earth pin of -"),
        "{}",
        found[0].message
    );
    assert!(found[0].message.contains("L1"));
    assert!(found[0].message.contains("page 1a"), "{}", found[0].message);
}

#[test]
fn two_phases_bridged_at_the_breaker_are_found_with_the_page() {
    let mut doc = Document::default();
    let d = draw(&mut doc);
    // A stray wire between the first and second output of -F4.
    let (a, _) = pin(&doc, d.f4, "2");
    let (b, _) = pin(&doc, d.f4, "4");
    wire(&mut doc, a, b);
    let found = shorts(doc.project());
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0]
            .message
            .contains("L1 and L2 are joined by the poles of"),
        "{}",
        found[0].message
    );
    assert!(found[0].message.contains("page"), "{}", found[0].message);
}

/// `cargo test -p novo-tests --test short_checks write_fixture -- --ignored`
#[test]
#[ignore = "writes qa/fixtures/cooling-tower-shorts.novo"]
fn write_fixture() {
    let mut doc = Document::default();
    let d = draw(&mut doc);
    l1_on_the_pe_pin(&mut doc, &d);
    let (a, _) = pin(&doc, d.f4, "2");
    let (b, _) = pin(&doc, d.f4, "4");
    wire(&mut doc, a, b);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/cooling-tower-shorts.novo");
    let _ = std::fs::remove_file(&path);
    novo_storage::Session::create(&path, doc.project()).unwrap();
}
