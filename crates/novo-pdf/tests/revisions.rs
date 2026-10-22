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

#[test]
fn an_appendix_adds_pages_after_the_drawing() {
    use novo_pdf::{Appendix, AppendixLine};
    let project = novo_core::Project::new("x");
    let lines = (0..120)
        .map(|i| AppendixLine::plain(format!("Line {i}")))
        .collect();
    let options = PdfOptions {
        appendix: Some(Appendix {
            title: "Design checks".into(),
            lines,
        }),
        ..Default::default()
    };
    let bytes = export_pdf(&project, &options).unwrap();
    let pdf = Pdf::load_mem(&bytes).unwrap();
    // 120 lines at 4.6 mm on A4 with 15 mm margins: 58 a page, three pages.
    assert_eq!(pdf.get_pages().len(), 3);
}

#[test]
fn review_note_markers_print_on_their_page_and_a_second_appendix_follows_the_first() {
    use novo_pdf::{Appendix, AppendixLine, PdfMarker};
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let first = doc.project().pages_in_order()[0].id;
    let options = PdfOptions {
        markers: BTreeMap::from([(
            first,
            vec![
                PdfMarker {
                    label: "N7".into(),
                    at: (60.0, 40.0),
                    resolved: false,
                },
                PdfMarker {
                    label: "N8".into(),
                    at: (60.0, 60.0),
                    resolved: true,
                },
            ],
        )]),
        appendix: Some(Appendix {
            title: "Design checks".into(),
            lines: vec![AppendixLine::plain("a problem")],
        }),
        extra_appendices: vec![Appendix {
            title: "Review notes".into(),
            lines: vec![AppendixLine::bold("1 note: 1 open, 0 resolved")],
        }],
        ..Default::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    let ids: Vec<_> = pdf.get_pages().into_values().collect();
    assert_eq!(
        ids.len(),
        4,
        "two drawing pages, then the checks, then the notes"
    );
    let page = |i: usize| String::from_utf8_lossy(&pdf.get_page_content(ids[i])).into_owned();
    assert!(
        page(0).contains("(N7) Tj") && page(0).contains("(N8) Tj"),
        "{}",
        page(0)
    );
    assert!(!page(1).contains("(N7)"), "markers stay on their own page");
    assert!(page(3).contains("1 note: 1 open"), "{}", page(3));
    let toc = pdf.get_toc().expect("outline");
    let titles: Vec<&str> = toc.toc.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(titles.last(), Some(&"Review notes"));
}
