//! The stand-alone report PDF: pages, footer, bookmarks, sign-off block.

use lopdf::Document as Pdf;
use novo_pdf::{ReportPdfOptions, export_report_pdf};
use novo_reports::commissioning::*;

fn input(checks: usize) -> CommissioningInput {
    CommissioningInput {
        project_name: "Press line".into(),
        checks: (0..checks as u64)
            .map(|i| CheckRow {
                id: i,
                page: format!("{}", i % 7 + 1),
                cabinet: "+CAB1".into(),
                tag: format!("-K{i}"),
                what: "Coil pulls in".into(),
                state: CheckState::Passed,
                ..CheckRow::default()
            })
            .collect(),
        sign_offs: vec![
            SignOff {
                role: "Commissioning engineer".into(),
                ..SignOff::default()
            },
            SignOff {
                role: "Customer".into(),
                ..SignOff::default()
            },
        ],
        ..CommissioningInput::default()
    }
}

fn options() -> ReportPdfOptions {
    ReportPdfOptions {
        title: "Press line, commissioning".into(),
        footer_right: "2026-10-15".into(),
        ..ReportPdfOptions::default()
    }
}

fn page_texts(pdf: &Pdf) -> Vec<String> {
    pdf.get_pages()
        .keys()
        .map(|n| pdf.extract_text(&[*n]).unwrap_or_default())
        .collect()
}

#[test]
fn long_check_list_pages_and_prints_footer() {
    let report = commissioning_report(&input(120));
    let bytes = export_report_pdf(&[&report], &options());
    let pdf = Pdf::load_mem(&bytes).expect("parses");
    let texts = page_texts(&pdf);
    // summary, cabinets, pages, 120 checks over several pages, sign-off.
    assert!(texts.len() >= 7, "{} pages", texts.len());
    let last = texts.len();
    assert!(
        texts
            .iter()
            .all(|t| t.contains("Press line, commissioning")),
        "footer on every page"
    );
    assert!(texts[last - 1].contains(&format!("page {last} / {last}")));
    assert!(texts[last - 1].contains("Customer"));
    assert!(texts[0].contains("Ready for sign-off"));
}

#[test]
fn deterministic_and_never_empty() {
    let report = commissioning_report(&input(5));
    let a = export_report_pdf(&[&report], &options());
    let b = export_report_pdf(&[&report], &options());
    assert_eq!(a, b);
    let blank = export_report_pdf(&[], &options());
    assert_eq!(Pdf::load_mem(&blank).expect("parses").get_pages().len(), 1);
}

#[test]
fn every_section_has_a_bookmark() {
    let report = commissioning_report(&input(5));
    let bytes = export_report_pdf(&[&report], &options());
    let pdf = Pdf::load_mem(&bytes).expect("parses");
    let marks = pdf.get_toc().expect("outline").toc;
    assert_eq!(marks.len(), report.sections.len());
    assert!(marks.iter().any(|m| m.title.contains("Sign-off")));
}

#[test]
fn cells_starting_with_page_are_printed() {
    let mut i = input(1);
    i.checks[0].what = "page 3 wiring".into();
    i.checks[0].note = "page 9 see drawing".into();
    let report = commissioning_report(&i);
    let bytes = export_report_pdf(&[&report], &options());
    let pdf = Pdf::load_mem(&bytes).expect("parses");
    let all = page_texts(&pdf).join("\n");
    assert!(all.contains("page 3 wiring"), "{all}");
    assert!(all.contains("page 9 see drawing"));
}
