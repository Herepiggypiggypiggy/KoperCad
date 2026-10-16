//! Opening a file the user picked, whatever its type.

use super::files::{SheetError, read_csv, read_xlsx};
use super::grid::Workbook;

/// Reads the file named `file_name` (only its extension counts):
/// `.xlsx`/`.xlsm` as an Excel workbook, `.csv`/`.tsv`/`.txt` as text
/// (UTF-8, else Windows-1252 as Excel saves it).
pub fn read_file(file_name: &str, bytes: &[u8]) -> Result<Workbook, SheetError> {
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "xlsx" | "xlsm" => read_xlsx(bytes),
        "csv" | "tsv" | "txt" => {
            let text = match std::str::from_utf8(bytes) {
                Ok(s) => s.to_owned(),
                Err(_) => bytes.iter().map(|&b| crate::dxf::cp1252(b)).collect(),
            };
            read_csv(&text, None)
        }
        other => Err(SheetError::UnknownFormat(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_is_utf8_or_windows_1252_and_other_files_are_refused() {
        let utf8 = read_file(
            "d.CSV",
            "Id;Tag;Description\n1;-K1;Sch\u{fc}tz\n".as_bytes(),
        )
        .unwrap();
        let latin = read_file("d.csv", b"Id;Tag;Description\n1;-K1;Sch\xfctz\n").unwrap();
        assert_eq!(utf8.tables[0].rows[0].cells, latin.tables[0].rows[0].cells);
        assert!(matches!(
            read_file("d.docx", b""),
            Err(SheetError::UnknownFormat(e)) if e == "docx"
        ));
        assert!(read_file("d.xlsx", b"not a zip").is_err());
    }
}
