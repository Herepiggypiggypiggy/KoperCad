//! Who made a package. A package is *signed* when it carries a `signature` file whose Ed25519
//! signature over the manifest verifies with a key the person's copy of KoperCAD trusts. The
//! manifest lists the SHA-256 of every other file, so one signature covers the whole package.
//!
//! Every judgement has its own name, so the window never has to say "unsigned" for something
//! that is not:
//!
//! | [`Trust`] | Meaning | Install |
//! | --- | --- | --- |
//! | `Signed` | verifies with a trusted key | no tick |
//! | `Unsigned` | the package has no signature | needs the tick |
//! | `UnknownKey` | signed, but with a key this PC has never heard of | needs the tick |
//! | `Revoked` | signed with a key that was withdrawn (stolen, retired, mistaken) | refused |
//! | `Tampered` | names a trusted key, but the signature does not fit the manifest | refused |
//!
//! Keys. [`TrustStore::builtin`] holds the keys that ship with the program: [`BUILTIN_KEYS`]
//! (code) and the data file `addons/keys/kopervonk.pub` (embedded at build time, may be empty or
//! absent). The person can add keys and revocations in `trusted-keys.toml`. A revocation always
//! beats a key, wherever either came from, and matches by key id *or* by the public key it
//! lists, so renaming a stolen key does not bring it back. Revocation is offline: it takes
//! effect when the program is updated with a longer list or the person adds a `[[revoked]]`
//! entry; a copy of KoperCAD that never updates never hears of it. See
//! `docs/design/addons-signing.md` for the step-by-step.
//!
//! A key may carry `valid_from` (`YYYY-MM-DD`). A dated signature (format 2, the date is inside
//! the signed bytes) older than the key's `valid_from` is refused. The date is what the signer
//! claims, so it catches mistakes and old test keys, not a thief: revocation is the answer to a
//! thief. Signature format 1 (no date) still verifies.

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AddonError, Result};

/// The first line of a `signature` file without a date.
pub const SIGNATURE_MAGIC: &str = "kopercad-addon-signature 1";
/// The first line of a `signature` file whose signed bytes include the signing date.
pub const SIGNATURE_MAGIC_DATED: &str = "kopercad-addon-signature 2";
/// Domain separation: a signature over a manifest cannot be replayed as anything else.
const DOMAIN: &[u8] = b"kopercad-addon-manifest-v1\n";
const DOMAIN_DATED: &[u8] = b"kopercad-addon-manifest-v2\n";

/// Public keys built into the program as code. Empty on purpose: the KoperVonk key goes in the
/// data file `addons/keys/kopervonk.pub` instead (a data-only commit, see
/// `docs/design/addons-signing.md`). Entries here are `("<key id>", "<owner>", "<64 hex digits>")`,
/// which is also the line `kopercad-cli addons keygen` prints. The private key never enters the
/// repository.
pub const BUILTIN_KEYS: &[(&str, &str, &str)] = &[];

/// Revoked keys built into the program as code: `("<key id>", "<reason>")`. The data file
/// takes the same `[[revoked]]` entries; prefer it.
pub const BUILTIN_REVOKED: &[(&str, &str)] = &[];

/// The embedded key file: `addons/keys/kopervonk.pub` at build time, empty when it is absent.
/// Same TOML as `trusted-keys.toml`.
const EMBEDDED_KEYS: &str = include_str!(concat!(env!("OUT_DIR"), "/embedded-keys.toml"));

/// A publisher key the program trusts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedKey {
    pub id: String,
    pub owner: String,
    /// 64 hex digits.
    pub public: String,
    /// `YYYY-MM-DD`: signatures dated earlier than this are refused. Optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_from: Option<String>,
}

impl TrustedKey {
    pub fn new(id: &str, owner: &str, public: &str) -> TrustedKey {
        TrustedKey {
            id: id.to_owned(),
            owner: owner.to_owned(),
            public: public.to_owned(),
            valid_from: None,
        }
    }

    /// The one-line form: `key <id> <public> <valid-from or -> <owner>`.
    pub fn line(&self) -> String {
        format!(
            "key {} {} {} {}",
            self.id,
            self.public,
            self.valid_from.as_deref().unwrap_or("-"),
            self.owner
        )
    }

