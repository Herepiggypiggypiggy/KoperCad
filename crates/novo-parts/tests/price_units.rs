//! Prices read from a maker file (the folder and command-line importer): the
//! price basis is honoured and values that cannot be prices are refused.

use std::path::Path;

use novo_parts::*;

fn import_prices(rows: &[(&str, &str)]) -> ImportOutcome {
    let dir = tempfile::tempdir().unwrap();
    let mut csv = String::from("Artikelnummer;Bezeichnung;Listenpreis\n");
    for (order, price) in rows {
        csv.push_str(&format!("{order};Klemme;{price}\n"));
    }
    let file = dir.path().join("prices.csv");
    std::fs::write(&file, csv).unwrap();
    run(&file, dir.path())
}

fn run(file: &Path, lib_dir: &Path) -> ImportOutcome {
    let lib = LocalLibrary::at(lib_dir);
    let sources = SourceTable::builtin();
    let fetcher = ScriptedFetcher::default();
    let mut job = ImportJob::new(&lib, &sources, &fetcher, "WAGO", "2002-1201", "2026-10-28");
    match job.import_file(file, ImportFlow::Manual).clone() {
        ImportState::Done(o) => o,
        other => panic!("{other:?}"),
    }
}

fn price<'a>(o: &'a ImportOutcome, order: &str) -> Option<&'a Price> {
    o.parts
        .iter()
        .find(|p| p.order_number == order)
        .unwrap_or_else(|| panic!("no part {order}"))
        .price
        .as_ref()
}

#[test]
fn a_price_per_hundred_or_thousand_keeps_its_basis() {
    let o = import_prices(&[
        ("2002-1201", "9 EUR/100"),
        ("2002-1202", "12,50 EUR per 1000 pcs"),
        ("2002-1203", "0,42 EUR/pc"),
    ]);
    let a = price(&o, "2002-1201").expect("price");
    assert_eq!((a.amount, a.per, a.currency.as_str()), (9.0, 100, "EUR"));
    let b = price(&o, "2002-1202").expect("price");
    assert_eq!((b.amount, b.per), (12.5, 1000));
    let c = price(&o, "2002-1203").expect("price");
    assert_eq!((c.amount, c.per), (0.42, 1));
}

#[test]
fn a_price_per_metre_is_not_taken_as_a_price_per_piece() {
    let o = import_prices(&[("2002-1201", "12.50 EUR/m")]);
    assert!(price(&o, "2002-1201").is_none());
    assert!(
        o.recipe
            .skipped
            .iter()
            .any(|s| s.reason.contains("per metre"))
    );
}

#[test]
fn negative_zero_and_absurd_prices_are_refused() {
    let o = import_prices(&[
        ("2002-1201", "-5 EUR"),
        ("2002-1202", "99999999999999 EUR"),
        ("2002-1203", "0 EUR"),
        ("2002-1204", "1,5 EUR"),
    ]);
    assert!(price(&o, "2002-1201").is_none());
    assert!(price(&o, "2002-1202").is_none());
    assert!(price(&o, "2002-1203").is_none());
    assert_eq!(price(&o, "2002-1204").unwrap().amount, 1.5);
    let reasons: Vec<_> = o.recipe.skipped.iter().map(|s| s.reason.as_str()).collect();
    assert!(
        reasons.contains(&"price that is zero or negative"),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&"price that is too large to be real"),
        "{reasons:?}"
    );
}

#[test]
fn an_ambiguous_thousands_separator_warns() {
    let o = import_prices(&[("2002-1201", "1.500 EUR")]);
    assert_eq!(price(&o, "2002-1201").unwrap().amount, 1.5);
    assert!(
        o.recipe
            .warnings
            .iter()
            .any(|w| w.starts_with("2002-1201:") && w.contains("1.500")),
        "{:?}",
        o.recipe.warnings
    );
}

#[test]
fn a_price_cell_with_non_ascii_capitals_does_not_panic() {
    let o = import_prices(&[
        ("2002-1201", "İİİİ 10 per 100"),
        ("2002-1202", "5 İ per 100 pcs"),
        ("2002-1203", "Ⱥ9 per 100"),
        ("2002-1204", "İ per 日"),
        ("2002-1205", "5 İ per 日本語"),
        ("2002-1206", "İİ 5 per 日日日日日"),
    ]);
    for order in [
        "2002-1201",
        "2002-1202",
        "2002-1203",
        "2002-1204",
        "2002-1205",
        "2002-1206",
    ] {
        assert!(price(&o, order).is_none(), "{order}");
    }
}

#[test]
fn a_price_in_exponent_notation_is_refused() {
    let o = import_prices(&[("2002-1201", "1e3 EUR")]);
    assert!(price(&o, "2002-1201").is_none());
    assert!(
        o.recipe
            .skipped
            .iter()
            .any(|s| s.reason == "price in exponent notation")
    );
}
