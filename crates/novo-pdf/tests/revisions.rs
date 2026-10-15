//! Revisions in the PDF (docs/DESIGN.md §5, §7): bookmarks of changed pages
//! carry a dot, and a title block prints the revision it is handed.

use std::collections::BTreeMap;

use lopdf::Document as Pdf;
use novo_canvas::model::TitleRevisions;
use novo_core::Document;
use novo_pdf::{PdfOptions, export_pdf};
use novo_tests::motor_starter;

fn parse(bytes: &[u8]) -> Pdf {
    Pdf::load_mem(bytes).expect("the export parses as a PDF")
}

#[test]
fn bookmarks_mark_the_pages_with_changes() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let first = doc.project().pages_in_order()[0].id;
    let options = PdfOptions {
        changed_pages: [first].into(),
        ..PdfOptions::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    let toc = pdf.get_toc().expect("outline");
    let titles: Vec<&str> = toc.toc.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles, vec!["● 1  Main circuit", "2  Control circuit"]);
}

#[test]
fn a_title_block_prints_the_page_and_set_revision() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let first = doc.project().pages_in_order()[0].id;
    let revisions = TitleRevisions {
        page: Some(("B".into(), "2026-10-15".into())),
        project: Some("D".into()),
        ..TitleRevisions::default()
    };
    let options = PdfOptions {
        title_revisions: BTreeMap::from([(first, revisions)]),
        ..PdfOptions::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    let ids: Vec<_> = pdf.get_pages().into_values().collect();
    let page = String::from_utf8_lossy(&pdf.get_page_content(ids[0])).into_owned();
    assert!(page.contains("(B  2026-10-15  (set D)) Tj"), "{page}");
    let other = String::from_utf8_lossy(&pdf.get_page_content(ids[1])).into_owned();
    assert!(
        !other.contains("(set D)"),
        "a page without an entry is unchanged"
    );
}
