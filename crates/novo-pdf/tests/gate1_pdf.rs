//! Gate 1: the motor starter project exports to a clean, navigable PDF.

use lopdf::{Document as Pdf, Object, ObjectId};
use novo_core::{Document, PageFormat, PageId, PaperSize, Project};
use novo_pdf::{PT_PER_MM, PageSet, PdfError, PdfOptions, SymbolNote, export_pdf};
use novo_tests::{MotorStarter, motor_starter};

fn gate1() -> (Document, MotorStarter) {
    let mut doc = Document::default();
    let handles = motor_starter(&mut doc).expect("fixture builds");
    (doc, handles)
}

fn parse(bytes: &[u8]) -> Pdf {
    Pdf::load_mem(bytes).expect("the export parses as a PDF")
}

fn pages(pdf: &Pdf) -> Vec<ObjectId> {
    pdf.get_pages().into_values().collect()
}

fn media_box(pdf: &Pdf, page: ObjectId) -> [f32; 4] {
    let dict = pdf.get_dictionary(page).unwrap();
    let arr = dict.get(b"MediaBox").unwrap().as_array().unwrap();
    let v: Vec<f32> = arr.iter().map(|o| o.as_float().unwrap()).collect();
    [v[0], v[1], v[2], v[3]]
}

fn content(pdf: &Pdf, page: ObjectId) -> String {
    String::from_utf8_lossy(&pdf.get_page_content(page)).into_owned()
}

fn link_targets(pdf: &Pdf, page: ObjectId) -> Vec<ObjectId> {
    pdf.get_page_annotations(page)
        .unwrap_or_default()
        .into_iter()
        .filter(|a| a.get(b"Subtype").and_then(Object::as_name).ok() == Some(b"Link"))
        .map(|a| {
            let action = a.get(b"A").unwrap().as_dict().unwrap();
            assert_eq!(action.get(b"S").unwrap().as_name().unwrap(), b"GoTo");
            let dest = action.get(b"D").unwrap().as_array().unwrap();
            dest[0].as_reference().unwrap()
        })
        .collect()
}

#[test]
fn gate1_exports_one_a3_landscape_page_per_project_page() {
    let (doc, _) = gate1();
    let bytes = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    assert!(bytes.starts_with(b"%PDF-1."), "PDF header");
    assert!(bytes.trim_ascii_end().ends_with(b"%%EOF"), "PDF trailer");

    let pdf = parse(&bytes);
    let ids = pages(&pdf);
    assert_eq!(ids.len(), doc.project().pages().count());
    assert_eq!(ids.len(), 2);
    for page in ids {
        let [x0, y0, x1, y1] = media_box(&pdf, page);
        assert_eq!((x0, y0), (0.0, 0.0));
        assert!((x1 - 420.0 * PT_PER_MM).abs() < 0.01, "A3 width {x1}");
        assert!((y1 - 297.0 * PT_PER_MM).abs() < 0.01, "A3 height {y1}");
    }
}

#[test]
fn bookmarks_name_every_page_in_order() {
    let (doc, _) = gate1();
    let pdf = parse(&export_pdf(doc.project(), &PdfOptions::default()).unwrap());
    let toc = pdf.get_toc().expect("outline");
    let titles: Vec<(&str, usize)> = toc.toc.iter().map(|t| (t.title.as_str(), t.page)).collect();
    assert_eq!(
        titles,
        vec![("1  Main circuit", 1), ("2  Control circuit", 2)]
    );
    let catalog = pdf.catalog().unwrap();
    assert_eq!(
        catalog.get(b"PageMode").unwrap().as_name().unwrap(),
        b"UseOutlines"
    );
    assert!(catalog.get(b"PageLabels").is_ok(), "page labels");
}

#[test]
fn device_tags_and_titles_are_printed_as_text() {
    let (doc, _) = gate1();
    let pdf = parse(&export_pdf(doc.project(), &PdfOptions::default()).unwrap());
    let ids = pages(&pdf);
    let main = content(&pdf, ids[0]);
    let control = content(&pdf, ids[1]);
    for tag in ["=A1+CAB-F1", "=A1+CAB-K1", "=A1+CAB-F2", "=A1+FLD-M1"] {
        assert!(main.contains(&format!("({tag}) Tj")), "{tag} on page 1");
    }
    for tag in ["=A1+CAB-F3", "=A1+CAB-S0", "=A1+CAB-S1", "=A1+CAB-K1"] {
        assert!(control.contains(&format!("({tag}) Tj")), "{tag} on page 2");
    }
    assert!(main.contains("(1  Main circuit) Tj"), "title on page 1");
    assert!(
        control.contains("(2  Control circuit) Tj"),
        "title on page 2"
    );
    // Print styling: nothing but black ink.
    for s in [&main, &control] {
        for line in s
            .lines()
            .filter(|l| l.ends_with(" RG") || l.ends_with(" rg"))
        {
            assert!(line.starts_with("0 0 0 "), "non-black colour: {line}");
        }
    }
}

#[test]
fn cross_reference_notes_are_printed_on_their_page() {
    let (doc, h) = gate1();
    let mut options = PdfOptions::default();
    options.notes.insert(
        h.control_page,
        vec![SymbolNote {
            symbol: h.k1_coil,
            lines: vec!["1-2 /1.3".into(), "13-14 /2.4".into()],
            warning: false,
        }],
    );
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    let ids = pages(&pdf);
    let control = content(&pdf, ids[1]);
    assert!(control.contains("(1-2 /1.3) Tj"), "{control}");
    assert!(control.contains("(13-14 /2.4) Tj"));
    assert!(!content(&pdf, ids[0]).contains("(1-2 /1.3) Tj"));
}

