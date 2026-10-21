//! Reading a machine's I/O list: CSV or Excel with a tag, an address, a
//! description and a signal type per row.
//!
//! The columns are found by their heading (English or Dutch), in any
//! order; a column nothing here uses is reported, not guessed at. A row
//! that cannot be placed (no signal type that can be told, an address
//! that is not a PLC address) is skipped with its line and the reason, so
//! the review list shows what was left out.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use thiserror::Error;

use crate::layout::{ChannelType, Space};
use crate::link::addr::{AddrKey, parse_address};
use crate::link::csvio;

/// One signal of the list.
#[derive(Debug, Clone, PartialEq)]
pub struct IoItem {
    /// Line of the file, from 1, blank lines counted.
    pub line: usize,
    /// The sensor or actuator (`B1`, `-Y3`): what the signal belongs to.
    pub tag: String,
    /// What the signal says (`Feed pump running`).
    pub description: String,
    pub kind: ChannelType,
    /// The address as the list wrote it; empty when it had none.
    pub address_text: String,
    /// The same address, read.
    pub address: Option<AddrKey>,
    /// The PLC station (CPU) the signal belongs to; empty for the first.
    pub station: String,
    /// The name the PLC program uses for it, when the list has one.
    pub plc_tag: String,
}

/// Something the review list shows about the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The line it is about; `None` for the whole file.
    pub line: Option<usize>,
    pub text: String,
}

/// An I/O list read from a file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct IoList {
    pub items: Vec<IoItem>,
    /// Rows left out and columns ignored.
    pub notes: Vec<Note>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ListError {
    #[error("cannot read the file: {0}")]
    Bad(String),
    #[error(
        "no heading row found: the list needs at least a signal type or an address column \
         (type, signal type, address) and a tag or description column"
    )]
    NoHeading,
}

#[derive(Default, Clone, Copy)]
struct Columns {
    tag: Option<usize>,
    address: Option<usize>,
    description: Option<usize>,
    kind: Option<usize>,
    station: Option<usize>,
    plc_tag: Option<usize>,
}

const TAG: &[&str] = &[
    "tag",
    "device",
    "device tag",
    "designation",
    "sensor",
    "actuator",
    "component",
    "item",
    "kks",
    "ident",
    "id",
    "apparaat",
    "component tag",
    "veldapparaat",
];
const ADDRESS: &[&str] = &[
    "address",
    "adres",
    "plc address",
    "io address",
    "i/o address",
    "logical address",
    "addr",
];
const DESCRIPTION: &[&str] = &[
    "description",
    "signal",
    "text",
    "comment",
    "function",
    "omschrijving",
    "beschrijving",
    "benaming",
    "signaal",
    "signal text",
    "name",
];
const KIND: &[&str] = &[
    "type",
    "signal type",
    "io type",
    "i/o type",
    "signaltype",
    "signaal type",
    "kind",
    "class",
    "soort",
    "io",
    "i/o",
    "direction",
];
const STATION: &[&str] = &["station", "plc", "cpu", "rack", "controller"];
const PLC_TAG: &[&str] = &[
    "plc tag",
    "symbol",
    "symbol name",
    "variable",
    "tag name",
    "symbool",
];

fn norm(cell: &str) -> String {
    cell.trim()
        .trim_start_matches('\u{feff}')
        .to_ascii_lowercase()
}

fn find(header: &[String], names: &[&str]) -> Option<usize> {
    header
        .iter()
        .position(|c| names.contains(&norm(c).as_str()))
}

fn columns(header: &[String]) -> Columns {
    let mut cols = Columns {
        plc_tag: find(header, PLC_TAG),
        address: find(header, ADDRESS),
        kind: find(header, KIND),
        station: find(header, STATION),
        ..Columns::default()
    };
    // `tag` and `name` are ambiguous words: when the list has a separate
    // PLC tag or symbol column they mean the device; a description column
    // wins `name` only when nothing else says it.
    cols.tag = find(header, TAG);
    cols.description = find(header, &DESCRIPTION[..DESCRIPTION.len() - 1])
        .or_else(|| find(header, &["name"]).filter(|&n| Some(n) != cols.tag));
    cols
}

