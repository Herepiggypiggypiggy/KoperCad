//! The revision table above the title block (docs/DESIGN.md §7).

use std::collections::BTreeMap;

use lopdf::Document as Pdf;
use novo_canvas::model::{RevisionRow, TitleRevisions};
use novo_core::Document;
use novo_pdf::{PdfOptions, export_pdf};
use novo_tests::motor_starter;

fn row(index: &str, date: &str, author: &str, description: &str) -> RevisionRow {
    RevisionRow {
        index: index.into(),
        date: date.into(),
        author: author.into(),
        description: description.into(),
    }
}

fn page_text(options: &PdfOptions, doc: &Document, page: usize) -> String {
    let pdf = Pdf::load_mem(&export_pdf(doc.project(), options).unwrap()).unwrap();
    let ids: Vec<_> = pdf.get_pages().into_values().collect();
    String::from_utf8_lossy(&pdf.get_page_content(ids[page])).into_owned()
}

#[test]
fn the_table_lists_the_latest_revisions_and_leaves_the_oldest_off() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let first = doc.project().pages_in_order()[0].id;
    // Seven revisions, newest first: five fit under the header.
    let table: Vec<RevisionRow> = ["G", "F", "E", "D", "C", "B", "A"]
        .into_iter()
        .map(|i| row(i, "2026-10-15", "TV", &format!("Change {i}")))
        .collect();
    let options = PdfOptions {
        title_revisions: BTreeMap::from([(
            first,
            TitleRevisions {
                project: Some("G".into()),
                table,
                ..TitleRevisions::default()
            },
        )]),
        ..PdfOptions::default()
    };
    let text = page_text(&options, &doc, 0);
    assert!(text.contains("(Changed) Tj"), "the header");
    for shown in ["Change G", "Change F", "Change E", "Change D", "Change C"] {
        assert!(text.contains(&format!("({shown}) Tj")), "{shown}");
    }
    for left_off in ["Change B", "Change A"] {
        assert!(!text.contains(&format!("({left_off}) Tj")), "{left_off}");
    }
    // A page without revisions draws no table.
    let plain = page_text(&options, &doc, 1);
    assert!(!plain.contains("(Changed) Tj"));
}
