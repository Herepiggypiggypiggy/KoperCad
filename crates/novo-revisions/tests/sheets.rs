use novo_core::*;
use novo_reports::{add_generated_pages, default_generated_pages};
use novo_revisions::{SheetChangeKind, changed_sheets};
use novo_tests::motor_starter;

fn project_with_reports() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let specs = default_generated_pages(doc.project());
    doc.transact("Generated pages", |tx| add_generated_pages(tx, &specs))
        .expect("pages");
    doc
}

#[test]
fn only_sheets_whose_content_changed_are_marked() {
    let mut doc = project_with_reports();
    let issued = doc.project().clone();
    assert!(changed_sheets(&issued, doc.project()).is_empty());

    // Retitling a drawn page changes the contents list, which shows page
    // titles, and nothing else.
    let page = doc
        .project()
        .pages()
        .find(|p| p.kind == PageKind::Schematic)
        .expect("a drawn page")
        .id;
    doc.transact("Retitle", |tx| {
        tx.update::<Page>(page, |p| p.title = "Motor feeder".into())
    })
    .unwrap();
    let changes = changed_sheets(&issued, doc.project());
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(changes[0].report, "toc");

    // A new device shows up in the device list, not in every sheet.
    doc.transact("Add device", |tx| {
        tx.add_device("=A1-Q99".parse().unwrap(), "")
    })
    .unwrap();
    let changes = changed_sheets(&issued, doc.project());
    assert!(!changes.is_empty());
    assert!(changes.iter().any(|c| c.report == "devices"), "{changes:?}");
    assert!(changes.iter().all(|c| c.report != "cables"), "{changes:?}");
}

#[test]
fn deleted_and_added_report_pages_are_marked() {
    let mut doc = project_with_reports();
    let issued = doc.project().clone();
    let bom = doc
        .project()
        .pages()
        .find(|p| matches!(&p.kind, PageKind::Generated { generator } if generator == "bom"))
        .expect("bom page")
        .id;
    doc.execute(Command::DeletePage { page: bom }).unwrap();

    // The deleted page's sheet is gone; the contents list no longer names it.
    let marks = |a: &Project, b: &Project, report: &str| {
        changed_sheets(a, b)
            .into_iter()
            .filter(|c| c.report == report)
            .map(|c| c.kind)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        marks(&issued, doc.project(), "bom"),
        [SheetChangeKind::Removed]
    );
    assert_eq!(
        marks(&issued, doc.project(), "toc"),
        [SheetChangeKind::Changed]
    );

    // Seen the other way round, the same sheet is new.
    assert_eq!(
        marks(doc.project(), &issued, "bom"),
        [SheetChangeKind::Added]
    );
}
