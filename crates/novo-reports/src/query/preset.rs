//! Saved queries ("presets") that travel with the project, and the report
//! that prints them.

use novo_core::{AddonBlock, EntityKind, Project, Result, Tx};
use serde::{Deserialize, Serialize};

use super::eval::{QueryResult, run};
use crate::report::{Dependencies, Report};
use crate::table::{Align, Cell, Column, ReportOutput, Row, Section};

/// The add-on id and block the presets are kept under in the project file.
/// The project does not look inside; older programs keep it untouched.
pub const STORE_ADDON: &str = "reports";
pub const STORE_BLOCK: &str = "presets";
const FORMAT: u32 = 1;
/// The longest name a preset may have.
pub const MAX_NAME: usize = 80;

/// One saved query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub sql: String,
}

/// The presets of a project, in the order they print.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Presets {
    pub list: Vec<Preset>,
    /// Set when the stored text could not be read (a newer program wrote it,
    /// or it was edited by hand). The text is kept as it is until a preset is
    /// saved.
    pub unreadable: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    presets: Vec<Preset>,
}

/// Starting points for a new preset: what it answers and the query.
pub const EXAMPLES: &[(&str, &str)] = &[
    (
        "Wires without a size",
        "SELECT number, \"from\", \"to\", color\nFROM wires\nWHERE cross_section_mm2 IS NULL\nORDER BY number",
    ),
    (
        "Devices without a part",
        "SELECT tag, description, kind\nFROM devices\nWHERE order_number IS NULL\nORDER BY tag",
    ),
    (
        "Terminals per strip",
        "SELECT strip, count(*) AS terminals, group_concat(name) AS names\nFROM terminals\nGROUP BY strip\nORDER BY strip",
    ),
    (
        "Parts to order",
        "SELECT manufacturer, order_number, description, sum(devices) AS devices\nFROM parts\nGROUP BY manufacturer, order_number, description\nORDER BY manufacturer, order_number",
    ),
    (
        "Cables with free cores",
        "SELECT tag, order_number, cores, unused_cores\nFROM cables\nWHERE unused_cores > 0\nORDER BY tag",
    ),
];

impl Presets {
    /// The presets stored in the project.
    pub fn of(project: &Project) -> Presets {
        let Some(block) = project.info().addon_data.get(STORE_ADDON, STORE_BLOCK) else {
            return Presets::default();
        };
        if block.format > FORMAT {
            return Presets {
                list: Vec::new(),
                unreadable: Some(
                    "These presets were saved by a newer KoperCAD; update to see them.".into(),
                ),
            };
        }
        match serde_json::from_str::<Stored>(&block.text) {
            Ok(stored) => Presets {
                list: stored.presets,
                unreadable: None,
            },
            Err(e) => Presets {
                list: Vec::new(),
                unreadable: Some(format!("The saved presets cannot be read: {e}")),
            },
        }
    }

    /// Writes the presets into the project (one undo step with the rest of
    /// the transaction). An empty list removes the stored block.
    pub fn save(&self, tx: &mut Tx<'_>) -> Result<()> {
        let text = if self.list.is_empty() {
            String::new()
        } else {
            serde_json::to_string_pretty(&Stored {
                presets: self.list.clone(),
            })
            .unwrap_or_default()
        };
        tx.set_addon_block(
            STORE_ADDON,
            STORE_BLOCK,
            AddonBlock {
                format: FORMAT,
                text,
            },
        )
    }

    pub fn get(&self, name: &str) -> Option<&Preset> {
        self.list
            .iter()
            .find(|p| p.name.trim().eq_ignore_ascii_case(name.trim()))
    }

    /// Adds a preset, or replaces the one with the same name (case aside).
    /// Fails with a sentence when the name is empty or too long.
    pub fn upsert(&mut self, name: &str, sql: &str) -> std::result::Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Give the report a name".into());
        }
        if name.chars().count() > MAX_NAME {
            return Err(format!("Keep the name under {MAX_NAME} characters"));
        }
        if sql.chars().count() > super::parse::MAX_QUERY_CHARS {
            return Err(format!(
                "Keep the query under {} characters",
                super::parse::MAX_QUERY_CHARS
            ));
        }
        let preset = Preset {
            name: name.to_owned(),
            sql: sql.trim().to_owned(),
        };
        match self
            .list
            .iter()
            .position(|p| p.name.trim().eq_ignore_ascii_case(name))
        {
            Some(i) => self.list[i] = preset,
            None => self.list.push(preset),
        }
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> bool {
        let before = self.list.len();
        self.list
            .retain(|p| !p.name.trim().eq_ignore_ascii_case(name.trim()));
        self.list.len() != before
    }
}

/// A query answer as a report table, with right-aligned number columns.
pub fn to_section(key: &str, title: &str, result: &QueryResult) -> Section {
    let columns = result
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let numeric = !result.rows.is_empty()
                && result
                    .rows
                    .iter()
                    .map(|r| &r[i])
                    .filter(|v| !v.is_null())
                    .all(|v| matches!(v, super::QValue::Int(_) | super::QValue::Num(_)));
            let mut col = Column::new(&c.name, &c.header, 20.0);
            if numeric {
                col.align = Align::Right;
            }
            col
        })
        .collect();
    let mut section = Section::new(key, title, columns);
    for (row, sources) in result.rows.iter().zip(&result.sources) {
        section.rows.push(Row::new(
            row.iter().map(super::QValue::to_cell).collect(),
            sources.clone(),
        ));
    }
    section
}

/// The "My reports" report: one table per saved preset, in order. A preset
/// that no longer runs (a column was renamed, a table is gone) prints its
/// problem instead of silently disappearing.
#[derive(Debug, Clone, Copy, Default)]
pub struct SavedQueries;

impl SavedQueries {
    pub const ID: &'static str = "queries";
}

impl Report for SavedQueries {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "My reports".into()
    }

    fn dependencies(&self) -> Dependencies {
        // A query can read any table, and the presets live in the project info.
        Dependencies::everything()
            .with_kinds(&[EntityKind::Cable, EntityKind::Terminal])
            .with_info()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let presets = Presets::of(project);
        let mut sections = Vec::new();
        if let Some(problem) = &presets.unreadable {
            let mut s = Section::new(
                "problem",
                "My reports",
                vec![Column::new("problem", "Problem", 80.0)],
            );
            s.push(vec![Cell::text(problem.clone())], Vec::new());
            sections.push(s);
        }
        for (i, preset) in presets.list.iter().enumerate() {
            let key = format!("preset-{}", i + 1);
            match run(project, &preset.sql) {
                Ok(result) => sections.push(to_section(&key, &preset.name, &result)),
                Err(e) => {
                    let mut s = Section::new(
                        &key,
                        &format!("{} (does not run)", preset.name),
                        vec![Column::new("problem", "Problem", 80.0)],
                    );
                    s.push(vec![Cell::text(e.describe(&preset.sql))], Vec::new());
                    sections.push(s);
                }
            }
        }
        if sections.is_empty() {
            sections.push(Section::new(
                "presets",
                "",
                vec![Column::new("name", "Report", 40.0)],
            ));
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}
