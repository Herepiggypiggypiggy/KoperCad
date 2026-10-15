//! The columns of each table: what they read from a record, which ones the
//! import may write, and how a typed cell becomes a value.

use std::collections::BTreeSet;

use novo_core::{
    AnyId, DeviceKind, DeviceTag, Endpoint, PinRef, Project, Record, TerminalKind, Value,
};
use novo_parts::{display_value, is_internal_key, parse_number, parse_value};

use super::grid::{Cell, TableKind, number_text};

/// The header prefix of property columns (as the bulk editor in
/// `novo-parts` writes them), so a property called `description` never
/// reads back as the Description column.
pub const PROPERTY_PREFIX: &str = "properties.";

/// What a column holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Slot {
    /// The stable key of the row. Never edited.
    Id,
    DeviceTag,
    DeviceDescription,
    DeviceKind,
    DeviceManufacturer,
    DeviceOrderNumber,
    TerminalStrip,
    TerminalName,
    TerminalLevel,
    TerminalOrder,
    TerminalJumper,
    TerminalKind,
    WirePage,
    WireFrom,
    WireTo,
    WireNumber,
    WireColor,
    WireCrossSection,
    WireCore,
    CableTag,
    CableDescription,
    CableLength,
    CableCores,
    Property(String),
}

/// One column of a table.
#[derive(Debug, Clone)]
pub(crate) struct Field {
    pub header: String,
    pub slot: Slot,
}

impl Field {
    fn new(header: &str, slot: Slot) -> Field {
        Field {
            header: header.to_owned(),
            slot,
        }
    }

    /// Whether the import may change this column. The others are shown for
    /// orientation and ignored on import.
    pub fn editable(&self) -> bool {
        matches!(
            self.slot,
            Slot::DeviceTag
                | Slot::DeviceDescription
                | Slot::TerminalName
                | Slot::TerminalLevel
                | Slot::TerminalOrder
                | Slot::TerminalJumper
                | Slot::WireNumber
                | Slot::WireColor
                | Slot::WireCrossSection
                | Slot::CableTag
                | Slot::CableDescription
                | Slot::CableLength
                | Slot::Property(_)
        )
    }

    pub fn is_id(&self) -> bool {
        self.slot == Slot::Id
    }
}

/// The header of a property column.
pub(crate) fn property_header(key: &str) -> String {
    format!("{PROPERTY_PREFIX}{key}")
}

/// Property key of a header, if it is a property column.
pub(crate) fn property_of_header(header: &str) -> Option<String> {
    let h = header.trim();
    let head = h.get(..PROPERTY_PREFIX.len())?;
    head.eq_ignore_ascii_case(PROPERTY_PREFIX)
        .then(|| h[PROPERTY_PREFIX.len()..].trim().to_owned())
        .filter(|k| !k.is_empty())
}

/// The rows of a table, in reading order.
pub(crate) fn ids(project: &Project, kind: TableKind) -> Vec<AnyId> {
    match kind {
        TableKind::Devices => {
            let mut devices: Vec<_> = project.devices().collect();
            devices.sort_by(|a, b| {
                novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()).then(a.id.cmp(&b.id))
            });
            devices.into_iter().map(|d| d.id.into()).collect()
        }
        TableKind::Terminals => {
            let mut strips: Vec<_> = project
                .devices()
                .filter(|d| project.terminals().any(|t| t.strip == d.id))
                .collect();
            strips.sort_by(|a, b| {
                novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()).then(a.id.cmp(&b.id))
            });
            strips
                .into_iter()
                .flat_map(|s| project.terminals_of_strip(s.id))
                .map(|t| t.id.into())
                .collect()
        }
        TableKind::Wires => project.wires().map(|w| w.id.into()).collect(),
        TableKind::Cables => {
            let mut cables: Vec<_> = project.cables().collect();
            cables.sort_by(|a, b| {
                novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()).then(a.id.cmp(&b.id))
            });
            cables.into_iter().map(|c| c.id.into()).collect()
        }
    }
}

