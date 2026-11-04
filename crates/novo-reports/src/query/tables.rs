//! The tables a query reads: a flat, read-only view of the project.
//!
//! Every table is rebuilt from the project when a query runs (a project is
//! small; nothing is kept that could go stale). Wires and devices come
//! from the same code as the wire list and the device list, so a query and
//! the report of the same name always agree on the numbers.
//!
//! Terminals are identified by their strip and name (`-X1:3`), the way the
//! drawing and the wire list name them.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{AnyId, Project, Properties};

use super::value::QValue;
use crate::devices::DeviceList;
use crate::order::tag_cmp;
use crate::report::Report;
use crate::wires::WireList;

/// One column of a table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableColumn {
    /// The name a query uses (`order_number`). Lowercase, no spaces.
    pub name: String,
    /// The header a report shows for it.
    pub header: String,
}

/// A table of the project, with its rows.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub name: &'static str,
    /// One line: what a row is.
    pub about: &'static str,
    pub columns: Vec<TableColumn>,
    pub rows: Vec<Vec<QValue>>,
    /// The model objects of each row (for jumping from a result to the drawing).
    pub sources: Vec<Vec<AnyId>>,
}

/// The tables and what a row of each one is.
pub const TABLES: &[(&str, &str)] = &[
    ("pages", "One row per drawing page, in reading order"),
    ("devices", "One row per device (the device list)"),
    ("terminals", "One row per terminal of a terminal strip"),
    ("wires", "One row per connection to cut (the wire list)"),
    ("cables", "One row per cable"),
    ("cores", "One row per cable core"),
    ("parts", "One row per part in the project"),
];

impl Table {
    fn new(name: &'static str, columns: &[(&str, &str)]) -> Table {
        Table {
            name,
            about: TABLES
                .iter()
                .find(|(n, _)| *n == name)
                .map_or("", |(_, a)| a),
            columns: columns
                .iter()
                .map(|(n, h)| TableColumn {
                    name: (*n).to_owned(),
                    header: (*h).to_owned(),
                })
                .collect(),
            rows: Vec::new(),
            sources: Vec::new(),
        }
    }

    fn push(&mut self, row: Vec<QValue>, sources: Vec<AnyId>) {
        debug_assert_eq!(row.len(), self.columns.len());
        self.rows.push(row);
        self.sources.push(sources);
    }

    /// The index of a column by name (case does not matter).
    pub fn column(&self, name: &str) -> Option<usize> {
        self.columns
            .iter()
            .position(|c| c.name.eq_ignore_ascii_case(name))
    }

    /// Adds one `prop_<name>` column per property key found on the rows'
    /// objects, so a value the person typed into a property sheet can be
    /// listed, filtered and sorted like any other column.
    fn add_properties<'a>(&mut self, lookup: impl Fn(AnyId) -> Option<&'a Properties>) {
        let mut keys: BTreeMap<String, String> = BTreeMap::new();
        for sources in &self.sources {
            if let Some(props) = sources.first().and_then(|s| lookup(*s)) {
                for key in props.keys() {
                    keys.entry(property_column(key))
                        .or_insert_with(|| key.clone());
                }
            }
        }
        for (column, key) in keys {
            if self.column(&column).is_some() {
                continue;
            }
            self.columns.push(TableColumn {
                name: column,
                header: key.clone(),
            });
            for (row, sources) in self.rows.iter_mut().zip(&self.sources) {
                let value = sources
                    .first()
                    .and_then(|s| lookup(*s))
                    .and_then(|p| p.get(&key))
                    .map_or(QValue::Null, QValue::from);
                row.push(value);
            }
        }
    }
}

/// The column name of a property: `prop_` and the key in lowercase with
/// everything but letters and digits turned into `_`.
pub(crate) fn property_column(key: &str) -> String {
    let body: String = key
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '_'
            }
        })
        .collect();
    format!("prop_{body}")
}

/// Builds one table by name, or `None` for a name that is not a table.
pub fn build(project: &Project, name: &str) -> Option<Table> {
    Some(match name {
        "pages" => pages(project),
        "devices" => devices(project),
        "terminals" => terminals(project),
        "wires" => wires(project),
        "cables" => cables(project),
        "cores" => cores(project),
        "parts" => parts(project),
        _ => return None,
    })
}