fn is_heading(cols: &Columns) -> bool {
    let known = [cols.tag, cols.description, cols.address, cols.kind]
        .iter()
        .flatten()
        .count();
    known >= 2 && (cols.kind.is_some() || cols.address.is_some())
}

/// Reads an `.xlsx`, `.csv` (comma, semicolon or tab) or text file.
pub fn read_list(bytes: &[u8]) -> Result<IoList, ListError> {
    let rows = if bytes.starts_with(b"PK") {
        sheet_rows(bytes)?
    } else {
        csvio::rows_with_blanks(&String::from_utf8_lossy(bytes))
    };
    from_rows(&rows)
}

fn cell_text(d: &Data) -> String {
    match d {
        Data::Empty => String::new(),
        Data::String(s) => s.clone(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) if f.fract() == 0.0 => format!("{f:.0}"),
        Data::Float(f) => f.to_string(),
        Data::Bool(b) => b.to_string(),
        other => other.to_string(),
    }
}

/// The rows of the first sheet that has a heading row.
fn sheet_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>, ListError> {
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e: calamine::XlsxError| ListError::Bad(e.to_string()))?;
    let mut first = None;
    for name in book.sheet_names() {
        let range = book
            .worksheet_range(&name)
            .map_err(|e| ListError::Bad(e.to_string()))?;
        // The range starts at the first used cell: pad so an index is the sheet row.
        let skipped = range.start().map_or(0, |(row, _)| row as usize);
        let rows: Vec<Vec<String>> = std::iter::repeat_n(Vec::new(), skipped)
            .chain(range.rows().map(|r| r.iter().map(cell_text).collect()))
            .collect();
        if rows.iter().take(30).any(|r| is_heading(&columns(r))) {
            return Ok(rows);
        }
        first.get_or_insert(rows);
    }
    first.ok_or_else(|| ListError::Bad("the workbook has no sheet".into()))
}

fn from_rows(rows: &[Vec<String>]) -> Result<IoList, ListError> {
    let (h, cols) = rows
        .iter()
        .take(30)
        .enumerate()
        .map(|(h, r)| (h, columns(r)))
        .find(|(_, c)| is_heading(c))
        .ok_or(ListError::NoHeading)?;
    let mut list = IoList::default();
    let used = [
        cols.tag,
        cols.address,
        cols.description,
        cols.kind,
        cols.station,
        cols.plc_tag,
    ];
    let ignored: Vec<&str> = rows[h]
        .iter()
        .enumerate()
        .filter(|(i, c)| !c.trim().is_empty() && !used.contains(&Some(*i)))
        .map(|(_, c)| c.trim())
        .collect();
    if !ignored.is_empty() {
        list.notes.push(Note {
            line: Some(h + 1),
            text: format!("columns not used: {}", ignored.join(", ")),
        });
    }
    for (i, row) in rows.iter().enumerate().skip(h + 1) {
        let get = |c: Option<usize>| {
            c.and_then(|c| row.get(c))
                .map(|s| s.trim().to_owned())
                .unwrap_or_default()
        };
        let line = i + 1;
        let (tag, description) = (get(cols.tag), get(cols.description));
        let (address_text, kind_text) = (get(cols.address), get(cols.kind));
        if tag.is_empty() && description.is_empty() && address_text.is_empty() {
            continue;
        }
        let address = if address_text.is_empty() {
            None
        } else {
            let key = read_address(&address_text);
            if key.is_none() {
                list.notes.push(Note {
                    line: Some(line),
                    text: format!(
                        "{address_text} is not a PLC input or output address, so the \
                         project assigns this signal's address"
                    ),
                });
            }
            key
        };
        let from_type = signal_kind(&kind_text);
        let from_address = address.map(kind_of_address);
        let Some(kind) = from_type.or(from_address) else {
            list.notes.push(Note {
                line: Some(line),
                text: format!(
                    "skipped {}: no signal type that can be told (type \"{kind_text}\", \
                     address \"{address_text}\")",
                    label(&tag, &description)
                ),
            });
            continue;
        };
        if let (Some(t), Some(a)) = (from_type, from_address)
            && t.space() != a.space()
        {
            list.notes.push(Note {
                line: Some(line),
                text: format!(
                    "{}: the type says {t} but the address {address_text} is in the \
                     other direction; the type is used and the address ignored",
                    label(&tag, &description)
                ),
            });
        }
        let address = address.filter(|a| kind_of_address(*a).space() == kind.space());
        list.items.push(IoItem {
            line,
            tag,
            description,
            kind,
            address_text,
            address,
            station: get(cols.station),
            plc_tag: get(cols.plc_tag),
        });
    }
    Ok(list)
}

