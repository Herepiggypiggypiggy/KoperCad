//! The tag list in the formats the PLC engineering tools take.
//!
//! What each file is, and how sure we are of it:
//! - CSV: our own layout, read back by [`import`](super::import) (tested).
//! - PLCopen XML (TC6 v2.01): global variables with `AT` addresses, which
//!   CODESYS-based tools read through Import PLCopenXML. Written from the
//!   public schema; checked here only by reading it back.
//! - TIA Portal: the column layout of a tag table exported to Excel. Written
//!   from the layout; not yet tried against TIA Portal itself.
//! - Studio 5000: the tag CSV (`TYPE,SCOPE,NAME,...`) with alias tags on the
//!   module data. Written from the published layout; not yet tried against
//!   Studio 5000 itself.

use rust_xlsxwriter::{Format, Workbook};

use crate::address::AddressStyle;
use crate::index::PlcIndex;
use crate::layout::Space;

use super::csvio;
use super::tags::{Tag, TagOptions, TagSet, tag_set};
use novo_core::Project;

/// Which file to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkFormat {
    /// Every channel of every station in one table (also the round trip).
    Csv,
    /// PLCopen XML, one file per station.
    PlcOpen,
    /// TIA Portal tag table (`.xlsx`), one file per station.
    Tia,
    /// Studio 5000 tag CSV with alias tags, one file per station.
    Studio5000,
    /// The address plan: cards, slots and address ranges.
    Hardware,
}

impl LinkFormat {
    pub const ALL: [LinkFormat; 5] = [
        LinkFormat::Csv,
        LinkFormat::PlcOpen,
        LinkFormat::Tia,
        LinkFormat::Studio5000,
        LinkFormat::Hardware,
    ];

    /// Menu text.
    pub fn label(self) -> &'static str {
        match self {
            LinkFormat::Csv => "I/O list (CSV, can be read back)",
            LinkFormat::PlcOpen => "PLCopen XML (CODESYS and compatible)",
            LinkFormat::Tia => "Tag table for TIA Portal (.xlsx)",
            LinkFormat::Studio5000 => "Tags for Studio 5000 (CSV)",
            LinkFormat::Hardware => "Address plan (CSV)",
        }
    }

    /// The command id and CLI name.
    pub fn key(self) -> &'static str {
        match self {
            LinkFormat::Csv => "csv",
            LinkFormat::PlcOpen => "plcopen",
            LinkFormat::Tia => "tia",
            LinkFormat::Studio5000 => "studio5000",
            LinkFormat::Hardware => "hardware",
        }
    }

    pub fn from_key(key: &str) -> Option<LinkFormat> {
        LinkFormat::ALL
            .into_iter()
            .find(|f| f.key().eq_ignore_ascii_case(key.trim()))
    }

    pub fn extension(self) -> &'static str {
        match self {
            LinkFormat::Csv | LinkFormat::Studio5000 | LinkFormat::Hardware => "csv",
            LinkFormat::PlcOpen => "xml",
            LinkFormat::Tia => "xlsx",
        }
    }
}

/// Settings of an export.
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    pub include_spares: bool,
    /// Written into file headers; empty leaves dates out, which keeps files
    /// reproducible.
    pub stamp: String,
    /// The project's name, for headers.
    pub project: String,
}

/// One file of an export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportFile {
    /// What goes between the project name and the extension: the station,
    /// or the kind of file. Empty for the unnamed station of a single-station
    /// project.
    pub suffix: String,
    pub extension: &'static str,
    pub bytes: Vec<u8>,
    /// How many tags the file holds.
    pub tags: usize,
}

/// An export and what was left out of it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Export {
    pub files: Vec<ExportFile>,
    pub warnings: Vec<String>,
}

