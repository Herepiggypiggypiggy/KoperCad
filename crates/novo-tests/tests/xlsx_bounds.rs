//! A tiny crafted `.xlsx` must not make any spreadsheet reader allocate
//! billions of cells. Each file below is a few hundred bytes; the reader has
//! to refuse it in words before the spreadsheet library builds the sheet
//! (before the guard, `calamine` sized a dense grid from the used area and
//! the program ran out of memory).

use std::io::Write;

use zip::write::SimpleFileOptions;

/// A valid workbook with one sheet whose XML is `sheet_xml`.
fn workbook(sheet_xml: &str) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let opts = SimpleFileOptions::default();
        let mut put = |name: &str, body: &str| {
            zip.start_file(name, opts).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        };
        put(
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/></Types>"#,
        );
        put(
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#,
        );
        put(
            "xl/workbook.xml",
            r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
        );
        put(
            "xl/_rels/workbook.xml.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/></Relationships>"#,
        );
        put("xl/worksheets/sheet1.xml", sheet_xml);
        zip.finish().unwrap();
    }
    out
}

const HEAD: &str = r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">"#;

/// One cell in A1 and one at the last cell of the sheet.
fn far_apart() -> Vec<u8> {
    workbook(&format!(
        r#"{HEAD}<sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Tag</t></is></c></row><row r="1048576"><c r="XFD1048576" t="inlineStr"><is><t>x</t></is></c></row></sheetData></worksheet>"#
    ))
}

/// A dimension tag that claims the whole sheet over two cells.
fn lying_dimension() -> Vec<u8> {
    workbook(&format!(
        r#"{HEAD}<dimension ref="A1:XFD1048576"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Tag</t></is></c></row></sheetData></worksheet>"#
    ))
}

/// A sheet of ordinary size, to show the guard lets real files through.
fn small() -> Vec<u8> {
    workbook(&format!(
        r#"{HEAD}<dimension ref="A1:C2"/><sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>Order number</t></is></c><c r="B1" t="inlineStr"><is><t>Manufacturer</t></is></c></row><row r="2"><c r="A2" t="inlineStr"><is><t>3RT2016</t></is></c><c r="B2" t="inlineStr"><is><t>Siemens</t></is></c></row></sheetData></worksheet>"#
    ))
}

fn crafted() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("far apart cells", far_apart()),
        ("lying dimension", lying_dimension()),
    ]
}

#[test]
fn the_crafted_files_are_tiny() {
    for (what, bytes) in crafted() {
        assert!(bytes.len() < 4000, "{what}: {} bytes", bytes.len());
    }
}

#[test]
fn the_parts_table_import_refuses_them_in_words() {
    for (what, bytes) in crafted() {
        let err = novo_parts::TableFile::read("parts.xlsx", &bytes).unwrap_err();
        assert!(err.to_string().contains("too large"), "{what}: {err}");
    }
    assert!(novo_parts::TableFile::read("parts.xlsx", &small()).is_ok());
}

#[test]
fn the_plc_io_list_refuses_them_in_words() {
    for (what, bytes) in crafted() {
        let err = novo_plc::generate::list::read_list(&bytes).unwrap_err();
        assert!(err.to_string().contains("too large"), "{what}: {err}");
    }
}

#[test]
fn the_plc_tag_file_reader_refuses_them_in_words() {
    for (what, bytes) in crafted() {
        let err = novo_plc::link::read(&bytes).unwrap_err();
        assert!(err.to_string().contains("too large"), "{what}: {err}");
    }
}

#[test]
fn the_excel_round_trip_and_spec_sheet_readers_refuse_them_in_words() {
    for (what, bytes) in crafted() {
        let err = novo_interop::sheets::read_xlsx(&bytes).unwrap_err();
        assert!(err.to_string().contains("too large"), "{what}: {err}");
        let err = novo_interop::sheets::read_grid(&bytes).unwrap_err();
        assert!(err.to_string().contains("too large"), "{what}: {err}");
    }
    let grid = novo_interop::sheets::read_grid(&small()).unwrap();
    assert_eq!(grid[1][1], "Siemens");
}
