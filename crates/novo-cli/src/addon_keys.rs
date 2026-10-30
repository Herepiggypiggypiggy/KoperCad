//! `kopercad-cli addons keygen | sign | verify`: the publisher's side of add-on trust. The
//! private key is created only at a path the person names, outside any git work tree, readable
//! by that user alone; it is never printed. No network, no admin rights.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use novo_addons::keyfile::KeyFile;
use novo_addons::pack::{Signer32, sign_package};
use novo_addons::trust::{Trust, TrustStore, is_date, sha256_hex};
use novo_addons::{AddonError, Package};

use crate::{Failure, exit, print};

fn fail(e: AddonError) -> Failure {
    Failure::new(exit::READ, e.to_string())
}

/// Today's date in UTC as `YYYY-MM-DD`; `SOURCE_DATE_EPOCH` (seconds) overrides the clock so a
/// build can be repeated byte for byte.
pub(crate) fn today_utc() -> String {
    let secs = std::env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
        });
    civil_from_days(secs.div_euclid(86_400))
}

/// Days since 1970-01-01 to `YYYY-MM-DD` (Howard Hinnant's civil-from-days).
fn civil_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// Whether `path` lies inside a git work tree (a `.git` in it or any parent folder).
fn inside_git_work_tree(path: &Path) -> bool {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    };
    abs.ancestors().skip(1).any(|dir| dir.join(".git").exists())
}

/// Creates `path` (it must not exist) readable by the current user only, and writes `text`.
fn write_private(path: &Path, text: &str, out: &mut dyn Write) -> Result<(), Failure> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| {
        Failure::new(
            exit::WRITE,
            format!(
                "{}: {e}{}",
                path.display(),
                if path.exists() {
                    " (it already exists; keygen never overwrites a key)"
                } else {
                    ""
                }
            ),
        )
    })?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|e| Failure::new(exit::WRITE, format!("{}: {e}", path.display())))?;
    drop(file);
    restrict_to_current_user(path, out)
}

/// Windows: a file under the user's profile is already limited to that user, SYSTEM and the
/// administrators; `icacls` (part of Windows, no elevation needed for one's own file) drops the
/// inherited entries so only the user remains. Unix: the 0600 above did it.
#[cfg(windows)]
fn restrict_to_current_user(path: &Path, out: &mut dyn Write) -> Result<(), Failure> {
    let user = std::env::var("USERNAME").unwrap_or_default();
    let done = !user.is_empty()
        && std::process::Command::new("icacls")
            .arg(path)
            .args(["/inheritance:r", "/grant:r", &format!("{user}:F")])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
    if done {
        print(
            out,
            &format!("Access to the key file is limited to {user}."),
        )?;
    } else {
        print(
            out,
            "warning: could not restrict the key file's access with icacls; it sits under your profile folder, \
             so other standard users cannot read it, but check its Security tab.",
        )?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn restrict_to_current_user(_path: &Path, _out: &mut dyn Write) -> Result<(), Failure> {
    Ok(())
}

pub(crate) fn keygen(
    id: &str,
    owner: &str,
    output: &Path,
    valid_from: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    let mut seed = [0u8; 32];
    getrandom::fill(&mut seed).map_err(|e| {
        Failure::new(
            exit::UNAVAILABLE,
            format!("the operating system gave no random numbers: {e}"),
        )
    })?;
    let key = KeyFile::new(id, owner, seed).map_err(fail)?;
    let trusted = key.trusted_key(valid_from).map_err(fail)?;
    if inside_git_work_tree(output) {
        return Err(Failure::new(
            exit::USAGE,
            format!(
                "{} is inside a git work tree. A private key must never sit in a repository; \
                 choose a folder in your profile such as %LOCALAPPDATA%\\KoperCAD\\keys",
                output.display()
            ),
        ));
    }
    if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|e| Failure::new(exit::WRITE, format!("{}: {e}", parent.display())))?;
    }
    write_private(output, &key.to_text(), out)?;
    let mut pub_path = output.as_os_str().to_owned();
    pub_path.push(".pub");
    let pub_path = PathBuf::from(pub_path);
    let block = trusted.toml_block();
    // The public file is not secret; failing to write it is only a nuisance.
    let pub_note = match std::fs::write(&pub_path, format!("{}\n", trusted.line())) {
        Ok(()) => format!("The public half is also in {}", pub_path.display()),
        Err(e) => format!("Could not write {}: {e}", pub_path.display()),
    };

    print(out, &format!("Created the signing key {id} for {owner}."))?;
    print(
        out,
        &format!(
            "Private key (secret, back it up, never commit it): {}",
            output.display()
        ),
    )?;
    print(out, &pub_note)?;
    print(
        out,
        &format!(
            "Public key fingerprint: {}",
            &sha256_hex(&novo_addons::trust::from_hex(&key.public_hex()).unwrap_or_default())[..16]
        ),
    )?;
    print(out, "")?;
    print(
        out,
        "Public key, to commit: add this ONE line at the end of addons/keys/kopervonk.pub (it is not secret):",
    )?;
    print(out, "-----")?;
    print(out, &trusted.line())?;
    print(out, "-----")?;
    print(
        out,
        "The same key as a TOML block (an equivalent form, also accepted there):",
    )?;
    print(out, "-----")?;
    for line in block.lines() {
        print(out, line)?;
    }
    print(out, "-----")?;
    print(
        out,
        "And as an entry of novo_addons::trust::BUILTIN_KEYS (only if you prefer code to the file):",
    )?;
    print(
        out,
        &format!(
            "    ({:?}, {:?}, {:?}),",
            key.id,
            key.owner,
            key.public_hex()
        ),
    )?;
    Ok(exit::OK)
}

