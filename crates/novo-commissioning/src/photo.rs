//! Photo files for punch items: content-addressed names and the text
//! encoding that lets pictures travel inside a `.kcfield` file.

use sha2::{Digest, Sha256};

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The file name for a picture: the start of its SHA-256 and a lower-case
/// extension (`jpg` when it has none or an odd one). The same picture always
/// gets the same name, on any device.
pub fn photo_name(bytes: &[u8], extension: &str) -> String {
    let digest = Sha256::digest(bytes);
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    let ext = extension.trim_start_matches('.').to_ascii_lowercase();
    let ext = if (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        ext
    } else {
        "jpg".to_owned()
    };
    format!("{hex}.{ext}")
}

/// Whether `name` is a name [`photo_name`] could have made, so a file read
/// from outside never reaches beyond the photos folder.
pub fn is_photo_name(name: &str) -> bool {
    let Some((stem, ext)) = name.split_once('.') else {
        return false;
    };
    stem.len() == 16
        && stem
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        && (1..=5).contains(&ext.len())
        && ext
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Whether `bytes` are the picture `name` was made from.
pub(crate) fn name_matches(name: &str, bytes: &[u8]) -> bool {
    let ext = name.split_once('.').map_or("", |(_, e)| e);
    photo_name(bytes, ext) == name
}

pub(crate) fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

pub(crate) fn decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buf = 0u32;
    let mut bits = 0;
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Some(out)
}
