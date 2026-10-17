//! Excel (`.xlsx`) and CSV export of report output.
//!
//! Export is never blocked: whatever the reports hold is written. Numbers
//! are written as numbers, so quantities can be summed in the spreadsheet.

use std::path::Path;

use rust_xlsxwriter::{DocProperties, ExcelDateTime, Format, Workbook, XlsxError};

use crate::table::{Cell, ReportOutput, Section};

/// Why an export failed (only writing can fail).
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("Excel export failed: {0}")]
    Xlsx(#[from] XlsxError),
    #[error("could not write the export: {0}")]
    Io(#[from] std::io::Error),
}

/// A sheet name Excel accepts: at most 31 characters, none of `[]:*?/\`,
/// unique within the workbook (`Wire list (2)`).
fn sheet_name(title: &str, taken: &[String]) -> String {
    let chars: Vec<char> = title.chars().collect();
    let mut clean = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !"[]:*?/\\".contains(c) {
            clean.push(c);
        } else if c == '/'
            && i > 0
            && chars[i - 1].is_alphanumeric()
            && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric())
        {
            // "I/O list" reads as "IO list", not "I_O list".
        } else {
            clean.push('_');
        }
    }
    // Excel rejects a leading or trailing apostrophe (also after cutting to
    // 31 characters) and the reserved name `History`.
    let tidy = |s: &str| s.trim().trim_matches('\'').trim().to_owned();
    let clean = tidy(&clean.chars().take(31).collect::<String>());
    let base: String = if clean.is_empty() || clean.eq_ignore_ascii_case("history") {
        format!(
            "{} report",
            if clean.is_empty() { "Untitled" } else { &clean }
        )
    } else {
        clean
    };
    let mut name = base.clone();
    let mut n = 2;
    while taken.iter().any(|t| t.eq_ignore_ascii_case(&name)) {
        let suffix = format!(" ({n})");
        let keep = 31 - suffix.chars().count();
        name = tidy(&base.chars().take(keep).collect::<String>()) + &suffix;
        n += 1;
    }
    name
}

/// Writes reports as one workbook, one sheet per report in the given order.
/// A sheet holds its sections one below the other: section title (when the
/// report has several), bold header row, rows, a blank row. With one
/// section the header row is frozen.
///
/// Output is deterministic: the workbook's creation date is fixed instead
/// of the current time, so the same reports always give the same bytes
/// (exports can be compared, cached and diffed).
pub fn xlsx_bytes(reports: &[&ReportOutput]) -> Result<Vec<u8>, ExportError> {
    let mut workbook = Workbook::new();
    let created = ExcelDateTime::from_ymd(2000, 1, 1)?;
    workbook.set_properties(&DocProperties::new().set_creation_datetime(&created));
    let bold = Format::new().set_bold();
    let mut taken: Vec<String> = Vec::new();
    for report in reports {
        let name = sheet_name(&report.title, &taken);
        let sheet = workbook.add_worksheet();
        sheet.set_name(&name)?;
        taken.push(name);

        let titled = report.sections.len() > 1;
        let mut widths: Vec<usize> = Vec::new();
        let mut row: u32 = 0;
        for section in &report.sections {
            if titled {
                let title = if section.title.is_empty() {
                    &report.title
                } else {
                    &section.title
                };
                sheet.write_string_with_format(row, 0, excel_text(title), &bold)?;
                row += 1;
            }
            for (c, column) in section.columns.iter().enumerate() {
                let c16 = col(c)?;
                sheet.write_string_with_format(row, c16, excel_text(&column.header), &bold)?;
                widen(&mut widths, c, column.header.chars().count());
            }
            if !titled {
                sheet.set_freeze_panes(row + 1, 0)?;
            }
            row += 1;
            for data in &section.rows {
                for (c, cell) in data.cells.iter().enumerate() {
                    let c16 = col(c)?;
                    match cell {
                        Cell::Empty => continue,
                        Cell::Text(text) => {
                            sheet.write_string(row, c16, excel_text(text))?;
                        }
                        Cell::Int(value) => {
                            sheet.write_number(row, c16, *value as f64)?;
                        }
                        Cell::Number(value) => {
                            sheet.write_number(row, c16, *value)?;
                        }
                    }
                    widen(&mut widths, c, cell.to_string().chars().count());
                }
                row += 1;
            }
            row += 1;
        }
        for (c, chars) in widths.into_iter().enumerate() {
            sheet.set_column_width(col(c)?, (chars + 2).clamp(6, 60) as f64)?;
        }
    }
    if reports.is_empty() {
        workbook.add_worksheet();
    }
    Ok(workbook.save_to_buffer()?)
}

