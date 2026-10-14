//! Live reports: change tracking, never-stale output, and the WS-13
//! done-criterion "reports regenerate within a second of any edit".

use std::time::{Duration, Instant};

use novo_core::{Device, Document, Page, Wire};
use novo_reports::{LiveReports, PageSetup, layout_page, paginate};
use novo_tests::{motor_starter, stress_page};

#[test]
fn edits_regenerate_only_affected_reports() {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture");
    let mut reports = LiveReports::standard();
    reports.all(doc.project());

    // A wire edit leaves the table of contents and bill of materials alone.
    let wire = doc
        .project()
        .wires_at(novo_core::Endpoint::pin(h.k1_coil, 0))
        .next()
        .expect("coil wire");
    let (_, changes) = doc
        .transact("Number", |tx| {
            tx.update::<Wire>(wire, |w| w.number = Some("1".into()))
        })
        .expect("edit");
    let mut stale = reports.notify(doc.project(), &changes);
    stale.sort_unstable();
    assert_eq!(stale, ["wires"]);
    for id in ["toc", "bom", "devices"] {
        assert!(reports.is_current(doc.project(), id), "{id}");
    }
    assert!(!reports.is_current(doc.project(), "wires"));
    let wires = reports.get(doc.project(), "wires").expect("wires");
    assert_eq!(wires.revision, doc.project().revision());
    assert!(wires.sections[0].text_rows().iter().any(|r| r[0] == "1"));

    // A page title changes the table of contents (and the lists that print
    // page.column), not the bill of materials.
    let (_, changes) = doc
        .transact("Title", |tx| {
            tx.update::<Page>(h.main_page, |p| p.title = "Power".into())
        })
        .expect("edit");
    let mut stale = reports.notify(doc.project(), &changes);
    stale.sort_unstable();
    assert_eq!(stale, ["devices", "toc", "wires"]);
    assert!(reports.is_current(doc.project(), "bom"));

    // Undo is just another change set.
    let changes = doc.undo().expect("undo").expect("something to undo");
    reports.notify(doc.project(), &changes);
    let toc = reports.get(doc.project(), "toc").expect("toc");
    assert_eq!(toc.sections[0].text_rows()[0][1], "Main circuit");
}

#[test]
fn a_missed_notification_never_leaves_a_stale_report() {
    let mut doc = Document::default();
    let h = motor_starter(&mut doc).expect("fixture");
    let mut reports = LiveReports::standard();
    reports.all(doc.project());

    // Edit 1 renames -M1 but nobody tells the cache.
    doc.transact("Rename", |tx| {
        tx.update::<Device>(h.m1, |d| d.description = "Fan".into())
    })
    .expect("edit");
    // Edit 2 touches only wires and is notified.
    let wire = doc.project().wires().next().expect("wire").id;
    let (_, changes) = doc
        .transact("Colour", |tx| {
            tx.update::<Wire>(wire, |w| w.color = Some("BK".into()))
        })
        .expect("edit");
    reports.notify(doc.project(), &changes);

    let devices = reports.get(doc.project(), "devices").expect("devices");
    assert!(
        devices.sections[0]
            .text_rows()
            .iter()
            .any(|r| r[3] == "Fan")
    );
}

#[test]
fn reports_regenerate_within_a_second_on_10k_objects() {
    let mut doc = Document::default();
    stress_page(&mut doc, 10_000).expect("stress page");
    let objects = doc.project().devices().count()
        + doc.project().symbols().count()
        + doc.project().wires().count();
    assert!(objects >= 10_000);
    let mut reports = LiveReports::standard();
    let setup = PageSetup::default();
    reports.all(doc.project());

    // Worst case: an edit every report reads (device, symbol and wire kinds
    // plus a page), then regenerate, page and lay out everything.
    let device = doc.project().devices().next().expect("device").id;
    let wire = doc.project().wires().next().expect("wire").id;
    let page = doc.project().pages().next().expect("page").id;
    let (_, changes) = doc
        .transact("Edit", |tx| {
            tx.update::<Device>(device, |d| d.description = "Edited".into())?;
            tx.update::<Wire>(wire, |w| w.number = Some("100".into()))?;
            tx.update::<Page>(page, |p| p.title = "Stress".into())
        })
        .expect("edit");

    let start = Instant::now();
    let stale = reports.notify(doc.project(), &changes).len();
    assert_eq!(stale, 4);
    let mut pages = 0;
    for output in reports.all(doc.project()) {
        let paged = paginate(output, &setup);
        for page in &paged {
            layout_page(output, page, &setup);
        }
        pages += paged.len();
    }
    let elapsed = start.elapsed();
    eprintln!("regenerated 4 reports ({pages} pages) on {objects} objects in {elapsed:?}");
    assert!(
        elapsed < Duration::from_secs(1),
        "regeneration took {elapsed:?}"
    );

    // An edit no report reads costs nothing.
    let start = Instant::now();
    let (_, changes) = doc
        .transact("Text", |tx| {
            let page = tx.project().pages().next().expect("page").id;
            tx.add_text(
                page,
                novo_core::TextContent::Literal {
                    text: "note".into(),
                },
                novo_core::TextPlacement::default(),
                None,
            )
        })
        .expect("edit");
    assert!(reports.notify(doc.project(), &changes).is_empty());
    reports.all(doc.project());
    assert!(start.elapsed() < Duration::from_millis(100));
}
