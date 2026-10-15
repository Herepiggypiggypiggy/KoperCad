//! Spreadsheet export in the app (novo-interop `sheets`): File > Export
//! spreadsheet writes the devices, terminals, wires and cables to one
//! `.xlsx` next to the project; the CSV variant writes one file per table.
//! Edit in Excel, then bring it back with the import dialog.
//!
//! Like the PDF, a saved project's export goes next to its file and takes
//! its name (`Plant 2.novo` gives `Plant 2 sheets.xlsx`). Only a file this
//! export wrote is replaced (an `.xlsx` that reads back as an export, a CSV
//! that starts with the `Id` column); anything else takes the next free
//! name, so the user's own files are never overwritten.

use std::path::{Path, PathBuf};

use novo_interop::sheets::{CsvOptions, export_project, read_xlsx, to_csv, xlsx_bytes};

use super::pdf::write_replacing;
use super::{Level, NovoApp, projects_dir};

impl NovoApp {
    /// Directory and file stem for exports of the open project.
    fn export_base(&mut self) -> Option<(PathBuf, String)> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        match file {
            Some(file) => {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                Some((file.parent()?.to_owned(), stem))
            }
            None => {
                let Some(dir) = projects_dir() else {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                };
                let stem = Path::new(&super::project_file_name(&name))
                    .with_extension("")
                    .to_string_lossy()
                    .into_owned();
                Some((dir, stem))
            }
        }
    }

    /// Exports the devices, terminals, wires and cables to one `.xlsx`.
    pub(super) fn export_spreadsheet(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = ours_or_free(&dir, &format!("{stem} sheets"), "xlsx", is_our_xlsx);
        self.export_spreadsheet_to(&path)
    }

    /// Exports to `path`.
    pub fn export_spreadsheet_to(&mut self, path: &Path) -> Option<PathBuf> {
        let replaced = path.exists();
        let book = export_project(self.backend.document()?.project());
        let result = xlsx_bytes(&book)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_replacing(path, &bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message(Level::Info, format!("{verb} {}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the spreadsheet: {e}"),
                );
                None
            }
        }
    }

    /// Exports each table as its own CSV file (`<stem> Devices.csv`, ...),
    /// semicolon separated with a byte order mark, as Excel in a decimal
    /// comma locale opens them.
    pub(super) fn export_spreadsheet_csv(&mut self) -> Vec<PathBuf> {
        let Some((dir, stem)) = self.export_base() else {
            return Vec::new();
        };
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let book = export_project(doc.project());
        let options = CsvOptions {
            delimiter: ';',
            ..CsvOptions::default()
        };
        let mut written = Vec::new();
        for table in &book.tables {
            let name = format!("{stem} {}", table.kind.name());
            let path = ours_or_free(&dir, &name, "csv", is_our_csv);
            let text = to_csv(table, &options);
            match write_replacing(&path, text.as_bytes()) {
                Ok(()) => written.push(path),
                Err(e) => {
                    self.message(
                        Level::Error,
                        format!("Could not write {}: {e}", path.display()),
                    );
                    return written;
                }
            }
        }
        if let Some(last) = written.last() {
            self.message(
                Level::Info,
                format!(
                    "Exported {} CSV files to {}",
                    written.len(),
                    last.parent().unwrap_or(&dir).display()
                ),
            );
            self.wb.last_export = Some(last.clone());
        }
        written
    }
}

/// An `.xlsx` that reads back as one of our exports.
fn is_our_xlsx(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| read_xlsx(&bytes).is_ok())
}

/// A CSV that starts (after the byte order mark) with the `Id` column.
fn is_our_csv(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        let text = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&bytes);
        text.starts_with(b"Id;") || text.starts_with(b"Id,") || text.starts_with(b"Id\t")
    })
}

/// `<dir>/<base>.<ext>` when free or ours to replace, else the first free
/// or ours of `<base> 2.<ext>`, `<base> 3.<ext>`...
fn ours_or_free(dir: &Path, base: &str, ext: &str, ours: fn(&Path) -> bool) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{base}.{ext}")
        } else {
            format!("{base} {n}.{ext}")
        };
        let path = dir.join(name);
        if !path.exists() || ours(&path) {
            return path;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_own_exports_are_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let theirs = dir.path().join("Plant sheets.xlsx");
        std::fs::write(&theirs, b"not an export").unwrap();
        let next = ours_or_free(dir.path(), "Plant sheets", "xlsx", is_our_xlsx);
        assert_eq!(next, dir.path().join("Plant sheets 2.xlsx"));

        let book = novo_interop::sheets::Workbook {
            tables: vec![novo_interop::sheets::Table::new(
                novo_interop::sheets::TableKind::Devices,
                vec!["Id".into(), "Tag".into()],
            )],
            ..Default::default()
        };
        std::fs::write(&next, xlsx_bytes(&book).unwrap()).unwrap();
        assert_eq!(
            ours_or_free(dir.path(), "Plant sheets", "xlsx", is_our_xlsx),
            next
        );

        let csv = dir.path().join("Plant Devices.csv");
        std::fs::write(&csv, "\u{feff}Id;Tag\r\n").unwrap();
        assert_eq!(
            ours_or_free(dir.path(), "Plant Devices", "csv", is_our_csv),
            csv
        );
        std::fs::write(&csv, "Name;Qty\r\n").unwrap();
        assert_eq!(
            ours_or_free(dir.path(), "Plant Devices", "csv", is_our_csv),
            dir.path().join("Plant Devices 2.csv")
        );
    }
}
