//! PDF export in the app (novo-pdf, WS-09), never blocked by problems.
//! A saved project's PDF goes next to its file and takes its name
//! (`Plant 2.novo` gives `Plant 2.pdf`), so projects with the same title
//! never share a PDF; a project with no file yet exports to
//! `<Documents>\KoperCAD\<name>.pdf` under a name not used yet.
//!
//! Only a PDF KoperCAD wrote (its `/Producer`) is ever replaced: a
//! user's own `<stem>.pdf` stays, and the export takes `<stem> 2.pdf`.
//! The PDF is written to a temporary file first and renamed into place, so
//! a failed export never leaves a half-written file.

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
            Some(file) => replaceable_path(&file.with_extension("pdf")),
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

    /// Exports only the page on screen, to `<project> page <name>.pdf` next
    /// to the project (a project with no file yet: in Documents).
    pub(super) fn export_pdf_page(&mut self) -> Option<PathBuf> {
        let page = self.current_page?;
        let page_name = self
            .backend
            .document()?
            .project()
            .page(page)
            .map(|p| p.name.to_string())?;
        let (dir, stem) = self.export_base()?;
        let file = format!("{stem} page {page_name}.pdf").replace(['/', '\\', ':'], "-");
        let path = replaceable_path(&dir.join(file));
        self.export_pdf_pages_to(&path, novo_pdf::PageSet::Pages(vec![page]))
    }

    /// The design checks as report pages after the drawing. A clean
    /// project prints none, so its PDF is the drawing alone.
    fn checks_appendix(&self) -> Option<novo_pdf::Appendix> {
        let lines = novo_checks::report_lines(self.backend.document()?.project());
        // Headline and summary only: nothing to report.
        (lines.len() > 2).then(|| novo_pdf::Appendix {
            title: "Design checks".into(),
            lines: lines
                .into_iter()
                .map(|(bold, text)| novo_pdf::AppendixLine { text, bold })
                .collect(),
        })
    }

    /// Exports every page to `path`.
    pub fn export_pdf_to(&mut self, path: &Path) -> Option<PathBuf> {
        self.export_pdf_pages_to(path, novo_pdf::PageSet::All)
    }

    /// Exports `pages` to `path`.
    pub fn export_pdf_pages_to(
        &mut self,
        path: &Path,
        pages: novo_pdf::PageSet,
    ) -> Option<PathBuf> {
        let summary = Summary::of(self.problems());
        let replaced = path.exists();
        let options = novo_pdf::PdfOptions {
            pages,
            notes: self.xref_notes_by_page(),
            title_revisions: self.all_title_revisions(),
            changed_pages: self.pages_changed_since_latest(),
            appendix: self.checks_appendix(),
            ..Default::default()
        };
        let doc = self.backend.document()?;
        let art = self
            .library()
            .map(|lib| novo_editor::art::LibraryArt::new(lib, options.style.symbol));
        let result = match &art {
            Some(art) => novo_pdf::export_pdf_with_art(doc.project(), &options, art),
            None => novo_pdf::export_pdf_with_art(
                doc.project(),
                &options,
                &novo_canvas::model::PlaceholderArt,
            ),
        }
        .and_then(|bytes| write_replacing(path, &bytes).map_err(novo_pdf::PdfError::from));
        match result {
            Ok(()) => {
                let remaining = if summary.is_clean() {
                    String::new()
                } else {
                    format!(" ({} remain)", summary.pill_text())
                };
                // Overwriting is never silent.
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(format!("{verb} {}{remaining}", path.display()), &path);
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

/// Bytes KoperCAD puts in every PDF it writes, and the ones it wrote under
/// its earlier name.
const OUR_PRODUCERS: [&[u8]; 2] = [b"/Producer (KoperCAD)", b"/Producer (NovoElectric)"];

/// Whether `path` is a PDF this app wrote and nobody changed since, so
/// replacing it loses nothing the user made. A viewer that saves comments
/// or markup appends an incremental update with its own `%%EOF` and keeps
/// our Producer, so a second `%%EOF` means the file is the user's now.
fn is_ours(path: &Path) -> bool {
    std::fs::read(path).is_ok_and(|bytes| {
        let count = |pattern: &[u8]| {
            bytes
                .windows(pattern.len())
                .filter(|w| *w == pattern)
                .count()
        };
        OUR_PRODUCERS.iter().map(|p| count(p)).sum::<usize>() == 1 && count(b"%%EOF") == 1
    })
}

/// `path` itself when it is free or ours to replace, else the first free or
/// ours of `<stem> 2.pdf`, `<stem> 3.pdf`...
pub(super) fn replaceable_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            path.to_owned()
        } else {
            path.with_file_name(format!("{stem} {n}.pdf"))
        };
        if !candidate.exists() || is_ours(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Writes `bytes` to a temporary file beside `path`, then renames it over
/// `path`. The temporary file is removed when anything fails.
pub(super) fn write_replacing(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{}.part", std::process::id()));
    let temp = path.with_file_name(name);
    let written = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

/// `<dir>/<name>.pdf`, or `<name> 2.pdf`, `<name> 3.pdf`... when taken, so
/// an untitled project never replaces another project's PDF.
pub(super) fn free_path(dir: &Path, name: &str) -> PathBuf {
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
    use super::*;

    #[test]
    fn only_our_own_pdfs_are_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Plant.pdf");
        assert_eq!(replaceable_path(&path), path, "free");

        let project = novo_core::Project::new("Plant");
        let ours = novo_pdf::export_pdf_with_art(
            &project,
            &Default::default(),
            &novo_canvas::model::PlaceholderArt,
        )
        .unwrap();
        write_replacing(&path, &ours).unwrap();
        assert!(is_ours(&path));
        assert_eq!(replaceable_path(&path), path, "ours");

        std::fs::write(&path, b"%PDF-1.7 the user's own drawing").unwrap();
        let next = dir.path().join("Plant 2.pdf");
        assert_eq!(replaceable_path(&path), next, "theirs stays");
        write_replacing(&next, &ours).unwrap();
        assert_eq!(
            replaceable_path(&path),
            next,
            "and the next export reuses ours"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            b"%PDF-1.7 the user's own drawing"
        );
    }

    #[test]
    fn an_export_marked_up_in_a_viewer_is_the_users() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Plant.pdf");
        let project = novo_core::Project::new("Plant");
        let mut bytes = novo_pdf::export_pdf_with_art(
            &project,
            &Default::default(),
            &novo_canvas::model::PlaceholderArt,
        )
        .unwrap();
        // What a viewer's "Save" appends after adding a comment.
        bytes.extend_from_slice(
            b"\n99 0 obj\n<< /Type /Annot /Subtype /Text /Contents (check K1) >>\nendobj\n\
              xref\n99 1\n0000000000 00000 n \ntrailer\n<< /Size 100 >>\nstartxref\n0\n%%EOF\n",
        );
        std::fs::write(&path, &bytes).unwrap();
        assert!(!is_ours(&path));
        assert_eq!(replaceable_path(&path), dir.path().join("Plant 2.pdf"));
    }

    #[test]
    fn a_failed_write_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        // A folder where the PDF should go: the rename cannot replace it.
        let path = dir.path().join("Plant.pdf");
        std::fs::create_dir(&path).unwrap();
        assert!(write_replacing(&path, b"%PDF").is_err());
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("Plant.pdf")]);
    }

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
