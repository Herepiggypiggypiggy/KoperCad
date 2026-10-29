//! Reads the parts database files of an EPLAN installation (the `.adb` and
//! `.eod` article files) into catalog parts.
//!
//! Knowledge only: the reader makes no assumption about the file's outer
//! container. It looks for zlib streams (`78 DA`, `78 9C`, `78 5E`), and a
//! stream is one article when it holds tagged UTF-8 fields: a tag byte
//! `F1..FE`, the bytes `U 00 00`, the text, and a NUL. Fields read:
//!
//! | tag | field |
//! | --- | --- |
//! | `F1` | full id `MAKER.number` |
//! | `F2` | type designation |
//! | `F3` | order number |
//! | `F4` | description, many languages: `nl_NL@text;en_US@text` |
//! | `F5` `F6` `F9` | further descriptions (technical text) |
//! | `F7` `F8` | maker code, supplier code |
//! | `FA` | macro path |
//! | `FB` | function letter (`K`, `F`, `X`, `Q`) |
//! | `FC` `FD` `FE` | height, width, depth in mm |
//!
//! Records that do not look like articles are ignored, so a file in some
//! other layout reports "no articles" instead of producing rubbish. Maker
//! codes are mapped to one name per maker with [`MakerAliases`].

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use flate2::read::ZlibDecoder;
use novo_core::Value;
use novo_parts::{
    CatalogPart, Dimensions, ImportReport, MakerAliases, Provenance, keys, parse_number,
};

use super::EplanError;

/// Largest article record read, bytes. Real records are well under 4 KiB.
const MAX_RECORD: u64 = 256 * 1024;

/// Key prefix for fields without a catalog field, as in the XML reader.
const KEEP: &str = "eplan.";

/// One maker code found in the file and what it was turned into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakerSeen {
    /// The code as written in the file.
    pub code: String,
    /// The name used in the catalog (the code itself when unknown).
    pub name: String,
    /// Whether the alias table knows the code.
    pub known: bool,
    /// Articles with this code.
    pub parts: usize,
}

/// Result of reading an article file.
#[derive(Debug, Clone, Default)]
pub struct EodRead {
    pub report: ImportReport,
    /// Maker codes in the file, most articles first.
    pub makers: Vec<MakerSeen>,
}

/// One decoded article: tag byte to text.
type Fields = BTreeMap<u8, String>;

/// Whether the bytes hold at least one article (to tell the file from XML or
/// delimited text without reading all of it).
pub fn looks_like_eod(bytes: &[u8]) -> bool {
    !bytes.starts_with(b"<") && records(bytes, Some(1)).next().is_some()
}

/// Reads every article in `bytes`. `default_manufacturer` is used for
/// articles that name no maker.
pub fn import_eod(
    bytes: &[u8],
    aliases: &MakerAliases,
    default_manufacturer: &str,
) -> Result<EodRead, EplanError> {
    let mut out = EodRead::default();
    let mut first_seen: HashMap<(String, String), usize> = HashMap::new();
    let mut codes: BTreeMap<String, usize> = BTreeMap::new();
    let mut n = 0usize;
    for fields in records(bytes, None) {
        n += 1;
        match to_part(&fields, aliases, default_manufacturer) {
            Ok((part, code)) => {
                let key = (
                    part.manufacturer.to_lowercase(),
                    part.order_number.to_lowercase(),
                );
                if let Some(first) = first_seen.get(&key) {
                    out.report.skipped.push((
                        n,
                        format!(
                            "{} {} is the same part as article {first} (maker codes that mean one maker are merged)",
                            part.manufacturer, part.order_number
                        ),
                    ));
                    continue;
                }
                first_seen.insert(key, n);
                *codes.entry(code).or_default() += 1;
                out.report.parts.push(part);
            }
            Err(why) => out.report.skipped.push((n, why)),
        }
    }
    if n == 0 {
        return Err(EplanError::NoArticles);
    }
    out.report.data_columns = vec![
        "description_nl".into(),
        "technical_text".into(),
        format!("{KEEP}macro_path"),
    ];
    out.makers = codes
        .into_iter()
        .map(|(code, parts)| {
            let resolved = aliases.resolve(&code);
            MakerSeen {
                known: resolved.is_some(),
                name: resolved.unwrap_or_else(|| code.clone()),
                code,
                parts,
            }
        })
        .collect();
    out.makers
        .sort_by(|a, b| b.parts.cmp(&a.parts).then(a.code.cmp(&b.code)));
    Ok(out)
}

