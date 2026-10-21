//! The single-line diagram prints as drawn sheets in the project PDF, and as
//! drawing plus schedule in the report PDF.

use lopdf::Document as Pdf;
use novo_core::{Document, Page, PageKind};
use novo_pdf::{PdfOptions, export_pdf};
use novo_reports::single_line::{self, SingleLineDiagram};
use novo_reports::{PageSetup, Report};
use novo_tests::{BigSpec, big_project, distribution_board};

fn with_page(doc: &mut Document) {
    doc.transact("Single-line page", |tx| {
        let page = tx.add_page("30")?;
        tx.update::<Page>(page, |p| {
            p.title = "Single-line diagram".into();
            p.kind = PageKind::Generated {
                generator: SingleLineDiagram::ID.into(),
            };
        })
    })
    .unwrap();
}

fn content(pdf: &Pdf, n: u32) -> String {
    let id = pdf.get_pages()[&n];
    String::from_utf8_lossy(&pdf.get_page_content(id)).into_owned()
}

#[test]
fn the_page_prints_the_drawn_diagram_in_the_page_frame() {
    let mut doc = Document::default();
    distribution_board(&mut doc).unwrap();
    with_page(&mut doc);
    let bytes = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let pdf = Pdf::load_mem(&bytes).expect("a PDF");
    // The board has one drawn page (a page of its own), plus the generated one.
    let last = pdf.get_pages().len() as u32;
    let text = content(&pdf, last);
    // Boxes are drawn as lines, so the page has many strokes and the
    // figures as text operators.
    assert!(
        text.matches(" l\n").count() + text.matches(" l ").count() > 50,
        "few strokes"
    );
    for needle in ["Single-line diagram", "Breaker", "Motor", "mm"] {
        // Text in a standard font is written as hex or literal strings;
        // compare on the decoded content stream.
        assert!(
            text.contains(needle) || text.contains(&hex(needle)),
            "{needle} not on the page"
        );
    }
}

fn hex(text: &str) -> String {
    text.bytes().map(|b| format!("{b:02X}")).collect()
}

#[test]
fn many_feeders_print_one_pdf_page_per_drawn_sheet() {
    let mut doc = Document::default();
    big_project(&mut doc, BigSpec::new(20)).unwrap();
    let before = {
        let bytes = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
        Pdf::load_mem(&bytes).unwrap().get_pages().len()
    };
    with_page(&mut doc);
    let sheets = single_line::sheet_count(&single_line::build(doc.project()));
    assert!(sheets >= 2);
    let bytes = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let pdf = Pdf::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), before + sheets);
}

#[test]
fn the_report_pdf_of_the_schedule_is_a_table_like_the_others() {
    let mut doc = Document::default();
    distribution_board(&mut doc).unwrap();
    let out = SingleLineDiagram.generate(doc.project());
    let bytes = novo_pdf::export_report_pdf(&[&out], &novo_pdf::ReportPdfOptions::default());
    let pdf = Pdf::load_mem(&bytes).unwrap();
    assert_eq!(pdf.get_pages().len(), 1);
    // And the drawing for the same project exists as layouts for export_layouts_pdf.
    let layouts = single_line::layouts(&single_line::build(doc.project()), &PageSetup::default());
    let drawn =
        novo_pdf::export_layouts_pdf(&layouts, "Board", &[(0, "Single-line diagram".into())]);
    assert_eq!(
        Pdf::load_mem(&drawn).unwrap().get_pages().len(),
        layouts.len()
    );
}
