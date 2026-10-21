//! Where the person's Anthropic API key lives.
//!
//! The key is kept in one small file under the user's own profile
//! (`%LOCALAPPDATA%\KoperCAD\ai\api-key` on Windows): no admin rights, never
//! inside a project file, never printed or logged. The file is not encrypted;
//! it is protected the way the rest of the profile is (owner-only permissions
//! on Linux and macOS, the profile's own access rules on Windows), and the
//! app says so. `ANTHROPIC_API_KEY` in the environment is used when no key is
//! stored, so scripted runs need no file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The folder KoperCAD keeps its Claude integration files in.
pub fn default_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("KoperCAD").join("ai"))
}

/// Where a key came from, for the settings screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySource {
    Stored,
    Environment,
}

/// An API key. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(String);

impl std::fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ApiKey({})", self.masked())
    }
}

impl ApiKey {
    /// Accepts a pasted key: surrounding space is dropped, and it must look
    /// like an Anthropic key (`sk-ant-…`, no inner whitespace).
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let t = text.trim();
        if t.is_empty() {
            return Err(KeyError::Empty);
        }
        if !t.starts_with("sk-ant-") || t.chars().any(char::is_whitespace) || t.len() < 20 {
            return Err(KeyError::NotAnAnthropicKey);
        }
        Ok(Self(t.to_string()))
    }

    /// The secret, for the request header only.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// `sk-ant-…abcd`: enough to recognise the key, not to use it.
    pub fn masked(&self) -> String {
        let tail: String = self
            .0
            .chars()
            .rev()
            .take(4)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        format!("sk-ant-…{tail}")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("The key is empty.")]
    Empty,
    #[error("That does not look like an Anthropic API key (it starts with sk-ant-).")]
    NotAnAnthropicKey,
    #[error("Could not write the key file: {0}")]
    Io(#[from] io::Error),
}

/// Reads and writes the key in one folder.
#[derive(Clone, Debug)]
pub struct KeyStore {
    dir: PathBuf,
}

impl KeyStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The store in the user's profile, if the platform has one.
    pub fn in_profile() -> Option<Self> {
        default_dir().map(Self::new)
    }

    pub fn path(&self) -> PathBuf {
        self.dir.join("api-key")
    }

    /// The stored key, else `ANTHROPIC_API_KEY`.
    pub fn load(&self) -> Option<(ApiKey, KeySource)> {
        if let Ok(text) = fs::read_to_string(self.path())
            && let Ok(k) = ApiKey::parse(&text)
        {
            return Some((k, KeySource::Stored));
        }
        let env = std::env::var("ANTHROPIC_API_KEY").ok()?;
        ApiKey::parse(&env)
            .ok()
            .map(|k| (k, KeySource::Environment))
    }

    pub fn save(&self, key: &ApiKey) -> Result<(), KeyError> {
        fs::create_dir_all(&self.dir)?;
        write_private(&self.path(), key.expose())?;
        Ok(())
    }

    /// Deletes the stored key (the environment variable is not ours to touch).
    pub fn remove(&self) -> io::Result<()> {
        match fs::remove_file(self.path()) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(text.as_bytes())?;
    // `mode` applies only when the file is created; tighten one that existed.
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> io::Result<()> {
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "sk-ant-api03-abcdefghijklmnop1234";

    #[test]
    fn a_pasted_key_is_trimmed_and_checked() {
        assert!(ApiKey::parse(&format!("  {KEY}\n")).is_ok());
        assert!(matches!(ApiKey::parse(""), Err(KeyError::Empty)));
        assert!(matches!(
            ApiKey::parse("sk-proj-abcdefghijklmnopqrstuvwxyz"),
            Err(KeyError::NotAnAnthropicKey)
        ));
        assert!(matches!(
            ApiKey::parse("sk-ant-has a space inside it xxx"),
            Err(KeyError::NotAnAnthropicKey)
        ));
    }

    #[test]
    fn debug_and_mask_never_show_the_secret() {
        let k = ApiKey::parse(KEY).unwrap();
        assert!(!format!("{k:?}").contains("abcdefghijklmnop"));
        assert_eq!(k.masked(), "sk-ant-…1234");
    }

    #[test]
    fn the_key_round_trips_through_the_store_and_can_be_removed() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::new(dir.path().join("ai"));
        store.save(&ApiKey::parse(KEY).unwrap()).unwrap();
        let (k, src) = store.load().unwrap();
        assert_eq!(k.expose(), KEY);
        assert_eq!(src, KeySource::Stored);
        store.remove().unwrap();
        store.remove().unwrap();
        assert!(!store.path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn the_key_file_is_owner_only_even_when_it_existed_with_looser_rights() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::new(dir.path());
        fs::write(store.path(), "old").unwrap();
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).unwrap();
        store.save(&ApiKey::parse(KEY).unwrap()).unwrap();
        let mode = fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0);
    }
}
