//! Text outside WinAnsi (Ω, ≤, Greek) prints in an embedded Inter subset;
//! everything else stays in Helvetica with nothing embedded.

use lopdf::{Dictionary, Document as Pdf, Object};
use novo_core::{Document, Page};
use novo_pdf::{PdfOptions, export_pdf};
use novo_tests::motor_starter;

fn starter(main_title: Option<&str>) -> Document {
    let mut doc = Document::default();
    let ms = motor_starter(&mut doc).expect("fixture builds");
    if let Some(title) = main_title {
        doc.transact("Title", |tx| {
            tx.update::<Page>(ms.main_page, |p| p.title = title.into())
        })
        .expect("title");
    }
    doc
}

fn parse(bytes: &[u8]) -> Pdf {
    Pdf::load_mem(bytes).expect("the export parses as a PDF")
}

fn deref<'a>(pdf: &'a Pdf, object: &'a Object) -> &'a Object {
    match object {
        Object::Reference(id) => pdf.get_object(*id).expect("reference resolves"),
        other => other,
    }
}

fn dict<'a>(pdf: &'a Pdf, object: &'a Object) -> &'a Dictionary {
    match deref(pdf, object) {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &s.dict,
        other => panic!("not a dictionary: {other:?}"),
    }
}

/// The fonts of each page, by resource name.
fn page_fonts(pdf: &Pdf) -> Vec<Vec<Vec<u8>>> {
    pdf.get_pages()
        .into_values()
        .map(|page| {
            let page = pdf.get_dictionary(page).unwrap();
            let resources = dict(pdf, page.get(b"Resources").unwrap());
            dict(pdf, resources.get(b"Font").unwrap())
                .iter()
                .map(|(name, _)| name.clone())
                .collect()
        })
        .collect()
}

#[test]
fn winansi_text_embeds_no_font() {
    let bytes = export_pdf(starter(None).project(), &PdfOptions::default()).unwrap();
    let pdf = parse(&bytes);
    for fonts in page_fonts(&pdf) {
        assert_eq!(fonts, [b"F1".to_vec()]);
    }
    assert!(!bytes.windows(9).any(|w| w == b"FontFile2"));
}

#[test]
fn a_line_outside_winansi_is_set_in_embedded_inter() {
    let doc = starter(Some("Heater 10 kΩ, ϑ ≤ 40 °C"));
    let bytes = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let pdf = parse(&bytes);

    // Only the page that needs it carries the second font.
    let fonts = page_fonts(&pdf);
    assert_eq!(fonts[0], [b"F1".to_vec(), b"F2".to_vec()]);
    assert_eq!(fonts[1], [b"F1".to_vec()]);
    let page = *pdf.get_pages().get(&1).unwrap();
    let content = String::from_utf8_lossy(&pdf.get_page_content(page)).into_owned();
    assert!(content.contains("/F2 1 Tf"));
    // The title is no longer printed with question marks.
    assert!(!content.contains("(Heater 10 k?"));

    let page = pdf.get_dictionary(page).unwrap();
    let resources = dict(&pdf, page.get(b"Resources").unwrap());
    let font = dict(
        &pdf,
        dict(&pdf, resources.get(b"Font").unwrap())
            .get(b"F2")
            .unwrap(),
    );
    assert_eq!(font.get(b"Subtype").unwrap().as_name().unwrap(), b"Type0");
    assert_eq!(
        font.get(b"Encoding").unwrap().as_name().unwrap(),
        b"Identity-H"
    );
    let base = font.get(b"BaseFont").unwrap().as_name().unwrap();
    assert!(base.ends_with(b"+Inter-Regular"), "{base:?}");
    assert_eq!(base[6], b'+', "six-letter subset tag");

    let cid = deref(&pdf, font.get(b"DescendantFonts").unwrap())
        .as_array()
        .unwrap();
    let cid = dict(&pdf, &cid[0]);
    assert_eq!(
        cid.get(b"Subtype").unwrap().as_name().unwrap(),
        b"CIDFontType2"
    );
    let descriptor = dict(&pdf, cid.get(b"FontDescriptor").unwrap());
    let file = match deref(&pdf, descriptor.get(b"FontFile2").unwrap()) {
        Object::Stream(s) => s,
        other => panic!("font file is not a stream: {other:?}"),
    };
    // A subset: a few kilobytes, not the 345 kB font.
    assert!(file.content.len() > 1000, "{}", file.content.len());
    assert!(file.content.len() < 60_000, "{}", file.content.len());

    // Searchable and copyable: Ω, ϑ and ≤ map back to Unicode.
    let to_unicode = match deref(&pdf, font.get(b"ToUnicode").unwrap()) {
        Object::Stream(s) => String::from_utf8_lossy(&s.content).into_owned(),
        other => panic!("ToUnicode is not a stream: {other:?}"),
    };
    for code in ["<03a9>", "<03d1>", "<2264>"] {
        assert!(to_unicode.to_lowercase().contains(code), "{code}");
    }
}

#[test]
fn exports_with_inter_are_deterministic() {
    let doc = starter(Some("10 kΩ"));
    let a = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let b = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    assert_eq!(a, b);
}
