//! Errors returned when opening, loading or saving a project file.

use std::path::PathBuf;

use novo_core::ModelError;
use thiserror::Error;

/// Why a project file could not be opened, read or written.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Another window or program has the file open.
    #[error("{} is already open in another window or program", .0.display())]
    Locked(PathBuf),
    /// The file is not a KoperCAD project (or not SQLite at all).
    #[error("{} is not a KoperCAD project file", .0.display())]
    NotAProject(PathBuf),
    /// The file was written by a newer KoperCAD that changed the format.
    #[error(
        "{} was saved by a newer version of KoperCAD (file format {found}, this version reads up to {supported})",
        path.display()
    )]
    NewerFormat {
        path: PathBuf,
        found: u32,
        supported: u32,
    },
    /// The file or its folder cannot be written, so it cannot be opened for
    /// editing; it can still be read.
    #[error(
        "{} is read-only (the file or its folder is write-protected). Open it read-only, or open it and use Save as to keep changes in a copy",
        .0.display()
    )]
    ReadOnly(PathBuf),
    /// The file holds a kind of object this version does not know, so it was
    /// saved by a newer KoperCAD that added one without a schema change.
    #[error(
        "{} holds a kind of object this version of KoperCAD does not know ({kind}); it was probably saved by a newer version",
        path.display()
    )]
    UnknownRecord { path: PathBuf, kind: String },
    /// The file is damaged or holds data the model refuses.
    #[error("{} is damaged: {reason}", path.display())]
    Corrupt { path: PathBuf, reason: String },
    /// The stored objects broke a model integrity rule while loading.
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Result type for storage operations.
pub type Result<T, E = StorageError> = std::result::Result<T, E>;

/// Whether a SQLite error means another connection holds the file.
pub(crate) fn is_busy(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)
    )
}

/// Whether a SQLite error means the file is not a database at all.
pub(crate) fn is_not_a_database(err: &rusqlite::Error) -> bool {
    matches!(
        err.sqlite_error_code(),
        Some(rusqlite::ErrorCode::NotADatabase)
    )
}

/// Whether a SQLite error on `path` means the file or its folder cannot be
/// written. "Cannot open" counts only when the file or its folder exists:
/// a missing folder, a bad name or a vanished drive is a different problem
/// and keeps its own error.
pub(crate) fn is_read_only(err: &rusqlite::Error, path: &std::path::Path) -> bool {
    match err.sqlite_error_code() {
        Some(rusqlite::ErrorCode::ReadOnly) => true,
        Some(rusqlite::ErrorCode::CannotOpen) => {
            path.exists() || path.parent().is_some_and(|dir| dir.is_dir())
        }
        _ => false,
    }
}
