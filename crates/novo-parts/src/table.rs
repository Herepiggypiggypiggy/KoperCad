//! Spreadsheet-style bulk edit of devices.
//!
//! A [`BulkTable`] lists devices as rows and their description, part and
//! properties as columns. It copies to the clipboard as tab-separated text
//! (Excel pastes it as a table), and edited text comes back through
//! [`apply_table`] as one command: changing 200 rows in Excel and pasting
//! them back is one undo step. The grid in the app edits single cells
//! through [`set_cell`], the same path.
//!
//! Copying the table and pasting it back unchanged changes nothing: property
//! columns are written as `properties.<key>`, so a property called `text`
//! or `article` never reads back as the Description or Order number column,
//! and cells holding tabs, line breaks or quotes are quoted.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Aspect, Command, Device, DeviceId, DeviceTag, Project, Record};
use thiserror::Error;

use crate::assign::{PartAllocator, count};
use crate::catalog::{Catalog, CatalogPart, PartKey};
use crate::csv::{cell_value, parse_delimited};
use crate::search::normalize_code;
use crate::sheet::{display_value, parse_value, property_key, property_label};

/// A column of the bulk table.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TableColumn {
    Tag,
    Description,
    Manufacturer,
    OrderNumber,
    TypeNumber,
    Property(String),
}

/// The header prefix of property columns on the clipboard.
pub const PROPERTY_PREFIX: &str = "properties.";

impl TableColumn {
    /// The header as written to the clipboard: built-in columns by name,
    /// properties as `properties.<key>` so they cannot collide with them.
    pub fn header(&self) -> String {
        match self {
            TableColumn::Tag => "Tag".into(),
            TableColumn::Description => "Description".into(),
            TableColumn::Manufacturer => "Manufacturer".into(),
            TableColumn::OrderNumber => "Order number".into(),
            TableColumn::TypeNumber => "Type number".into(),
            TableColumn::Property(key) => format!("{PROPERTY_PREFIX}{key}"),
        }
    }

    /// The header as shown in the grid (`Rated current [A]`).
    pub fn label(&self) -> String {
        match self {
            TableColumn::Property(key) => match property_label(key) {
                (label, Some(unit)) => format!("{label} [{unit}]"),
                (label, None) => label,
            },
            other => other.header(),
        }
    }

    /// Reads a header back. `properties.<key>` is always that property,
    /// with the key taken as written. Other headers are matched against the
    /// built-in columns (in English, German and Dutch) first; what is left
    /// becomes a property, from its key or its label (`Rated current [A]`).
    pub fn from_header(header: &str) -> Option<TableColumn> {
        let trimmed = header.trim();
        if let Some(prefix) = trimmed.get(..PROPERTY_PREFIX.len())
            && prefix.eq_ignore_ascii_case(PROPERTY_PREFIX)
        {
            let key = trimmed[PROPERTY_PREFIX.len()..].trim();
            return (!key.is_empty()).then(|| TableColumn::Property(key.to_owned()));
        }
        let squashed = normalize_code(header);
        let column = match squashed.as_str() {
            "" => return None,
            "tag"
            | "device"
            | "devicetag"
            | "bmk"
            | "betriebsmittelkennzeichen"
            | "kenmerk"
            | "referencedesignation" => TableColumn::Tag,
            _ => match crate::csv::column_for(header) {
                crate::csv::Column::Manufacturer => TableColumn::Manufacturer,
                crate::csv::Column::OrderNumber => TableColumn::OrderNumber,
                crate::csv::Column::TypeNumber => TableColumn::TypeNumber,
                crate::csv::Column::Description => TableColumn::Description,
                _ => {
                    // `Rated current [A]` → `rated_current_a`.
                    let key = property_key(header)?;
                    TableColumn::Property(key)
                }
            },
        };
        Some(column)
    }
}

/// One device row.
#[derive(Debug, Clone, PartialEq)]
pub struct BulkRow {
    pub device: DeviceId,
    pub cells: Vec<String>,
}

/// Devices as a table; see the module docs.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BulkTable {
    pub columns: Vec<TableColumn>,
    pub rows: Vec<BulkRow>,
}