/// [`xlsx_bytes`] written to a file. The workbook is written next to the
/// target first and then renamed over it, so a failed export never leaves a
/// truncated file in place of the previous one.
pub fn save_xlsx(reports: &[&ReportOutput], path: impl AsRef<Path>) -> Result<(), ExportError> {
    let path = path.as_ref();
    let bytes = xlsx_bytes(reports)?;
    let mut temp = path.as_os_str().to_owned();
    temp.push(".partial");
    let temp = std::path::PathBuf::from(temp);
    let written = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    Ok(written?)
}

/// Excel's limit on the characters in one cell.
const EXCEL_CELL_CHARS: usize = 32_767;

/// Text as Excel takes it: cut to the cell limit with a trailing `…`, so
/// an oversized cell never blocks the export.
fn excel_text(text: &str) -> String {
    if text.chars().count() <= EXCEL_CELL_CHARS {
        text.to_owned()
    } else {
        let mut cut: String = text.chars().take(EXCEL_CELL_CHARS - 1).collect();
        cut.push('…');
        cut
    }
}

fn col(index: usize) -> Result<u16, XlsxError> {
    u16::try_from(index).map_err(|_| XlsxError::RowColumnLimitError)
}

fn widen(widths: &mut Vec<usize>, column: usize, chars: usize) {
    if widths.len() <= column {
        widths.resize(column + 1, 0);
    }
    widths[column] = widths[column].max(chars);
}

/// How to write CSV.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsvOptions {
    /// `,` by default; `;` is what Excel expects in locales with a decimal
    /// comma (German, Dutch).
    pub delimiter: char,
    /// Start with a UTF-8 byte order mark so Excel reads `mm²` correctly.
    pub bom: bool,
    /// Keep spreadsheets from evaluating text as a formula: a text field
    /// starting with `=`, `+`, `-`, `@`, tab or CR (every IEC 81346 tag:
    /// `=A1+CAB-K1`, `-K1`) gets a leading `'`. Excel shows that apostrophe
    /// in the cell; for clean cells use the xlsx export. Turn this off for
    /// CSV read by other programs. Numbers are never changed.
    pub excel_safe: bool,
}

impl Default for CsvOptions {
    fn default() -> Self {
        CsvOptions {
            delimiter: ',',
            bom: true,
            excel_safe: true,
        }
    }
}

fn csv_field(text: &str, delimiter: char) -> String {
    if text.contains([delimiter, '"', '\n', '\r']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// A text field as written, neutralised when `excel_safe` asks for it.
fn csv_text(text: &str, options: &CsvOptions) -> String {
    if options.excel_safe && text.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{text}")
    } else {
        text.to_owned()
    }
}

fn csv_line(fields: impl IntoIterator<Item = String>, options: &CsvOptions, out: &mut String) {
    let line: Vec<String> = fields
        .into_iter()
        .map(|f| csv_field(&f, options.delimiter))
        .collect();
    out.push_str(&line.join(&options.delimiter.to_string()));
    out.push_str("\r\n");
}

fn csv_section(section: &Section, options: &CsvOptions, out: &mut String) {
    csv_line(
        section.columns.iter().map(|c| csv_text(&c.header, options)),
        options,
        out,
    );
    for row in &section.rows {
        let fields = row
            .cells
            .iter()
            .map(|cell| match (cell, options.delimiter) {
                // With `;` as the delimiter the locale writes decimals with a comma.
                (Cell::Number(value), ';') if section.decimal == '.' => {
                    value.to_string().replace('.', ",")
                }
                (Cell::Text(text), _) => csv_text(text, options),
                _ => section.cell_text(cell),
            });
        csv_line(fields, options, out);
    }
}

/// One report as CSV (RFC 4180, CRLF line ends). With several sections each
/// starts with its title on a line of its own, after a blank line.
pub fn to_csv(report: &ReportOutput, options: &CsvOptions) -> String {
    let mut out = String::new();
    if options.bom {
        out.push('\u{feff}');
    }
    let titled = report.sections.len() > 1;
    for (i, section) in report.sections.iter().enumerate() {
        if titled {
            if i > 0 {
                out.push_str("\r\n");
            }
            let title = if section.title.is_empty() {
                report.title.clone()
            } else {
                section.title.clone()
            };
            csv_line([csv_text(&title, options)], options, &mut out);
        }
        csv_section(section, options, &mut out);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::sheet_name;

    #[test]
    fn sheet_names_excel_accepts() {
        assert_eq!(sheet_name("I/O list", &[]), "IO list");
        assert_eq!(sheet_name("a / b: c?", &[]), "a _ b_ c_");
        assert_eq!(
            sheet_name("Wire list", &["Wire list".into()]),
            "Wire list (2)"
        );
    }
}