#[test]
fn contactor_parts_link_across_pages() {
    let (doc, _) = gate1();
    let pdf = parse(&export_pdf(doc.project(), &PdfOptions::default()).unwrap());
    let ids = pages(&pdf);
    // -K1's main contacts (page 1) lead to its coil (page 2), and the coil
    // leads back to page 1.
    let from_main = link_targets(&pdf, ids[0]);
    assert!(from_main.contains(&ids[1]), "page 1 links to page 2");
    let from_control = link_targets(&pdf, ids[1]);
    assert!(from_control.contains(&ids[0]), "page 2 links to page 1");
    // No link points at its own page.
    assert!(!from_main.contains(&ids[0]));
    assert!(!from_control.contains(&ids[1]));
}

#[test]
fn links_can_be_switched_off() {
    let (doc, _) = gate1();
    let options = PdfOptions {
        links: false,
        bookmarks: false,
        ..PdfOptions::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    for page in pages(&pdf) {
        assert!(link_targets(&pdf, page).is_empty());
    }
    assert!(pdf.catalog().unwrap().get(b"Outlines").is_err());
}

#[test]
fn export_is_deterministic() {
    let (doc, _) = gate1();
    let a = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let b = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    assert_eq!(a, b, "same project, same bytes");
    // A separately built copy of the same project too.
    let (doc2, _) = gate1();
    let c = export_pdf(doc2.project(), &PdfOptions::default()).unwrap();
    assert_eq!(a, c);
}

#[test]
fn page_set_exports_only_the_chosen_pages_in_project_order() {
    let (doc, h) = gate1();
    let options = PdfOptions {
        pages: PageSet::Pages(vec![h.control_page]),
        ..PdfOptions::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    let ids = pages(&pdf);
    assert_eq!(ids.len(), 1);
    assert!(content(&pdf, ids[0]).contains("(2  Control circuit) Tj"));
    // The coil's contacts are on a page that is not exported: no dead link.
    assert!(link_targets(&pdf, ids[0]).is_empty());

    // The list order does not matter; duplicates count once.
    let options = PdfOptions {
        pages: PageSet::Pages(vec![h.control_page, h.main_page, h.control_page]),
        ..PdfOptions::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    let toc = pdf.get_toc().unwrap();
    assert_eq!(toc.toc.len(), 2);
    assert_eq!(toc.toc[0].title, "1  Main circuit");
}

#[test]
fn unknown_pages_are_an_error() {
    let (doc, _) = gate1();
    let options = PdfOptions {
        pages: PageSet::Pages(vec![PageId(9_999_999)]),
        ..PdfOptions::default()
    };
    assert!(matches!(
        export_pdf(doc.project(), &options),
        Err(PdfError::UnknownPage(PageId(9_999_999)))
    ));
}

#[test]
fn empty_project_gives_one_blank_page() {
    let project = Project::new("Empty");
    let bytes = export_pdf(&project, &PdfOptions::default()).unwrap();
    let pdf = parse(&bytes);
    let ids = pages(&pdf);
    assert_eq!(ids.len(), 1);
    assert!(pdf.catalog().unwrap().get(b"Outlines").is_err());
    let empty_set = PdfOptions {
        pages: PageSet::Pages(Vec::new()),
        ..PdfOptions::default()
    };
    assert_eq!(
        pages(&parse(&export_pdf(&project, &empty_set).unwrap())).len(),
        1
    );
}

#[test]
fn empty_page_prints_its_frame_and_portrait_a4_keeps_its_size() {
    let mut doc = Document::default();
    doc.transact("pages", |tx| {
        let blank = tx.add_page("1")?;
        let a4 = tx.add_page("2")?;
        tx.update::<novo_core::Page>(blank, |p| p.title = "Ünïcode Ω title".into())?;
        tx.update::<novo_core::Page>(a4, |p| {
            p.format = PageFormat {
                size: PaperSize::A4,
                landscape: false,
                ..PageFormat::default()
            }
        })?;
        Ok(())
    })
    .unwrap();
    let pdf = parse(&export_pdf(doc.project(), &PdfOptions::default()).unwrap());
    let ids = pages(&pdf);
    assert_eq!(ids.len(), 2);
    let first = content(&pdf, ids[0]);
    assert!(first.contains(" re"), "frame drawn on an empty page");
    let [_, _, w, h] = media_box(&pdf, ids[1]);
    assert!((w - 210.0 * PT_PER_MM).abs() < 0.01 && (h - 297.0 * PT_PER_MM).abs() < 0.01);
    // Unicode bookmark titles survive; text outside WinAnsi prints as `?`.
    let toc = pdf.get_toc().unwrap();
    assert_eq!(toc.toc[0].title, "1  Ünïcode Ω title");
}

#[test]
fn write_pdf_writes_the_same_bytes() {
    let (doc, _) = gate1();
    let dir = std::env::temp_dir().join(format!("novo-pdf-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("gate1.pdf");
    let options = PdfOptions::default();
    novo_pdf::write_pdf(
        doc.project(),
        &options,
        &novo_canvas::model::PlaceholderArt,
        &path,
    )
    .unwrap();
    let written = std::fs::read(&path).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!(written, export_pdf(doc.project(), &options).unwrap());
}
