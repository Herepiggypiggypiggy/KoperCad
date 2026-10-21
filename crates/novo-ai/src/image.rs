//! A picture the person chose to send with a message.

use serde_json::{Value, json};

#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub media_type: String,
    /// Standard base64 of the file's bytes.
    pub data: String,
}

impl Image {
    pub fn png(bytes: &[u8]) -> Self {
        Self {
            media_type: "image/png".into(),
            data: base64(bytes),
        }
    }

    /// The Messages API content block.
    pub fn block(&self) -> Value {
        json!({"type": "image", "source": {
            "type": "base64", "media_type": self.media_type, "data": self.data}})
    }
}

/// Standard base64 with padding.
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_vectors() {
        for (plain, coded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), coded);
        }
    }

    #[test]
    fn the_block_has_the_api_shape() {
        let b = Image::png(&[1, 2, 3]).block();
        assert_eq!(b["source"]["media_type"], "image/png");
        assert_eq!(b["source"]["data"], "AQID");
    }
}