/// The key to sign with: a key file, or base64 of one in an environment variable.
fn load_key(
    key: &Option<PathBuf>,
    key_env: &Option<String>,
    key_id: &Option<String>,
) -> Result<KeyFile, Failure> {
    match (key, key_env) {
        (Some(path), None) => {
            check_permissions(path);
            let text = std::fs::read_to_string(path)
                .map_err(|e| Failure::new(exit::READ, format!("{}: {e}", path.display())))?;
            match KeyFile::parse(&text) {
                Ok(k) => Ok(k),
                // the older seed-only file needs its id on the command line
                Err(_) => match (KeyFile::parse_bare_seed(&text), key_id) {
                    (Ok(seed), Some(id)) => KeyFile::new(id, "unknown", seed).map_err(fail),
                    (Ok(_), None) => Err(Failure::new(
                        exit::USAGE,
                        "this key file holds only a seed; add --key-id",
                    )),
                    _ => Err(fail(AddonError::Key(
                        "not a KoperCAD add-on key file".into(),
                    ))),
                },
            }
        }
        (None, Some(var)) => {
            let value = std::env::var(var).map_err(|_| {
                Failure::new(
                    exit::USAGE,
                    format!("the environment variable {var} is not set"),
                )
            })?;
            KeyFile::from_base64(&value).map_err(fail)
        }
        _ => Err(Failure::new(
            exit::USAGE,
            "name the key with --key <file> or --key-env <VARIABLE>",
        )),
    }
}

#[cfg(unix)]
fn check_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path)
        && meta.permissions().mode() & 0o077 != 0
    {
        eprintln!(
            "warning: {} can be read by other users; run chmod 600 on it",
            path.display()
        );
    }
}

#[cfg(not(unix))]
fn check_permissions(_path: &Path) {}

pub(crate) struct SignArgs {
    pub file: PathBuf,
    pub key: Option<PathBuf>,
    pub key_env: Option<String>,
    pub key_id: Option<String>,
    pub output: Option<PathBuf>,
    pub date: Option<String>,
    pub no_date: bool,
    pub replace: bool,
}