/// The columns of a table for this project: the fixed ones, then one
/// `properties.<key>` column for every property any row has.
pub(crate) fn fields(project: &Project, kind: TableKind) -> Vec<Field> {
    let mut fields = vec![Field::new("Id", Slot::Id)];
    match kind {
        TableKind::Devices => fields.extend([
            Field::new("Tag", Slot::DeviceTag),
            Field::new("Description", Slot::DeviceDescription),
            Field::new("Kind", Slot::DeviceKind),
            Field::new("Manufacturer", Slot::DeviceManufacturer),
            Field::new("Order number", Slot::DeviceOrderNumber),
        ]),
        TableKind::Terminals => fields.extend([
            Field::new("Strip", Slot::TerminalStrip),
            Field::new("Name", Slot::TerminalName),
            Field::new("Level", Slot::TerminalLevel),
            Field::new("Order", Slot::TerminalOrder),
            Field::new("Jumper", Slot::TerminalJumper),
            Field::new("Kind", Slot::TerminalKind),
        ]),
        TableKind::Wires => fields.extend([
            Field::new("Page", Slot::WirePage),
            Field::new("From", Slot::WireFrom),
            Field::new("To", Slot::WireTo),
            Field::new("Number", Slot::WireNumber),
            Field::new("Color", Slot::WireColor),
            Field::new("Cross-section [mm²]", Slot::WireCrossSection),
            Field::new("Cable core", Slot::WireCore),
        ]),
        TableKind::Cables => fields.extend([
            Field::new("Tag", Slot::CableTag),
            Field::new("Description", Slot::CableDescription),
            Field::new("Length [m]", Slot::CableLength),
            Field::new("Cores", Slot::CableCores),
        ]),
    }
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for id in ids(project, kind) {
        if let Some(mut record) = project.record(id)
            && let Some(props) = record.properties_mut()
        {
            keys.extend(props.keys().filter(|k| !is_internal_key(k)).cloned());
        }
    }
    fields.extend(
        keys.into_iter()
            .map(|k| Field::new(&property_header(&k), Slot::Property(k))),
    );
    fields
}

fn endpoint_text(project: &Project, end: Endpoint) -> String {
    match end {
        Endpoint::Pin { symbol, pin } => project
            .pin_designation(PinRef::new(symbol, pin))
            .unwrap_or_default(),
        Endpoint::Junction { .. } => "junction".into(),
        Endpoint::Free { .. } => "open end".into(),
    }
}

fn terminal_kind_text(kind: TerminalKind) -> &'static str {
    match kind {
        TerminalKind::Feedthrough => "feedthrough",
        TerminalKind::ProtectiveEarth => "PE",
        TerminalKind::Neutral => "neutral",
        TerminalKind::Fuse => "fuse",
        TerminalKind::Disconnect => "disconnect",
    }
}

fn opt_number(n: Option<f64>) -> Cell {
    n.map_or(Cell::Empty, Cell::Number)
}

fn value_cell(value: &Value) -> Cell {
    match value {
        Value::Int(i) => Cell::Number(*i as f64),
        Value::Float(f) => Cell::Number(*f),
        other => Cell::text(display_value(other)),
    }
}

