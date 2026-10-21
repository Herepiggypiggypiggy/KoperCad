//! Reading a tag file back: any of the formats [`write`](super::write)
//! makes, plus the same files after the PLC programmer has edited them.

use std::io::Cursor;

use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
use quick_xml::events::{BytesStart, Event};
use thiserror::Error;

use crate::layout::ChannelType;

use super::csvio;

/// One tag of a file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileTag {
    /// Line (or row) of the file, from 1, for messages.
    pub line: usize,
    pub address: String,
    /// Card tag, channel type and number, when the file has them (our CSV).
    pub card: String,
    pub kind: Option<ChannelType>,
    pub channel: Option<u16>,
    pub name: String,
    pub comment: String,
    /// As written in the file (`BOOL`, `Bool`, `INT`); may be empty.
    pub data_type: String,
}

/// Which kind of file it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Csv,
    PlcOpen,
    Tia,
    Studio5000,
}

impl FileKind {
    pub fn label(self) -> &'static str {
        match self {
            FileKind::Csv => "I/O list CSV",
            FileKind::PlcOpen => "PLCopen XML",
            FileKind::Tia => "TIA Portal tag table",
            FileKind::Studio5000 => "Studio 5000 tags",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TagFile {
    pub kind: FileKind,
    pub tags: Vec<FileTag>,
}

#[derive(Debug, Error, PartialEq)]
pub enum ReadError {
    #[error("this is not a tag file KoperCAD knows: {0}")]
    Unknown(String),
    #[error("cannot read the file: {0}")]
    Bad(String),
}

/// Reads a tag file, finding out which format it is from its content.
pub fn read(bytes: &[u8]) -> Result<TagFile, ReadError> {
    if bytes.starts_with(b"PK") {
        return xlsx(bytes);
    }
    let text = String::from_utf8_lossy(bytes);
    if text
        .trim_start_matches('\u{feff}')
        .trim_start()
        .starts_with('<')
    {
        return plcopen(&text);
    }
    let rows = csvio::rows(&text);
    let first = rows
        .first()
        .and_then(|r| r.first())
        .map(|s| s.trim().to_ascii_lowercase());
    if matches!(first.as_deref(), Some("remark" | "type")) && rows.iter().any(|r| is_alias(r)) {
        return Ok(studio(&rows));
    }
    table(&rows, FileKind::Csv)
}

fn is_alias(row: &[String]) -> bool {
    row.first()
        .is_some_and(|c| c.trim().eq_ignore_ascii_case("alias"))
}

fn studio(rows: &[Vec<String>]) -> TagFile {
    let mut tags = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        if !is_alias(row) {
            continue;
        }
        let get = |n: usize| row.get(n).map(|s| s.trim().to_owned()).unwrap_or_default();
        tags.push(FileTag {
            line: i + 1,
            name: get(2),
            comment: get(3),
            address: get(5),
            ..FileTag::default()
        });
    }
    TagFile {
        kind: FileKind::Studio5000,
        tags,
    }
}

fn data_text(d: &Data) -> String {
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

fn xlsx(bytes: &[u8]) -> Result<TagFile, ReadError> {
    novo_parts::xlsx_guard::check_xlsx(bytes).map_err(|e| ReadError::Bad(e.0))?;
    let mut book: Xlsx<_> = open_workbook_from_rs(Cursor::new(bytes.to_vec()))
        .map_err(|e: calamine::XlsxError| ReadError::Bad(e.to_string()))?;
    for name in book.sheet_names() {
        let range = book
            .worksheet_range(&name)
            .map_err(|e| ReadError::Bad(e.to_string()))?;
        let rows: Vec<Vec<String>> = range
            .rows()
            .map(|r| r.iter().map(data_text).collect())
            .collect();
        if let Ok(file) = table(&rows, FileKind::Tia) {
            return Ok(file);
        }
    }
    Err(ReadError::Unknown("no sheet has a tag table".into()))
}

/// Finds the header row (a name or tag column plus an address or card
/// column) and reads the rows under it.
fn table(rows: &[Vec<String>], kind: FileKind) -> Result<TagFile, ReadError> {
    let find = |row: &[String], names: &[&str]| {
        row.iter().position(|c| {
            let c = c.trim().trim_start_matches('\u{feff}').to_ascii_lowercase();
            names.iter().any(|n| c == *n)
        })
    };
    for (h, header) in rows.iter().enumerate() {
        let name = find(header, &["tag", "name"]);
        let address = find(header, &["address", "logical address"]);
        let card = find(header, &["card"]);
        let channel = find(header, &["channel"]);
        if name.is_none() || (address.is_none() && (card.is_none() || channel.is_none())) {
            continue;
        }
        let comment = find(header, &["signal", "comment"]);
        let kind_col = find(header, &["type"]);
        let data = find(header, &["data type"]);
        let mut tags = Vec::new();
        for (i, row) in rows.iter().enumerate().skip(h + 1) {
            let get = |c: Option<usize>| {
                c.and_then(|c| row.get(c))
                    .map(|s| s.trim().to_owned())
                    .unwrap_or_default()
            };
            let tag = FileTag {
                line: i + 1,
                address: get(address),
                card: get(card),
                kind: ChannelType::from_code(&get(kind_col)),
                channel: get(channel).parse().ok(),
                name: get(name),
                comment: get(comment),
                data_type: get(data),
            };
            if tag.address.is_empty() && tag.card.is_empty() {
                continue;
            }
            tags.push(tag);
        }
        let kind = if address.is_some() && card.is_none() {
            FileKind::Tia
        } else {
            kind
        };
        return Ok(TagFile { kind, tags });
    }
    Err(ReadError::Unknown(
        "no header row with a tag name and an address".into(),
    ))
}

fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    s.rsplit(':').next().unwrap_or("").to_owned()
}

fn attr(e: &BytesStart<'_>, key: &str, reader_decoder: quick_xml::encoding::Decoder) -> String {
    e.attributes()
        .flatten()
        .find(|a| local(a.key.as_ref()) == key)
        .and_then(|a| {
            a.decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, reader_decoder)
                .ok()
                .map(|v| v.into_owned())
        })
        .unwrap_or_default()
}

