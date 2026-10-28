//! Who made a package. A package is *signed* when it carries a `signature` file whose Ed25519
//! signature over the manifest verifies with a key the person's copy of KoperCAD trusts. The
//! manifest lists the SHA-256 of every other file, so one signature covers the whole package.
//!
//! Anything else is *unsigned, from file*: it can still be installed, but only after the person
//! has read the publisher, the permissions and the fingerprint and ticked a box (plan 5b).
//! Because an add-on runs sandboxed and cannot touch files or the network, the worst an
//! unsigned add-on can do is give wrong results inside what its permissions allow.

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{AddonError, Result};

/// The first line of a `signature` file.
pub const SIGNATURE_MAGIC: &str = "kopercad-addon-signature 1";
/// Domain separation: a signature over a manifest cannot be replayed as anything else.
const DOMAIN: &[u8] = b"kopercad-addon-manifest-v1\n";

/// Public keys built into the program. Empty until the release thread generates the KoperVonk
/// signing key: the private key never enters the repository, only its public half goes here as
/// `("koper-vonk-<year>", "KoperVonk", "<64 hex digits>")`.
pub const BUILTIN_KEYS: &[(&str, &str, &str)] = &[];

/// A publisher key the program trusts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustedKey {
    pub id: String,
    pub owner: String,
    /// 64 hex digits.
    pub public: String,
}

/// The keys in use: the built-in ones and the ones the person added in `trusted-keys.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustStore {
    keys: Vec<TrustedKey>,
}

#[derive(Deserialize, Serialize, Default)]
struct KeyFile {
    #[serde(default)]
    key: Vec<TrustedKey>,
}

impl TrustStore {
    pub fn builtin() -> TrustStore {
        TrustStore {
            keys: BUILTIN_KEYS
                .iter()
                .map(|(id, owner, public)| TrustedKey {
                    id: (*id).to_owned(),
                    owner: (*owner).to_owned(),
                    public: (*public).to_owned(),
                })
                .collect(),
        }
    }

    pub fn with(mut self, key: TrustedKey) -> TrustStore {
        self.keys.retain(|k| k.id != key.id);
        self.keys.push(key);
        self
    }

    /// Adds the keys of a `trusted-keys.toml`. Returns the problems found; a bad entry is
    /// skipped, it never makes the other keys unusable.
    pub fn extend_from_toml(&mut self, text: &str) -> Vec<String> {
        let mut problems = Vec::new();
        match toml::from_str::<KeyFile>(text) {
            Err(e) => problems.push(format!("trusted-keys.toml: {e}")),
            Ok(file) => {
                for k in file.key {
                    if parse_public(&k.public).is_none() {
                        problems.push(format!("trusted key {}: not a 64-digit hex key", k.id));
                    } else if self.keys.iter().any(|b| b.id == k.id) {
                        problems.push(format!(
                            "trusted key {}: this id is already used; the first one wins",
                            k.id
                        ));
                    } else {
                        self.keys.push(k);
                    }
                }
            }
        }
        problems
    }

    pub fn keys(&self) -> &[TrustedKey] {
        &self.keys
    }

    pub fn find(&self, id: &str) -> Option<&TrustedKey> {
        self.keys.iter().find(|k| k.id == id)
    }
}

/// How far a package can be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trust {
    /// The signature verifies with a trusted key.
    Signed { key_id: String, owner: String },
    /// No usable signature. `note` says why (no signature file, or an unknown key).
    Unsigned { note: String },
}