/// What the project holds in a column of one row.
pub(crate) fn read(project: &Project, id: AnyId, field: &Field) -> Cell {
    if field.slot == Slot::Id {
        return Cell::Number(id.raw() as f64);
    }
    let Some(record) = project.record(id) else {
        return Cell::Empty;
    };
    if let Slot::Property(key) = &field.slot {
        let mut record = record;
        return record
            .properties_mut()
            .and_then(|p| p.get(key))
            .map_or(Cell::Empty, value_cell);
    }
    match (&record, &field.slot) {
        (Record::Device(d), Slot::DeviceTag) => Cell::text(d.tag.to_string()),
        (Record::Device(d), Slot::DeviceDescription) => Cell::text(&d.description),
        (Record::Device(d), Slot::DeviceKind) => Cell::text(match d.kind {
            DeviceKind::General => "general",
            DeviceKind::TerminalStrip => "terminal strip",
            DeviceKind::Connector => "connector",
            DeviceKind::Plc => "plc",
        }),
        (Record::Device(d), Slot::DeviceManufacturer | Slot::DeviceOrderNumber) => {
            let manufacturer = field.slot == Slot::DeviceManufacturer;
            let list: Vec<&str> = d
                .parts
                .iter()
                .filter_map(|p| project.part(*p))
                .map(|p| {
                    if manufacturer {
                        p.manufacturer.as_str()
                    } else {
                        p.order_number.as_str()
                    }
                })
                .collect();
            Cell::text(list.join("; "))
        }
        (Record::Terminal(t), Slot::TerminalStrip) => Cell::text(
            project
                .device(t.strip)
                .map(|d| d.tag.to_string())
                .unwrap_or_default(),
        ),
        (Record::Terminal(t), Slot::TerminalName) => Cell::text(&t.name),
        (Record::Terminal(t), Slot::TerminalLevel) => Cell::Number(f64::from(t.level)),
        (Record::Terminal(t), Slot::TerminalOrder) => Cell::Number(t.order as f64),
        (Record::Terminal(t), Slot::TerminalJumper) => opt_number(t.jumper.map(f64::from)),
        (Record::Terminal(t), Slot::TerminalKind) => Cell::text(terminal_kind_text(t.kind)),
        (Record::Wire(w), Slot::WirePage) => Cell::text(
            project
                .page(w.page)
                .map(|p| p.name.clone())
                .unwrap_or_default(),
        ),
        (Record::Wire(w), Slot::WireFrom) => Cell::text(endpoint_text(project, w.from)),
        (Record::Wire(w), Slot::WireTo) => Cell::text(endpoint_text(project, w.to)),
        (Record::Wire(w), Slot::WireNumber) => Cell::text(w.number.clone().unwrap_or_default()),
        (Record::Wire(w), Slot::WireColor) => Cell::text(w.color.clone().unwrap_or_default()),
        (Record::Wire(w), Slot::WireCrossSection) => opt_number(w.cross_section_mm2),
        (Record::Wire(w), Slot::WireCore) => match project.core_of_wire(w.id) {
            Some((cable, core)) => {
                let c = project.cable(cable);
                Cell::text(format!(
                    "{}:{}",
                    c.map(|c| c.tag.to_string()).unwrap_or_default(),
                    c.and_then(|c| c.cores.get(core))
                        .map_or("", |c| c.name.as_str())
                ))
            }
            None => Cell::Empty,
        },
        (Record::Cable(c), Slot::CableTag) => Cell::text(c.tag.to_string()),
        (Record::Cable(c), Slot::CableDescription) => Cell::text(&c.description),
        (Record::Cable(c), Slot::CableLength) => opt_number(c.length_m),
        (Record::Cable(c), Slot::CableCores) => Cell::Number(c.cores.len() as f64),
        _ => Cell::Empty,
    }
}

/// A cell understood for its column.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Parsed {
    Text(String),
    /// A text that may be cleared (`None`).
    OptText(Option<String>),
    Tag(DeviceTag),
    Number(Option<f64>),
    Property(Option<Value>),
}

impl Parsed {
    /// The value as one comparable string: two cells that mean the same
    /// thing (`2,5`, 2.5 and `2.50`) give the same string.
    pub fn canon(&self) -> String {
        match self {
            Parsed::Text(t) => t.clone(),
            Parsed::OptText(t) => t.clone().unwrap_or_default(),
            Parsed::Tag(t) => t.to_string(),
            Parsed::Number(n) => n.map(number_text).unwrap_or_default(),
            Parsed::Property(v) => v.as_ref().map(ToString::to_string).unwrap_or_default(),
        }
    }

    /// The value as the preview shows it.
    pub fn shown(&self) -> String {
        match self {
            Parsed::Property(Some(v)) => display_value(v),
            other => other.canon(),
        }
    }
}

fn whole(text: &str, header: &str, min: i64, max: i64) -> Result<f64, String> {
    let n = parse_number(text).ok_or_else(|| format!("{header}: '{text}' is not a number"))?;
    if n.fract() != 0.0 {
        return Err(format!("{header}: '{text}' is not a whole number"));
    }
    if n < min as f64 || n > max as f64 {
        return Err(format!("{header}: {text} is outside {min} to {max}"));
    }
    Ok(n)
}

