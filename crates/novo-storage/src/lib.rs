//! Project file (owned by WS-04): one SQLite file per project, autosave,
//! crash recovery and schema migrations.
//!
//! A project is one `.novo` file, a SQLite database with one row per model
//! object (the object as JSON) plus a little metadata. SQLite gives atomic
//! saves for free: a crash or power cut mid-save leaves the last good save.
//!
//! - [`ProjectFile`] is the file itself: create, open (locked against a
//!   second writer, upgraded if older), load, full save and incremental save.
//! - [`Session`] is what the app holds while a project is open: it tracks
//!   unsaved changes, autosaves them to a recovery file next to the project
//!   and offers them back after a crash.
//! - [`schema`] holds the schema versions and how to add a migration.
//!
//! ```
//! use novo_core::{DeviceTag, Document};
//! use novo_storage::Session;
//!
//! let dir = tempfile::tempdir()?;
//! let path = dir.path().join("motor.novo");
//!
//! let mut doc = Document::default();
//! let mut session = Session::create(&path, doc.project())?;
//! let (_, changes) = doc.transact("Add contactor", |tx| {
//!     tx.add_device(DeviceTag::parse("-K1")?, "Main contactor")
//! })?;
//! session.record(&changes);
//! session.save(doc.project())?;
//! session.close()?;
//!
//! let opened = Session::open(&path)?;
//! assert_eq!(opened.project.to_json(), doc.project().to_json());
//! assert!(opened.recovered.is_none());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod file;
pub mod schema;
mod session;

pub use error::{Result, StorageError};
pub use file::{PROJECT_EXTENSION, ProjectFile};
pub use session::{
    AUTOSAVE_SUFFIX, DEFAULT_AUTOSAVE_INTERVAL, Opened, Recovered, Session, SetAside, autosave_path,
};
