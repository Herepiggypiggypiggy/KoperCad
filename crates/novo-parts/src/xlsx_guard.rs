//! A bound on what an `.xlsx` may make the program allocate, checked before
//! the workbook is handed to the spreadsheet library.
//!
//! The library builds each sheet as a dense grid covering every cell from
//! the first used one to the last, and sizes it from the sheet's
//! `<dimension>` tag. A file of a few hundred bytes with a cell in `A1` and
//! another in `XFD1048576` therefore asks for billions of cells, and a
//! row-count check made after the range is built comes too late. This
//! module reads the worksheet XML as a stream, never building anything, and
//! refuses the workbook when the used area, the cell count or the unzipped
//! size is beyond the limits. Every spreadsheet reader in the program
//! (parts tables, the PLC list and tag files, the Excel round trip) calls
//! [`check_xlsx`] first.
//!
//! Honest limits: the scan reads the unzipped sheet XML once (at most the
//! unzipped-size limit, 256 MiB), which takes a moment on a huge file but
//! allocates only a small buffer. It looks at `<dimension>`, `<row>` and
//! `<c>` tags, not at what the cells hold, so a workbook within the limits
//! can still be large; the limits are sized so that stays near 300 MiB.

use std::io::{Cursor, Read};

use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

/// What a workbook may be.
#[derive(Debug, Clone, Copy)]
pub struct XlsxLimits {
    /// Most the parts of the archive may add up to when unzipped.
    pub max_unzipped_bytes: u64,
    /// Most rows between a sheet's first and last used row.
    pub max_rows: u64,
    /// Most cells in a sheet's used rectangle (and most cell tags).
    pub max_cells: u64,
}

impl XlsxLimits {
    /// Limits for table-like sheets: parts lists, I/O lists, tag exports.
    pub const TABLES: XlsxLimits = XlsxLimits {
        max_unzipped_bytes: 256 * 1024 * 1024,
        max_rows: 200_000,
        max_cells: 8_000_000,
    };
}

/// A workbook that is beyond the limits. The message is for the user.
#[derive(Debug, Clone, PartialEq, Error)]
#[error("{0}")]
pub struct XlsxTooLarge(pub String);

/// Checks `bytes` against [`XlsxLimits::TABLES`]. A file that is not a zip
/// archive passes (the spreadsheet library reports that in its own words).
pub fn check_xlsx(bytes: &[u8]) -> Result<(), XlsxTooLarge> {
    check_xlsx_with(bytes, XlsxLimits::TABLES)
}

/// Like [`check_xlsx`] with other limits.
pub fn check_xlsx_with(bytes: &[u8], limits: XlsxLimits) -> Result<(), XlsxTooLarge> {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else {
        return Ok(());
    };
    let unzipped: u64 = (0..archive.len())
        .filter_map(|i| archive.by_index_raw(i).ok().map(|f| f.size()))
        .sum();
    if unzipped > limits.max_unzipped_bytes {
        return Err(XlsxTooLarge(
            "the workbook expands to far more data than a table holds; export the sheet as CSV"
                .into(),
        ));
    }
    for i in 0..archive.len() {
        let Ok(file) = archive.by_index(i) else {
            continue;
        };
        let name = file.name().to_owned();
        if !(name.starts_with("xl/worksheets/") && name.ends_with(".xml")) {
            continue;
        }
        scan_sheet(file, limits).map_err(|why| {
            XlsxTooLarge(format!(
                "sheet {} is too large to import ({why}); export the table you need as CSV",
                name.trim_start_matches("xl/worksheets/")
                    .trim_end_matches(".xml")
            ))
        })?;
    }
    Ok(())
}

/// Streams one worksheet's XML and stops at the first limit it breaks.
///
/// It reads the XML the way the spreadsheet library does (quick-xml,
/// elements matched by local name, so a namespace prefix such as `x:c`
/// counts, and attributes through the attribute iterator, so single quotes
/// and spaces around `=` count), so no spelling the library accepts slips
/// past. An XML error ends the scan: the library fails on it too.
fn scan_sheet(xml: impl Read, limits: XlsxLimits) -> Result<(), String> {
    let mut reader = quick_xml::Reader::from_reader(std::io::BufReader::new(xml));
    let mut bbox = Bbox::default();
    let mut cells = 0u64;
    let (mut row, mut col) = (0u64, 0u64);
    let mut buf = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buf);
        let element = match &event {
            Ok(Event::Start(e) | Event::Empty(e)) => e,
            Ok(Event::Eof) | Err(_) => return Ok(()),
            Ok(_) => {
                buf.clear();
                continue;
            }
        };
        match element.local_name().as_ref() {
            b"c" => {
                cells += 1;
                if cells > limits.max_cells {
                    return Err(format!("more than {} cells", limits.max_cells));
                }
                col += 1;
                if let Some((r, c)) = attribute(element, b"r").and_then(|v| cell_ref(&v)) {
                    (row, col) = (r, c);
                }
                bbox.add(row.max(1), col, limits)?;
            }
            b"row" => {
                row = attribute(element, b"r")
                    .and_then(|v| std::str::from_utf8(&v).ok()?.trim().parse().ok())
                    .unwrap_or(row + 1);
                col = 0;
            }
            b"dimension" => {
                if let Some(reference) = attribute(element, b"ref") {
                    let mut ends = reference.split(|b| *b == b':').filter_map(cell_ref);
                    if let (Some(first), Some(last)) = (ends.next(), ends.next()) {
                        let mut dim = Bbox::default();
                        dim.add(first.0, first.1, limits)?;
                        dim.add(last.0, last.1, limits)?;
                    }
                }
            }
            _ => {}
        }
        buf.clear();
    }
}

