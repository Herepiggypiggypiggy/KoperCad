//! A plain grid of text in and out of `.xlsx`, for the spec sheet that
//! generates pages (`novo_workflow::generate`). Unlike the project tables
//! there is no fixed layout and no baseline: the first row is the header, the
//! rest are rows. Reading takes the sheet called `Spec` when there is one,
//! else the first sheet; numbers keep their shortest text (`7.5`, not
//! `7.500000`).

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use rust_xlsxwriter::{Format, Workbook as XlsxBook};

use super::files::SheetError;
use super::grid::number_text;

/// The sheet name a written grid gets, and the one a read prefers.
pub const SPEC_SHEET: &str = "Spec";

fn text_of(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_owned(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => number_text(*f),
        Data::Bool(b) => b.to_string(),
        Data::Error(e) => e.to_string(),
        Data::DateTime(d) => number_text(d.as_f64()),
        Data::DateTimeIso(s) | Data::DurationIso(s) => s.clone(),
    }
}

/// Reads one sheet of an `.xlsx` as rows of text. Blank trailing rows are
/// dropped and every row is padded to the header's width.
pub fn read_grid(bytes: &[u8]) -> Result<Vec<Vec<String>>, SheetError> {
    novo_parts::xlsx_guard::check_xlsx(bytes).map_err(|e| SheetError::Read(e.0))?;
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e: calamine::XlsxError| SheetError::Read(e.to_string()))?;
    let names = book.sheet_names();
    let name = names
        .iter()
        .find(|n| n.eq_ignore_ascii_case(SPEC_SHEET))
        .or_else(|| names.iter().find(|n| !n.starts_with('~')))
        .cloned()
        .ok_or(SheetError::Empty)?;
    let range = book
        .worksheet_range(&name)
        .map_err(|e| SheetError::Read(format!("sheet {name}: {e}")))?;
    let Some((_, first_col)) = range.start() else {
        return Err(SheetError::Empty);
    };
    let mut rows: Vec<Vec<String>> = range
        .rows()
        .map(|cells| {
            let mut row = vec![String::new(); first_col as usize];
            row.extend(cells.iter().map(text_of));
            row
        })
        .collect();
    while rows.last().is_some_and(|r| r.iter().all(|c| c.is_empty())) {
        rows.pop();
    }
    let width = rows.first().map_or(0, Vec::len);
    for row in &mut rows {
        row.resize(width.max(row.len()), String::new());
    }
    if rows.is_empty() {
        return Err(SheetError::Empty);
    }
    Ok(rows)
}

/// Writes rows of text (the first is the header) as an `.xlsx`. Cells are
/// text, so tags and structures such as `=A1` are never read as formulas;
/// the header is bold and the columns are as wide as their longest cell.
pub fn write_grid(rows: &[Vec<String>]) -> Result<Vec<u8>, SheetError> {
    let mut book = XlsxBook::new();
    let sheet = book.add_worksheet();
    sheet.set_name(SPEC_SHEET)?;
    let bold = Format::new().set_bold();
    let mut widths: Vec<usize> = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            if widths.len() <= c {
                widths.resize(c + 1, 8);
            }
            widths[c] = widths[c].max(cell.chars().count() + 2);
            if r == 0 {
                sheet.write_string_with_format(r as u32, c as u16, cell, &bold)?;
            } else {
                sheet.write_string(r as u32, c as u16, cell)?;
            }
        }
    }
    for (c, w) in widths.iter().enumerate() {
        sheet.set_column_width(c as u16, (*w).min(60) as f64)?;
    }
    sheet.set_freeze_panes(1, 0)?;
    Ok(book.save_to_buffer()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_round_trips_and_keeps_structures_as_text() {
        let rows = vec![
            vec!["Macro".to_string(), "Structure".into(), "power".into()],
            vec!["starter".into(), "=A1+CAB1".into(), "7.5".into()],
            vec!["starter".into(), "".into(), "".into()],
        ];
        let bytes = write_grid(&rows).unwrap();
        let back = read_grid(&bytes).unwrap();
        assert_eq!(back, rows);
    }

    #[test]
    fn an_empty_grid_is_an_error() {
        let bytes = write_grid(&[]).unwrap();
        assert!(matches!(read_grid(&bytes), Err(SheetError::Empty)));
    }
}
