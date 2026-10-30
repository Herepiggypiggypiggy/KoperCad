//! The private signing key as a file, and the base64 form a CI secret holds it in. Used by
//! `kopercad-cli addons keygen | sign` and by the tests; the program itself never reads a
//! private key. The text format is deliberately plain:
//!
//! ```text
//! kopercad-addon-key 1
//! id: koper-vonk-2026
//! owner: KoperVonk
//! seed: <64 hex digits>
//! ```
//!
//! The id travels with the seed so the one secret `ADDON_SIGNING_KEY` (this text, base64
//! encoded) is all CI needs. A file with only the 64 hex digits (the older `pack --seed-file`
//! form) is accepted by [`KeyFile::parse_bare_seed`].

use crate::error::{AddonError, Result};
use crate::trust::{TrustedKey, from_hex, is_date, is_key_id, public_key_for_seed, to_hex};

/// The first line of a key file.
pub const KEY_MAGIC: &str = "kopercad-addon-key 1";

/// A private signing key. `Debug` never shows the seed.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyFile {
    pub id: String,
    pub owner: String,
    pub seed: [u8; 32],
}

impl std::fmt::Debug for KeyFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "KeyFile {{ id: {:?}, owner: {:?}, seed: <hidden> }}",
            self.id, self.owner
        )
    }
}

fn err(why: impl Into<String>) -> AddonError {
    AddonError::Key(why.into())
}

impl KeyFile {
    /// A key from a seed, with the id and owner checked.
    pub fn new(id: &str, owner: &str, seed: [u8; 32]) -> Result<KeyFile> {
        if !is_key_id(id) {
            return Err(err(format!(
                "the key id {id:?} must be 3 to 48 characters of a-z 0-9 - _ . (for example koper-vonk-2026)"
            )));
        }
        if owner.trim().is_empty() || owner.contains(['\n', '\r']) || owner.len() > 80 {
            return Err(err(
                "the owner must be one line of text, at most 80 characters",
            ));
        }
        Ok(KeyFile {
            id: id.to_owned(),
            owner: owner.trim().to_owned(),
            seed,
        })
    }

    /// The file text. Contains the secret: write it only where the person chose.
    pub fn to_text(&self) -> String {
        format!(
            "{KEY_MAGIC}\nid: {}\nowner: {}\nseed: {}\n",
            self.id,
            self.owner,
            to_hex(&self.seed)
        )
    }

    pub fn parse(text: &str) -> Result<KeyFile> {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(KEY_MAGIC) {
            return Err(err(
                "this is not a KoperCAD add-on key file (the first line should be `kopercad-addon-key 1`)",
            ));
        }
        let (mut id, mut owner, mut seed) = (None, None, None);
        for line in lines {
            if let Some(v) = line.strip_prefix("id:") {
                id = Some(v.trim());
            } else if let Some(v) = line.strip_prefix("owner:") {
                owner = Some(v.trim());
            } else if let Some(v) = line.strip_prefix("seed:") {
                seed = Some(v.trim());
            }
        }
        let (Some(id), Some(owner), Some(seed)) = (id, owner, seed) else {
            return Err(err("the key file lacks an id, owner or seed line"));
        };
        let seed = KeyFile::parse_bare_seed(seed)?;
        KeyFile::new(id, owner, seed)
    }

    /// 64 hex digits, nothing else.
    pub fn parse_bare_seed(text: &str) -> Result<[u8; 32]> {
        from_hex(text.trim())
            .and_then(|v| <[u8; 32]>::try_from(v).ok())
            .ok_or_else(|| err("the seed must be 64 hex digits"))
    }

    /// Reads the base64 text a CI secret holds: base64 of [`KeyFile::to_text`].
    pub fn from_base64(secret: &str) -> Result<KeyFile> {
        let bytes = base64_decode(secret).ok_or_else(|| {
            err("the secret is not base64 (encode the key file as one line of base64)")
        })?;
        let text = String::from_utf8(bytes).map_err(|_| err("the secret is not a key file"))?;
        KeyFile::parse(&text)
    }

    /// What goes into the GitHub secret: base64 of the file text, one line.
    pub fn to_base64(&self) -> String {
        base64_encode(self.to_text().as_bytes())
    }

    /// The public key as 64 hex digits.
    pub fn public_hex(&self) -> String {
        to_hex(&public_key_for_seed(&self.seed))
    }

    /// The matching trusted key, optionally valid only from `valid_from` (`YYYY-MM-DD`).
    pub fn trusted_key(&self, valid_from: Option<&str>) -> Result<TrustedKey> {
        if let Some(d) = valid_from
            && !is_date(d)
        {
            return Err(err(format!("{d:?} is not a date like 2026-11-01")));
        }
        let mut k = TrustedKey::new(&self.id, &self.owner, &self.public_hex());
        k.valid_from = valid_from.map(str::to_owned);
        Ok(k)
    }
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Standard base64; whitespace is ignored and padding is optional. `None` for anything else.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    let mut padding = false;
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            padding = true;
            continue;
        }
        if padding {
            return None;
        }
        let v = B64.iter().position(|b| *b == c)? as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    // leftover bits must be zero padding bits of a whole byte
    (acc == 0).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_vectors() {
        for (plain, enc) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(plain.as_bytes()), enc);
            assert_eq!(base64_decode(enc).unwrap(), plain.as_bytes());
        }
        assert_eq!(base64_decode("Zm9v\nYmFy\r\n").unwrap(), b"foobar");
        assert_eq!(base64_decode("Zm9vYg").unwrap(), b"foob");
        assert!(base64_decode("Zm9v!").is_none());
        assert!(base64_decode("Zg==Zg").is_none());
    }

    #[test]
    fn a_key_file_round_trips_and_hides_its_seed() {
        let k = KeyFile::new("koper-vonk-2026", "KoperVonk", [3u8; 32]).unwrap();
        assert_eq!(KeyFile::parse(&k.to_text()).unwrap(), k);
        assert_eq!(KeyFile::from_base64(&k.to_base64()).unwrap(), k);
        assert!(!format!("{k:?}").contains(&to_hex(&[3u8; 32])));
        let tk = k.trusted_key(Some("2026-11-01")).unwrap();
        assert_eq!(tk.public, k.public_hex());
        assert_eq!(tk.valid_from.as_deref(), Some("2026-11-01"));
        assert!(k.trusted_key(Some("soon")).is_err());
    }

    #[test]
    fn bad_key_files_are_refused_in_words() {
        assert!(KeyFile::new("X", "KoperVonk", [0; 32]).is_err());
        assert!(KeyFile::new("koper-vonk-2026", "  ", [0; 32]).is_err());
        assert!(KeyFile::parse("hello").is_err());
        assert!(KeyFile::parse("kopercad-addon-key 1\nid: koper-vonk-2026\n").is_err());
        let bad_seed = "kopercad-addon-key 1\nid: koper-vonk-2026\nowner: K\nseed: 12\n";
        assert!(KeyFile::parse(bad_seed).is_err());
        assert!(KeyFile::from_base64("%%%").is_err());
        assert!(KeyFile::parse_bare_seed(&to_hex(&[1u8; 32])).is_ok());
    }
}