/// Writes `format` for a project.
pub fn export(
    project: &Project,
    index: &PlcIndex,
    format: LinkFormat,
    options: &ExportOptions,
) -> Export {
    let mut out = Export::default();
    let mut tag_options = TagOptions {
        include_spares: options.include_spares,
        ..TagOptions::default()
    };
    match format {
        LinkFormat::Hardware => {
            out.files.push(ExportFile {
                suffix: "address plan".into(),
                extension: "csv",
                bytes: hardware(project, index).into_bytes(),
                tags: 0,
            });
            return out;
        }
        LinkFormat::Csv => {
            tag_options.require_address = false;
            let set = tag_set(project, index, index.style, &tag_options);
            out.warnings = set.warnings.clone();
            out.files.push(ExportFile {
                suffix: "I-O list".into(),
                extension: "csv",
                bytes: csv(&set).into_bytes(),
                tags: set.tags.len(),
            });
            return out;
        }
        LinkFormat::Studio5000 => tag_options.max_name = 40,
        LinkFormat::PlcOpen | LinkFormat::Tia => {}
    }
    let style = match format {
        LinkFormat::Studio5000 => AddressStyle::Rockwell,
        LinkFormat::Tia => AddressStyle::Siemens,
        _ => AddressStyle::Iec,
    };
    let set = tag_set(project, index, style, &tag_options);
    out.warnings = set.warnings.clone();
    let stations = set.stations();
    for station in &stations {
        let tags: Vec<&Tag> = set.of_station(station).collect();
        let name = if station.is_empty() {
            String::new()
        } else {
            station.clone()
        };
        let bytes = match format {
            LinkFormat::PlcOpen => plcopen(&tags, station, options).into_bytes(),
            LinkFormat::Tia => tia(&tags, station),
            _ => studio(&tags, options).into_bytes(),
        };
        out.files.push(ExportFile {
            suffix: name,
            extension: format.extension(),
            bytes,
            tags: tags.len(),
        });
    }
    out
}

/// Column names of the CSV, which [`import`](super::import) recognises.
pub const CSV_HEADER: [&str; 13] = [
    "Station",
    "Card",
    "Slot",
    "Address",
    "Type",
    "Channel",
    "Tag",
    "Data type",
    "Signal",
    "Terminal",
    "Connected to",
    "Drawn at",
    "Spare",
];

fn csv(set: &TagSet) -> String {
    let mut out = String::from("\u{feff}");
    out.push_str(&csvio::line(&CSV_HEADER, ';'));
    for t in &set.tags {
        let slot = t.slot.map(|s| s.to_string()).unwrap_or_default();
        let channel = t.channel.to_string();
        out.push_str(&csvio::line(
            &[
                &t.station,
                &t.card_tag,
                &slot,
                &t.address_text,
                t.kind.code(),
                &channel,
                &t.name,
                t.data_type(),
                &t.comment,
                &t.terminal,
                &t.connected,
                &t.drawn_at,
                if t.spare { "spare" } else { "" },
            ],
            ';',
        ));
    }
    out
}

fn xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 && !matches!(c, '\n' | '\t') => {}
            c => out.push(c),
        }
    }
    out
}

fn plcopen(tags: &[&Tag], station: &str, options: &ExportOptions) -> String {
    let name = if station.is_empty() { "PLC" } else { station };
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    s.push_str("<project xmlns=\"http://www.plcopen.org/xml/tc6_0201\">\n");
    s.push_str(&format!(
        "  <fileHeader companyName=\"\" productName=\"KoperCAD\" productVersion=\"{}\" creationDateTime=\"{}\"/>\n",
        env!("CARGO_PKG_VERSION"),
        xml(if options.stamp.is_empty() { "1970-01-01T00:00:00" } else { &options.stamp }),
    ));
    s.push_str(&format!(
        "  <contentHeader name=\"{}\">\n    <coordinateInfo>\n      <fbd><scaling x=\"1\" y=\"1\"/></fbd>\n      <ld><scaling x=\"1\" y=\"1\"/></ld>\n      <sfc><scaling x=\"1\" y=\"1\"/></sfc>\n    </coordinateInfo>\n  </contentHeader>\n",
        xml(&if options.project.is_empty() { name.to_owned() } else { options.project.clone() }),
    ));
    s.push_str("  <types>\n    <dataTypes/>\n    <pous/>\n  </types>\n");
    s.push_str("  <instances>\n    <configurations>\n");
    s.push_str(&format!(
        "      <configuration name=\"{}\">\n        <globalVars name=\"IO\">\n",
        xml(&identifier_or(name))
    ));
    for t in tags {
        s.push_str(&format!(
            "          <variable name=\"{}\" address=\"{}\">\n            <type><{ty}/></type>\n",
            xml(&t.name),
            xml(&t.address_text),
            ty = t.data_type(),
        ));
        if !t.comment.is_empty() {
            s.push_str(&format!(
                "            <documentation><xhtml xmlns=\"http://www.w3.org/1999/xhtml\">{}</xhtml></documentation>\n",
                xml(&t.comment)
            ));
        }
        s.push_str("          </variable>\n");
    }
    s.push_str("        </globalVars>\n      </configuration>\n    </configurations>\n  </instances>\n</project>\n");
    s
}

