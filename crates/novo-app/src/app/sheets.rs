//! Spreadsheet export in the app (novo-interop sheets). Export >
//! Spreadsheet writes the devices, terminals, wires and cables of the
//! project as an Excel workbook next to the project file
//! (`Plant 2.novo` gives `Plant 2 tables.xlsx`), like the PDF export, with
//! no dialog: editing the workbook and importing it back is the round
//! trip. Unlike the PDF an existing workbook is never replaced, because
//! the user may have edited it: the next export takes `... tables 2.xlsx`.

use std::path::{Path, PathBuf};

use super::{Level, NovoApp, projects_dir};

/// `<dir>/<stem> tables.xlsx`, or `<stem> tables 2.xlsx`... when taken.
fn free_workbook(dir: &Path, stem: &str) -> PathBuf {
    let mut n = 1;
    loop {
        let name = if n == 1 {
            format!("{stem} tables.xlsx")
        } else {
            format!("{stem} tables {n}.xlsx")
        };
        let path = dir.join(name);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

impl NovoApp {
    /// Exports the project's tables to a new workbook (see the module
    /// notes). Returns the file written.
    pub(super) fn export_sheet(&mut self) -> Option<PathBuf> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let (dir, stem) = match file {
            Some(file) => (
                file.parent().map(Path::to_owned).unwrap_or_default(),
                file.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name),
            ),
            None => {
                let Some(dir) = projects_dir() else {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                };
                let stem = Path::new(&super::project_file_name(&name))
                    .with_extension("")
                    .to_string_lossy()
                    .into_owned();
                (dir, stem)
            }
        };
        self.export_sheet_to(&free_workbook(&dir, &stem))
    }

    /// Exports the project's tables to `path`.
    pub fn export_sheet_to(&mut self, path: &Path) -> Option<PathBuf> {
        let doc = self.backend.document()?;
        let workbook = novo_interop::sheets::export_project(doc.project());
        let result = novo_interop::sheets::xlsx_bytes(&workbook)
            .map_err(|e| e.to_string())
            .and_then(|bytes| {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            });
        match result {
            Ok(()) => {
                self.message(Level::Info, format!("Exported {}", path.display()));
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export the tables: {e}"));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workbook_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let first = free_workbook(dir.path(), "Plant");
        assert_eq!(first, dir.path().join("Plant tables.xlsx"));
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(
            free_workbook(dir.path(), "Plant"),
            dir.path().join("Plant tables 2.xlsx")
        );
    }
}