impl BulkTable {
    /// Every device (or only `only`, when given), sorted by tag, with tag,
    /// description, part columns and one column per property in use.
    pub fn devices(project: &Project, only: Option<&[DeviceId]>) -> Self {
        let mut devices: Vec<&Device> = match only {
            Some(ids) => ids.iter().filter_map(|&id| project.device(id)).collect(),
            None => project.devices().collect(),
        };
        devices.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
        let keys: BTreeSet<&str> = devices
            .iter()
            .flat_map(|d| d.properties.keys().map(String::as_str))
            .collect();
        let mut columns = vec![
            TableColumn::Tag,
            TableColumn::Description,
            TableColumn::Manufacturer,
            TableColumn::OrderNumber,
            TableColumn::TypeNumber,
        ];
        columns.extend(keys.into_iter().map(|k| TableColumn::Property(k.into())));
        let rows = devices
            .iter()
            .map(|d| BulkRow {
                device: d.id,
                cells: columns.iter().map(|c| cell(project, d, c)).collect(),
            })
            .collect();
        BulkTable { columns, rows }
    }

    /// Tab-separated text with a header row, for the clipboard. A cell
    /// holding a tab, a line break or a quote is quoted RFC 4180 style
    /// (as Excel itself copies such cells), so it pastes back unchanged.
    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        let header: Vec<String> = self.columns.iter().map(|c| quote(&c.header())).collect();
        out.push_str(&header.join("\t"));
        out.push('\n');
        for row in &self.rows {
            let cells: Vec<String> = row.cells.iter().map(|c| quote(c)).collect();
            out.push_str(&cells.join("\t"));
            out.push('\n');
        }
        out
    }
}

/// Quotes a TSV field when it needs it, doubling inner quotes.
fn quote(field: &str) -> String {
    if field.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

fn cell(project: &Project, device: &Device, column: &TableColumn) -> String {
    let part = device.parts.first().and_then(|&p| project.part(p));
    match column {
        TableColumn::Tag => device.tag.to_string(),
        TableColumn::Description => device.description.clone(),
        TableColumn::Manufacturer => part.map(|p| p.manufacturer.clone()).unwrap_or_default(),
        TableColumn::OrderNumber => part.map(|p| p.order_number.clone()).unwrap_or_default(),
        TableColumn::TypeNumber => part.map(|p| p.type_number.clone()).unwrap_or_default(),
        TableColumn::Property(key) => device
            .properties
            .get(key)
            .map(display_value)
            .unwrap_or_default(),
    }
}

/// Why pasted text could not be applied at all.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BulkError {
    #[error("nothing to apply")]
    Empty,
    #[error("the first row needs a Tag column to know which device each row is")]
    NoTagColumn,
    #[error("the tag column cannot be changed here; rename a device with F2")]
    TagColumn,
    #[error("no device {0}")]
    UnknownDevice(String),
    #[error(
        "columns {first} and {second} both set {field}; rename one (properties as properties.<name>)"
    )]
    DuplicateColumn {
        first: String,
        second: String,
        field: String,
    },
}

/// The result of applying a table.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BulkOutcome {
    /// One command for all changes, or `None` if nothing changed.
    pub command: Option<Command>,
    /// Devices that change.
    pub changed: usize,
    /// Rows that were skipped (1-based line in the pasted text) and why.
    pub skipped: Vec<(usize, String)>,
}

/// Finds the device a table row means: the exact tag, or, when the row
/// gives only the product aspect (`-K1`, no `=` or `+` levels), the one
/// device with that product designation. A tag that names a function or
/// location matches only exactly, so `=ZZ9-K1` never edits `=A1-K1`.
pub fn find_device(project: &Project, text: &str) -> Option<DeviceId> {
    let tag = DeviceTag::parse(text.trim()).ok()?;
    if let [id] = project.devices_by_tag(&tag)[..] {
        return Some(id);
    }
    if !tag.levels(Aspect::Function).is_empty() || !tag.levels(Aspect::Location).is_empty() {
        return None;
    }
    let product = tag.levels(Aspect::Product);
    let mut matches = project
        .devices()
        .filter(|d| d.tag.levels(Aspect::Product) == product);
    match (matches.next(), matches.next()) {
        (Some(d), None) => Some(d.id),
        _ => None,
    }
}

