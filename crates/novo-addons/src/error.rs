//! Errors of the add-on manager. Every message is written for the person at the screen: it says
//! what is wrong with the file and, where there is one, what to do.

use thiserror::Error;

/// Why a package could not be read, installed or loaded.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AddonError {
    /// The file is not a readable `.kcaddon` (not a zip, truncated, bad entry).
    #[error("this is not a readable add-on file: {0}")]
    NotAPackage(String),
    /// The package is over one of the safety limits (size, entry count, expansion).
    #[error("the add-on is too large or expands suspiciously ({0}); it was not opened")]
    TooLarge(String),
    /// An entry name that could write outside the add-on's folder, or that Windows cannot hold.
    #[error("the add-on contains an unsafe file name ({0}); it was not installed")]
    UnsafePath(String),
    /// `manifest.toml` is missing, unreadable or breaks a rule.
    #[error("the add-on's manifest is not valid: {0}")]
    Manifest(String),
    /// A file's SHA-256 differs from the manifest, or a file is missing or not listed.
    #[error("the add-on's files do not match its manifest ({0}); it may be damaged or changed")]
    Integrity(String),
    /// The signature file is present but does not verify.
    #[error("the add-on's signature is not valid ({0}); it was not installed")]
    BadSignature(String),
    /// The signature names a trusted key but does not fit the manifest.
    #[error(
        "the add-on was changed after it was signed, or was not signed by the holder of its key: {0}; it was not installed"
    )]
    Tampered(String),
    /// The add-on needs a different program or add-on API version.
    #[error("{0}")]
    Incompatible(String),
    /// An unsigned or untrusted add-on was not explicitly accepted.
    #[error(
        "this add-on is not signed by a publisher you trust; tick the box to install it anyway"
    )]
    NotAccepted,
    /// The same version is installed, or the file is older than the installed one.
    #[error("{0}")]
    Version(String),
    /// No such add-on is installed.
    #[error("no add-on named {0} is installed")]
    Unknown(String),
    /// Nothing older to go back to.
    #[error("{0} has no earlier version to go back to")]
    NoPrevious(String),
    /// The engine of an add-on could not be loaded or refused to start.
    #[error("the add-on's engine could not be started: {0}")]
    Engine(String),
    /// A signing key file or secret is unusable.
    #[error("signing key: {0}")]
    Key(String),
    /// The per-user folder could not be read or written.
    #[error("the add-on folder could not be used: {0}")]
    Io(String),
}

impl From<std::io::Error> for AddonError {
    fn from(e: std::io::Error) -> Self {
        AddonError::Io(e.to_string())
    }
}

/// Shorthand used across the crate.
pub type Result<T> = std::result::Result<T, AddonError>;
