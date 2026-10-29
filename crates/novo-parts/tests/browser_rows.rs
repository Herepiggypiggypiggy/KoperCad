//! The Parts window's rows come from one read of the library, however many
//! parts the catalog holds (PIR-03: one read per part cost 30 us a part a frame).

use novo_parts::*;

#[test]
fn five_thousand_rows_read_the_library_once_and_still_mark_stored_parts() {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    let mut catalog = Catalog::new();
    for n in 0..5000 {
        catalog.upsert(CatalogPart::new("Acme", format!("BIG-{n:05}")));
    }
    for order in ["BIG-00007", "BIG-04999"] {
        lib.store(
            &NewSource {
                manufacturer: "Acme",
                order_number: order,
                file_name: "x.stp",
                url: None,
                flow: ImportFlow::Manual,
                imported: "2026-10-28",
                terms_url: None,
            },
            b"x",
        )
        .unwrap();
    }
    let sources = SourceTable::builtin();
    let t = std::time::Instant::now();
    let rows = browser_rows(&catalog, &lib, &sources, "");
    let took = t.elapsed();
    assert_eq!(rows.len(), 5000);
    let stored: Vec<_> = rows
        .iter()
        .filter(|r| r.action == RowAction::InLibrary)
        .map(|r| r.part.order_number.as_str())
        .collect();
    assert_eq!(stored, ["BIG-00007", "BIG-04999"]);
    assert!(took.as_millis() < 500, "{took:?}");
}

#[test]
fn the_catalog_revision_changes_with_every_edit() {
    let mut catalog = Catalog::new();
    let r0 = catalog.revision();
    catalog.upsert(CatalogPart::new("Acme", "A-1"));
    let r1 = catalog.revision();
    catalog.upsert(CatalogPart::new("Acme", "A-1"));
    let r2 = catalog.revision();
    catalog.retain(|_| true);
    let r3 = catalog.revision();
    catalog.remove(&CatalogPart::new("Acme", "A-1").key());
    let r4 = catalog.revision();
    let all = [r0, r1, r2, r3, r4];
    for (i, a) in all.iter().enumerate() {
        for b in &all[i + 1..] {
            assert_ne!(a, b, "{all:?}");
        }
    }
}
