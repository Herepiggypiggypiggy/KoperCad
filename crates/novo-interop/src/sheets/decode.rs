//! Text of a CSV file: UTF-8, else Windows-1252 as Excel saves "CSV".

/// Decodes the bytes of a CSV file. A leading byte order mark is dropped.
pub fn decode_csv(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.trim_start_matches('\u{feff}').to_owned(),
        Err(_) => bytes.iter().map(|&b| cp1252(b)).collect(),
    }
}

fn cp1252(b: u8) -> char {
    const HIGH: [char; 32] = [
        '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž',
        '\u{8f}', '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}',
        'ž', 'Ÿ',
    ];
    match b {
        0x80..=0x9f => HIGH[(b - 0x80) as usize],
        _ => b as char,
    }
}

#[cfg(test)]
mod tests {
    use super::decode_csv;

    #[test]
    fn utf8_and_windows_1252() {
        assert_eq!(decode_csv("\u{feff}Sch\u{fc}tz".as_bytes()), "Sch\u{fc}tz");
        assert_eq!(decode_csv(b"Sch\xfctz \x80"), "Sch\u{fc}tz \u{20ac}");
    }
}
