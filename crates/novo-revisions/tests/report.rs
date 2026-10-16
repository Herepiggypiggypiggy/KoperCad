use novo_core::*;
use novo_reports::Cell;
use novo_revisions::{IndexRule, Side, change_report, compare, issue_revision};
use novo_storage::Session;

fn pins() -> Vec<Pin> {
    vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)]
}

#[test]
fn the_change_report_has_a_table_per_kind_of_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::default();
    let ((page, k1, lamp), _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            let k1 = tx.add_device(DeviceTag::parse("-K1").unwrap(), "Relay")?;
            let role = SymbolRole::Device {
                device: k1,
                main: true,
            };
            tx.place_symbol(page, "coil", Placement::default(), pins(), role)?;
            let lamp = tx.place_symbol(
                page,
                "lamp",
                Placement::at(Point::new(40.0, 0.0)),
                pins(),
                SymbolRole::Unassigned,
            )?;
            Ok((page, k1, lamp))
        })
        .unwrap();
    let mut session = Session::create(dir.path().join("p.novo"), doc.project()).unwrap();
    let a = issue_revision(
        &mut doc,
        &mut session,
        IndexRule::Letters,
        "2026-10-15",
        "TV",
        "First",
    )
    .unwrap()
    .revision;
    doc.transact("Edit", |tx| {
        tx.update::<Device>(k1, |d| d.description = "Pump relay".into())?;
        tx.move_symbol(lamp, Placement::at(Point::new(80.0, 10.0)))?;
        tx.add_part("Siemens", "3RT2015")?;
        Ok(())
    })
    .unwrap();

    let comparison = compare(
        doc.project(),
        |id| session.snapshot(id),
        Side::Revision(a),
        Side::Now,
    )
    .unwrap();
    let report = change_report(&comparison, "Changes A to now", |p| {
        doc.project()
            .page(p)
            .map(|p| p.name.clone())
            .unwrap_or_default()
    });
    assert_eq!(report.title, "Changes A to now");
    let objects = report.section("changes").expect("a changed objects table");
    let text = objects.text_rows();
    let device = text
        .iter()
        .find(|r| r[2] == "Device -K1")
        .expect("device row");
    assert_eq!(device[0], "1");
    assert_eq!(device[1], "Changed");
    assert_eq!(device[3], "description: Relay → Pump relay");
    let moved = text
        .iter()
        .find(|r| r[2].starts_with("Symbol lamp"))
        .expect("lamp row");
    assert_eq!(moved[3], "position: moved 40 mm right, 10 mm down");
    let part = text.iter().find(|r| r[2].starts_with("Part")).unwrap();
    assert_eq!((part[0].as_str(), part[1].as_str()), ("", "Added"));
    assert!(matches!(
        objects.rows[0].cells[3],
        Cell::Text(_) | Cell::Empty
    ));
    assert_eq!(objects.rows[0].sources.len(), 1);
    // Nothing changed in the project information or the sheets.
    assert!(report.section("info").is_none());
    let _ = page;

    // It prints through the stand-alone report PDF.
    let pdf = novo_pdf::export_report_pdf(
        &[&report],
        &novo_pdf::ReportPdfOptions {
            title: report.title.clone(),
            ..Default::default()
        },
    );
    assert!(pdf.starts_with(b"%PDF"));

    // Comparing a state with itself gives a report with no tables.
    let same = compare(
        doc.project(),
        |id| session.snapshot(id),
        Side::Now,
        Side::Now,
    )
    .unwrap();
    assert!(
        change_report(&same, "No changes", |_| String::new())
            .sections
            .is_empty()
    );
}
