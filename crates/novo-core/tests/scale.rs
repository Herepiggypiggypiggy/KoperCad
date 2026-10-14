//! The model at the size of a large real project (10k symbols). Timings are
//! in `cargo run --release -p novo-core --example scale`; this checks that
//! everything stays correct at that size.

use novo_core::*;

#[path = "common/scale_project.rs"]
mod scale_project;
use scale_project::{PAGES, SYMBOLS_PER_PAGE, build};

#[test]
fn ten_thousand_symbols() {
    let mut doc = Document::default();
    let phases = build(&mut doc);
    let project = doc.project();
    assert_eq!(project.symbols().count(), PAGES * (SYMBOLS_PER_PAGE + 2));
    assert_eq!(project.wires().count(), PAGES * (SYMBOLS_PER_PAGE + 1));

    let net = project.netlist();
    // Each symbol's output pin opens a new net; the feed's net is one more.
    assert_eq!(net.nets().len(), PAGES * (SYMBOLS_PER_PAGE + 1));
    assert_eq!(net.potential_conflicts().count(), 0);
    // L1 feeds page 1 and crosses switched poles 0 and 3, so it reaches the
    // nets after them; the net after pole 1 (not switched) has none.
    let page = project.pages().next().unwrap().id;
    let contacts: Vec<SymbolId> = project
        .page_contents(page)
        .symbols
        .into_iter()
        .filter(|&s| project.symbol(s).unwrap().symbol.0 == "iec/contacts/no")
        .collect();
    assert_eq!(
        net.potential_of(Endpoint::pin(contacts[0], 1)),
        Some(phases[0])
    );
    assert_eq!(net.potential_of(Endpoint::pin(contacts[1], 1)), None);

    // One edit and its undo on a big project.
    let symbol = contacts[50];
    doc.transact("Move", |tx| {
        tx.move_symbol(symbol, Placement::at(Point::new(5.0, 5.0)))
    })
    .unwrap();
    doc.undo().unwrap();

    // Save and load.
    let loaded = Project::from_json(&doc.project().to_json()).unwrap();
    assert_eq!(loaded.symbols().count(), PAGES * (SYMBOLS_PER_PAGE + 2));
}
