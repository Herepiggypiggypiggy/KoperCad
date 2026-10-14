//! Generated pages (reports) print as one PDF page per report sheet.

use lopdf::{Document as Pdf, Object};
use novo_core::{DeviceTag, Document, Page, PageId, PageKind};
use novo_pdf::{PageSet, PdfOptions, export_pdf};
use novo_reports::LiveReports;
use novo_tests::motor_starter;

/// The Gate 1 starter plus a device list on page "20" long enough for
/// three A3 sheets, and a page "21" with an unknown generator.
fn project() -> (Document, PageId, PageId) {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    let ((devices, unknown), _) = doc
        .transact("Report pages", |tx| {
            let generated = |generator: &str| PageKind::Generated {
                generator: generator.into(),
            };
            let devices = tx.add_page("20")?;
            tx.update::<Page>(devices, |p| {
                p.title = "Devices".into();
                p.kind = generated("devices");
            })?;
            let unknown = tx.add_page("21")?;
            tx.update::<Page>(unknown, |p| p.kind = generated("no-such-report"))?;
            for i in 1..=80 {
                tx.add_device(DeviceTag::parse(&format!("=A9-K{i}"))?, "Relay")?;
            }
            Ok((devices, unknown))
        })
        .expect("pages");
    (doc, devices, unknown)
}

fn parse(bytes: &[u8]) -> Pdf {
    Pdf::load_mem(bytes).expect("the export parses as a PDF")
}

fn content(pdf: &Pdf, n: usize) -> String {
    let id = pdf.get_pages()[&(n as u32)];
    String::from_utf8_lossy(&pdf.get_page_content(id)).into_owned()
}

fn labels(pdf: &Pdf) -> Vec<String> {
    let catalog = pdf.catalog().unwrap();
    let labels = catalog.get(b"PageLabels").unwrap().as_dict().unwrap();
    let nums = labels.get(b"Nums").unwrap().as_array().unwrap();
    nums.chunks(2)
        .map(|pair| {
            let dict = match &pair[1] {
                Object::Reference(r) => pdf.get_dictionary(*r).unwrap(),
                Object::Dictionary(d) => d,
                other => panic!("label {other:?}"),
            };
            let prefix = dict.get(b"P").unwrap().as_str().unwrap();
            String::from_utf8_lossy(prefix).into_owned()
        })
        .collect()
}

#[test]
fn a_generated_page_prints_one_pdf_page_per_sheet() {
    let (doc, devices, _) = project();
    let expected = LiveReports::standard().sheets_of_page(doc.project(), devices);
    assert_eq!(expected.len(), 3, "the fixture needs three sheets");

    let pdf = parse(&export_pdf(doc.project(), &PdfOptions::default()).unwrap());
    // Two schematic pages, three sheets of 20, the empty frame of 21.
    assert_eq!(pdf.get_pages().len(), 6);
    assert_eq!(labels(&pdf), ["1", "2", "20.1", "20.2", "20.3", "21"]);
    let toc = pdf.get_toc().expect("outline");
    let titles: Vec<&str> = toc.toc.iter().map(|t| t.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "1  Main circuit",
            "2  Control circuit",
            "20.1  Devices",
            "20.2  Devices",
            "20.3  Devices",
            "21",
        ]
    );

    // Each sheet holds its own rows and its own "page n / 3".
    for (k, n) in [(1, 3), (2, 4), (3, 5)] {
        let c = content(&pdf, n);
        assert!(c.contains(&format!("(page {k} / 3) Tj")), "sheet {k}");
        assert!(c.contains("(Device list"), "heading on sheet {k}");
        // Every cell is clipped to its box.
        assert!(c.contains(" re\nW\nn\n"), "clip on sheet {k}");
    }
    assert!(content(&pdf, 3).contains("(=A9-K1) Tj"));
    assert!(content(&pdf, 5).contains("(=A9-K80) Tj"));
    assert!(!content(&pdf, 3).contains("(=A9-K80) Tj"));
    // The unknown report prints an empty frame, no table.
    assert!(!content(&pdf, 6).contains("\nW\n"));
}

#[test]
fn a_page_set_with_a_generated_page_keeps_all_its_sheets() {
    let (doc, devices, _) = project();
    let options = PdfOptions {
        pages: PageSet::Pages(vec![devices]),
        ..PdfOptions::default()
    };
    let pdf = parse(&export_pdf(doc.project(), &options).unwrap());
    assert_eq!(labels(&pdf), ["20.1", "20.2", "20.3"]);
}

#[test]
fn generated_pages_export_deterministically_and_in_black() {
    let (doc, _, _) = project();
    let a = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    let b = export_pdf(doc.project(), &PdfOptions::default()).unwrap();
    assert_eq!(a, b);
    let pdf = parse(&a);
    for n in 3..=5 {
        for line in content(&pdf, n)
            .lines()
            .filter(|l| l.ends_with(" RG") || l.ends_with(" rg"))
        {
            assert!(line.starts_with("0 0 0 "), "non-black colour: {line}");
        }
    }
}

