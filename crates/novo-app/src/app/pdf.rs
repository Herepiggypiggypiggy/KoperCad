//! PDF export in the app (novo-pdf, WS-09), never blocked by problems.
//! A saved project's PDF goes next to its file and takes its name
//! (`Plant 2.novo` gives `Plant 2.pdf`), so projects with the same title
//! never share a PDF; a project with no file yet exports to
//! `<Documents>\NovoElectric\<name>.pdf` under a name not used yet.

use std::path::{Path, PathBuf};

use novo_checks::Summary;

use super::{Level, NovoApp, projects_dir};

impl NovoApp {
    /// Exports every page to the project's PDF (see the module notes).
    pub(super) fn export_pdf(&mut self) -> Option<PathBuf> {
        let name = self.project_name()?;
        // An untitled project's file is a temporary one; it has no name yet.
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let path = match file {
            Some(file) => file.with_extension("pdf"),
            None => {
                let Some(dir) = projects_dir() else {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                };
                free_path(&dir, &name)
            }
        };
        self.export_pdf_to(&path)
    }

    /// Exports every page to `path`.
    pub fn export_pdf_to(&mut self, path: &Path) -> Option<PathBuf> {
        let summary = Summary::of(self.problems());
        let replaced = path.exists();
        let options = novo_pdf::PdfOptions {
            notes: self.xref_notes_by_page(),
            ..Default::default()
        };
        let doc = self.backend.document()?;
        let art = self
            .library()
            .map(|lib| novo_editor::art::LibraryArt::new(lib, options.style.symbol));
        let result = std::fs::create_dir_all(path.parent().unwrap_or(path))
            .map_err(novo_pdf::PdfError::from)
            .and_then(|()| match &art {
                Some(art) => novo_pdf::write_pdf(doc.project(), &options, art, path),
                None => novo_pdf::write_pdf(
                    doc.project(),
                    &options,
                    &novo_canvas::model::PlaceholderArt,
                    path,
                ),
            });
        match result {
            Ok(()) => {
                let remaining = if summary.is_clean() {
                    String::new()
                } else {
                    format!(" ({} remain)", summary.pill_text())
                };
                // Overwriting is never silent.
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message(Level::Info, format!("{verb} {}{remaining}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export the PDF: {e}"));
                None
            }
        }
    }
}

/// `<dir>/<name>.pdf`, or `<name> 2.pdf`, `<name> 3.pdf`... when taken, so
/// an untitled project never replaces another project's PDF.
fn free_path(dir: &Path, name: &str) -> PathBuf {
    let base = Path::new(&super::project_file_name(name))
        .with_extension("")
        .to_string_lossy()
        .into_owned();
    let mut n = 1;
    loop {
        let file = if n == 1 {
            format!("{base}.pdf")
        } else {
            format!("{base} {n}.pdf")
        };
        let path = dir.join(file);
        if !path.exists() {
            return path;
        }
        n += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::free_path;

    #[test]
    fn an_untitled_export_never_takes_a_used_name() {
        let dir = tempfile::tempdir().unwrap();
        let first = free_path(dir.path(), "Pumps: v1");
        assert_eq!(first, dir.path().join("Pumps_ v1.pdf"));
        std::fs::write(&first, b"x").unwrap();
        assert_eq!(
            free_path(dir.path(), "Pumps: v1"),
            dir.path().join("Pumps_ v1 2.pdf")
        );
    }
}
