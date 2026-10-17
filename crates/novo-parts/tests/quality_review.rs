//! Parts quality: the review queue and its ledger file.

use novo_core::Value;
use novo_parts::{
    Asset, AssetKind, Catalog, CatalogPart, PartTerminal, Provenance, ReviewLedger, Trust,
    review_path_for,
};

fn imported(maker: &str, order: &str, file: &str) -> CatalogPart {
    let mut p = CatalogPart::new(maker, order);
    p.provenance = Some(Provenance {
        source: "import:bmecat".into(),
        file: file.into(),
        ..Default::default()
    });
    p
}

fn authored(order: &str) -> CatalogPart {
    let mut p = CatalogPart::new("Own", order);
    p.provenance = Some(Provenance {
        source: "authored".into(),
        ..Default::default()
    });
    p
}

#[test]
fn imported_parts_are_untrusted_until_reviewed() {
    let ledger = ReviewLedger::new();
    let p = imported("WAGO", "2002-1201", "wago.xml");
    assert_eq!(ledger.trust_of(&p), Trust::Unreviewed);
    assert!(ledger.trust_of(&p).needs_review());
}

#[test]
fn authored_and_unrecorded_parts_are_not_queued() {
    let ledger = ReviewLedger::new();
    assert_eq!(ledger.trust_of(&authored("1")), Trust::Authored);
    assert_eq!(
        ledger.trust_of(&CatalogPart::new("X", "2")),
        Trust::Unrecorded
    );
    let mut c = Catalog::new();
    c.upsert(authored("1"));
    c.upsert(CatalogPart::new("X", "2"));
    assert!(ledger.queue(&c).is_empty());
}

#[test]
fn approval_holds_until_the_data_changes() {
    let mut ledger = ReviewLedger::new();
    let mut p = imported("WAGO", "2002-1201", "wago.xml");
    ledger.approve(&p, "2026-10-17");
    assert_eq!(
        ledger.trust_of(&p),
        Trust::Reviewed {
            date: "2026-10-17".into()
        }
    );
    p.data.insert("rated_current_a".into(), Value::Float(32.0));
    assert_eq!(ledger.trust_of(&p), Trust::ChangedSinceReview);
    assert!(ledger.trust_of(&p).needs_review());
}

#[test]
fn local_additions_do_not_undo_a_review() {
    let mut ledger = ReviewLedger::new();
    let mut p = imported("WAGO", "2002-1201", "wago.xml");
    p.terminals = vec![PartTerminal {
        name: "1".into(),
        ..Default::default()
    }];
    ledger.approve(&p, "2026-10-17");
    p.assets.push(Asset {
        kind: AssetKind::Step,
        path: "Sources/wago/x.stp".into(),
        sha256: String::new(),
    });
    p.terminals[0].position_mm = Some([1.0, 2.0, 3.0]);
    assert!(matches!(ledger.trust_of(&p), Trust::Reviewed { .. }));
}

#[test]
fn a_rejection_is_remembered_with_its_note() {
    let mut ledger = ReviewLedger::new();
    let p = imported("WAGO", "2002-1201", "wago.xml");
    ledger.reject(&p, "2026-10-17", "wrong depth");
    assert_eq!(
        ledger.trust_of(&p),
        Trust::Rejected {
            date: "2026-10-17".into(),
            note: "wrong depth".into()
        }
    );
    assert!(!ledger.trust_of(&p).needs_review());
}

#[test]
fn the_ledger_finds_a_respelled_part_by_key_but_asks_again() {
    let mut ledger = ReviewLedger::new();
    let p = imported("WAGO", "2002-1201", "wago.xml");
    ledger.approve(&p, "2026-10-17");
    let mut same = p.clone();
    same.order_number = "2002 1201".into();
    // Found by its normalised key, but the text differs from what was reviewed.
    assert_eq!(ledger.trust_of(&same), Trust::ChangedSinceReview);
}

#[test]
fn the_queue_lists_thin_parts_first_and_approve_all_clears_it() {
    let mut c = Catalog::new();
    let mut rich = imported("A", "AAAA1", "a.xml");
    rich.price = Some(novo_parts::Price {
        amount: 1.0,
        currency: "EUR".into(),
        per: 1,
        date: "2026-10-01".into(),
    });
    c.upsert(rich);
    c.upsert(imported("B", "BBBB1", "b.xml"));
    c.upsert(authored("CCCC1"));
    let mut ledger = ReviewLedger::new();
    let queue = ledger.queue(&c);
    assert_eq!(queue.len(), 2);
    assert!(queue[0].percent <= queue[1].percent);
    assert_eq!(queue[0].order_number, "BBBB1");
    let keys: Vec<_> = queue.iter().map(|q| q.key.clone()).collect();
    assert_eq!(ledger.approve_all(&c, &keys, "2026-10-17"), 2);
    assert!(ledger.queue(&c).is_empty());
    // A second call has nothing left to approve.
    assert_eq!(ledger.approve_all(&c, &keys, "2026-10-18"), 0);
}

#[test]
fn the_ledger_round_trips_through_its_file_next_to_the_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let path = review_path_for(&dir.path().join("parts.json"));
    assert_eq!(path.file_name().unwrap(), "parts-review.json");
    assert!(ReviewLedger::load(&path).unwrap().is_empty());
    let p = imported("WAGO", "2002-1201", "wago.xml");
    let mut ledger = ReviewLedger::new();
    ledger.approve(&p, "2026-10-17");
    ledger.save(&path).unwrap();
    let back = ReviewLedger::load(&path).unwrap();
    assert_eq!(back, ledger);
    assert!(matches!(back.trust_of(&p), Trust::Reviewed { .. }));
    assert!(!dir.path().join("parts-review.json.tmp").exists());
}

#[test]
fn a_broken_or_newer_file_is_an_error_not_an_empty_ledger() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parts-review.json");
    std::fs::write(&path, "not json").unwrap();
    assert!(ReviewLedger::load(&path).is_err());
    std::fs::write(&path, r#"{"format":99,"entries":{}}"#).unwrap();
    let err = ReviewLedger::load(&path).unwrap_err();
    assert!(err.to_string().contains("newer"), "{err}");
}