/// Applies tab-separated (or `;`/`,`-separated) text with a header row to
/// the devices it names, as one command. Columns: see [`TableColumn`]; the
/// Tag column is required and only identifies the row. An empty cell
/// clears the value (an empty order number removes the part). Part columns
/// resolve against the project's parts, then the catalog; an order number
/// found in neither becomes a new project part.
pub fn apply_table(
    project: &Project,
    catalog: &Catalog,
    text: &str,
) -> Result<BulkOutcome, BulkError> {
    let rows = parse_delimited(text);
    let Some((header, body)) = rows.split_first() else {
        return Err(BulkError::Empty);
    };
    let columns: Vec<Option<TableColumn>> =
        header.iter().map(|h| TableColumn::from_header(h)).collect();
    // Two columns writing one field would make the later one win silently.
    for (i, column) in columns.iter().enumerate() {
        let Some(column) = column else { continue };
        if let Some(j) = columns[..i].iter().position(|c| c.as_ref() == Some(column)) {
            return Err(BulkError::DuplicateColumn {
                first: header[j].trim().to_owned(),
                second: header[i].trim().to_owned(),
                field: column.label(),
            });
        }
    }
    let tag_index = columns
        .iter()
        .position(|c| c == &Some(TableColumn::Tag))
        .ok_or(BulkError::NoTagColumn)?;

    let mut edits = Edits::new(project, catalog);
    let mut skipped = Vec::new();
    for (index, row) in body.iter().enumerate() {
        let line = index + 2;
        let tag = row.get(tag_index).map_or("", |s| s.trim());
        if tag.is_empty() {
            skipped.push((line, "no tag".into()));
            continue;
        }
        let Some(device) = find_device(project, tag) else {
            skipped.push((line, format!("no device {tag}")));
            continue;
        };
        let cells: BTreeMap<&TableColumn, &str> = columns
            .iter()
            .zip(row)
            .filter_map(|(c, v)| c.as_ref().map(|c| (c, v.trim())))
            .collect();
        edits.apply_row(device, &cells);
    }
    let mut outcome = edits.finish();
    outcome.skipped = skipped;
    Ok(outcome)
}

/// Sets one cell of the table, as one command (the grid's edit path).
pub fn set_cell(
    project: &Project,
    catalog: &Catalog,
    device: DeviceId,
    column: &TableColumn,
    value: &str,
) -> Result<Option<Command>, BulkError> {
    if column == &TableColumn::Tag {
        return Err(BulkError::TagColumn);
    }
    if project.device(device).is_none() {
        return Err(BulkError::UnknownDevice(device.to_string()));
    }
    let mut edits = Edits::new(project, catalog);
    let mut cells = BTreeMap::new();
    cells.insert(column, value.trim());
    edits.apply_row(device, &cells);
    Ok(edits.finish().command)
}

/// Sets one column to the same value on many devices (fill down, or a
/// paste of one value over a selected range), as one command.
pub fn fill(
    project: &Project,
    catalog: &Catalog,
    devices: &[DeviceId],
    column: &TableColumn,
    value: &str,
) -> Result<Option<Command>, BulkError> {
    if column == &TableColumn::Tag {
        return Err(BulkError::TagColumn);
    }
    let mut edits = Edits::new(project, catalog);
    let mut cells = BTreeMap::new();
    cells.insert(column, value.trim());
    for &device in devices {
        edits.apply_row(device, &cells);
    }
    Ok(edits.finish().command)
}

struct Edits<'a> {
    project: &'a Project,
    catalog: &'a Catalog,
    parts: PartAllocator<'a>,
    inserts: Vec<Command>,
    devices: BTreeMap<DeviceId, Device>,
}

impl<'a> Edits<'a> {
    fn new(project: &'a Project, catalog: &'a Catalog) -> Self {
        Edits {
            project,
            catalog,
            parts: PartAllocator::new(project),
            inserts: Vec::new(),
            devices: BTreeMap::new(),
        }
    }