    /// Reads [`TrustedKey::line`] back.
    pub fn from_line(line: &str) -> std::result::Result<TrustedKey, String> {
        let mut parts = line.trim().splitn(5, ' ');
        let (Some("key"), Some(id), Some(public), Some(from), Some(owner)) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return Err(format!(
                "key line {line:?}: it should read `key <id> <64 hex digits> <valid-from or -> <owner>`"
            ));
        };
        Ok(TrustedKey {
            id: id.to_owned(),
            owner: owner.trim().to_owned(),
            public: public.to_owned(),
            valid_from: (from != "-").then(|| from.to_owned()),
        })
    }

    /// The `[[key]]` block for `addons/keys/kopervonk.pub` or `trusted-keys.toml`.
    pub fn toml_block(&self) -> String {
        let mut s = format!(
            "[[key]]\nid = {:?}\nowner = {:?}\npublic = {:?}\n",
            self.id, self.owner, self.public
        );
        if let Some(d) = &self.valid_from {
            s.push_str(&format!("valid_from = {d:?}\n"));
        }
        s
    }
}

/// A withdrawn key. It matches a signature by `id`, and also by `public` when given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokedKey {
    pub id: String,
    /// Why, in words the person can read: "stolen", "retired in favour of koper-vonk-2027".
    #[serde(default)]
    pub reason: String,
    /// 64 hex digits; when present, a trusted key with this public key is revoked under any id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public: Option<String>,
}

/// The keys in use: the built-in ones and the ones the person added in `trusted-keys.toml`,
/// and the revocations from both.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustStore {
    keys: Vec<TrustedKey>,
    revoked: Vec<RevokedKey>,
}

#[derive(Deserialize, Serialize, Default)]
struct KeyFile {
    #[serde(default)]
    key: Vec<TrustedKey>,
    #[serde(default)]
    revoked: Vec<RevokedKey>,
}

/// Whether `s` is a calendar-shaped `YYYY-MM-DD`.
pub fn is_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| s.as_bytes()[r].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let m: u32 = s[5..7].parse().unwrap_or(0);
    let d: u32 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&m) && (1..=31).contains(&d)
}

/// A key id: 3 to 48 characters of `a-z 0-9 - _ .`, starting with a letter or digit.
pub fn is_key_id(s: &str) -> bool {
    (3..=48).contains(&s.len())
        && s.bytes().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'-' | b'_' | b'.')
        })
        && !s.starts_with(['-', '_', '.'])
}

impl TrustStore {
    /// The keys and revocations that ship with the program: [`BUILTIN_KEYS`],
    /// [`BUILTIN_REVOKED`] and the embedded `addons/keys/kopervonk.pub`.
    pub fn builtin() -> TrustStore {
        let mut store = TrustStore {
            keys: BUILTIN_KEYS
                .iter()
                .map(|(id, owner, public)| TrustedKey::new(id, owner, public))
                .collect(),
            revoked: BUILTIN_REVOKED
                .iter()
                .map(|(id, reason)| RevokedKey {
                    id: (*id).to_owned(),
                    reason: (*reason).to_owned(),
                    public: None,
                })
                .collect(),
        };
        // A problem in the embedded file is a mistake in the repository; a test fails on it
        // (`the_embedded_key_file_is_valid`), the program just skips the bad entry.
        let _ = store.extend_from_toml(EMBEDDED_KEYS);
        store
    }

    /// Problems in the embedded key file, for the test that guards the data-only commit.
    pub fn embedded_problems() -> Vec<String> {
        TrustStore::default().extend_from_toml(EMBEDDED_KEYS)
    }

    pub fn with(mut self, key: TrustedKey) -> TrustStore {
        self.keys.retain(|k| k.id != key.id);
        self.keys.push(key);
        self
    }

    pub fn with_revoked(mut self, revoked: RevokedKey) -> TrustStore {
        self.revoked.push(revoked);
        self
    }

