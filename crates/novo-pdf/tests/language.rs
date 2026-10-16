//! The PDF in another language (DOC-2): titles, headers, pager words and
//! designer title blocks follow the language; data does not.

use lopdf::Document as Pdf;
use novo_core::Document;
use novo_pdf::{ReportPdfOptions, export_layouts_pdf, export_report_pdf};
use novo_reports::designer::{Fields, builtin, preview_data};
use novo_reports::generate_all;
use novo_reports::i18n::{ProjectTable, Translator, render_in};
use novo_tests::motor_starter;

fn nl() -> Translator {
    Translator::new("nl", &ProjectTable::default())
}

fn text_of(bytes: &[u8]) -> String {
    let pdf = Pdf::load_mem(bytes).expect("valid pdf");
    let pages: Vec<u32> = pdf.get_pages().keys().copied().collect();
    pdf.extract_text(&pages).expect("text")
}

#[test]
fn report_pdf_prints_in_dutch() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let all = generate_all(doc.project());
    let devices = all.iter().find(|r| r.report == "devices").expect("devices");
    let english = text_of(&export_report_pdf(&[devices], &ReportPdfOptions::default()));
    let options = ReportPdfOptions {
        language: nl(),
        ..ReportPdfOptions::default()
    };
    let dutch = text_of(&export_report_pdf(&[devices], &options));
    assert!(english.contains("Device list"));
    assert!(dutch.contains("Apparatenlijst"), "{dutch}");
    assert!(!dutch.contains("Device list"));
    assert!(dutch.contains("Omschrijving") || dutch.contains("Naam") || dutch.contains("Functie"));
    // Project data is the same in both.
    assert!(devices.sections[0].rows.iter().all(|r| {
        let tag = r.cells[0].to_string();
        tag.is_empty() || (english.contains(&tag) && dutch.contains(&tag))
    }));
}

#[test]
fn designer_pdf_prints_in_dutch() {
    let t = builtin::report_a4("Device list", "devices");
    let rendered = render_in(&t, &preview_data(&t, 60), &Fields::sample(), &nl());
    let dutch = text_of(&export_layouts_pdf(&rendered.pages, "Apparatenlijst", &[]));
    assert!(dutch.contains("Getekend/gecontr."), "{dutch}");
    assert!(dutch.contains("1 van "));
    assert!(!dutch.contains("Drawn / checked"));
}
