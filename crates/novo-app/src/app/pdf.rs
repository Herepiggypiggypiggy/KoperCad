//! PDF export in the app (novo-pdf, WS-09): every page to
//! `<Documents>\NovoElectric\<project>.pdf`, never blocked by problems.

use std::path::PathBuf;

use novo_checks::Summary;

use super::{Level, NovoApp, projects_dir};

impl NovoApp {
    /// Exports every page to `<Documents>\NovoElectric\<project>.pdf`.
    /// Never blocked by problems; the toast says how many remain.
    pub(super) fn export_pdf(&mut self) -> Option<PathBuf> {
        let name = self.project_name()?;
        let Some(dir) = projects_dir() else {
            self.message(Level::Error, "Could not find the Documents folder");
            return None;
        };
        // An untitled project's file is a temporary one; it has no name yet.
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let path = dir.join(pdf_file_name(file.as_deref(), &name));
        self.export_pdf_to(&path)
    }

    /// Exports every page to `path`.
    pub fn export_pdf_to(&mut self, path: &std::path::Path) -> Option<PathBuf> {
        let summary = Summary::of(self.problems());
        let doc = self.backend.document()?;
        let options = novo_pdf::PdfOptions::default();
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
                self.message(
                    Level::Info,
                    format!("Exported {}{remaining}", path.display()),
                );
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

/// The PDF's file name: the project file's own name (`Plant 2.novo` gives
/// `Plant 2.pdf`), so two projects with the same title never overwrite
/// each other's PDF; the project name while it has no file yet.
fn pdf_file_name(file: Option<&std::path::Path>, name: &str) -> String {
    match file.and_then(|f| f.file_stem()) {
        Some(stem) => format!("{}.pdf", stem.to_string_lossy()),
        None => std::path::Path::new(&super::project_file_name(name))
            .with_extension("pdf")
            .to_string_lossy()
            .into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::pdf_file_name;
    use std::path::Path;

    #[test]
    fn the_pdf_is_named_after_the_project_file() {
        let a = pdf_file_name(Some(Path::new("/p/a/Plant.novo")), "Pumps");
        let b = pdf_file_name(Some(Path::new("/p/b/Plant 2.novo")), "Pumps");
        assert_eq!(a, "Plant.pdf");
        assert_eq!(b, "Plant 2.pdf");
        assert_eq!(pdf_file_name(None, "Pumps: v1"), "Pumps_ v1.pdf");
    }
}
