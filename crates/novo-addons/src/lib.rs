//! Add-on manager core (lane C0b): `.kcaddon` packages, the per-user install registry, version
//! compatibility, permissions and (lane C0b-2) the add-on engine host. Plan:
//! `plans/controller-live-view.md` section 5b; where add-ons appear in the program:
//! `docs/design/visual-redesign-addons.md`; user and author guide: `docs/design/addons.md`.
//!
//! Pure: no UI, no network, no admin rights. Everything is per user, under a root folder the
//! caller names (`%LOCALAPPDATA%\KoperCAD\addons` in the app). A package is read and verified in
//! memory first ([`Package::read`]); only a package that passed is written to disk
//! ([`Store::install`]), and what is on disk is checked again each time the program starts.
//!
//! The base program knows nothing about any particular add-on. An add-on adds rows to tables the
//! base owns: commands, panels, data files for kinds the base reads, icons.

pub mod error;
pub mod extensions;
pub mod icons;
pub mod layout;
pub mod manifest;
pub mod pack;
pub mod package;
pub mod report;
pub mod store;
pub mod trust;
pub mod version;

pub use error::{AddonError, Result};

/// The folder add-ons live in, under the per-user local data folder
/// (`%LOCALAPPDATA%` on Windows): `<local>/KoperCAD/addons`.
pub fn default_root(local_data: &std::path::Path) -> std::path::PathBuf {
    local_data.join("KoperCAD").join("addons")
}
pub use extensions::{ExtCommand, ExtPanel, Extensions, Home};
pub use manifest::{API_VERSION, Manifest, Permission};
pub use package::{Limits, Package};
pub use store::{
    AddonSet, AppInfo, Change, Compat, Entry, Inspection, InstallOptions, Status, Store,
};
pub use trust::{Trust, TrustStore};
pub use version::{Requirement, Version};
