//! Catalog schema v2 and the 100,000-part scale target.

use std::time::Instant;

use novo_parts::{
    Catalog, CatalogPart, Classification, ConnectionKind, Lifecycle, PartRef, PartTerminal, Price,
    Provenance, Side,
};

fn rich() -> CatalogPart {
    CatalogPart {
        manufacturer: "WAGO".into(),
        order_number: "2002-1201".into(),
        description: "feed-through terminal block".into(),
        function: "terminal".into(),
        class: Classification {
            etim: Some("EC000897".into()),
            eclass: Some("27-14-11-01".into()),
        },
        terminals: vec![PartTerminal {
            name: "1".into(),
            kind: ConnectionKind::PushIn,
            wire_mm2: Some((0.08, 4.0)),
            side: Side::Top,
            position_mm: Some([2.5, 70.0, 10.0]),
        }],
        price: Some(Price {
            amount: 12.5,
            currency: "EUR".into(),
            per: 100,
            date: "2026-10-15".into(),
        }),
        lifecycle: Lifecycle::PhasingOut,
        replaced_by: Some(PartRef {
            manufacturer: "WAGO".into(),
            order_number: "2002-1202".into(),
        }),
        provenance: Some(Provenance {
            source: "import:bmecat".into(),
            file: "wago.xml".into(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn v2_fields_round_trip_through_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parts.json");
    let mut catalog = Catalog::new();
    catalog.upsert(rich());
    catalog.save(&path).unwrap();
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .contains("\"format\": 2")
    );
    let back = Catalog::load(&path).unwrap();
    assert_eq!(back, catalog);
    let part = back.find("wago", "2002 1201").unwrap();
    assert_eq!(part.price.as_ref().unwrap().each(), 0.125);
    assert_eq!(part.lifecycle, Lifecycle::PhasingOut);
}

#[test]
fn a_version_1_file_still_loads_with_unknown_fields() {
    let v1 =
        r#"{"format":1,"parts":[{"manufacturer":"Acme","order_number":"A-1","description":"x"}]}"#;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.json");
    std::fs::write(&path, v1).unwrap();
    let catalog = Catalog::load(&path).unwrap();
    let part = catalog.find("Acme", "A1").unwrap();
    assert_eq!(part.lifecycle, Lifecycle::Unknown);
    assert!(part.price.is_none() && part.terminals.is_empty() && part.provenance.is_none());
}

#[test]
fn unknown_enum_values_from_a_newer_writer_load_as_unknown() {
    let json = r#"{"format":2,"parts":[{"manufacturer":"A","order_number":"1",
        "lifecycle":"on_hold","terminals":[{"name":"1","kind":"magic","side":"inside"}]}]}"#;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.json");
    std::fs::write(&path, json).unwrap();
    let part = Catalog::load(&path).unwrap().parts()[0].clone();
    assert_eq!(part.lifecycle, Lifecycle::Unknown);
    assert_eq!(part.terminals[0].kind, ConnectionKind::Unknown);
}

#[test]
fn search_finds_by_classification_code_and_function() {
    let mut catalog = Catalog::new();
    catalog.upsert(rich());
    assert_eq!(catalog.search("ec000897", 5).len(), 1);
    assert_eq!(catalog.search("27-14-11-01", 5).len(), 1);
    assert_eq!(catalog.search("terminal", 5).len(), 1);
    assert!(catalog.search("ec000001", 5).is_empty());
}

fn big(n: usize) -> Catalog {
    (0..n)
        .map(|i| {
            let maker = ["Siemens", "Phoenix Contact", "WAGO", "Schneider Electric"][i % 4];
            CatalogPart {
                manufacturer: maker.into(),
                order_number: format!("{}-{:06}", 1000 + i % 90, i),
                description: format!("Terminal block {} mm2 variant {}", 1 + i % 16, i),
                keywords: vec!["terminal".into()],
                ..Default::default()
            }
        })
        .collect()
}

#[test]
fn a_hundred_thousand_parts_build_and_search_quickly() {
    let t = Instant::now();
    let catalog = big(100_000);
    let build = t.elapsed();
    assert_eq!(catalog.len(), 100_000);

    let t = Instant::now();
    let by_order = catalog.search("1002-050042", 20);
    let by_words = catalog.search("terminal block 16 mm2", 20);
    let lookup = catalog.find("WAGO", "1002-000002");
    let elapsed = t.elapsed();
    assert_eq!(by_order[0].order_number, "1002-050042");
    assert!(!by_words.is_empty());
    assert!(lookup.is_some());
    // The target is 100 ms for a search in a release build; a debug build is
    // allowed ten times that so `cargo test` stays meaningful.
    let limit_ms = if cfg!(debug_assertions) { 1500 } else { 100 };
    assert!(
        elapsed.as_millis() < limit_ms,
        "two searches and a lookup took {elapsed:?} (build {build:?})"
    );
}

#[test]
fn value_sources_and_let_through_tables_round_trip_and_default_to_unknown() {
    use novo_parts::{DataSource, LetThroughPoint, eng};
    let mut part = CatalogPart::new("Acme", "MCB-C16");
    part.data
        .insert(eng::ICU_KA.into(), novo_core::Value::Float(10.0));
    part.sources
        .insert(eng::ICU_KA.into(), DataSource::MakerDatasheet);
    part.let_through = vec![LetThroughPoint {
        ik_ka: 10.0,
        i2t_a2s: 30_000.0,
        ip_ka: 4.2,
    }];
    let mut catalog = Catalog::new();
    catalog.upsert(part);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.json");
    catalog.save(&path).unwrap();
    let back = Catalog::load(&path).unwrap();
    let part = back.find("Acme", "MCBC16").unwrap();
    assert_eq!(part.source_of(eng::ICU_KA), DataSource::MakerDatasheet);
    assert_eq!(part.source_of(eng::ICS_KA), DataSource::Unknown);
    assert_eq!(part.let_through.len(), 1);
}

#[test]
fn retain_removes_many_parts_in_one_pass_and_keeps_lookups_right() {
    let mut catalog = big(20_000);
    let removed = catalog.retain(|p| p.manufacturer != "WAGO");
    assert_eq!(removed, 5_000);
    assert_eq!(catalog.len(), 15_000);
    assert!(catalog.find("WAGO", "1002-000002").is_none());
    assert!(catalog.find("Siemens", "1000-000000").is_some());
    assert_eq!(catalog.search("1003-000003", 5).len(), 1);
}