    fn apply_row(&mut self, id: DeviceId, cells: &BTreeMap<&TableColumn, &str>) {
        let Some(original) = self.project.device(id) else {
            return;
        };
        let mut device = self.devices.remove(&id).unwrap_or_else(|| original.clone());
        for (column, &value) in cells {
            // A cell that still shows the current value leaves it alone, so
            // an unchanged paste changes nothing even where the text does
            // not parse back to the same value (spaces, `4.0` vs `4`).
            if **column != TableColumn::OrderNumber
                && value == cell(self.project, original, column).trim()
            {
                continue;
            }
            match column {
                TableColumn::Tag | TableColumn::Manufacturer | TableColumn::TypeNumber => {}
                TableColumn::OrderNumber => {}
                TableColumn::Description => device.description = value.into(),
                TableColumn::Property(key) => {
                    if value.is_empty() {
                        device.properties.remove(key);
                    } else {
                        let old = device.properties.get(key);
                        let new = match old {
                            Some(old) => parse_value(value, Some(old))
                                .unwrap_or_else(|| novo_core::Value::Text(value.into())),
                            None => cell_value(value),
                        };
                        device.properties.insert(key.clone(), new);
                    }
                }
            }
        }
        if let Some(&order) = cells.get(&TableColumn::OrderNumber) {
            let manufacturer = cells.get(&TableColumn::Manufacturer).copied();
            let type_number = cells.get(&TableColumn::TypeNumber).copied();
            self.set_part(&mut device, manufacturer, order, type_number);
        }
        self.devices.insert(id, device);
    }

    fn set_part(
        &mut self,
        device: &mut Device,
        manufacturer: Option<&str>,
        order: &str,
        type_number: Option<&str>,
    ) {
        if order.is_empty() {
            if !device.parts.is_empty() {
                device.parts.remove(0);
            }
            return;
        }
        let current = device.parts.first().and_then(|&p| self.project.part(p));
        let manufacturer = manufacturer.filter(|m| !m.is_empty());
        // Unchanged part: nothing to do.
        if let Some(current) = current
            && normalize_code(&current.order_number) == normalize_code(order)
            && manufacturer
                .is_none_or(|m| normalize_code(m) == normalize_code(&current.manufacturer))
        {
            return;
        }
        let part: CatalogPart = match manufacturer {
            Some(m) => self
                .project
                .parts()
                .find(|p| PartKey::of_model(p) == PartKey::new(m, order))
                .map(CatalogPart::from_model)
                .or_else(|| self.catalog.find(m, order).cloned())
                .unwrap_or_else(|| {
                    let mut p = CatalogPart::new(m, order);
                    p.type_number = type_number.unwrap_or_default().into();
                    p
                }),
            None => self
                .project
                .parts()
                .find(|p| normalize_code(&p.order_number) == normalize_code(order))
                .map(CatalogPart::from_model)
                .or_else(|| self.catalog.find_order_number(order).cloned())
                .unwrap_or_else(|| {
                    let mut p = CatalogPart::new("", order);
                    p.type_number = type_number.unwrap_or_default().into();
                    p
                }),
        };
        let id = self.parts.part_id(&part, &mut self.inserts);
        match device.parts.first_mut() {
            Some(first) => *first = id,
            None => device.parts.push(id),
        }
        device.parts.dedup();
    }

    fn finish(self) -> BulkOutcome {
        let mut commands = Vec::new();
        let mut changed = 0;
        let mut device_commands = Vec::new();
        for (id, device) in self.devices {
            if self.project.device(id) != Some(&device) {
                changed += 1;
                device_commands.push(Command::Replace {
                    record: Record::Device(device),
                });
            }
        }
        if changed == 0 {
            return BulkOutcome::default();
        }
        // Only parts a changed device uses are inserted.
        commands.extend(self.inserts.into_iter().filter(|c| match c {
            Command::Insert {
                record: Record::Part(p),
            } => device_commands.iter().any(|d| match d {
                Command::Replace {
                    record: Record::Device(d),
                } => d.parts.contains(&p.id),
                _ => false,
            }),
            _ => true,
        }));
        commands.extend(device_commands);
        BulkOutcome {
            command: Some(Command::Batch {
                label: format!("Bulk edit {}", count(changed, "device")),
                commands,
            }),
            changed,
            skipped: Vec::new(),
        }
    }
}