impl Trust {
    pub fn is_signed(&self) -> bool {
        matches!(self, Trust::Signed { .. })
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

/// The message a signature covers.
pub fn signed_message(manifest: &[u8]) -> Vec<u8> {
    let mut m = DOMAIN.to_vec();
    m.extend_from_slice(manifest);
    m
}

/// Builds the text of a `signature` file.
pub fn signature_file(key_id: &str, signature: &[u8; 64]) -> String {
    format!(
        "{SIGNATURE_MAGIC}\nkey: {key_id}\nsig: {}\n",
        to_hex(signature)
    )
}

/// Judges a package's `signature` file (if any) against `trust`. A signature that names a
/// trusted key and does not verify is an error: it means the package was changed after
/// signing, which is exactly what the check is for.
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
    if lines.next().map(str::trim) != Some(SIGNATURE_MAGIC) {
        return bad("the signature file has an unknown format");
    }
    let (mut key_id, mut sig_hex) = (None, None);
    for line in lines {
        if let Some(v) = line.strip_prefix("key:") {
            key_id = Some(v.trim());
        } else if let Some(v) = line.strip_prefix("sig:") {
            sig_hex = Some(v.trim());
        }
    }
    let (Some(key_id), Some(sig_hex)) = (key_id, sig_hex) else {
        return bad("the signature file lacks a key or a signature line");
    };
    let Some(trusted) = trust.find(key_id) else {
        return Ok(Trust::Unsigned {
            note: format!("signed with the key {key_id}, which this KoperCAD does not trust"),
        });
    };
    let Some(key) = parse_public(&trusted.public) else {
        return bad("the trusted key on this PC is damaged");
    };
    let sig_bytes: Option<[u8; 64]> = from_hex(sig_hex).and_then(|v| v.try_into().ok());
    let Some(sig_bytes) = sig_bytes else {
        return bad("the signature is not 128 hex digits");
    };
    key.verify(
        &signed_message(manifest),
        &Signature::from_bytes(&sig_bytes),
    )
    .map_err(|_| {
        AddonError::BadSignature(format!("it does not match the manifest for key {key_id}"))
    })?;
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
        let pk = TrustedKey {
            id: "test-key".into(),
            owner: "Test Publisher".into(),
            public: to_hex(sk.verifying_key().as_bytes()),
        };
        (sk, pk)
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
        let manifest = b"[addon]\nid = \"x\"\n";
        let sig = signature_file("test-key", &sk.sign(&signed_message(manifest)).to_bytes());

        let t = judge(manifest, Some(sig.as_bytes()), &store).unwrap();
        assert_eq!(
            t,
            Trust::Signed {
                key_id: "test-key".into(),
                owner: "Test Publisher".into()
            }
        );
        // changed manifest
        assert!(matches!(
            judge(b"[addon]\nid = \"y\"\n", Some(sig.as_bytes()), &store),
            Err(AddonError::BadSignature(_))
        ));
        // no signature
        assert!(!judge(manifest, None, &store).unwrap().is_signed());
        // key this PC does not know
        let unknown = judge(manifest, Some(sig.as_bytes()), &TrustStore::default()).unwrap();
        assert!(matches!(unknown, Trust::Unsigned { ref note } if note.contains("test-key")));
        // garbage
        for junk in [
            "",
            "hello",
            "kopercad-addon-signature 1\nkey: test-key\n",
            "kopercad-addon-signature 1\nkey: test-key\nsig: 12\n",
        ] {
            assert!(
                matches!(
                    judge(manifest, Some(junk.as_bytes()), &store),
                    Err(AddonError::BadSignature(_))
                ),
                "{junk:?}"
            );
        }
        // a signature for another purpose (no domain prefix) does not verify
        let raw = signature_file("test-key", &sk.sign(manifest).to_bytes());
        assert!(judge(manifest, Some(raw.as_bytes()), &store).is_err());
    }

    #[test]
    fn user_keys_load_and_bad_ones_are_reported() {
        let (_, pk) = test_key();
        let mut store = TrustStore::default();
        let text = format!(
            "[[key]]\nid = \"{}\"\nowner = \"{}\"\npublic = \"{}\"\n[[key]]\nid = \"bad\"\nowner = \"x\"\npublic = \"12\"\n",
            pk.id, pk.owner, pk.public
        );
        let problems = store.extend_from_toml(&text);
        assert_eq!(store.keys().len(), 1);
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("bad"));
        assert!(!store.extend_from_toml("not toml [[").is_empty());
    }
}