fn label(tag: &str, description: &str) -> String {
    match (tag.is_empty(), description.is_empty()) {
        (false, false) => format!("{tag} ({description})"),
        (false, true) => tag.to_owned(),
        (true, false) => description.to_owned(),
        (true, true) => "a row".to_owned(),
    }
}

/// The address in any style, with the German `E` and `A` for input and
/// output and spaces (`E 0.0`) allowed.
pub fn read_address(text: &str) -> Option<AddrKey> {
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let upper = compact.trim_start_matches('%').to_ascii_uppercase();
    let mut chars = upper.chars();
    let first = chars.next()?;
    let second = chars.next()?;
    let german =
        matches!(first, 'E' | 'A') && (second.is_ascii_digit() || matches!(second, 'W' | 'X'));
    let english = if german {
        let letter = if first == 'E' { 'I' } else { 'Q' };
        format!("{letter}{}", &upper[1..])
    } else {
        upper
    };
    // A list is a user's file: no real controller has bytes up to u32::MAX, and the plan adds to them.
    parse_address(&english).filter(|key| match key {
        AddrKey::Bit { byte, .. } | AddrKey::Word { byte, .. } => *byte <= MAX_BYTE,
        AddrKey::Point { point, .. } => *point <= MAX_BYTE,
        AddrKey::Analog { .. } => true,
    })
}

/// The highest byte address a list may use.
const MAX_BYTE: u32 = 65_535;

/// The channel type an address implies when the list has no type: a bit
/// is a digital channel, a word an analog one.
pub fn kind_of_address(key: AddrKey) -> ChannelType {
    match (key.space(), key.is_word()) {
        (Space::Input, false) => ChannelType::DigitalInput,
        (Space::Output, false) => ChannelType::DigitalOutput,
        (Space::Input, true) => ChannelType::AnalogInput,
        (Space::Output, true) => ChannelType::AnalogOutput,
    }
}