/// Understands a cell for an editable column. `like` is the value the
/// property has now, so a number stays a number.
pub(crate) fn parse(field: &Field, cell: &Cell, like: Option<&Value>) -> Result<Parsed, String> {
    let text = cell.as_text();
    let header = field.header.as_str();
    match &field.slot {
        Slot::DeviceTag | Slot::CableTag => {
            // CSV written for Excel puts an apostrophe before a tag.
            let t = text.strip_prefix('\'').unwrap_or(&text).trim();
            if t.is_empty() {
                return Err(format!("{header}: the tag is empty"));
            }
            DeviceTag::parse(t).map(Parsed::Tag).map_err(|e| {
                if t.starts_with('#') {
                    format!("{header}: '{t}' looks like a spreadsheet error, not a tag ({e})")
                } else {
                    format!("{header}: '{t}' is not a valid tag ({e})")
                }
            })
        }
        Slot::DeviceDescription | Slot::CableDescription => Ok(Parsed::Text(text)),
        Slot::TerminalName => {
            if text.is_empty() {
                Err(format!("{header}: the name is empty"))
            } else {
                Ok(Parsed::Text(text))
            }
        }
        Slot::WireNumber | Slot::WireColor => {
            Ok(Parsed::OptText((!text.is_empty()).then_some(text)))
        }
        Slot::TerminalLevel => whole(&text, header, 1, 255).map(|n| Parsed::Number(Some(n))),
        Slot::TerminalOrder => {
            whole(&text, header, -1_000_000_000, 1_000_000_000).map(|n| Parsed::Number(Some(n)))
        }
        Slot::TerminalJumper => {
            if text.is_empty() {
                Ok(Parsed::Number(None))
            } else {
                whole(&text, header, 0, i64::from(u32::MAX)).map(|n| Parsed::Number(Some(n)))
            }
        }
        Slot::WireCrossSection | Slot::CableLength => {
            if text.is_empty() {
                return Ok(Parsed::Number(None));
            }
            match parse_number(&text) {
                Some(n) if n >= 0.0 => Ok(Parsed::Number(Some(n))),
                Some(_) => Err(format!("{header}: {text} is negative")),
                None => Err(format!("{header}: '{text}' is not a number")),
            }
        }
        Slot::Property(_) => {
            if text.is_empty() {
                return Ok(Parsed::Property(None));
            }
            parse_value(&text, like)
                .map(|v| Parsed::Property(Some(v)))
                .ok_or_else(|| format!("{header}: '{text}' does not fit the value that is there"))
        }
        _ => Err(format!("{header} cannot be edited")),
    }
}

/// Writes a parsed value into a copy of the record.
pub(crate) fn apply(record: &mut Record, field: &Field, value: &Parsed) {
    match (record, &field.slot, value) {
        (Record::Device(d), Slot::DeviceTag, Parsed::Tag(t)) => d.tag = t.clone(),
        (Record::Device(d), Slot::DeviceDescription, Parsed::Text(t)) => d.description = t.clone(),
        (Record::Terminal(t), Slot::TerminalName, Parsed::Text(n)) => t.name = n.clone(),
        (Record::Terminal(t), Slot::TerminalLevel, Parsed::Number(Some(n))) => t.level = *n as u8,
        (Record::Terminal(t), Slot::TerminalOrder, Parsed::Number(Some(n))) => t.order = *n as i64,
        (Record::Terminal(t), Slot::TerminalJumper, Parsed::Number(n)) => {
            t.jumper = n.map(|n| n as u32);
        }
        (Record::Wire(w), Slot::WireNumber, Parsed::OptText(t)) => w.number = t.clone(),
        (Record::Wire(w), Slot::WireColor, Parsed::OptText(t)) => w.color = t.clone(),
        (Record::Wire(w), Slot::WireCrossSection, Parsed::Number(n)) => w.cross_section_mm2 = *n,
        (Record::Cable(c), Slot::CableTag, Parsed::Tag(t)) => c.tag = t.clone(),
        (Record::Cable(c), Slot::CableDescription, Parsed::Text(t)) => c.description = t.clone(),
        (Record::Cable(c), Slot::CableLength, Parsed::Number(n)) => c.length_m = *n,
        (record, Slot::Property(key), Parsed::Property(v)) => {
            if let Some(props) = record.properties_mut() {
                match v {
                    Some(v) => props.insert(key.clone(), v.clone()),
                    None => props.remove(key),
                };
            }
        }
        _ => {}
    }
}
