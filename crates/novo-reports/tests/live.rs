//! Live reports: change tracking, never-stale output, and the WS-13
//! done-criterion "reports regenerate within a second of any edit".

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use novo_core::{Device, Document, EntityKind, Page, PageKind, Project, Wire};
use novo_reports::{
    Cell, Column, Dependencies, LiveReports, PageSetup, Report, ReportOutput, Section,
    TableOfContents, layout_page, paginate,
};
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
    // The diagrams read wires too, and the contents list follows the
    // sheets of every report.
    assert_eq!(
        stale,
        [
            "cables",
            "cableschedule",
            "io",
            "plugs",
            "terminals",
            "toc",
            "wires"
        ]
    );
    for id in ["bom", "devices"] {
        assert!(reports.is_current(doc.project(), id), "{id}");
        // Rolled forward, so the output itself says it is current.
        let output = reports.get(doc.project(), id).expect("built in");
        assert_eq!(output.revision, doc.project().revision(), "{id}");
    }
    assert!(!reports.is_current(doc.project(), "wires"));
    let wires = reports.get(doc.project(), "wires").expect("wires");
    assert_eq!(wires.revision, doc.project().revision());
    assert!(wires.sections[0].text_rows().iter().any(|r| r[0] == "1"));

    reports.all(doc.project());
    // A page title changes the table of contents (and the lists that print
    // page.column), not the bill of materials.
    let (_, changes) = doc
        .transact("Title", |tx| {
            tx.update::<Page>(h.main_page, |p| p.title = "Power".into())
        })
        .expect("edit");
    let mut stale = reports.notify(doc.project(), &changes);
    stale.sort_unstable();
    assert_eq!(
        stale,
        [
            "cables",
            "cableschedule",
            "devices",
            "io",
            "plugs",
            "terminals",
            "toc",
            "wires"
        ]
    );
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
    // A page for every report, so the contents list counts their sheets.
    doc.transact("Report pages", |tx| {
        for (i, generator) in ["bom", "devices", "wires", "terminals", "cables"]
            .into_iter()
            .enumerate()
        {
            let page = tx.add_page((90 + i).to_string())?;
            tx.update::<Page>(page, |p| {
                p.kind = PageKind::Generated {
                    generator: generator.into(),
                }
            })?;
        }
        Ok(())
    })
    .expect("report pages");
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

    // The done-criterion is one second; a debug build (what `cargo test`
    // runs, also on slow CI runners) gets slack, as in novo-canvas's perf
    // test. Release builds must meet the second itself.
    let slack = if cfg!(debug_assertions) { 3 } else { 1 };
    let start = Instant::now();
    let stale = reports.notify(doc.project(), &changes).len();
    assert_eq!(stale, 9);
    let mut pages = 0;
    for output in reports.all(doc.project()) {
        let paged = paginate(output, &setup);
        for page in &paged {
            layout_page(output, page, &setup);
        }
        pages += paged.len();
    }
    let elapsed = start.elapsed();
    eprintln!("regenerated {stale} reports ({pages} pages) on {objects} objects in {elapsed:?}");
    assert!(
        elapsed < Duration::from_secs(1) * slack,
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
    assert!(start.elapsed() < Duration::from_millis(100) * slack);
}

/// A report that counts how often it is generated, with `rows` rows.
struct Counted {
    runs: Arc<AtomicUsize>,
    rows: usize,
}

impl Report for Counted {
    fn id(&self) -> &str {
        "counted"
    }
    fn title(&self) -> String {
        "Counted".into()
    }
    fn dependencies(&self) -> Dependencies {
        Dependencies::on(&[EntityKind::Device])
    }
    fn generate(&self, project: &Project) -> ReportOutput {
        self.runs.fetch_add(1, Ordering::SeqCst);
        let mut section = Section::new("rows", "Rows", vec![Column::new("n", "N", 20.0)]);
        for n in 0..self.rows {
            section.push(vec![Cell::text(n.to_string())], Vec::new());
        }
        ReportOutput {
            report: "counted".into(),
            title: self.title(),
            revision: project.revision(),
            sections: vec![section],
        }
    }
}

#[test]
fn the_contents_list_reuses_the_cached_reports() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    doc.transact("Pages", |tx| {
        for (name, generator) in [("20", "counted"), ("21", "counted"), ("30", "toc")] {
            let page = tx.add_page(name)?;
            tx.update::<Page>(page, |p| {
                p.kind = PageKind::Generated {
                    generator: generator.into(),
                }
            })?;
        }
        Ok(())
    })
    .expect("pages");
    let runs = Arc::new(AtomicUsize::new(0));
    let mut reports = LiveReports::standard();
    reports.register(Box::new(Counted {
        runs: runs.clone(),
        rows: 200,
    }));

    let toc = reports.get(doc.project(), "toc").expect("toc").clone();
    // Built once for two pages, and the page itself then reuses it.
    assert_eq!(runs.load(Ordering::SeqCst), 1);
    reports.get(doc.project(), "counted");
    reports.all(doc.project());
    assert_eq!(runs.load(Ordering::SeqCst), 1);

    // A registered report the standard set does not know is listed per
    // sheet too; 200 rows take more than one sheet.
    let rows = toc.sections[0].text_rows();
    let counted: Vec<&str> = rows
        .iter()
        .filter(|r| r[4] == "Generated: counted")
        .map(|r| r[0].as_str())
        .collect();
    assert!(counted.len() >= 4, "{counted:?}");
    assert!(
        counted.contains(&"20a") && counted.contains(&"21b"),
        "{counted:?}"
    );
    // Otherwise the cached path and the standalone one agree.
    let standalone = TableOfContents.generate(doc.project());
    let without = |o: &ReportOutput| -> Vec<Vec<String>> {
        o.sections[0]
            .text_rows()
            .into_iter()
            .filter(|r| r[4] != "Generated: counted")
            .collect()
    };
    assert_eq!(without(&toc), without(&standalone));
}

#[test]
fn a_long_contents_list_reflows_onto_lettered_sheets() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let toc = doc
        .transact("Pages", |tx| {
            let toc = tx.add_page("0")?;
            tx.update::<Page>(toc, |p| {
                p.kind = PageKind::Generated {
                    generator: "toc".into(),
                }
            })?;
            for i in 100..300 {
                tx.add_page(i.to_string())?;
            }
            Ok(toc)
        })
        .expect("pages")
        .0;
    let mut reports = LiveReports::standard();
    let sheets = reports.sheets_of_page(doc.project(), toc);
    assert!(sheets.len() >= 2, "{} sheets", sheets.len());
    let names: Vec<&str> = sheets.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(&names[..2], ["0a", "0b"]);
    // Every page is listed exactly once across the sheets.
    let output = reports.get(doc.project(), "toc").expect("toc");
    let rows = output.sections[0].text_rows();
    assert_eq!(rows.iter().filter(|r| r[0] == "299").count(), 1);
}
