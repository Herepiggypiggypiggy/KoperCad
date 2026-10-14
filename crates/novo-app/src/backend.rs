//! The hook between the shell and the project file.
//!
//! The open project is a [`novo_core::Document`]: the shell reads it and the
//! canvas edits it through `Document::transact`, so every edit shares one undo
//! history. A [`ProjectBackend`] owns that document and creates, opens and
//! saves it. `novo-storage` (WS-04) provides the file-backed implementation;
//! [`MemoryBackend`] keeps projects in memory until then.

use std::fmt;
use std::path::Path;

use novo_core::{Document, ModelError, PageId, Project};

#[derive(Debug)]
pub enum BackendError {
    /// The feature is not built yet.
    Unavailable(&'static str),
    Io(std::io::Error),
    Model(ModelError),
    /// The file exists but is not a project this version can read.
    InvalidProject(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::Unavailable(what) => write!(f, "{what} is not available yet"),
            BackendError::Io(e) => write!(f, "{e}"),
            BackendError::Model(e) => write!(f, "{e}"),
            BackendError::InvalidProject(why) => write!(f, "not a valid project: {why}"),
        }
    }
}

impl std::error::Error for BackendError {}

impl From<std::io::Error> for BackendError {
    fn from(e: std::io::Error) -> Self {
        BackendError::Io(e)
    }
}

impl From<ModelError> for BackendError {
    fn from(e: ModelError) -> Self {
        BackendError::Model(e)
    }
}

/// Owns the open project and moves it to and from disk.
///
/// At most one project is open at a time.
pub trait ProjectBackend {
    /// Starts a new, unsaved project, replacing any open one.
    fn new_project(&mut self, name: &str) -> Result<(), BackendError>;

    /// Opens a project file, replacing any open project.
    fn open(&mut self, path: &Path) -> Result<(), BackendError>;

    /// Writes the open project to `path`. On success the implementation must
    /// call `Document::mark_saved`, which clears the unsaved-changes flag.
    fn save(&mut self, path: &Path) -> Result<(), BackendError>;

    fn close(&mut self);

    fn document(&self) -> Option<&Document>;

    fn document_mut(&mut self) -> Option<&mut Document>;
}

/// Creates projects in memory so the shell works before the project file
/// exists. It cannot read or write files.
#[derive(Default)]
pub struct MemoryBackend {
    document: Option<Document>,
}

impl ProjectBackend for MemoryBackend {
    fn new_project(&mut self, name: &str) -> Result<(), BackendError> {
        self.document = Some(new_document(name)?);
        Ok(())
    }

    fn open(&mut self, _path: &Path) -> Result<(), BackendError> {
        Err(BackendError::Unavailable("Opening project files"))
    }

    fn save(&mut self, _path: &Path) -> Result<(), BackendError> {
        Err(BackendError::Unavailable("Saving project files"))
    }

    fn close(&mut self) {
        self.document = None;
    }

    fn document(&self) -> Option<&Document> {
        self.document.as_ref()
    }

    fn document_mut(&mut self) -> Option<&mut Document> {
        self.document.as_mut()
    }
}

/// A fresh project with one empty page, saved and with no undo history.
pub fn new_document(name: &str) -> Result<Document, ModelError> {
    let mut project = Project::new(name);
    project.transact("New project", |tx| tx.add_page("1"))?;
    Ok(Document::new(project))
}

/// Pages in display order, with the label the project panel shows.
pub fn page_labels(document: &Document) -> Vec<(PageId, String)> {
    document
        .project()
        .pages_in_order()
        .into_iter()
        .map(|p| {
            let label = if p.title.is_empty() {
                p.name.clone()
            } else {
                format!("{} {}", p.name, p.title)
            };
            (p.id, label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_document_has_one_page_and_is_clean() {
        let doc = new_document("Plant").unwrap();
        assert_eq!(doc.project().info().name, "Plant");
        assert_eq!(page_labels(&doc).len(), 1);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
    }
}