/// The raw value of attribute `name` (matched by local name).
fn attribute(element: &BytesStart<'_>, name: &[u8]) -> Option<Vec<u8>> {
    element
        .attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .map(|a| a.value.into_owned())
}

/// The used rectangle of a sheet so far.
#[derive(Default)]
struct Bbox {
    rows: Option<(u64, u64)>,
    cols: Option<(u64, u64)>,
}

impl Bbox {
    fn add(&mut self, row: u64, col: u64, limits: XlsxLimits) -> Result<(), String> {
        let grow = |range: &mut Option<(u64, u64)>, v: u64| {
            *range = Some(match *range {
                Some((lo, hi)) => (lo.min(v), hi.max(v)),
                None => (v, v),
            });
        };
        grow(&mut self.rows, row);
        grow(&mut self.cols, col);
        let (r, c) = (self.rows.unwrap_or((0, 0)), self.cols.unwrap_or((0, 0)));
        let height = r.1 - r.0 + 1;
        let width = c.1 - c.0 + 1;
        if height > limits.max_rows {
            return Err(format!(
                "rows {} to {} span more than {} rows",
                r.0, r.1, limits.max_rows
            ));
        }
        if height.saturating_mul(width) > limits.max_cells {
            return Err(format!(
                "its used area is {height} rows by {width} columns, more than {} cells",
                limits.max_cells
            ));
        }
        Ok(())
    }
}

/// `B7` as (row 7, column 2); `None` for anything else.
fn cell_ref(text: &[u8]) -> Option<(u64, u64)> {
    let letters = text.iter().take_while(|b| b.is_ascii_alphabetic()).count();
    let (l, d) = text.split_at(letters);
    if l.is_empty() || l.len() > 3 || d.is_empty() || !d.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let col = l.iter().fold(0u64, |a, b| {
        a * 26 + u64::from(b.to_ascii_uppercase() - b'A' + 1)
    });
    let row: u64 = std::str::from_utf8(d).ok()?.parse().ok()?;
    Some((row, col))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sheet(xml: &str) -> Result<(), String> {
        scan_sheet(xml.as_bytes(), XlsxLimits::TABLES)
    }

    #[test]
    fn a_small_sheet_passes() {
        sheet(
            r#"<worksheet><dimension ref="A1:C3"/><sheetData><row r="1"><c r="A1"/><c r="C1"/></row><row r="3"><c r="B3"/></row></sheetData></worksheet>"#,
        )
        .unwrap();
    }

    #[test]
    fn a_huge_dimension_is_refused() {
        assert!(sheet(r#"<worksheet><dimension ref="A1:XFD1048576"/></worksheet>"#).is_err());
    }

    #[test]
    fn two_far_apart_cells_are_refused() {
        assert!(
            sheet(r#"<sheetData><row r="1"><c r="A1"/></row><row r="1048576"><c r="XFD1048576"/></row></sheetData>"#)
                .is_err()
        );
    }

    #[test]
    fn cells_without_references_are_counted() {
        let one_row = format!("<row>{}</row>", "<c/>".repeat(100));
        let rows = one_row.repeat(100_000);
        assert!(sheet(&format!("<sheetData>{rows}</sheetData>")).is_err());
    }

    #[test]
    fn every_spelling_the_library_accepts_is_refused() {
        for xml in [
            r#"<x:worksheet><x:dimension ref="A1:XFD1048576"/></x:worksheet>"#,
            r#"<worksheet><dimension ref='A1:XFD1048576'/></worksheet>"#,
            r#"<worksheet><dimension ref = "A1:XFD1048576"/></worksheet>"#,
            r#"<worksheet><dimension
                 ref="A1:XFD1048576"/></worksheet>"#,
            r#"<x:sheetData><x:row r="1"><x:c r="A1"/></x:row><x:row r="1048576"><x:c r="XFD1048576"/></x:row></x:sheetData>"#,
            r#"<sheetData><row r='1'><c r='A1'/></row><row r = '1048576'><c r = 'XFD1048576'/></row></sheetData>"#,
            r#"<sheetData><row r="1"><c r="A1"></c></row><row r="1048576"><c r="XFD1048576"><v>1</v></c></row></sheetData>"#,
        ] {
            assert!(sheet(xml).is_err(), "{xml}");
        }
    }

    #[test]
    fn a_dribbling_reader_is_still_checked() {
        struct Dribble<'a>(&'a [u8]);
        impl Read for Dribble<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                let n = self.0.len().min(3).min(out.len());
                out[..n].copy_from_slice(&self.0[..n]);
                self.0 = &self.0[n..];
                Ok(n)
            }
        }
        let xml = br#"<sheetData><row r="1"><c r="A1"/></row><row r="1048576"><c r="XFD1048576"/></row></sheetData>"#;
        assert!(scan_sheet(Dribble(xml), XlsxLimits::TABLES).is_err());
    }
}
