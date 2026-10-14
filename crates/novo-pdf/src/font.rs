//! The standard Helvetica font: WinAnsi encoding and glyph widths.
//!
//! Helvetica is one of the PDF standard 14 fonts, so every reader has it and
//! nothing is embedded. Its widths come from the Adobe font metrics (units
//! of 1/1000 of the font size), which lets labels be anchored exactly
//! (right-aligned and centred text lines up in print).
//!
//! TODO(WS-09): embed a subset of Inter (novo-theme) as a CID font so
//! characters outside WinAnsi (Ω, Greek, CJK) print; until then they are
//! written as `?`.

/// Name of the font resource in every page's resource dictionary.
pub const FONT_NAME: &[u8] = b"F1";

/// Distance from the top of a text line to its baseline, as a fraction of
/// the font size. Helvetica's ascender is 0.718 and its descender 0.207; the
/// remaining 0.075 of a line is split above and below, so the glyphs sit in
/// the middle of the line box the canvas anchors labels by.
pub const BASELINE: f32 = 0.756;

/// Widths of the printable ASCII characters 0x20..=0x7E.
const ASCII_WIDTHS: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, // 0x20
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, // 0x30
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // 0x40
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, // 0x50
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // 0x60
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // 0x70
];

/// The WinAnsi code of a character, or `?` when it has none.
pub fn encode_char(c: char) -> u8 {
    let code = c as u32;
    match code {
        0x20..=0x7E | 0xA0..=0xFF => code as u8,
        _ => match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            '\t' => b' ',
            _ => b'?',
        },
    }
}

/// A string in WinAnsi bytes.
pub fn encode(text: &str) -> Vec<u8> {
    text.chars().map(encode_char).collect()
}

/// Width of a WinAnsi code in 1/1000 of the font size.
pub fn width(code: u8) -> u16 {
    match code {
        0x20..=0x7E => ASCII_WIDTHS[(code - 0x20) as usize],
        0x85 | 0x89 | 0x97 | 0x99 | 0x8C | 0x9C => 1000,
        0x91 | 0x92 | 0x82 => 222,
        0x93 | 0x94 | 0x84 => 333,
        0x95 => 350,
        0xA0 => 278,
        0xB0 => 400,
        0xB2 | 0xB3 | 0xB9 => 333,
        0xB7 => 278,
        0xB1 | 0xD7 | 0xF7 => 584,
        _ => 556,
    }
}

/// Width of encoded text in units of the font size.
pub fn text_width(encoded: &[u8]) -> f32 {
    encoded.iter().map(|&c| f32::from(width(c))).sum::<f32>() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_round_trips_and_has_afm_widths() {
        assert_eq!(encode("-K1:A1"), b"-K1:A1".to_vec());
        assert_eq!(width(b' '), 278);
        assert_eq!(width(b'0'), 556);
        assert_eq!(width(b'W'), 944);
        assert_eq!(width(b'~'), 584);
        assert!((text_width(b"10") - 1.112).abs() < 1e-6);
    }

    #[test]
    fn non_winansi_characters_become_question_marks() {
        assert_eq!(encode("2 Ω"), b"2 ?".to_vec());
        assert_eq!(encode("°±µ"), vec![0xB0, 0xB1, 0xB5]);
        assert_eq!(encode("–€"), vec![0x96, 0x80]);
    }
}
