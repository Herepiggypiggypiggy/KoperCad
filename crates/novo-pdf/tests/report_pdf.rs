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

#[test]
fn report_bytes_writes_each_format() {
    use novo_pdf::{ReportFileFormat, report_bytes};
    use std::path::Path;
    let report = commissioning_report(&input(3));
    for name in ["a.pdf", "a.csv", "a.xlsx"] {
        let format = ReportFileFormat::from_path(Path::new(name)).expect("known");
        let bytes = report_bytes(&report, format, &options()).expect("written");
        assert!(!bytes.is_empty(), "{name}");
    }
    assert!(ReportFileFormat::from_path(Path::new("a.txt")).is_none());
    let csv = report_bytes(
        &report,
        ReportFileFormat::from_path(Path::new("a.csv")).unwrap(),
        &options(),
    )
    .unwrap();
    assert!(
        String::from_utf8(csv)
            .unwrap()
            .contains("Progress per cabinet")
    );
}

fn table(key: &str, rows: usize, flow: bool) -> novo_reports::Section {
    use novo_reports::{Cell, Column, Section};
    let mut s = Section::new(key, key, vec![Column::new("n", "No", 10.0)]);
    for i in 0..rows {
        s.push(vec![Cell::Int(i as i64)], Vec::new());
    }
    s.flow = flow;
    s
}

fn pages_of(sections: Vec<novo_reports::Section>) -> usize {
    let report = novo_reports::ReportOutput {
        report: "flow".into(),
        title: "Flow".into(),
        revision: 0,
        sections,
    };
    let bytes = export_report_pdf(&[&report], &ReportPdfOptions::default());
    Pdf::load_mem(&bytes).expect("valid pdf").get_pages().len()
}

#[test]
fn a_flowing_section_shares_the_sheet_of_the_one_before() {
    // Without the flag every section starts a new page.
    assert_eq!(
        pages_of(vec![table("a", 2, false), table("b", 5, false)]),
        2
    );
    // With it a short table follows on the same sheet.
    assert_eq!(pages_of(vec![table("a", 2, false), table("b", 5, true)]), 1);
    // A long one starts there and continues on the next sheet.
    assert_eq!(
        pages_of(vec![table("a", 2, false), table("b", 30, true)]),
        2
    );
    // Too little room left for a few rows: it starts a new page.
    assert_eq!(
        pages_of(vec![table("a", 24, false), table("b", 5, true)]),
        2
    );
}

#[test]
fn the_bench_cut_list_starts_its_first_wire_group_under_the_summary() {
    use novo_reports::{BenchCut, Report};
    let mut doc = novo_core::Document::default();
    novo_tests::motor_starter(&mut doc).expect("fixture");
    let output = BenchCut.generate(doc.project());
    assert!(output.sections.len() >= 2);
    assert!(!output.sections[0].flow);
    assert_eq!(output.sections.iter().filter(|s| s.flow).count(), 1);
    let bytes = export_report_pdf(&[&output], &ReportPdfOptions::default());
    let pages = Pdf::load_mem(&bytes).expect("valid pdf").get_pages().len();
    assert!(pages < output.sections.len(), "{pages} pages");
}

#[test]
fn the_ferrule_table_prints_on_one_sheet_when_it_fits() {
    use novo_reports::{FerruleStandard, FerruleTable, bench_jobs};
    let mut doc = novo_core::Document::default();
    novo_tests::motor_starter(&mut doc).expect("fixture");
    let jobs = bench_jobs(doc.project());
    let table = FerruleTable::build(doc.project(), &jobs, &FerruleStandard::default());
    assert!(table.sections.len() >= 2, "ferrules and the standard");
    let bytes = export_report_pdf(&[&table], &ReportPdfOptions::default());
    let pages = Pdf::load_mem(&bytes).expect("valid pdf").get_pages().len();
    assert_eq!(pages, 1, "{} sections", table.sections.len());
}

fn flow_report(sections: Vec<novo_reports::Section>) -> novo_reports::ReportOutput {
    novo_reports::ReportOutput {
        report: "flow".into(),
        title: "Flow".into(),
        revision: 0,
        sections,
    }
}

#[test]
fn a_flowing_section_keeps_its_bookmark_on_the_shared_sheet() {
    let report = flow_report(vec![table("a", 2, false), table("b", 5, true)]);
    let bytes = export_report_pdf(&[&report], &ReportPdfOptions::default());
    let pdf = Pdf::load_mem(&bytes).expect("valid pdf");
    let marks = pdf.get_toc().expect("outline").toc;
    let titles: Vec<&str> = marks.iter().map(|m| m.title.as_str()).collect();
    assert_eq!(titles, ["Flow · a", "Flow · b"]);
    assert_eq!(marks[0].page, marks[1].page, "one shared sheet");
}

#[test]
fn a_continued_flowing_table_numbers_its_sheets_by_the_document_only() {
    let report = flow_report(vec![table("a", 2, false), table("b", 30, true)]);
    let bytes = export_report_pdf(&[&report], &ReportPdfOptions::default());
    let texts = page_texts(&Pdf::load_mem(&bytes).expect("valid pdf"));
    assert_eq!(texts.len(), 2);
    assert!(texts[1].contains("(continued)"), "{}", texts[1]);
    assert_eq!(texts[1].matches("page ").count(), 1, "{}", texts[1]);
    assert!(texts[1].contains("page 2 / 2"), "{}", texts[1]);
}

#[test]
fn two_outputs_with_the_same_report_id_do_not_flow_into_each_other() {
    let first = flow_report(vec![table("a", 2, false)]);
    let second = flow_report(vec![table("b", 5, true)]);
    let bytes = export_report_pdf(&[&first, &second], &ReportPdfOptions::default());
    let pages = Pdf::load_mem(&bytes).expect("valid pdf").get_pages().len();
    assert_eq!(pages, 2);
}

#[test]
fn a_column_header_too_narrow_for_one_line_runs_onto_two() {
    use novo_core::Document;
    use novo_reports::generate_all;
    let mut doc = Document::default();
    novo_tests::motor_starter(&mut doc).expect("fixture");
    let all = generate_all(doc.project());
    let wires = all.iter().find(|r| r.report == "wires").expect("wire list");
    let bytes = export_report_pdf(&[wires], &ReportPdfOptions::default());
    let pdf = Pdf::load_mem(&bytes).expect("parses");
    let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
    let text = pdf.extract_text(&pages).expect("text");
    assert!(!text.contains("Route lengt..."), "{text}");
    assert!(!text.contains("Length ba..."), "{text}");
    assert!(
        text.contains("Route length") && text.contains("basis"),
        "{text}"
    );
}

#[test]
fn no_report_cell_is_cut_with_an_ellipsis_while_the_page_has_room() {
    let mut doc = novo_core::Document::default();
    novo_tests::motor_starter(&mut doc).expect("fixture");
    let all = novo_reports::generate_all(doc.project());
    let mut cut = Vec::new();
    for report in &all {
        // The single-line table has 18 columns, some with whole sentences: it is
        // wider than the page by design and cuts its longest cells.
        if report.report == "singleline" {
            continue;
        }
        let pdf = export_report_pdf(&[report], &ReportPdfOptions::default());
        let text = {
            let pdf = lopdf::Document::load_mem(&pdf).expect("valid pdf");
            let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
            pdf.extract_text(&pages).expect("text")
        };
        if text.contains("...") {
            let lines: Vec<&str> = text.lines().filter(|l| l.contains("...")).collect();
            cut.push(format!("{}: {lines:?}", report.report));
        }
    }
    assert!(cut.is_empty(), "reports with cut text: {cut:?}");
}