fn pages(project: &Project) -> Table {
    let mut table = Table::new(
        "pages",
        &[
            ("no", "No."),
            ("name", "Page"),
            ("title", "Title"),
            ("kind", "Kind"),
            ("page_group", "Page group"),
            ("symbols", "Symbols"),
            ("wires", "Wires"),
            ("locked", "Locked"),
        ],
    );
    for (i, page) in project.pages_in_order().into_iter().enumerate() {
        let contents = project.page_contents(page.id);
        let kind = match &page.kind {
            novo_core::PageKind::Schematic => "schematic".to_owned(),
            novo_core::PageKind::Overview => "overview".to_owned(),
            novo_core::PageKind::Generated { generator } => format!("generated {generator}"),
        };
        table.push(
            vec![
                QValue::from(i + 1),
                QValue::text(page.name.clone()),
                QValue::text(page.title.clone()),
                QValue::Text(kind),
                QValue::text(page.structure.to_string()),
                QValue::from(contents.symbols.len()),
                QValue::from(contents.wires.len()),
                QValue::Bool(page.locked.is_some()),
            ],
            vec![page.id.into()],
        );
    }
    table.add_properties(|id| match id {
        AnyId::Page(p) => project.page(p).map(|p| &p.properties),
        _ => None,
    });
    table
}

fn devices(project: &Project) -> Table {
    let output = DeviceList.generate(project);
    let mut table = from_report(
        "devices",
        &output.sections[0],
        &[
            ("tag", "Tag"),
            ("function", "Function"),
            ("location", "Location"),
            ("description", "Description"),
            ("kind", "Kind"),
            ("order_number", "Part"),
            ("manufacturer", "Manufacturer"),
            ("drawn_at", "Drawn at"),
        ],
    );
    table.columns.push(TableColumn {
        name: "parts".into(),
        header: "Parts".into(),
    });
    for (row, sources) in table.rows.iter_mut().zip(&table.sources) {
        let count = match sources.first() {
            Some(AnyId::Device(d)) => project.device(*d).map_or(0, |d| d.parts.len()),
            _ => 0,
        };
        row.push(QValue::from(count));
    }
    table.add_properties(|id| match id {
        AnyId::Device(d) => project.device(d).map(|d| &d.properties),
        _ => None,
    });
    table
}

fn wires(project: &Project) -> Table {
    let output = WireList::build(project, true);
    let mut table = from_report(
        "wires",
        &output.sections[0],
        &[
            ("number", "Wire no."),
            ("from", "From"),
            ("to", "To"),
            ("potential", "Potential"),
            ("color", "Colour"),
            ("cross_section_mm2", "mm²"),
            ("cable", "Cable"),
            ("location", "Location"),
            ("length_mm", "Route length mm"),
            ("length_basis", "Length basis"),
            ("torque", "Torque"),
        ],
    );
    table.columns.push(TableColumn {
        name: "page".into(),
        header: "Page".into(),
    });
    for (row, sources) in table.rows.iter_mut().zip(&table.sources) {
        let page = sources
            .first()
            .and_then(|s| match s {
                AnyId::Wire(w) => project.wire(*w),
                _ => None,
            })
            .and_then(|w| project.page(w.page))
            .map_or(QValue::Null, |p| QValue::text(p.name.clone()));
        row.push(page);
    }
    table.add_properties(|id| match id {
        AnyId::Wire(w) => project.wire(w).map(|w| &w.properties),
        _ => None,
    });
    table
}

/// A table that has the columns and rows of a report's section.
fn from_report(
    name: &'static str,
    section: &crate::table::Section,
    columns: &[(&str, &str)],
) -> Table {
    let mut table = Table::new(name, columns);
    for row in &section.rows {
        // The report's columns are in the order given above.
        let values: Vec<QValue> = row.cells.iter().cloned().map(QValue::from).collect();
        let mut values = values;
        values.resize(table.columns.len(), QValue::Null);
        table.push(values, row.sources.clone());
    }
    table
}

fn terminals(project: &Project) -> Table {
    let mut table = Table::new(
        "terminals",
        &[
            ("strip", "Strip"),
            ("name", "Terminal"),
            ("designation", "Designation"),
            ("kind", "Kind"),
            ("level", "Level"),
            ("jumper", "Jumper"),
            ("order_number", "Part"),
        ],
    );
    let mut all: Vec<_> = project.terminals().collect();
    all.sort_by(|a, b| {
        let tag = |t: &novo_core::Terminal| project.device(t.strip).map(|d| d.tag.clone());
        match (tag(a), tag(b)) {
            (Some(x), Some(y)) => tag_cmp(&x, &y),
            _ => std::cmp::Ordering::Equal,
        }
        .then(a.order.cmp(&b.order))
        .then(a.id.cmp(&b.id))
    });
    for t in all {
        let strip = project.device(t.strip).map(|d| d.tag.to_string());
        let kind = format!("{:?}", t.kind).to_lowercase();
        table.push(
            vec![
                strip.map_or(QValue::Null, QValue::text),
                QValue::text(t.name.clone()),
                project
                    .terminal_designation(t.id)
                    .map_or(QValue::Null, QValue::text),
                QValue::Text(kind),
                QValue::Int(i64::from(t.level)),
                t.jumper.map_or(QValue::Null, |j| QValue::Int(i64::from(j))),
                t.part
                    .and_then(|p| project.part(p))
                    .map_or(QValue::Null, |p| QValue::text(p.order_number.clone())),
            ],
            vec![t.id.into()],
        );
    }
    table.add_properties(|id| match id {
        AnyId::Terminal(t) => project.terminal(t).map(|t| &t.properties),
        _ => None,
    });
    table
}

