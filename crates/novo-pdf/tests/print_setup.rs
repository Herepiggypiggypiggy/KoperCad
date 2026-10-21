//! Print setup in the PDF: paper, scale, margins, page numbers and the way a
//! sheet is cut off when it does not fit.

use lopdf::{Document as Pdf, Object};
use novo_core::Document;
use novo_pdf::print::{self, Orientation, PrintPaper, PrintScale, PrintSetup};
use novo_pdf::{PageSet, PdfOptions, export_pdf};
use novo_tests::motor_starter;

fn doc() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    doc
}

fn parse(bytes: &[u8]) -> Pdf {
    Pdf::load_mem(bytes).expect("the export parses as a PDF")
}

fn media_mm(pdf: &Pdf, n: u32) -> (f32, f32) {
    let id = pdf.get_pages()[&n];
    let page = pdf.get_dictionary(id).unwrap();
    let b = page.get(b"MediaBox").unwrap().as_array().unwrap();
    let num = |i: usize| match &b[i] {
        Object::Real(r) => *r,
        Object::Integer(i) => *i as f32,
        o => panic!("{o:?}"),
    };
    (num(2) * 25.4 / 72.0, num(3) * 25.4 / 72.0)
}

fn options(print: Option<PrintSetup>) -> PdfOptions {
    PdfOptions {
        print,
        ..PdfOptions::default()
    }
}

#[test]
fn without_a_setup_the_output_is_unchanged() {
    let d = doc();
    let plain = export_pdf(d.project(), &options(None)).unwrap();
    let as_sheet = export_pdf(
        d.project(),
        &options(Some(PrintSetup {
            paper: PrintPaper::AsSheet,
            margin_mm: 0.0,
            scale: PrintScale::Percent(100.0),
            page_numbers: false,
            ..PrintSetup::default()
        })),
    )
    .unwrap();
    let a = parse(&plain);
    let b = parse(&as_sheet);
    assert_eq!(a.get_pages().len(), b.get_pages().len());
    assert_eq!(media_mm(&a, 1), media_mm(&b, 1));
}

#[test]
fn every_sheet_goes_on_the_chosen_paper() {
    let d = doc();
    let setup = PrintSetup {
        paper: PrintPaper::A4,
        ..PrintSetup::default()
    };
    let bytes = export_pdf(d.project(), &options(Some(setup))).unwrap();
    let pdf = parse(&bytes);
    let pages = pdf.get_pages().len();
    assert!(pages >= 1);
    for n in 1..=pages as u32 {
        let (w, h) = media_mm(&pdf, n);
        let ok = |a: f32, b: f32| (w - a).abs() < 0.1 && (h - b).abs() < 0.1;
        assert!(ok(297.0, 210.0) || ok(210.0, 297.0), "page {n}: {w} x {h}");
    }
}

#[test]
fn page_numbers_count_the_printed_pages_and_a_scale_is_named() {
    let d = doc();
    let setup = PrintSetup {
        paper: PrintPaper::A4,
        ..PrintSetup::default()
    };
    let bytes = export_pdf(d.project(), &options(Some(setup))).unwrap();
    let pdf = parse(&bytes);
    let n = pdf.get_pages().len() as u32;
    let first = pdf.extract_text(&[1]).unwrap();
    assert!(first.contains(&format!("Page 1 of {n}")), "{first}");
    assert!(
        first.contains("Printed at"),
        "scaled sheets say so: {first}"
    );
    let last = pdf.extract_text(&[n]).unwrap();
    assert!(last.contains(&format!("Page {n} of {n}")), "{last}");

    let none = PrintSetup {
        paper: PrintPaper::A4,
        page_numbers: false,
        ..PrintSetup::default()
    };
    let bytes = export_pdf(d.project(), &options(Some(none))).unwrap();
    let text = parse(&bytes).extract_text(&[1]).unwrap();
    assert!(!text.contains("Page 1 of"), "{text}");
}

#[test]
fn a_page_selection_is_numbered_among_the_selected_pages() {
    let d = doc();
    let first = d.project().pages_in_order()[0].id;
    let mut o = options(Some(PrintSetup::default()));
    o.pages = PageSet::Pages(vec![first]);
    let pdf = parse(&export_pdf(d.project(), &o).unwrap());
    assert_eq!(pdf.get_pages().len(), 1);
    assert!(pdf.extract_text(&[1]).unwrap().contains("Page 1 of 1"));
}

#[test]
fn a_sheet_that_does_not_fit_is_clipped_to_the_printable_box() {
    let d = doc();
    let setup = PrintSetup {
        paper: PrintPaper::A4,
        scale: PrintScale::Percent(100.0),
        orientation: Orientation::Landscape,
        ..PrintSetup::default()
    };
    let bytes = export_pdf(d.project(), &options(Some(setup.clone()))).unwrap();
    let pdf = parse(&bytes);
    let id = pdf.get_pages()[&1];
    let content = String::from_utf8_lossy(&pdf.get_page_content(id)).into_owned();
    assert!(content.contains("W n") || content.contains("W\nn"), "clip");
    let sheet = d.project().pages_in_order()[0].format.size_mm();
    let p = print::place((sheet.0 as f32, sheet.1 as f32), &setup);
    assert!(p.cut_off, "an A3 sheet at 100 % on A4 is cut off");
}

#[test]
fn the_pdf_is_the_same_every_time() {
    let d = doc();
    let o = options(Some(PrintSetup::default()));
    assert_eq!(
        export_pdf(d.project(), &o).unwrap(),
        export_pdf(d.project(), &o).unwrap()
    );
}