fn plcopen(xml: &str) -> Result<TagFile, ReadError> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let dec = reader.decoder();
    let mut tags = Vec::new();
    let mut open: Option<FileTag> = None;
    let mut in_type = false;
    let mut in_doc = false;
    let mut doc = String::new();
    let mut seen_project = false;
    // Lines are counted as the reader moves on, not from the start each time.
    let (mut line, mut seen) = (0usize, 0usize);
    loop {
        let event = reader
            .read_event()
            .map_err(|e| ReadError::Bad(e.to_string()))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name = local(e.name().as_ref());
                match name.as_str() {
                    "project" => seen_project = true,
                    "variable" => {
                        open = Some(FileTag {
                            line: {
                                let at = reader.buffer_position() as usize;
                                line += xml[seen..at].matches('\n').count();
                                seen = at;
                                line + 1
                            },
                            name: attr(&e, "name", dec),
                            address: attr(&e, "address", dec),
                            ..FileTag::default()
                        });
                    }
                    "type" if open.is_some() && !empty => in_type = true,
                    "documentation" if open.is_some() && !empty => {
                        in_doc = true;
                        doc.clear();
                    }
                    other if in_type => {
                        if let Some(t) = open.as_mut()
                            && t.data_type.is_empty()
                        {
                            t.data_type = other.to_owned();
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => match local(e.name().as_ref()).as_str() {
                "type" => in_type = false,
                "documentation" => {
                    in_doc = false;
                    if let Some(t) = open.as_mut() {
                        t.comment = doc.trim().to_owned();
                    }
                }
                "variable" => {
                    if let Some(t) = open.take() {
                        tags.push(t);
                    }
                }
                _ => {}
            },
            Event::Text(t) if in_doc => {
                doc.push_str(&t.decode().map_err(|e| ReadError::Bad(e.to_string()))?);
            }
            Event::CData(t) if in_doc => doc.push_str(&String::from_utf8_lossy(&t.into_inner())),
            Event::GeneralRef(r) if in_doc => {
                if let Ok(Some(c)) = r.resolve_char_ref() {
                    doc.push(c);
                } else if let Ok(name) = r.decode() {
                    match name.as_ref() {
                        "amp" => doc.push('&'),
                        "lt" => doc.push('<'),
                        "gt" => doc.push('>'),
                        "quot" => doc.push('"'),
                        "apos" => doc.push('\''),
                        _ => {}
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !seen_project {
        return Err(ReadError::Unknown("this XML is not PLCopen".into()));
    }
    tags.retain(|t| !t.address.is_empty());
    Ok(TagFile {
        kind: FileKind::PlcOpen,
        tags,
    })
}