pub(crate) fn sign(a: SignArgs, out: &mut dyn Write) -> Result<i32, Failure> {
    let key = load_key(&a.key, &a.key_env, &a.key_id)?;
    let date = match (&a.date, a.no_date) {
        (Some(_), true) => {
            return Err(Failure::new(exit::USAGE, "use either --date or --no-date"));
        }
        (_, true) => None,
        (Some(d), false) => {
            if !is_date(d) {
                return Err(Failure::new(
                    exit::USAGE,
                    format!("{d:?} is not a date like 2026-11-01"),
                ));
            }
            Some(d.clone())
        }
        (None, false) => Some(today_utc()),
    };
    let bytes = std::fs::read(&a.file)
        .map_err(|e| Failure::new(exit::READ, format!("{}: {e}", a.file.display())))?;
    let signer = Signer32 {
        key_id: &key.id,
        seed: key.seed,
        date: date.as_deref(),
    };
    let signed = sign_package(&bytes, &signer, a.replace).map_err(fail)?;
    // Read it back with only this key trusted: what is written is what verifies.
    let trust = TrustStore::default().with(key.trusted_key(None).map_err(fail)?);
    let check = Package::read(&signed, &trust).map_err(fail)?;
    if !check.trust.is_signed() {
        return Err(Failure::new(
            exit::PROBLEMS,
            format!("internal check failed: {}", check.trust.sentence()),
        ));
    }
    let target = a.output.clone().unwrap_or_else(|| a.file.clone());
    let mut tmp = target.as_os_str().to_owned();
    tmp.push(".signing");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, &signed)
        .and_then(|()| std::fs::rename(&tmp, &target))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            Failure::new(exit::WRITE, format!("{}: {e}", target.display()))
        })?;
    print(
        out,
        &format!(
            "Signed {} {} with key {}{}",
            check.manifest.addon.name,
            check.manifest.addon.version,
            key.id,
            date.as_deref()
                .map(|d| format!(", dated {d}"))
                .unwrap_or_default()
        ),
    )?;
    print(
        out,
        &format!("Wrote {} ({} bytes)", target.display(), signed.len()),
    )?;
    print(out, &format!("sha256 {}", sha256_hex(&signed)))?;
    Ok(exit::OK)
}

pub(crate) fn verify(
    file: &Path,
    store_trust: &TrustStore,
    only_keys: &[PathBuf],
    public: Option<(&str, &str)>,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    let bytes = std::fs::read(file)
        .map_err(|e| Failure::new(exit::READ, format!("{}: {e}", file.display())))?;
    // Given keys replace what the program knows: "does this verify with exactly this key?"
    let trust = if only_keys.is_empty() && public.is_none() {
        store_trust.clone()
    } else {
        let mut t = TrustStore::default();
        for path in only_keys {
            let text = std::fs::read_to_string(path)
                .map_err(|e| Failure::new(exit::READ, format!("{}: {e}", path.display())))?;
            if let Some(problem) = t.extend_from_toml(&text).into_iter().next() {
                return Err(Failure::new(
                    exit::READ,
                    format!("{}: {problem}", path.display()),
                ));
            }
        }
        if let Some((id, hex)) = public {
            t = t.with(novo_addons::trust::TrustedKey::new(
                id,
                "the given key",
                hex,
            ));
        }
        t
    };
    match Package::read(&bytes, &trust) {
        Ok(p) => {
            print(
                out,
                &format!("{} {}", p.manifest.addon.name, p.manifest.addon.version),
            )?;
            print(out, &p.trust.sentence())?;
            if let Trust::Signed { key_id, .. } = &p.trust {
                print(
                    out,
                    &format!("key {key_id}, manifest fingerprint {}", p.fingerprint),
                )?;
                Ok(exit::OK)
            } else {
                Ok(exit::PROBLEMS)
            }
        }
        Err(e) => {
            print(out, &format!("Not verified: {e}"))?;
            Ok(exit::PROBLEMS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_come_out_right() {
        assert_eq!(civil_from_days(0), "1970-01-01");
        assert_eq!(civil_from_days(19_723), "2024-01-01");
        assert_eq!(civil_from_days(20_513), "2026-03-01");
        assert_eq!(civil_from_days(20_512), "2026-02-28");
        assert!(is_date(&today_utc()));
    }
}