    /// Adds the keys and revocations of a `trusted-keys.toml`. Besides TOML, a line of the form
    /// `key <id> <64 hex digits> <valid-from or -> <owner...>` adds one key (the single line
    /// `kopercad-cli addons keygen` prints). Returns the problems found; a bad entry is skipped,
    /// it never makes the other keys unusable.
    pub fn extend_from_toml(&mut self, text: &str) -> Vec<String> {
        let mut problems = Vec::new();
        let mut rest = String::new();
        let mut lined = Vec::new();
        for line in text.lines() {
            let t = line.trim();
            if t.starts_with("key ") {
                match TrustedKey::from_line(t) {
                    Ok(k) => lined.push(k),
                    Err(e) => problems.push(e),
                }
            } else {
                rest.push_str(line);
                rest.push('\n');
            }
        }
        match toml::from_str::<KeyFile>(&rest) {
            Err(e) => problems.push(format!("trusted-keys.toml: {e}")),
            Ok(file) => {
                for k in lined.into_iter().chain(file.key) {
                    self.add_key(k, &mut problems);
                }
                for r in file.revoked {
                    if r.id.is_empty() {
                        problems.push("a revoked entry has no id".to_owned());
                    } else if r
                        .public
                        .as_deref()
                        .is_some_and(|p| parse_public(p).is_none())
                    {
                        problems.push(format!("revoked key {}: not a 64-digit hex key", r.id));
                    } else {
                        self.revoked.push(r);
                    }
                }
            }
        }
        problems
    }

    fn add_key(&mut self, k: TrustedKey, problems: &mut Vec<String>) {
        if parse_public(&k.public).is_none() {
            problems.push(format!("trusted key {}: not a 64-digit hex key", k.id));
        } else if !is_key_id(&k.id) {
            problems.push(format!(
                "trusted key {:?}: the id must be 3 to 48 characters of a-z 0-9 - _ .",
                k.id
            ));
        } else if k.valid_from.as_deref().is_some_and(|d| !is_date(d)) {
            problems.push(format!(
                "trusted key {}: valid_from must look like 2026-11-01",
                k.id
            ));
        } else if self.keys.iter().any(|b| b.id == k.id) {
            problems.push(format!(
                "trusted key {}: this id is already used; the first one wins",
                k.id
            ));
        } else {
            self.keys.push(k);
        }
    }

    pub fn keys(&self) -> &[TrustedKey] {
        &self.keys
    }

    pub fn revoked(&self) -> &[RevokedKey] {
        &self.revoked
    }

    pub fn find(&self, id: &str) -> Option<&TrustedKey> {
        self.keys.iter().find(|k| k.id == id)
    }

    /// The revocation that applies to a signature naming `key_id`: one with that id, or one
    /// that lists the public key `key_id` stands for here.
    pub fn revocation_of(&self, key_id: &str) -> Option<&RevokedKey> {
        let public = self.find(key_id).map(|k| k.public.to_lowercase());
        self.revoked.iter().find(|r| {
            r.id == key_id
                || matches!((&r.public, &public), (Some(rp), Some(p)) if rp.to_lowercase() == *p)
        })
    }
}

/// How far a package can be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// The signature verifies with a trusted key.
    Signed { key_id: String, owner: String },
    /// The package has no signature.
    Unsigned { note: String },
    /// Signed, but with a key this PC does not know. Cannot be told from a forgery here.
    UnknownKey { key_id: String },
    /// Signed with a key that was withdrawn. Never installable.
    Revoked {
        key_id: String,
        /// The key's owner when the key is known, else empty.
        owner: String,
        reason: String,
    },
    /// Names a trusted key but the signature does not fit the manifest: changed after signing,
    /// or signed by someone who does not hold that key. Never installable.
    Tampered { key_id: String, owner: String },
}

impl Trust {
    pub fn is_signed(&self) -> bool {
        matches!(self, Trust::Signed { .. })
    }

    /// A state that can never be installed, with or without the tick.
    pub fn is_refused(&self) -> bool {
        matches!(self, Trust::Revoked { .. } | Trust::Tampered { .. })
    }

