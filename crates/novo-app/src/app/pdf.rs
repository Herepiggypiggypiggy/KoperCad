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
        let path = dir
            .join(super::project_file_name(&name))
            .with_extension("pdf");
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