/// Reads the signal type cell: a code (`DI`, `DQ`, `DO`, `RO`, `AI`,
/// `AQ`, `AO`) or words in English or Dutch (`digital input`,
/// `4-20 mA in`, `uitgang`, `relais`). `None` when nothing in it says.
pub fn signal_kind(text: &str) -> Option<ChannelType> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(kind) = ChannelType::from_code(t) {
        return Some(kind);
    }
    let lower = t.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| lower.contains(w));
    let word = |w: &str| {
        lower
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|x| x == w)
    };
    if has(&["relay", "relais"]) {
        return Some(ChannelType::RelayOutput);
    }
    let output = has(&["output", "uitgang", "actuator", "valve", "klep"])
        || word("out")
        || word("q")
        || word("ao")
        || word("do");
    let input =
        has(&["input", "ingang", "sensor", "schakelaar"]) || word("in") || word("ai") || word("di");
    let analog = has(&[
        "analog", "analoog", "4-20", "4..20", "0-10", "0..10", "pt100",
    ]) || word("ma")
        || word("ai")
        || word("ao");
    match (output, input, analog) {
        (true, false, true) => Some(ChannelType::AnalogOutput),
        (true, false, false) => Some(ChannelType::DigitalOutput),
        (false, true, true) => Some(ChannelType::AnalogInput),
        (false, true, false) => Some(ChannelType::DigitalInput),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_types_are_told_in_words_and_codes() {
        use ChannelType::*;
        for (text, want) in [
            ("DI", Some(DigitalInput)),
            ("do", Some(DigitalOutput)),
            ("Digital input", Some(DigitalInput)),
            ("digitale ingang", Some(DigitalInput)),
            ("Digital output 24V", Some(DigitalOutput)),
            ("uitgang", Some(DigitalOutput)),
            ("Relay output", Some(RelayOutput)),
            ("4-20 mA in", Some(AnalogInput)),
            ("Analog input", Some(AnalogInput)),
            ("analoge uitgang 0-10V", Some(AnalogOutput)),
            ("AO", Some(AnalogOutput)),
            ("", None),
            ("pump", None),
        ] {
            assert_eq!(signal_kind(text), want, "{text}");
        }
    }

    #[test]
    fn addresses_are_read_in_every_style_including_german() {
        let bit = |space, byte, bit| Some(AddrKey::Bit { space, byte, bit });
        assert_eq!(read_address("%IX4.3"), bit(Space::Input, 4, 3));
        assert_eq!(read_address("E 4.3"), bit(Space::Input, 4, 3));
        assert_eq!(read_address("a0.1"), bit(Space::Output, 0, 1));
        assert_eq!(
            read_address("EW64"),
            Some(AddrKey::Word {
                space: Space::Input,
                byte: 64
            })
        );
        assert_eq!(read_address("M0.0"), None);
    }

    #[test]
    fn a_list_is_read_with_its_columns_in_any_order() {
        let text = "Omschrijving;Adres;Type;Tag\n\
                    Pomp 1 draait;E0.0;DI;-B1\n\
                    Klep 3;;uitgang;-Y3\n\
                    ;;;\n\
                    Onbekend;;?;-X9\n";
        let list = read_list(text.as_bytes()).unwrap();
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].tag, "-B1");
        assert_eq!(list.items[0].kind, ChannelType::DigitalInput);
        assert!(list.items[0].address.is_some());
        assert_eq!(list.items[1].kind, ChannelType::DigitalOutput);
        assert_eq!(list.notes.len(), 1);
        // The line in the file, the empty one included.
        assert_eq!(list.notes[0].line, Some(5));
    }

    #[test]
    fn a_note_names_the_line_of_the_file_with_blank_lines_counted() {
        let list = read_list(b"tag,type\n\n\nB1,DI\nB2,?\n").unwrap();
        assert_eq!(list.notes.len(), 1);
        assert_eq!(list.notes[0].line, Some(5));
    }

    #[test]
    fn an_address_near_the_top_of_u32_is_left_out_and_never_panics() {
        let text = "tag,address,type\nB1,I4294967295.0,DI\nB2,IW4294967294,AI\nB3,I0.0,DI\n";
        let list = read_list(text.as_bytes()).unwrap();
        assert!(read_address("I4294967295.0").is_none());
        assert!(read_address("IW4294967294").is_none());
        let plan = crate::generate::plan(
            &novo_core::Project::default(),
            &list,
            &crate::generate::Options::default(),
        );
        assert!(!plan.cards.is_empty());
    }

    #[test]
    fn the_type_wins_over_an_address_in_the_other_direction() {
        let list = read_list(b"tag,address,type\nB1,Q0.0,DI\n").unwrap();
        assert_eq!(list.items[0].kind, ChannelType::DigitalInput);
        assert!(list.items[0].address.is_none());
        assert_eq!(list.notes.len(), 1);
    }

    #[test]
    fn a_file_without_a_heading_is_refused() {
        assert_eq!(read_list(b"a,b\n1,2\n"), Err(ListError::NoHeading));
    }
}
