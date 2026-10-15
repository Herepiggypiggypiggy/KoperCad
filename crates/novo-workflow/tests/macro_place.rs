//! Placing a macro: tags, wire numbers, preview and undo.

use novo_core::*;
use novo_workflow::macro_place::{PlaceOptions, WireNumbers, place_macro, preview};

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
fn tags_continue_and_placed_wires_are_numbered_by_the_scheme() {
    let (mut doc, page) = target();
    let placed = place(&mut doc, page, &PlaceOptions::default());
    assert_eq!(placed.report.tags, vec![tag("=A1-M1"), tag("=A1-Q2")]);
    assert_eq!(placed.report.cleared_wires, 1);
    assert!(placed.report.kept_wire_numbers.is_empty());
    assert!(!placed.report.reused_definition);
    assert_eq!(placed.report.numbered_wires, vec!["1".to_string()]);
    let mut numbers: Vec<_> = doc
        .project()
        .wires()
        .filter_map(|w| w.number.clone())
        .collect();
    numbers.sort();
    assert_eq!(numbers, ["1", "W1"], "the target's own number is untouched");
}

#[test]
fn the_project_scheme_decides_the_placed_numbers() {
    let (mut doc, page) = target();
    doc.transact("Scheme", |tx| {
        let mut scheme = novo_numbering::Scheme::new(novo_numbering::SchemeKind::ByPage);
        scheme.prefix = "C".into();
        scheme.store(tx)
    })
    .unwrap();
    let placed = place(&mut doc, page, &PlaceOptions::default());
    assert_eq!(placed.report.numbered_wires, vec!["C1.1".to_string()]);
    // A second placement continues the series.
    let again = place(&mut doc, page, &PlaceOptions::default());
    assert_eq!(again.report.numbered_wires, vec!["C1.2".to_string()]);
}

#[test]
fn clear_leaves_the_placed_wires_unnumbered() {
    let (mut doc, page) = target();
    let options = PlaceOptions {
        wire_numbers: WireNumbers::Clear,
        ..PlaceOptions::default()
    };
    let placed = place(&mut doc, page, &options);
    assert!(placed.report.numbered_wires.is_empty());
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
    assert_eq!(placed.report.numbered_wires, vec!["1".to_string()]);

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
