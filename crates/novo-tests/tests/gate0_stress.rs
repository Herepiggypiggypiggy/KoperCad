//! Gate 0 model-side checks on a 10,000-object page. The 60 fps criterion
//! itself needs the canvas (WS-03) on a real GPU; these tests cover the model
//! work a frame or an edit depends on, with generous limits so CI machines
//! do not flake. Timings print with `cargo test -- --nocapture`.

use std::time::{Duration, Instant};

use novo_core::*;
use novo_tests::stress_page;

const OBJECTS: usize = 10_000;

/// Budgets are tight for release builds (scripts/scale-bench.sh); a debug
/// build on a shared runner gets ten times as long. The guard is never off.
fn budget(base: Duration) -> Duration {
    base * if cfg!(debug_assertions) { 10 } else { 1 }
}

fn timed<R>(label: &str, f: impl FnOnce() -> R) -> (R, Duration) {
    let start = Instant::now();
    let r = f();
    let took = start.elapsed();
    eprintln!("{label}: {took:?}");
    (r, took)
}

#[test]
fn ten_thousand_objects_build_query_save_and_undo() {
    let mut doc = Document::default();
    let (page, _) = timed("build", || stress_page(&mut doc, OBJECTS).unwrap());
    let p = doc.project();
    let contents = p.page_contents(page);
    let count = contents.symbols.len() + contents.wires.len() + p.devices().count();
    assert!(count >= OBJECTS, "{count} objects");

    // Reading one page's contents is what every frame does.
    let (_, read) = timed("page_contents", || p.page_contents(page));
    assert!(
        read < budget(Duration::from_millis(500)),
        "page read took {read:?}"
    );

    let (net, took) = timed("netlist", || p.netlist());
    assert_eq!(net.nets().len(), OBJECTS.div_ceil(3) + 1);
    assert!(took < Duration::from_secs(5));

    let (json, _) = timed("to_json", || p.to_json());
    let (back, took) = timed("from_json", || Project::from_json(&json).unwrap());
    assert_eq!(back.to_json(), json);
    assert!(took < Duration::from_secs(10));

    // One small edit on a big page stays cheap, and so does its undo.
    let symbol = contents.symbols[OBJECTS / 6];
    let (_, took) = timed("move one symbol", || {
        doc.transact("Move", |tx| {
            tx.move_symbol(symbol, Placement::at(Point::new(-50.0, -50.0)))
        })
        .unwrap()
    });
    assert!(
        took < budget(Duration::from_millis(200)),
        "edit took {took:?}"
    );
    let (_, took) = timed("undo move", || doc.undo().unwrap());
    assert!(
        took < budget(Duration::from_millis(200)),
        "undo took {took:?}"
    );

    let (_, took) = timed("undo whole page", || doc.undo().unwrap());
    assert_eq!(doc.project().records().count(), 0);
    assert!(took < Duration::from_secs(10));
}
