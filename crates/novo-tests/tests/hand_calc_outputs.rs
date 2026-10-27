//! Outputs checked against sums done by hand, with the awkward data a real
//! parts list brings: the same article typed two ways, quantities that must
//! add up, lengths that must sum, text that CSV and Excel must not corrupt.

use novo_core::{CableCore, DeviceTag, Document};
use novo_reports::{CsvOptions, ReportOutput};

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

fn report(doc: &Document, id: &str) -> ReportOutput {
    novo_reports::generate_all(doc.project())
        .into_iter()
        .find(|r| r.report == id)
        .unwrap_or_else(|| panic!("no report {id}"))
}

fn bom_rows(doc: &Document) -> Vec<Vec<String>> {
    report(doc, "bom")
        .section("parts")
        .map(|s| s.text_rows())
        .unwrap_or_default()
}

#[test]
fn bom_merges_one_article_typed_differently_and_adds_up() {
    let mut doc = Document::default();
    doc.transact("setup", |tx| {
        let p1 = tx.add_part("Siemens", "3RT2015-1BB41")?;
        let p2 = tx.add_part("SIEMENS ", "3rt2015-1bb41 ")?;
        let p3 = tx.add_part("Siemens", "3RT2015-1BB41")?; // exact duplicate record
        let p4 = tx.add_part("ACME", "A2")?;
        let p5 = tx.add_part("ACME", "A10")?;
        let p6 = tx.add_part("ACME", "A1")?;
        let p7 = tx.add_part("ACME", "")?;
        let p8 = tx.add_part("Müller & Söhne", "Ø-1,5\"")?;
        for (i, p) in [p1, p2, p3, p4, p5, p6, p7, p8].into_iter().enumerate() {
            let d = tx.add_device(tag(&format!("-K{}", i + 1)), "dev")?;
            tx.assign_part(d, p)?;
        }
        let d = tx.add_device(tag("-K20"), "twice")?;
        tx.assign_part(d, p1)?;
        tx.assign_part(d, p1)?; // twice, same part
        let cp = tx.add_part("Lapp", "1119304")?;
        for (n, len) in [("=A-W1", 12.5), ("=A-W2", 7.25), ("=A-W3", 0.1)] {
            let c = tx.add_cable(tag(n), vec![CableCore::new("BN")])?;
            tx.update::<novo_core::Cable>(c, |c| {
                c.part = Some(cp);
                c.length_m = Some(len);
            })?;
        }
        Ok(())
    })
    .unwrap();
    let rows = bom_rows(&doc);
    let line = |maker: &str, number: &str| -> Vec<String> {
        rows.iter()
            .find(|r| r[2] == maker && r[3] == number)
            .unwrap_or_else(|| panic!("no BOM line {maker} {number}: {rows:?}"))
            .clone()
    };
    // Four uses of one article typed three ways (K1, K2 in other case and
    // with spaces, K3 a second record, K20), counted once each, in tag order.
    let siemens = line("Siemens", "3RT2015-1BB41");
    assert_eq!(siemens[1], "4", "{rows:?}");
    assert_eq!(siemens[7], "-K1, -K2, -K3, -K20");
    assert_eq!(
        rows.iter()
            .filter(|r| r[2].to_lowercase().trim() == "siemens")
            .count(),
        1
    );
    // 12.5 + 7.25 + 0.1 m of one cable article, three cables.
    let lapp = line("Lapp", "1119304");
    assert_eq!((lapp[1].as_str(), lapp[6].as_str()), ("3", "19.85"));
    // Natural order of order numbers: A1, A2, A10.
    let acme: Vec<&str> = rows
        .iter()
        .filter(|r| r[2] == "ACME")
        .map(|r| r[3].as_str())
        .collect();
    assert_eq!(acme, ["", "A1", "A2", "A10"]);
    // Quotes, commas and non-Latin letters survive untouched.
    assert_eq!(line("Müller & Söhne", "Ø-1,5\"")[1], "1");
}

/// A report cell written to CSV and read back by a plain CSV reader gives
/// the same text, whatever it holds.
#[test]
fn csv_keeps_awkward_text_whole() {
    let mut doc = Document::default();
    let nasty = [
        "plain",
        "comma, inside",
        "semi;colon",
        "quote \" inside",
        "two\nlines",
        "tab\tinside",
        "Ünïcödé 日本語 🙂",
        "  leading and trailing  ",
    ];
    doc.transact("setup", |tx| {
        for (i, text) in nasty.iter().enumerate() {
            tx.add_device(tag(&format!("-K{}", i + 1)), *text)?;
        }
        Ok(())
    })
    .unwrap();
    let devices = report(&doc, "devices");
    for sep in [',', ';'] {
        let csv = novo_reports::to_csv(
            &devices,
            &CsvOptions {
                delimiter: sep,
                ..CsvOptions::default()
            },
        );
        let rows = parse_csv(csv.trim_start_matches('\u{feff}'), sep);
        let header = &rows[0];
        let col = header
            .iter()
            .position(|h| h.to_lowercase().contains("description"))
            .unwrap_or_else(|| panic!("no description column in {header:?}"));
        let got: Vec<&str> = rows[1..].iter().map(|r| r[col].as_str()).collect();
        for text in nasty {
            assert!(
                got.contains(&text),
                "sep {sep:?}: `{text}` did not survive CSV: {got:?}"
            );
        }
        let widths: std::collections::BTreeSet<usize> = rows.iter().map(Vec::len).collect();
        assert_eq!(widths.len(), 1, "ragged rows with sep {sep:?}: {widths:?}");
    }
}

/// RFC 4180 reader: quoted fields may hold the separator, quotes and newlines.
fn parse_csv(text: &str, sep: char) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut field) = (Vec::new(), Vec::new(), String::new());
    let (mut quoted, mut chars) = (false, text.chars().peekable());
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                chars.next();
                field.push('"');
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, c) if c == sep => row.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// A pin 9e18 mm away (a damaged file) used to make the design checks ask for
/// 6.5 GB and abort the program.
#[test]
fn a_huge_coordinate_does_not_make_the_checks_allocate_gigabytes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/damaged-huge-coordinate.novo");
    let project = novo_storage::ProjectFile::read(path).expect("reads");
    let _ = novo_checks::check(&project);
}