fn identifier_or(name: &str) -> String {
    super::tags::identifier(name, 64).unwrap_or_else(|| "PLC".into())
}

/// The columns of a TIA Portal tag table export.
pub const TIA_HEADER: [&str; 10] = [
    "Name",
    "Path",
    "Data Type",
    "Logical Address",
    "Comment",
    "Hmi Visible",
    "Hmi Accessible",
    "Hmi Writeable",
    "Typeobject ID",
    "Version ID",
];

fn tia(tags: &[&Tag], station: &str) -> Vec<u8> {
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    let title = if station.is_empty() {
        "PLC tags".to_owned()
    } else {
        format!("PLC tags {station}")
    };
    // Sheet names are at most 31 characters and cannot hold these.
    let title: String = title
        .chars()
        .filter(|c| !matches!(c, '[' | ']' | ':' | '*' | '?' | '/' | '\\'))
        .take(31)
        .collect();
    let _ = sheet.set_name(title);
    let bold = Format::new().set_bold();
    for (c, h) in TIA_HEADER.iter().enumerate() {
        let _ = sheet.write_string_with_format(0, c as u16, *h, &bold);
    }
    for (r, t) in tags.iter().enumerate() {
        let row = r as u32 + 1;
        let ty = if t.kind.is_analog() { "Int" } else { "Bool" };
        let cells: [&str; 8] = [
            &t.name,
            "Default tag table",
            ty,
            &t.address_text,
            &t.comment,
            "True",
            "True",
            "True",
        ];
        for (c, v) in cells.iter().enumerate() {
            let _ = sheet.write_string(row, c as u16, *v);
        }
    }
    let _ = sheet.set_column_width(0, 32);
    let _ = sheet.set_column_width(1, 20);
    let _ = sheet.set_column_width(3, 16);
    let _ = sheet.set_column_width(4, 48);
    book.save_to_buffer().unwrap_or_default()
}

fn studio(tags: &[&Tag], options: &ExportOptions) -> String {
    let mut s = String::new();
    s.push_str("remark,CSV-Import-Export\r\n");
    if !options.stamp.is_empty() {
        s.push_str(&format!("remark,Date = {}\r\n", options.stamp));
    }
    s.push_str("remark,Owner = KoperCAD\r\n");
    s.push_str("TYPE,SCOPE,NAME,DESCRIPTION,DATATYPE,SPECIFIER,ATTRIBUTES\r\n");
    for t in tags {
        s.push_str(&format!(
            "ALIAS,,{},{},,{},\"(RADIX := Decimal)\"\r\n",
            t.name,
            csvio::quoted(&t.comment),
            t.address_text,
        ));
    }
    s
}

/// The address plan: one row per card with its slot, its channels and the
/// byte range it takes in each space.
pub fn hardware(project: &Project, index: &PlcIndex) -> String {
    let mut out = String::from("\u{feff}");
    out.push_str(&csvio::line(
        &[
            "Station",
            "Slot",
            "Card",
            "Channels",
            "Input from",
            "Input to",
            "Input bytes",
            "Output from",
            "Output to",
            "Output bytes",
            "Channels drawn",
            "Spare",
        ],
        ';',
    ));
    for card in index.cards() {
        let layout = card.channels();
        let range = |space: Space| {
            card.range(space)
                .map(|r| (r.start.to_string(), (r.end - 1).to_string()))
                .unwrap_or_default()
        };
        let (i0, i1) = range(Space::Input);
        let (o0, o1) = range(Space::Output);
        let total: u32 = crate::layout::ChannelType::ALL
            .iter()
            .map(|k| u32::from(layout.count(*k)))
            .sum();
        let drawn: u32 = crate::layout::ChannelType::ALL
            .iter()
            .map(|k| {
                (0..layout.count(*k))
                    .filter(|n| !index.drawn(card.device, *k, *n).is_empty())
                    .count() as u32
            })
            .sum();
        out.push_str(&csvio::line(
            &[
                &card.station,
                &card.slot.map(|s| s.to_string()).unwrap_or_default(),
                &card_label(project, card),
                &layout.to_string(),
                &i0,
                &i1,
                &layout.bytes(Space::Input).to_string(),
                &o0,
                &o1,
                &layout.bytes(Space::Output).to_string(),
                &drawn.to_string(),
                &total.saturating_sub(drawn).to_string(),
            ],
            ';',
        ));
    }
    out
}

fn card_label(project: &Project, card: &crate::card::Card) -> String {
    project
        .device(card.device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default()
}