/// Articles found in `bytes`, at most `limit`.
fn records(bytes: &[u8], limit: Option<usize>) -> impl Iterator<Item = Fields> + '_ {
    let mut at = 0usize;
    let mut found = 0usize;
    std::iter::from_fn(move || {
        if limit.is_some_and(|l| found >= l) {
            return None;
        }
        while at + 2 <= bytes.len() {
            let (a, b) = (bytes[at], bytes[at + 1]);
            let header = a == 0x78
                && matches!(b, 0x01 | 0x5e | 0x9c | 0xda)
                && (u16::from(a) * 256 + u16::from(b)) % 31 == 0;
            if !header {
                at += 1;
                continue;
            }
            let mut decoder = ZlibDecoder::new(&bytes[at..]);
            let mut raw = Vec::new();
            let ok = (&mut decoder)
                .take(MAX_RECORD)
                .read_to_end(&mut raw)
                .is_ok();
            if !ok || raw.len() as u64 >= MAX_RECORD {
                at += 1;
                continue;
            }
            let fields = fields_of(&raw);
            if fields.contains_key(&0xf1) {
                at += usize::try_from(decoder.total_in()).unwrap_or(1).max(1);
                found += 1;
                return Some(fields);
            }
            at += 1;
        }
        None
    })
}

/// Tagged fields of a decoded record.
fn fields_of(raw: &[u8]) -> Fields {
    let mut out = Fields::new();
    let mut i = 0usize;
    while i + 4 <= raw.len() {
        let tag = raw[i];
        if (0xf1..=0xfe).contains(&tag) && raw[i + 1..i + 4] == *b"U\0\0" {
            let start = i + 4;
            let end = raw[start..]
                .iter()
                .position(|&b| b == 0)
                .map_or(raw.len(), |p| start + p);
            let text = String::from_utf8_lossy(&raw[start..end]).trim().to_owned();
            if !text.is_empty() {
                out.entry(tag).or_insert(text);
            }
            i = end + 1;
        } else {
            i += 1;
        }
    }
    out
}

/// `nl_NL@text;en_US@text` as `(language, text)`. Text without markers is
/// one entry with an empty language.
fn languages(text: &str) -> Vec<(String, String)> {
    let b = text.as_bytes();
    let marker = |i: usize| {
        i + 6 <= b.len()
            && b[i].is_ascii_lowercase()
            && b[i + 1].is_ascii_lowercase()
            && b[i + 2] == b'_'
            && b[i + 3].is_ascii_uppercase()
            && b[i + 4].is_ascii_uppercase()
            && b[i + 5] == b'@'
            && (i == 0 || b[i - 1] == b';')
    };
    let starts: Vec<usize> = (0..b.len()).filter(|&i| marker(i)).collect();
    if starts.is_empty() {
        return vec![(String::new(), text.trim().to_owned())];
    }
    starts
        .iter()
        .enumerate()
        .map(|(n, &s)| {
            let end = starts.get(n + 1).map_or(b.len(), |&e| e - 1);
            (
                text[s..s + 5].to_owned(),
                text[s + 6..end.max(s + 6)].trim().to_owned(),
            )
        })
        .collect()
}

/// The text in the wanted language prefix (`en`, `nl`), if present.
fn in_language(all: &[(String, String)], prefix: &str) -> Option<String> {
    all.iter()
        .find(|(l, t)| l.starts_with(prefix) && !t.is_empty())
        .map(|(_, t)| t.clone())
}

/// English first, then Dutch, then whatever comes first.
fn best(all: &[(String, String)]) -> String {
    in_language(all, "en")
        .or_else(|| in_language(all, "nl"))
        .or_else(|| {
            all.iter()
                .find(|(_, t)| !t.is_empty())
                .map(|(_, t)| t.clone())
        })
        .unwrap_or_default()
}

