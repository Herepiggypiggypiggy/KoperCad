//! The change report (docs/DESIGN.md §7): a PDF listing what changed
//! between a revision and the project now, by page. It is written next to
//! the project as `<name> changes <index>.pdf` the way the drawing PDF is,
//! so it never replaces a file KoperCAD did not write.

use std::path::{Path, PathBuf};

use novo_core::PageId;
use novo_revisions::{Side, change_report, compare};

use super::pdf::{free_path, replaceable_path, write_replacing};
use super::{Level, NovoApp, projects_dir};

impl NovoApp {
    /// The `export.change_report` command: the changes since the latest
    /// revision, as a PDF.
    pub(super) fn export_change_report(&mut self) -> Option<PathBuf> {
        let name = self.project_name()?;
        let index = {
            let project = self.backend.document()?.project();
            let Some(latest) = project.latest_revision() else {
                self.message(
                    Level::Info,
                    "No revision yet: Issue revision first, then the change report compares with it",
                );
                return None;
            };
            latest.index.clone()
        };
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let path = match file {
            Some(file) => {
                let stem = file
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                replaceable_path(&file.with_file_name(format!("{stem} changes {index}.pdf")))
            }
            None => {
                let Some(dir) = projects_dir() else {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                };
                free_path(&dir, &format!("{name} changes {index}"))
            }
        };
        self.export_change_report_to(&path)
    }

    /// Writes the report of the changes since the latest revision to `path`.
    pub fn export_change_report_to(&mut self, path: &Path) -> Option<PathBuf> {
        let doc = self.backend.document()?;
        let project = doc.project();
        let latest = project.latest_revision()?;
        let (id, index) = (latest.id, latest.index.clone());
        let comparison = match compare(
            project,
            |r| Ok::<_, String>(self.backend.revision_snapshot(r)),
            Side::Revision(id),
            Side::Now,
        ) {
            Ok(c) => c,
            Err(e) => {
                self.message(Level::Warning, e.to_string());
                return None;
            }
        };
        let page_name = |page: PageId| {
            project
                .page(page)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "(removed)".to_owned())
        };
        let title = format!("{}: changes since revision {index}", project.info().name);
        let report = change_report(&comparison, &title, page_name);
        let options = novo_pdf::ReportPdfOptions {
            title: title.clone(),
            footer_right: format!("since revision {index}"),
            language: self.report_translator(),
            ..Default::default()
        };
        let bytes = novo_pdf::export_report_pdf(&[&report], &options);
        let replaced = path.exists();
        match write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                let none = if comparison.is_empty() {
                    " (nothing changed)"
                } else {
                    ""
                };
                self.message_file(format!("{verb} {}{none}", path.display()), &path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not write the report: {e}"));
                None
            }
        }
    }
}