fn cables(project: &Project) -> Table {
    let mut table = Table::new(
        "cables",
        &[
            ("tag", "Cable"),
            ("description", "Description"),
            ("order_number", "Part"),
            ("length_m", "Length m"),
            ("cores", "Cores"),
            ("shields", "Shields"),
            ("unused_cores", "Unused cores"),
        ],
    );
    let mut all: Vec<_> = project.cables().collect();
    all.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
    for c in all {
        let shields = c.cores.iter().filter(|k| k.shield).count();
        let unused = c
            .cores
            .iter()
            .filter(|k| !k.shield && k.wire.is_none())
            .count();
        table.push(
            vec![
                QValue::text(c.tag.to_string()),
                QValue::text(c.description.clone()),
                c.part
                    .and_then(|p| project.part(p))
                    .map_or(QValue::Null, |p| QValue::text(p.order_number.clone())),
                c.length_m.map_or(QValue::Null, QValue::Num),
                QValue::from(c.cores.len() - shields),
                QValue::from(shields),
                QValue::from(unused),
            ],
            vec![c.id.into()],
        );
    }
    table.add_properties(|id| match id {
        AnyId::Cable(c) => project.cable(c).map(|c| &c.properties),
        _ => None,
    });
    table
}

fn cores(project: &Project) -> Table {
    let mut table = Table::new(
        "cores",
        &[
            ("cable", "Cable"),
            ("core", "Core"),
            ("cross_section_mm2", "mm²"),
            ("shield", "Shield"),
            ("wire_number", "Wire no."),
            ("connected", "Connected"),
        ],
    );
    let mut all: Vec<_> = project.cables().collect();
    all.sort_by(|a, b| tag_cmp(&a.tag, &b.tag).then(a.id.cmp(&b.id)));
    for c in all {
        for core in &c.cores {
            let wire = core.wire.and_then(|w| project.wire(w));
            table.push(
                vec![
                    QValue::text(c.tag.to_string()),
                    QValue::text(core.name.clone()),
                    core.cross_section_mm2.map_or(QValue::Null, QValue::Num),
                    QValue::Bool(core.shield),
                    wire.and_then(|w| w.number.clone())
                        .map_or(QValue::Null, QValue::text),
                    QValue::Bool(wire.is_some()),
                ],
                vec![c.id.into()],
            );
        }
    }
    table
}

fn parts(project: &Project) -> Table {
    let mut table = Table::new(
        "parts",
        &[
            ("manufacturer", "Manufacturer"),
            ("order_number", "Order number"),
            ("type_number", "Type"),
            ("description", "Description"),
            ("devices", "Devices"),
        ],
    );
    let mut used: HashMap<novo_core::PartId, BTreeSet<novo_core::DeviceId>> = HashMap::new();
    for d in project.devices() {
        for p in &d.parts {
            used.entry(*p).or_default().insert(d.id);
        }
    }
    let mut all: Vec<_> = project.parts().collect();
    all.sort_by(|a, b| {
        a.manufacturer
            .to_lowercase()
            .cmp(&b.manufacturer.to_lowercase())
            .then_with(|| novo_core::natural_cmp(&a.order_number, &b.order_number))
            .then(a.id.cmp(&b.id))
    });
    for p in all {
        table.push(
            vec![
                QValue::text(p.manufacturer.clone()),
                QValue::text(p.order_number.clone()),
                QValue::text(p.type_number.clone()),
                QValue::text(p.description.clone()),
                QValue::from(used.get(&p.id).map_or(0, BTreeSet::len)),
            ],
            vec![p.id.into()],
        );
    }
    table.add_properties(|id| match id {
        AnyId::Part(p) => project.part(p).map(|p| &p.properties),
        _ => None,
    });
    table
}

/// The tables and columns of a project, for a column picker (property
/// columns included).
pub fn schema(project: &Project) -> Vec<Table> {
    TABLES
        .iter()
        .filter_map(|(name, _)| build(project, name))
        .map(|mut t| {
            t.rows.clear();
            t.sources.clear();
            t
        })
        .collect()
}