fn to_part(
    f: &Fields,
    aliases: &MakerAliases,
    default_manufacturer: &str,
) -> Result<(CatalogPart, String), String> {
    let get = |tag: u8| f.get(&tag).map(String::as_str).filter(|t| !t.is_empty());
    let id = get(0xf1).ok_or("no article id")?;
    let (id_code, id_rest) = id.split_once('.').unwrap_or(("", id));
    let code = get(0xf7)
        .or(Some(id_code).filter(|c| !c.is_empty()))
        .or(get(0xf8))
        .unwrap_or_else(|| default_manufacturer.trim());
    let order = get(0xf3)
        .or(Some(id_rest).filter(|r| !r.is_empty()))
        .ok_or("no order number")?;
    if code.is_empty() {
        return Err(format!(
            "{id}: no maker (type one under \"Manufacturer for rows without one\")"
        ));
    }
    let mut part = CatalogPart {
        manufacturer: aliases.display(code),
        order_number: order.to_owned(),
        type_number: get(0xf2).unwrap_or_default().to_owned(),
        provenance: Some(Provenance {
            source: "import:eplan".into(),
            licence_note: "Read from the user's own EPLAN parts database".into(),
            ..Provenance::default()
        }),
        ..CatalogPart::default()
    };

    let short = get(0xf4).map(languages).unwrap_or_default();
    part.description = best(&short);
    if let Some(nl) = in_language(&short, "nl")
        && nl != part.description
    {
        part.data
            .insert("description_nl".into(), Value::Text(nl.clone()));
        part.keywords.push(nl);
    }
    let long: Vec<String> = [0xf9, 0xf5, 0xf6]
        .iter()
        .filter_map(|&t| get(t))
        .map(|t| best(&languages(t)))
        .filter(|t| !t.is_empty() && *t != part.description)
        .collect();
    if !long.is_empty() {
        part.data
            .insert("technical_text".into(), Value::Text(long.join(" | ")));
    }
    if let Some(m) = get(0xfa) {
        part.data
            .insert(format!("{KEEP}macro_path"), Value::Text(m.to_owned()));
    }
    if let Some(letter) = get(0xfb) {
        part.letter_codes = vec![letter.to_uppercase()];
    }

    let dim = |tag| get(tag).and_then(parse_number).filter(|v| *v > 0.0);
    let (h, w, d) = (dim(0xfc), dim(0xfd), dim(0xfe));
    match (h, w, d) {
        (Some(height_mm), Some(width_mm), Some(depth_mm)) => {
            part.dimensions = Some(Dimensions {
                width_mm,
                height_mm,
                depth_mm,
            });
        }
        _ => {
            for (key, v) in [
                (keys::HEIGHT_MM, h),
                (keys::WIDTH_MM, w),
                (keys::DEPTH_MM, d),
            ] {
                if let Some(v) = v {
                    part.data.insert(key.into(), Value::Float(v));
                }
            }
        }
    }
    Ok((part, code.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    /// One invented article record, zlib-compressed, with the tags in
    /// `fields` (tag, text).
    fn record(fields: &[(u8, &str)]) -> Vec<u8> {
        let mut raw = vec![0x01, 0x00, 0x07];
        for (tag, text) in fields {
            raw.push(*tag);
            raw.extend_from_slice(b"U\0\0");
            raw.extend_from_slice(text.as_bytes());
            raw.push(0);
        }
        let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
        z.write_all(&raw).unwrap();
        z.finish().unwrap()
    }

    /// A file: junk, then each record behind some framing bytes.
    fn file(records: &[Vec<u8>]) -> Vec<u8> {
        let mut out = b"EXAMPLE-DB\0\x01\x02x".to_vec();
        for r in records {
            out.extend_from_slice(&[0xaa, 0xbb, 0x00, 0x10]);
            out.extend_from_slice(r);
        }
        out.extend_from_slice(b"tail");
        out
    }

    fn sample() -> Vec<u8> {
        file(&[
            record(&[
                (0xf1, "SE.TEST100"),
                (0xf2, "TST-100"),
                (0xf3, "TEST100"),
                (
                    0xf4,
                    "nl_NL@Testschakelaar;en_US@Test switch;de_DE@Testschalter",
                ),
                (0xf9, "en_US@10 A; 3 poles"),
                (0xfa, "$(MD_MACROS)\\Example\\Switch.ema"),
                (0xfb, "q"),
                (0xfc, "90,5"),
                (0xfd, "45"),
                (0xfe, "60"),
            ]),
            record(&[
                (0xf1, "SCHNEI.TEST100"),
                (0xf3, "TEST100"),
                (0xf4, "Same part, other code"),
            ]),
            record(&[(0xf1, "XQZ.W1"), (0xf4, "en_US@Widget"), (0xfd, "12")]),
            record(&[(0xf2, "no id here")]),
        ])
    }

    #[test]
    fn reads_articles_from_an_unknown_container() {
        let aliases = MakerAliases::builtin();
        let read = import_eod(&sample(), &aliases, "").unwrap();
        let p = &read.report.parts;
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].manufacturer, "Schneider Electric");
        assert_eq!(p[0].order_number, "TEST100");
        assert_eq!(p[0].type_number, "TST-100");
        assert_eq!(p[0].description, "Test switch");
        assert_eq!(
            p[0].data.get("description_nl"),
            Some(&Value::Text("Testschakelaar".into()))
        );
        assert_eq!(
            p[0].data.get("technical_text"),
            Some(&Value::Text("10 A; 3 poles".into()))
        );
        assert_eq!(p[0].letter_codes, vec!["Q".to_owned()]);
        let d = p[0].dimensions.unwrap();
        assert_eq!((d.height_mm, d.width_mm, d.depth_mm), (90.5, 45.0, 60.0));
        assert!(p[0].keywords.contains(&"Testschakelaar".to_owned()));
        // A partial size is kept as data, not invented.
        assert!(p[1].dimensions.is_none());
        assert_eq!(p[1].data.get("width_mm"), Some(&Value::Float(12.0)));
        assert_eq!(p[1].manufacturer, "XQZ");
        assert_eq!(p[0].provenance.as_ref().unwrap().source, "import:eplan");
    }

    #[test]
    fn a_second_code_of_one_maker_is_reported_not_added_twice() {
        let read = import_eod(&sample(), &MakerAliases::builtin(), "").unwrap();
        // The record without an id is not an article and is not counted.
        assert_eq!(read.report.skipped.len(), 1);
        assert!(read.report.skipped[0].1.contains("same part as article 1"));
        let se = read.makers.iter().find(|m| m.code == "SE").unwrap();
        assert!(se.known && se.name == "Schneider Electric" && se.parts == 1);
        let q = read.makers.iter().find(|m| m.code == "XQZ").unwrap();
        assert!(!q.known);
    }

    #[test]
    fn user_alias_changes_the_maker_name() {
        let mut aliases = MakerAliases::builtin();
        aliases.set("XQZ", "Example Works");
        let read = import_eod(&sample(), &aliases, "").unwrap();
        assert_eq!(read.report.parts[1].manufacturer, "Example Works");
    }

    #[test]
    fn other_files_are_not_articles() {
        assert!(!looks_like_eod(b"<xml/>"));
        assert!(!looks_like_eod(b"Order number;Type\nA;B\n"));
        assert!(looks_like_eod(&sample()));
        assert_eq!(
            import_eod(b"plain text", &MakerAliases::builtin(), "").unwrap_err(),
            EplanError::NoArticles
        );
    }

    #[test]
    fn default_maker_fills_a_missing_code() {
        let bytes = file(&[record(&[(0xf1, "ABC123"), (0xf4, "Thing")])]);
        let read = import_eod(&bytes, &MakerAliases::builtin(), "rittal").unwrap();
        assert_eq!(read.report.parts[0].manufacturer, "Rittal");
        assert_eq!(read.report.parts[0].order_number, "ABC123");
    }

    #[test]
    fn language_split_keeps_semicolons_inside_text() {
        let l = languages("nl_NL@a; b;en_US@c");
        assert_eq!(l[0], ("nl_NL".into(), "a; b".into()));
        assert_eq!(l[1], ("en_US".into(), "c".into()));
    }
}
