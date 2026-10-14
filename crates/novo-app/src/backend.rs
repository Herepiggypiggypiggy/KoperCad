//! The hook between the shell and the project: model, undo history and file.
//!
//! The shell never touches the model directly. It asks a [`ProjectBackend`]
//! to create, open, save and undo, and reads back what it needs to draw its
//! panels. `novo-core` (WS-02) and `novo-storage` (WS-04) provide the real
//! implementation; [`PlaceholderBackend`] stands in until then.

use std::fmt;
use std::path::Path;

/// Identifies a page within the open project.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageId(pub u64);

/// What the project panel shows for one page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageInfo {
    pub id: PageId,
    pub title: String,
}

#[derive(Debug)]
pub enum BackendError {
    /// The feature is not built yet.
    Unavailable(&'static str),
    Io(std::io::Error),
    /// The file exists but is not a project this version can read.
    InvalidProject(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendError::Unavailable(what) => write!(f, "{what} is not available yet"),
            BackendError::Io(e) => write!(f, "{e}"),
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

/// Everything the shell needs from the open project.
///
/// At most one project is open at a time. Methods that read the project
/// return empty values when none is open.
pub trait ProjectBackend {
    /// Starts a new, unsaved project, replacing any open one.
    fn new_project(&mut self, name: &str) -> Result<(), BackendError>;

    /// Opens a project file, replacing any open project.
    fn open(&mut self, path: &Path) -> Result<(), BackendError>;

    /// Writes the open project to `path` and clears the unsaved flag.
    fn save(&mut self, path: &Path) -> Result<(), BackendError>;

    fn close(&mut self);

    fn is_open(&self) -> bool;

    /// Display name of the open project.
    fn project_name(&self) -> Option<String>;

    /// True when the project has changes that are not saved.
    fn is_dirty(&self) -> bool;

    fn pages(&self) -> Vec<PageInfo>;

    fn can_undo(&self) -> bool {
        false
    }
    fn can_redo(&self) -> bool {
        false
    }
    fn undo(&mut self) {}
    fn redo(&mut self) {}
}

/// Keeps a project in memory so the shell can be exercised before the model
/// and the project file exist. It cannot read or write files.
#[derive(Default)]
pub struct PlaceholderBackend {
    open: Option<String>,
}

impl ProjectBackend for PlaceholderBackend {
    fn new_project(&mut self, name: &str) -> Result<(), BackendError> {
        self.open = Some(name.to_owned());
        Ok(())
    }

    fn open(&mut self, _path: &Path) -> Result<(), BackendError> {
        Err(BackendError::Unavailable("Opening project files"))
    }

    fn save(&mut self, _path: &Path) -> Result<(), BackendError> {
        Err(BackendError::Unavailable("Saving project files"))
    }

    fn close(&mut self) {
        self.open = None;
    }

    fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn project_name(&self) -> Option<String> {
        self.open.clone()
    }

    fn is_dirty(&self) -> bool {
        false
    }

    fn pages(&self) -> Vec<PageInfo> {
        if self.open.is_some() {
            vec![PageInfo {
                id: PageId(1),
                title: "Page 1".to_owned(),
            }]
        } else {
            Vec::new()
        }
    }
}
