//! Designer templates as PDF: pages, bookmarks, text and a golden content
//! stream (regenerate with `UPDATE_GOLDEN=1 cargo test -p novo-pdf --test
//! designer_pdf` after an intended change, and review the diff).

use lopdf::Document as Pdf;
use novo_pdf::export_layouts_pdf;
use novo_reports::designer::{Fields, builtin, preview_data, render};

fn rendered(rows: usize) -> Vec<novo_reports::PageLayout> {
    let template = builtin::report_a4("Device list", "devices");
    render(&template, &preview_data(&template, rows), &Fields::sample()).pages
}

#[test]
fn a_template_becomes_a_pdf_with_every_page_and_bookmark() {
    let pages = rendered(60);
    assert!(pages.len() >= 3);
    let bytes = export_layouts_pdf(&pages, "Device list", &[(0, "Device list".into())]);
    let pdf = Pdf::load_mem(&bytes).expect("valid pdf");
    assert_eq!(pdf.get_pages().len(), pages.len());
    let first = pdf.extract_text(&[1]).expect("text");
    for want in ["Device list", "Sample project", "1 of 3", "tag-1"] {
        assert!(first.contains(want), "page 1 lacks {want}: {first}");
    }
    let last = pdf.extract_text(&[pages.len() as u32]).expect("text");
    assert!(last.contains(&format!("{0} of {0}", pages.len())));
    assert!(bytes.windows(8).any(|w| w == b"/Outline"));
    assert_eq!(
        bytes,
        export_layouts_pdf(&pages, "Device list", &[(0, "Device list".into())])
    );
}

#[test]
fn an_empty_layout_list_still_opens() {
    let bytes = export_layouts_pdf(&[], "none", &[]);
    assert_eq!(Pdf::load_mem(&bytes).expect("valid").get_pages().len(), 1);
}

#[test]
fn golden_content_stream_of_the_device_list_template() {
    let bytes = export_layouts_pdf(&rendered(30), "Device list", &[]);
    let pdf = Pdf::load_mem(&bytes).expect("valid pdf");
    let mut got = String::new();
    for (n, id) in pdf.get_pages() {
        let content = pdf.get_page_content(id);
        got.push_str(&format!(
            "% page {n}\n{}\n",
            String::from_utf8_lossy(&content)
        ));
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/designer_device_list.txt");
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &got).expect("write golden");
    }
    let want = std::fs::read_to_string(&path).expect("golden file (UPDATE_GOLDEN=1 creates it)");
    // Checkouts on Windows may convert line endings.
    assert_eq!(got.replace("\r\n", "\n"), want.replace("\r\n", "\n"));
}