    /// A short stable name for scripts and the JSON report: `signed`, `unsigned`,
    /// `unknown-key`, `revoked` or `tampered`.
    pub fn state(&self) -> &'static str {
        match self {
            Trust::Signed { .. } => "signed",
            Trust::Unsigned { .. } => "unsigned",
            Trust::UnknownKey { .. } => "unknown-key",
            Trust::Revoked { .. } => "revoked",
            Trust::Tampered { .. } => "tampered",
        }
    }

    /// One line in plain words, the same in the window, the command line and the install
    /// prompt. It never relies on colour: each state is spelled out.
    pub fn sentence(&self) -> String {
        match self {
            Trust::Signed { owner, .. } => format!("Signed by {owner}"),
            Trust::Unsigned { note } => format!("Unsigned, from file: {note}"),
            Trust::UnknownKey { key_id } => {
                format!("Signed with an unknown key ({key_id}): this PC cannot tell who made it")
            }
            Trust::Revoked { key_id, reason, .. } => format!(
                "Signed with a revoked key ({key_id}){}: the publisher withdrew it",
                if reason.is_empty() {
                    String::new()
                } else {
                    format!(", {reason}")
                }
            ),
            Trust::Tampered { key_id, .. } => format!(
                "Signature does not match (key {key_id}): the file was changed after it was signed"
            ),
        }
    }

    /// The plain-words reason for a refused package, `None` for the others.
    pub fn refusal(&self) -> Option<String> {
        match self {
            Trust::Revoked { key_id, reason, .. } => Some(format!(
                "it was signed with the key {key_id}, which has been revoked{}",
                if reason.is_empty() {
                    String::new()
                } else {
                    format!(" ({reason})")
                }
            )),
            Trust::Tampered { key_id, .. } => Some(format!(
                "its signature does not match its manifest for the key {key_id}: it was changed after it was signed, or it was not signed by the holder of that key"
            )),
            _ => None,
        }
    }
}

fn parse_public(hex: &str) -> Option<VerifyingKey> {
    let bytes = from_hex(hex)?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

/// Decodes lower- or upper-case hex.
pub fn from_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok())
        .collect()
}

