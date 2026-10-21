//! Scale guard: a 60-page generated project must stay interactive. The
//! limits are generous (debug build, shared CI machines); they catch a
//! quadratic loop coming back, not a slow afternoon. Full numbers come from
//! `cargo run --release -p novo-tests --example scale_bench`.

use std::time::{Duration, Instant};

use novo_core::Document;
use novo_reports::standard_reports;
use novo_tests::{BigSpec, big_project};

fn big() -> Document {
    let mut doc = Document::default();
    big_project(&mut doc, BigSpec::new(30)).expect("generator");
    doc
}

#[test]
fn generator_makes_the_promised_size() {
    let doc = big();
    let p = doc.project();
    assert_eq!(p.pages().count(), 60);
    // Not one circuit repeated: motor starters (with cables), heaters, I/O.
    let letters: std::collections::BTreeSet<char> = p
        .devices()
        .filter_map(|d| d.tag.letter_code().chars().next())
        .collect();
    for letter in ['F', 'K', 'M', 'E', 'Y', 'A', 'B'] {
        assert!(
            letters.contains(&letter),
            "no -{letter} device in {letters:?}"
        );
    }
    assert!(p.cables().count() >= 5 && p.cables().count() < 30);
    assert!(p.devices().count() >= 30 * 3);
    assert!(p.wires().count() >= 30 * 8);
    let parts: std::collections::BTreeSet<_> = p.parts().map(|x| x.order_number.clone()).collect();
    assert!(parts.len() >= 40, "{} distinct parts", parts.len());
    let lengths: std::collections::BTreeSet<u64> = p
        .cables()
        .filter_map(|c| c.length_m)
        .map(|l| l as u64)
        .collect();
    assert!(lengths.len() >= 4, "cable lengths {lengths:?}");
    // The same spec makes the same project.
    let again = big();
    assert_eq!(again.project().to_json(), p.to_json());
    // One undo step takes it all back.
    let mut doc = doc;
    doc.undo().unwrap();
    assert_eq!(doc.project().records().count(), 0);
}

/// The cable reports once built the netlist for every cable (6 s each on 300
/// cables); the cost must not grow with cables times project size.
#[test]
fn cable_reports_build_the_netlist_once() {
    let doc = big();
    let p = doc.project();
    for report in standard_reports() {
        if !matches!(report.id(), "cables" | "cableschedule") {
            continue;
        }
        let start = Instant::now();
        let out = report.generate(p);
        let took = start.elapsed();
        assert!(!out.sections.is_empty());
        assert!(
            took < Duration::from_secs(3),
            "{} took {took:?} for 30 cables",
            report.id()
        );
    }
}
