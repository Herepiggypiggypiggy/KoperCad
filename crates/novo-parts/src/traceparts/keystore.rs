//! The per-user TraceParts key. It lives in the user's roaming profile
//! (`%APPDATA%\KoperCAD\traceparts.json`), never in a project file, the
//! parts catalog, a library bundle or the repository.
//!
//! The file is plain JSON, not encrypted: encryption that a standard user
//! can use without admin rights (DPAPI) needs `unsafe` calls, which this
//! workspace denies. On Unix the file is made readable by the owner only;
//! on Windows it inherits the profile's ACL, which already limits it to the
//! user. The key never appears in `Debug` output or error messages.
//!
//! `KOPERCAD_TRACEPARTS_TENANT` and `KOPERCAD_TRACEPARTS_KEY` override the
//! file (scripts, CI), so a key need not be written to disk at all.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::error::TpError;

pub const ENV_TENANT: &str = "KOPERCAD_TRACEPARTS_TENANT";
pub const ENV_KEY: &str = "KOPERCAD_TRACEPARTS_KEY";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    #[serde(rename = "tenantUid")]
    pub tenant_uid: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("tenant_uid", &self.tenant_uid)
            .field("api_key", &"<hidden>")
            .finish()
    }
}

impl Credentials {
    pub fn new(tenant_uid: &str, api_key: &str) -> Result<Self, TpError> {
        let c = Credentials {
            tenant_uid: tenant_uid.trim().to_string(),
            api_key: api_key.trim().to_string(),
        };
        c.validate()?;
        Ok(c)
    }

    /// The rules the token endpoint states: a UUID that is not all zeros,
    /// and a key of 4 to 50 characters.
    pub fn validate(&self) -> Result<(), TpError> {
        if !is_uuid(&self.tenant_uid) {
            return Err(TpError::BadKey(
                "the tenant id must look like 123e4567-e89b-12d3-a456-426614174000".into(),
            ));
        }
        if self.tenant_uid.chars().all(|c| c == '0' || c == '-') {
            return Err(TpError::BadKey("the tenant id is all zeros".into()));
        }
        let n = self.api_key.chars().count();
        if !(4..=50).contains(&n) {
            return Err(TpError::BadKey(
                "the API key must be 4 to 50 characters".into(),
            ));
        }
        Ok(())
    }
}

fn is_uuid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let lens = [8, 4, 4, 4, 12];
    parts.len() == 5
        && parts
            .iter()
            .zip(lens)
            .all(|(p, n)| p.len() == n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    Environment,
    File,
}

pub struct KeyStore {
    path: PathBuf,
}

impl KeyStore {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        KeyStore { path: path.into() }
    }

    /// The default place, `%APPDATA%\KoperCAD\traceparts.json`.
    pub fn default_store() -> Option<Self> {
        let dir = novo_storage::profile::data_dir()?;
        Some(Self::at(dir.join("KoperCAD").join("traceparts.json")))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The key to use now: environment first, then the file. `Ok(None)` when
    /// none is set.
    pub fn load(&self) -> Result<Option<(Credentials, KeySource)>, TpError> {
        let env = |n: &str| std::env::var(n).ok().filter(|v| !v.trim().is_empty());
        if let (Some(t), Some(k)) = (env(ENV_TENANT), env(ENV_KEY)) {
            return Ok(Some((Credentials::new(&t, &k)?, KeySource::Environment)));
        }
        let text = match fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(TpError::Io(e.to_string())),
        };
        let c: Credentials = serde_json::from_str(&text)
            .map_err(|_| TpError::BadKey("the key file is damaged".into()))?;
        c.validate()?;
        Ok(Some((c, KeySource::File)))
    }

    pub fn save(&self, creds: &Credentials) -> Result<(), TpError> {
        creds.validate()?;
        let io = |e: std::io::Error| TpError::Io(e.to_string());
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir).map_err(io)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(creds).map_err(|e| TpError::Io(e.to_string()))?;
<<<<<<< HEAD
        fs::write(&tmp, text).map_err(io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600)).map_err(io)?;
        }
        fs::rename(&tmp, &self.path).map_err(io)
=======
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let written = opts.open(&tmp).and_then(|mut f| {
            use std::io::Write;
            f.write_all(text.as_bytes())
        });
        let result = written.and_then(|()| fs::rename(&tmp, &self.path));
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(io)
>>>>>>> origin/main
    }

    pub fn clear(&self) -> Result<(), TpError> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(TpError::Io(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "123e4567-e89b-12d3-a456-426614174000";

    #[test]
    fn saves_loads_and_clears() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::at(dir.path().join("sub").join("traceparts.json"));
        let c = Credentials::new(T, "secret-key").unwrap();
        store.save(&c).unwrap();
        let (back, src) = store.load().unwrap().unwrap();
        assert_eq!(back, c);
        assert_eq!(src, KeySource::File);
        store.clear().unwrap();
        assert!(store.load().unwrap().is_none());
        store.clear().unwrap();
    }

    #[test]
    fn rejects_what_the_api_would() {
        assert!(Credentials::new("nope", "abcd").is_err());
        assert!(Credentials::new("00000000-0000-0000-0000-000000000000", "abcd").is_err());
        assert!(Credentials::new(T, "abc").is_err());
        assert!(Credentials::new(T, &"x".repeat(51)).is_err());
        assert!(Credentials::new(T, "abcd").is_ok());
    }

    #[test]
    fn debug_hides_the_key() {
        let c = Credentials::new(T, "topsecret").unwrap();
        assert!(!format!("{c:?}").contains("topsecret"));
    }

    #[test]
    fn a_damaged_file_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("k.json");
        fs::write(&p, "{ not json").unwrap();
        assert!(matches!(KeyStore::at(p).load(), Err(TpError::BadKey(_))));
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = KeyStore::at(dir.path().join("k.json"));
        store.save(&Credentials::new(T, "abcd").unwrap()).unwrap();
        let mode = fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0);
    }
}