/// Lower-case hex.
pub fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// SHA-256 as 64 lower-case hex digits.
pub fn sha256_hex(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

/// The public key (32 bytes) for an Ed25519 seed. For tools that sign (`kopercad-cli addons pack`
/// and CI) and tests; the program itself only ever verifies.
pub fn public_key_for_seed(seed: &[u8; 32]) -> [u8; 32] {
    ed25519_dalek::SigningKey::from_bytes(seed)
        .verifying_key()
        .to_bytes()
}

/// The message a format 1 signature covers.
pub fn signed_message(manifest: &[u8]) -> Vec<u8> {
    let mut m = DOMAIN.to_vec();
    m.extend_from_slice(manifest);
    m
}

/// The message a format 2 (dated) signature covers.
pub fn signed_message_dated(date: &str, manifest: &[u8]) -> Vec<u8> {
    let mut m = DOMAIN_DATED.to_vec();
    m.extend_from_slice(format!("date: {date}\n").as_bytes());
    m.extend_from_slice(manifest);
    m
}

/// Builds the text of a format 1 `signature` file.
pub fn signature_file(key_id: &str, signature: &[u8; 64]) -> String {
    format!(
        "{SIGNATURE_MAGIC}\nkey: {key_id}\nsig: {}\n",
        to_hex(signature)
    )
}

/// Builds the text of a format 2 `signature` file.
pub fn signature_file_dated(key_id: &str, date: &str, signature: &[u8; 64]) -> String {
    format!(
        "{SIGNATURE_MAGIC_DATED}\nkey: {key_id}\ndate: {date}\nsig: {}\n",
        to_hex(signature)
    )
}

/// Signs `manifest` and returns the text of the `signature` file. Ed25519 is deterministic, so
/// the same seed, id, date and manifest always give the same text. With `date` the signature is
/// format 2. This is what `kopercad-cli addons sign` and `pack` use.
pub fn sign_manifest(
    seed: &[u8; 32],
    key_id: &str,
    date: Option<&str>,
    manifest: &[u8],
) -> Result<String> {
    let sk = SigningKey::from_bytes(seed);
    Ok(match date {
        Some(d) => {
            if !is_date(d) {
                return Err(AddonError::BadSignature(format!(
                    "{d:?} is not a date like 2026-11-01"
                )));
            }
            let sig = sk.sign(&signed_message_dated(d, manifest));
            signature_file_dated(key_id, d, &sig.to_bytes())
        }
        None => {
            let sig = sk.sign(&signed_message(manifest));
            signature_file(key_id, &sig.to_bytes())
        }
    })
}

/// Judges a package's `signature` file (if any) against `trust`. An `Err` is a signature file
/// that cannot be read at all; a readable signature always gets one of the [`Trust`] states,
/// including the refused ones, so callers can say exactly what is wrong.
pub fn judge(manifest: &[u8], signature: Option<&[u8]>, trust: &TrustStore) -> Result<Trust> {
    let Some(raw) = signature else {
        return Ok(Trust::Unsigned {
            note: "the package has no signature".into(),
        });
    };
    let bad = |why: &str| Err(AddonError::BadSignature(why.to_owned()));
    let Ok(text) = std::str::from_utf8(raw) else {
        return bad("the signature file is not text");
    };
    if raw.len() > 1024 {
        return bad("the signature file is too large");
    }
    let mut lines = text.lines();
    let dated = match lines.next().map(str::trim) {
        Some(SIGNATURE_MAGIC) => false,
        Some(SIGNATURE_MAGIC_DATED) => true,
        _ => return bad("the signature file has an unknown format"),
    };
    let (mut key_id, mut sig_hex, mut date) = (None, None, None);
    for line in lines {
        if let Some(v) = line.strip_prefix("key:") {
            key_id = Some(v.trim());
        } else if let Some(v) = line.strip_prefix("sig:") {
            sig_hex = Some(v.trim());
        } else if let Some(v) = line.strip_prefix("date:") {
            date = Some(v.trim());
        }
    }
    let (Some(key_id), Some(sig_hex)) = (key_id, sig_hex) else {
        return bad("the signature file lacks a key or a signature line");
    };
    if dated != date.is_some() || date.is_some_and(|d| !is_date(d)) {
        return bad("the signature file's date line is missing or not a date like 2026-11-01");
    }
    // A withdrawn key is judged before anything else: whether the bytes still verify no longer
    // matters, and the person must hear "revoked", not "unknown".
    if let Some(r) = trust.revocation_of(key_id) {
        return Ok(Trust::Revoked {
            key_id: key_id.to_owned(),
            owner: trust
                .find(key_id)
                .map(|k| k.owner.clone())
                .unwrap_or_default(),
            reason: r.reason.clone(),
        });
    }
    let Some(trusted) = trust.find(key_id) else {
        return Ok(Trust::UnknownKey {
            key_id: key_id.to_owned(),
        });
    };
    let Some(key) = parse_public(&trusted.public) else {
        return bad("the trusted key on this PC is damaged");
    };
    let sig_bytes: Option<[u8; 64]> = from_hex(sig_hex).and_then(|v| v.try_into().ok());
    let Some(sig_bytes) = sig_bytes else {
        return bad("the signature is not 128 hex digits");
    };
    let message = match date {
        Some(d) => signed_message_dated(d, manifest),
        None => signed_message(manifest),
    };
    if key
        .verify_strict(&message, &Signature::from_bytes(&sig_bytes))
        .is_err()
    {
        return Ok(Trust::Tampered {
            key_id: key_id.to_owned(),
            owner: trusted.owner.clone(),
        });
    }
    if let (Some(d), Some(from)) = (date, trusted.valid_from.as_deref())
        && d < from
    {
        return bad(&format!(
            "it is dated {d}, before the key {key_id} became valid on {from}"
        ));
    }
    Ok(Trust::Signed {
        key_id: key_id.to_owned(),
        owner: trusted.owner.clone(),
    })
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    pub(crate) fn test_key() -> (SigningKey, TrustedKey) {
        let sk = SigningKey::from_bytes(&[7u8; 32]);
        let pk = TrustedKey::new(
            "test-key",
            "Test Publisher",
            &to_hex(sk.verifying_key().as_bytes()),
        );
        (sk, pk)
    }

    const MANIFEST: &[u8] = b"[addon]\nid = \"x\"\n";

    fn signed_by(seed: [u8; 32], id: &str, date: Option<&str>) -> String {
        sign_manifest(&seed, id, date, MANIFEST).unwrap()
    }

    #[test]
    fn hex_round_trips() {
        let b = [0u8, 1, 0xab, 0xff];
        assert_eq!(from_hex(&to_hex(&b)).unwrap(), b);
        assert!(from_hex("abc").is_none());
        assert!(from_hex("zz").is_none());
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn signed_unsigned_and_tampered() {
        let (sk, pk) = test_key();
        let store = TrustStore::default().with(pk);
        let sig = signature_file("test-key", &sk.sign(&signed_message(MANIFEST)).to_bytes());

        let t = judge(MANIFEST, Some(sig.as_bytes()), &store).unwrap();
        assert_eq!(
            t,
            Trust::Signed {
                key_id: "test-key".into(),
                owner: "Test Publisher".into()
            }
        );
        // changed manifest after signing: tampered, with its own message, never "unsigned"
        let t = judge(b"[addon]\nid = \"y\"\n", Some(sig.as_bytes()), &store).unwrap();
        assert!(matches!(t, Trust::Tampered { ref key_id, .. } if key_id == "test-key"));
        assert!(t.is_refused() && !t.is_signed());
        assert!(t.refusal().unwrap().contains("changed after it was signed"));
        // no signature
        let none = judge(MANIFEST, None, &store).unwrap();
        assert!(matches!(none, Trust::Unsigned { .. }) && !none.is_refused());
        // key this PC does not know
        let unknown = judge(MANIFEST, Some(sig.as_bytes()), &TrustStore::default()).unwrap();
        assert_eq!(
            unknown,
            Trust::UnknownKey {
                key_id: "test-key".into()
            }
        );
        assert!(!unknown.is_refused());
        // garbage
        for junk in [
            "",
            "hello",
            "kopercad-addon-signature 1\nkey: test-key\n",
            "kopercad-addon-signature 1\nkey: test-key\nsig: 12\n",
            // a dated header without a date, a date under the undated header
            "kopercad-addon-signature 2\nkey: test-key\nsig: 12\n",
            "kopercad-addon-signature 1\nkey: test-key\ndate: 2026-01-01\nsig: 12\n",
            "kopercad-addon-signature 2\nkey: test-key\ndate: 2026-13-01\nsig: 12\n",
        ] {
            assert!(
                matches!(
                    judge(MANIFEST, Some(junk.as_bytes()), &store),
                    Err(AddonError::BadSignature(_))
                ),
                "{junk:?}"
            );
        }
        // a signature for another purpose (no domain prefix) does not verify
        let raw = signature_file("test-key", &sk.sign(MANIFEST).to_bytes());
        assert!(matches!(
            judge(MANIFEST, Some(raw.as_bytes()), &store).unwrap(),
            Trust::Tampered { .. }
        ));
    }

    #[test]
    fn the_wrong_key_fails() {
        let (_, pk) = test_key();
        let store = TrustStore::default().with(pk);
        // signed by somebody else's seed but claiming to be test-key
        let forged = signed_by([9u8; 32], "test-key", None);
        assert!(matches!(
            judge(MANIFEST, Some(forged.as_bytes()), &store).unwrap(),
            Trust::Tampered { .. }
        ));
        // a signature file cannot be moved to another manifest, dated or not
        let dated = signed_by([7u8; 32], "test-key", Some("2026-11-01"));
        assert!(
            judge(MANIFEST, Some(dated.as_bytes()), &store)
                .unwrap()
                .is_signed()
        );
        assert!(matches!(
            judge(b"other", Some(dated.as_bytes()), &store).unwrap(),
            Trust::Tampered { .. }
        ));
        // the date is inside the signature: editing it breaks it
        let edited = dated.replace("2026-11-01", "2026-12-01");
        assert!(matches!(
            judge(MANIFEST, Some(edited.as_bytes()), &store).unwrap(),
            Trust::Tampered { .. }
        ));
        // and a dated file cannot be downgraded to an undated one by dropping the line
        let stripped = dated
            .replace("signature 2", "signature 1")
            .replace("date: 2026-11-01\n", "");
        assert!(matches!(
            judge(MANIFEST, Some(stripped.as_bytes()), &store).unwrap(),
            Trust::Tampered { .. }
        ));
    }

    #[test]
    fn signing_is_deterministic() {
        let a = signed_by([7u8; 32], "test-key", Some("2026-11-01"));
        let b = signed_by([7u8; 32], "test-key", Some("2026-11-01"));
        assert_eq!(a, b);
        assert_ne!(a, signed_by([7u8; 32], "test-key", None));
        assert!(sign_manifest(&[7u8; 32], "test-key", Some("tomorrow"), MANIFEST).is_err());
    }

    #[test]
    fn a_revoked_key_says_revoked_not_unsigned() {
        let (_, pk) = test_key();
        let sig = signed_by([7u8; 32], "test-key", None);
        let revoked = RevokedKey {
            id: "test-key".into(),
            reason: "the laptop was stolen".into(),
            public: None,
        };
        // trusted and revoked: revoked wins
        let store = TrustStore::default()
            .with(pk.clone())
            .with_revoked(revoked.clone());
        let t = judge(MANIFEST, Some(sig.as_bytes()), &store).unwrap();
        assert_eq!(
            t,
            Trust::Revoked {
                key_id: "test-key".into(),
                owner: "Test Publisher".into(),
                reason: "the laptop was stolen".into()
            }
        );
        assert!(t.is_refused());
        assert!(t.refusal().unwrap().contains("stolen"));
        // revoked and not (any longer) in the list of keys: still revoked, never unknown
        let only_revoked = TrustStore::default().with_revoked(revoked);
        assert!(matches!(
            judge(MANIFEST, Some(sig.as_bytes()), &only_revoked).unwrap(),
            Trust::Revoked { ref owner, .. } if owner.is_empty()
        ));
        // revoked even though the signature no longer fits (revocation is judged first)
        let t = judge(b"changed", Some(sig.as_bytes()), &only_revoked).unwrap();
        assert!(matches!(t, Trust::Revoked { .. }));
    }

    #[test]
    fn a_revoked_public_key_stays_revoked_under_a_new_id() {
        let (_, pk) = test_key();
        let renamed = TrustedKey::new("fresh-name", "Test Publisher", &pk.public);
        let store = TrustStore::default()
            .with(renamed)
            .with_revoked(RevokedKey {
                id: "test-key".into(),
                reason: "retired".into(),
                public: Some(pk.public.to_uppercase()),
            });
        let sig = signed_by([7u8; 32], "fresh-name", None);
        assert!(matches!(
            judge(MANIFEST, Some(sig.as_bytes()), &store).unwrap(),
            Trust::Revoked { .. }
        ));
    }

    #[test]
    fn several_keys_rotation_and_valid_from() {
        let old_seed = [7u8; 32];
        let new_seed = [8u8; 32];
        let old = TrustedKey::new(
            "koper-vonk-2026",
            "KoperVonk",
            &to_hex(&public_key_for_seed(&old_seed)),
        );
        let mut new = TrustedKey::new(
            "koper-vonk-2027",
            "KoperVonk",
            &to_hex(&public_key_for_seed(&new_seed)),
        );
        new.valid_from = Some("2027-01-01".into());
        let store = TrustStore::default().with(old).with(new);
        // both keys are valid side by side
        for (seed, id) in [(old_seed, "koper-vonk-2026"), (new_seed, "koper-vonk-2027")] {
            let sig = signed_by(seed, id, None);
            assert!(
                judge(MANIFEST, Some(sig.as_bytes()), &store)
                    .unwrap()
                    .is_signed()
            );
        }
        // a signature dated before the new key's valid_from is refused, on or after is fine
        let early = signed_by(new_seed, "koper-vonk-2027", Some("2026-12-31"));
        let err = judge(MANIFEST, Some(early.as_bytes()), &store).unwrap_err();
        assert!(
            err.to_string().contains("became valid on 2027-01-01"),
            "{err}"
        );
        let ok = signed_by(new_seed, "koper-vonk-2027", Some("2027-01-01"));
        assert!(
            judge(MANIFEST, Some(ok.as_bytes()), &store)
                .unwrap()
                .is_signed()
        );
        // retiring the old key leaves packages from the new one alone
        let store = store.with_revoked(RevokedKey {
            id: "koper-vonk-2026".into(),
            reason: "retired".into(),
            public: None,
        });
        let sig = signed_by(old_seed, "koper-vonk-2026", None);
        assert!(matches!(
            judge(MANIFEST, Some(sig.as_bytes()), &store).unwrap(),
            Trust::Revoked { .. }
        ));
        assert!(
            judge(MANIFEST, Some(ok.as_bytes()), &store)
                .unwrap()
                .is_signed()
        );
    }

    #[test]
    fn user_keys_load_and_bad_ones_are_reported() {
        let (_, pk) = test_key();
        let mut store = TrustStore::default();
        let text = format!(
            "[[key]]\nid = \"{}\"\nowner = \"{}\"\npublic = \"{}\"\n[[key]]\nid = \"bad\"\nowner = \"x\"\npublic = \"12\"\n\
             [[key]]\nid = \"dated-bad\"\nowner = \"x\"\npublic = \"{}\"\nvalid_from = \"soon\"\n\
             [[key]]\nid = \"X\"\nowner = \"x\"\npublic = \"{}\"\n\
             [[revoked]]\nid = \"old-one\"\nreason = \"stolen\"\n[[revoked]]\nid = \"bad-pub\"\npublic = \"zz\"\n",
            pk.id, pk.owner, pk.public, pk.public, pk.public
        );
        let problems = store.extend_from_toml(&text);
        assert_eq!(store.keys().len(), 1);
        assert_eq!(store.revoked().len(), 1);
        assert_eq!(problems.len(), 4, "{problems:?}");
        assert!(problems[0].contains("bad"));
        assert!(!store.extend_from_toml("not toml [[").is_empty());
        // a key block round-trips through its own text
        let mut back = TrustStore::default();
        let mut k = pk.clone();
        k.valid_from = Some("2026-11-01".into());
        assert!(back.extend_from_toml(&k.toml_block()).is_empty());
        assert_eq!(back.keys(), &[k]);
    }

    #[test]
    fn the_one_line_form_round_trips_next_to_toml_and_rotation() {
        let (_, pk) = test_key();
        let mut dated = pk.clone();
        dated.valid_from = Some("2026-11-01".into());
        assert_eq!(TrustedKey::from_line(&dated.line()).unwrap(), dated);
        let mut plain = pk.clone();
        plain.owner = "Koper Vonk BV".into();
        assert_eq!(TrustedKey::from_line(&plain.line()).unwrap(), plain);
        // comments, a line, a TOML block and a revocation in one file
        let other = to_hex(&public_key_for_seed(&[8u8; 32]));
        let text = format!(
            "# comment\n{}\n[[key]]\nid = \"{}\"\nowner = \"B\"\npublic = \"{}\"\n[[revoked]]\nid = \"old\"\n",
            dated.line(),
            "second-key",
            other
        );
        let mut store = TrustStore::default();
        assert_eq!(store.extend_from_toml(&text), Vec::<String>::new());
        assert_eq!(store.keys().len(), 2);
        assert_eq!(store.revoked().len(), 1);
        assert_eq!(
            store.find("test-key").unwrap().valid_from.as_deref(),
            Some("2026-11-01")
        );
        // a bad line is reported and the rest still loads
        let mut store = TrustStore::default();
        let bad = format!("key only-two-words\n{}\n", plain.line());
        assert_eq!(store.extend_from_toml(&bad).len(), 1);
        assert_eq!(store.keys().len(), 1);
    }

    #[test]
    fn the_embedded_key_file_is_valid() {
        // The data-only commit that adds the KoperVonk key must keep this green.
        assert_eq!(TrustStore::embedded_problems(), Vec::<String>::new());
        let builtin = TrustStore::builtin();
        for k in builtin.keys() {
            assert!(is_key_id(&k.id));
            assert!(
                builtin.revocation_of(&k.id).is_none(),
                "{} is revoked",
                k.id
            );
        }
    }

    #[test]
    fn date_and_id_shapes() {
        assert!(is_date("2026-11-01") && !is_date("2026-1-01") && !is_date("2026-13-01"));
        assert!(!is_date("2026-11-0x") && !is_date("20261101") && !is_date("2026-11-32"));
        assert!(is_key_id("koper-vonk-2026") && !is_key_id("ab") && !is_key_id("Koper"));
        assert!(!is_key_id("-lead") && !is_key_id("has space"));
    }
}
