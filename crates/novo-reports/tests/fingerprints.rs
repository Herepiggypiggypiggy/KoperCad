//! Per-sheet content fingerprints, for revisions to mark changed sheets.

use novo_core::{DeviceTag, Document, Page, PageKind};
use novo_reports::{LiveReports, SheetFingerprint};

/// A device list on page "20" long enough for three sheets.
fn project(devices: usize) -> Document {
    let mut doc = Document::default();
    doc.transact("Pages", |tx| {
        let page = tx.add_page("20")?;
        tx.update::<Page>(page, |p| {
            p.title = "Devices".into();
            p.kind = PageKind::Generated {
                generator: "devices".into(),
            };
        })?;
        for i in 1..=devices {
            tx.add_device(DeviceTag::parse(&format!("=A9-K{i}"))?, "Relay")?;
        }
        Ok(())
    })
    .expect("pages");
    doc
}

fn prints(doc: &Document) -> Vec<SheetFingerprint> {
    LiveReports::standard().sheet_fingerprints(doc.project())
}

#[test]
fn every_sheet_has_a_fingerprint_and_equal_content_gives_equal_ones() {
    let doc = project(80);
    let a = prints(&doc);
    assert_eq!(
        a.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["20a", "20b", "20c"]
    );
    assert!(a.iter().all(|f| f.report == "devices"));
    assert_eq!(a, prints(&doc), "deterministic");
    let hashes: std::collections::BTreeSet<u64> = a.iter().map(|f| f.hash).collect();
    assert_eq!(hashes.len(), 3, "different rows, different hashes");
}

#[test]
fn only_the_sheets_whose_content_changed_differ() {
    let mut doc = project(80);
    let before = prints(&doc);
    // One more device at the end: only the last sheet gets a row.
    doc.transact("More", |tx| {
        tx.add_device(DeviceTag::parse("=A9-K81")?, "Relay")
    })
    .expect("device");
    let after = prints(&doc);
    assert_eq!(after.len(), before.len());
    assert_eq!(before[0].hash, after[0].hash);
    assert_eq!(before[1].hash, after[1].hash);
    assert_ne!(before[2].hash, after[2].hash);
}

#[test]
fn renaming_the_page_does_not_change_the_content_hash() {
    let mut doc = project(80);
    let before = prints(&doc);
    let page = before[0].page;
    doc.transact("Rename", |tx| {
        tx.update::<Page>(page, |p| p.name = "30".into())
    })
    .expect("rename");
    let after = prints(&doc);
    assert_eq!(
        after.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["30a", "30b", "30c"]
    );
    assert_eq!(
        before.iter().map(|f| f.hash).collect::<Vec<_>>(),
        after.iter().map(|f| f.hash).collect::<Vec<_>>()
    );
}
