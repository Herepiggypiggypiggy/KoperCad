//! Placing a macro: tags, wire numbers, preview and undo.

use novo_core::*;
use novo_workflow::macro_place::{
    PlaceOptions, WireNumbers, capture, captured_parts, free_origin, parts_from_json,
    parts_to_json, place_macro, preview, selection_origin,
};

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

/// Draws breaker -Q1 wired to motor -M1 (wire numbered `W1`) on a page and
/// captures it as the block `starter`.
fn starter() -> BlockDefinition {
    let mut doc = Document::default();
    let (def, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.update::<Page>(page, |p| p.structure = tag("=A1"))?;
            let q = tx.add_device(tag("=A1-Q1"), "Breaker")?;
            let m = tx.add_device(tag("=A1-M1"), "Motor")?;
            let mut sym = |x: f64, d: DeviceId| {
                tx.place_symbol(
                    page,
                    "x",
                    Placement::at(Point::new(x, 0.0)),
                    pins(),
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )
            };
            let (q1, m1) = (sym(10.0, q)?, sym(30.0, m)?);
            let wire = tx.connect(Endpoint::pin(q1, 1), Endpoint::pin(m1, 0))?;
            tx.update::<Wire>(wire, |w| w.number = Some("W1".into()))?;
            let id = tx.define_block(
                "starter",
                &[q1.into(), m1.into()],
                Point::new(10.0, 0.0),
                Vec::new(),
            )?;
            Ok(tx.project().block_definition(id).unwrap().clone())
        })
        .unwrap();
    def
}

/// A project with one page and a breaker -Q1 already drawn, and a wire
/// numbered `W1`.
fn target() -> (Document, PageId) {
    let mut doc = Document::default();
    let (page, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.update::<Page>(page, |p| p.structure = tag("=A1"))?;
            let q = tx.add_device(tag("=A1-Q1"), "Breaker")?;
            let a = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(0.0, 0.0)),
                pins(),
                SymbolRole::Device {
                    device: q,
                    main: true,
                },
            )?;
            let b = tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(0.0, 50.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            let wire = tx.connect(Endpoint::pin(a, 1), Endpoint::pin(b, 0))?;
            tx.update::<Wire>(wire, |w| w.number = Some("W1".into()))?;
            Ok(page)
        })
        .unwrap();
    (doc, page)
}

fn place(
    doc: &mut Document,
    page: PageId,
    options: &PlaceOptions,
) -> novo_workflow::macro_place::Placed {
    let (placed, _) = doc
        .transact("Place macro", |tx| {
            place_macro(tx, starter(), page, Point::new(100.0, 0.0), options)
        })
        .unwrap();
    placed
}

#[test]
fn tags_continue_and_wire_numbers_are_cleared() {
    let (mut doc, page) = target();
    let placed = place(&mut doc, page, &PlaceOptions::default());
    assert_eq!(placed.report.tags, vec![tag("=A1-M1"), tag("=A1-Q2")]);
    assert_eq!(placed.report.cleared_wires, 1);
    assert!(placed.report.kept_wire_numbers.is_empty());
    assert!(!placed.report.reused_definition);
    let numbered = doc.project().wires().filter(|w| w.number.is_some()).count();
    assert_eq!(numbered, 1, "only the target's own wire is numbered");
}

#[test]
fn keep_drops_a_clashing_number_and_says_so() {
    let (mut doc, page) = target();
    let options = PlaceOptions {
        wire_numbers: WireNumbers::Keep,
        ..PlaceOptions::default()
    };
    let placed = place(&mut doc, page, &options);
    assert_eq!(placed.report.clashes, vec!["W1".to_string()]);
    assert!(placed.report.kept_wire_numbers.is_empty());

    // In an empty project the number survives.
    let mut empty = Document::default();
    let (p, _) = empty.transact("Page", |tx| tx.add_page("1")).unwrap();
    let placed = place(&mut empty, p, &options);
    assert_eq!(placed.report.kept_wire_numbers, vec!["W1".to_string()]);
}

#[test]
fn preview_matches_the_placement_and_changes_nothing() {
    let (mut doc, page) = target();
    let before = doc.project().to_json();
    let options = PlaceOptions::default();
    let report = preview(
        doc.project(),
        &starter(),
        page,
        Point::new(100.0, 0.0),
        &options,
    )
    .unwrap();
    assert_eq!(doc.project().to_json(), before);
    let placed = place(&mut doc, page, &options);
    assert_eq!(report, placed.report);
}

#[test]
fn one_undo_step_and_the_definition_is_reused() {
    let (mut doc, page) = target();
    let depth = doc.undo_depth();
    let first = place(&mut doc, page, &PlaceOptions::default());
    assert_eq!(doc.undo_depth(), depth + 1);
    let second = place(&mut doc, page, &PlaceOptions::default());
    assert!(second.report.reused_definition);
    assert_eq!(first.definition, second.definition);
    assert_eq!(second.report.tags, vec![tag("=A1-M2"), tag("=A1-Q3")]);
    doc.undo().unwrap();
    doc.undo().unwrap();
    assert_eq!(doc.project().block_definitions().count(), 0);
    assert_eq!(doc.project().devices().count(), 1);
}

#[test]
fn a_different_definition_under_the_same_name_is_refused() {
    let (mut doc, page) = target();
    place(&mut doc, page, &PlaceOptions::default());
    let mut other = starter();
    other.description = "x".into();
    other.members.pop();
    let result = doc.transact("Place macro", |tx| {
        place_macro(tx, other, page, Point::default(), &PlaceOptions::default())
    });
    assert!(matches!(result, Err(ModelError::DuplicateBlock(_))));
}

#[test]
fn capture_leaves_the_project_alone_and_places_elsewhere() {
    let (mut doc, page) = target();
    let items: Vec<AnyId> = doc.project().symbols().map(|s| s.id.into()).collect();
    let before = doc.project().to_json();
    let def = capture(doc.project(), "pair", "Two symbols", &items).unwrap();
    assert_eq!(doc.project().to_json(), before);
    assert_eq!(def.name, "pair");
    assert_eq!(def.description, "Two symbols");
    assert_eq!(
        selection_origin(doc.project(), &items),
        Point::new(0.0, 0.0)
    );

    let origin = free_origin(doc.project(), page, 20.0);
    assert_eq!(origin, Point::new(20.0, 0.0));
    let (placed, _) = doc
        .transact("Place macro", |tx| {
            place_macro(tx, def, page, origin, &PlaceOptions::default())
        })
        .unwrap();
    assert_eq!(placed.report.tags, vec![tag("=A1-Q2")]);
}

#[test]
fn capture_refuses_terminals_and_an_empty_page_has_the_origin() {
    let mut doc = Document::default();
    let (_page, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let strip = tx.add_terminal_strip(tag("=A1-X1"))?;
            let t = tx.add_terminal(strip, "1")?;
            tx.place_symbol(
                page,
                "terminal",
                Placement::at(Point::new(5.0, 5.0)),
                pins(),
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            Ok(page)
        })
        .unwrap();
    let items: Vec<AnyId> = doc.project().symbols().map(|s| s.id.into()).collect();
    assert!(capture(doc.project(), "x", "", &items).is_err());
    let (empty, _) = doc.transact("Page", |tx| tx.add_page("2")).unwrap();
    assert_eq!(
        free_origin(doc.project(), empty, 20.0),
        Point::new(0.0, 0.0)
    );
}

#[test]
fn free_origin_clears_the_pins_of_a_wide_symbol() {
    let mut doc = Document::default();
    let (page, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.place_symbol(
                page,
                "x",
                Placement::at(Point::new(10.0, 4.0)),
                vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 50.0, 0.0)],
                SymbolRole::Unassigned,
            )?;
            Ok(page)
        })
        .unwrap();
    assert_eq!(
        free_origin(doc.project(), page, 20.0),
        Point::new(80.0, 4.0)
    );
}

#[test]
fn parts_travel_with_the_macro_and_are_added_once() {
    let (mut doc, page) = target();
    let (items, _) = doc
        .transact("Part", |tx| {
            let q = tx.project().devices().next().unwrap().id;
            let part = tx.add_part("ABB", "2CDS253001R0164")?;
            tx.update::<Part>(part, |p| p.description = "MCB 1P 16A".into())?;
            tx.assign_part(q, part)?;
            Ok(tx
                .project()
                .symbols()
                .map(|s| AnyId::from(s.id))
                .collect::<Vec<_>>())
        })
        .unwrap();
    let def = capture(doc.project(), "pair", "", &items).unwrap();
    let parts = captured_parts(doc.project(), &def);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].order_number, "2CDS253001R0164");

    // Through the file's JSON and into a project that has no such part.
    let parts = parts_from_json(&parts_to_json(&parts));
    assert_eq!(parts.len(), 1);
    assert!(parts_from_json(&serde_json::json!([{"device": "x"}, 3])).is_empty());
    let mut empty = Document::default();
    let (p, _) = empty.transact("Page", |tx| tx.add_page("1")).unwrap();
    let options = PlaceOptions {
        parts,
        ..PlaceOptions::default()
    };
    let place_here = |doc: &mut Document| {
        let def = def.clone();
        let options = options.clone();
        doc.transact("Place", move |tx| {
            place_macro(tx, def, p, Point::default(), &options)
        })
        .unwrap()
        .0
    };
    let first = place_here(&mut empty);
    assert_eq!(first.report.parts_assigned, 1);
    place_here(&mut empty);
    assert_eq!(empty.project().parts().count(), 1, "the part is added once");
    let part = empty.project().parts().next().unwrap();
    assert_eq!(part.description, "MCB 1P 16A");
    let assigned = empty
        .project()
        .devices()
        .filter(|d| d.parts == vec![part.id])
        .count();
    assert_eq!(assigned, 2);

    // Without parts in the options none is assigned.
    let plain = doc
        .transact("Place", |tx| {
            place_macro(
                tx,
                def.clone(),
                page,
                Point::new(100.0, 0.0),
                &PlaceOptions::default(),
            )
        })
        .unwrap()
        .0;
    assert_eq!(plain.report.parts_assigned, 0);
}
