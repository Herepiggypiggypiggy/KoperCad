//! Front matter: the cover pages at the start of a project, generated from
//! what the project already knows. One generated page with five sheets:
//! title, revisions, structure overview, general information and project
//! information. Nothing is typed on the pages; the fields come from the
//! project info (Project > Project settings > Project information), the
//! revision list and the structure.
//!
//! The report is not proposed by default: a project template (File > New
//! from template) or Reports > Add generated page adds it.

use novo_core::{DeviceTag, EntityKind, Project, Properties, Value, natural_cmp};

use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The project properties the cover pages read. All are plain text.
pub mod keys {
    pub const CUSTOMER: &str = "project.customer";
    pub const ORDER: &str = "project.order_number";
    pub const SITE: &str = "project.site";
    pub const DRAWN_BY: &str = "project.drawn_by";
    pub const APPROVED_BY: &str = "project.approved_by";
    pub const ISSUE_DATE: &str = "project.issue_date";
    /// Safe working load, tonnes.
    pub const CRANE_SWL_T: &str = "crane.swl_t";
    /// Span between the runway rails, metres.
    pub const CRANE_SPAN_M: &str = "crane.span_m";
    /// Lifting height, metres.
    pub const CRANE_LIFT_M: &str = "crane.lift_m";
    /// Duty classification, e.g. an ISO 4301-1 class or an FEM group.
    pub const CRANE_DUTY: &str = "crane.duty_class";
    /// Control voltage, e.g. `48 V AC`.
    pub const CONTROL: &str = "crane.control";

    /// The fields of Project settings > Project information: (key, label).
    pub const FIELDS: &[(&str, &str)] = &[
        (CUSTOMER, "Customer"),
        (ORDER, "Order number"),
        (SITE, "Site"),
        (DRAWN_BY, "Drawn by"),
        (APPROVED_BY, "Approved by"),
        (ISSUE_DATE, "Issue date"),
        (CRANE_SWL_T, "Safe working load (t)"),
        (CRANE_SPAN_M, "Span (m)"),
        (CRANE_LIFT_M, "Lifting height (m)"),
        (CRANE_DUTY, "Duty classification"),
        (CONTROL, "Control voltage"),
    ];
}

/// The cover pages as one report with a section per sheet.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrontMatter;

impl FrontMatter {
    pub const ID: &'static str = novo_core::FRONT_GENERATOR;
}

impl Report for FrontMatter {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Front matter".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[Page, Device, Wire, Terminal, Cable, Structure]).with_info()
    }

    /// The counts change when records come or go; editing a wire, terminal
    /// or cable leaves the cover pages as they are.
    fn affected_by(&self, changes: &novo_core::ChangeSet) -> bool {
        use EntityKind::*;
        let counted = |k: EntityKind| matches!(k, Wire | Terminal | Cable);
        changes.info
            || changes.added.iter().chain(&changes.removed).any(|id| {
                matches!(
                    id.kind(),
                    Page | Device | Wire | Terminal | Cable | Structure
                )
            })
            || changes
                .modified
                .iter()
                .any(|id| !counted(id.kind()) && self.dependencies().kinds.contains(&id.kind()))
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections: vec![
                title_sheet(project),
                revision_sheet(project),
                structure_sheet(project),
                general_sheet(project),
                project_sheet(project),
            ],
        }
    }
}

fn prop(props: &Properties, key: &str) -> String {
    match props.get(key) {
        Some(Value::Text(t)) => t.trim().to_owned(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// A field/value table; a missing value prints as a dash so a blank cover
/// field is visible on paper.
fn fields(key: &str, title: &str, rows: Vec<(&str, String)>) -> Section {
    let mut s = Section::new(
        key,
        title,
        vec![
            Column::new("field", "Field", 30.0),
            Column::new("value", "Value", 70.0),
        ],
    );
    for (label, value) in rows {
        let value = if value.is_empty() { "-".into() } else { value };
        s.push(vec![Cell::text(label), Cell::text(value)], Vec::new());
    }
    s
}

fn title_sheet(project: &Project) -> Section {
    let info = project.info();
    let p = &info.properties;
    let revision = project
        .latest_revision()
        .map(|r| r.index.clone())
        .unwrap_or_default();
    fields(
        "title",
        "Title page",
        vec![
            ("Project", info.name.clone()),
            ("Description", info.description.clone()),
            ("Customer", prop(p, keys::CUSTOMER)),
            ("Order number", prop(p, keys::ORDER)),
            ("Site", prop(p, keys::SITE)),
            ("Revision", revision),
            ("Issue date", prop(p, keys::ISSUE_DATE)),
            ("Drawn by", prop(p, keys::DRAWN_BY)),
            ("Approved by", prop(p, keys::APPROVED_BY)),
        ],
    )
}

fn revision_sheet(project: &Project) -> Section {
    let mut s = Section::new(
        "revisions",
        "Revisions",
        vec![
            Column::new("index", "Rev", 10.0),
            Column::new("date", "Date", 20.0),
            Column::new("author", "By", 20.0),
            Column::new("description", "Change", 50.0),
        ],
    );
    let mut revisions: Vec<_> = project.revisions().collect();
    revisions.reverse();
    for r in revisions {
        s.push(
            vec![
                Cell::text(r.index.clone()),
                Cell::text(r.date.clone()),
                Cell::text(r.author.clone()),
                Cell::text(r.description.clone()),
            ],
            Vec::new(),
        );
    }
    if s.rows.is_empty() {
        s.push(
            vec![
                Cell::text("-"),
                Cell::Empty,
                Cell::Empty,
                Cell::text("No revision issued yet"),
            ],
            Vec::new(),
        );
    }
    s
}

/// Whether `inner` is `outer` or sits below it (same function levels, and
/// location levels that start with the outer ones).
fn within(outer: &DeviceTag, inner: &DeviceTag) -> bool {
    inner.function.starts_with(&outer.function) && inner.location.starts_with(&outer.location)
}

fn structure_sheet(project: &Project) -> Section {
    let mut s = Section::new(
        "structure",
        "Structure overview",
        vec![
            Column::new("tag", "Structure", 22.0),
            Column::new("description", "Description", 40.0),
            Column::new("kind", "Kind", 14.0),
            Column::new("pages", "Pages", 10.0).right(),
            Column::new("devices", "Devices", 10.0).right(),
        ],
    );
    let mut nodes: Vec<_> = project.structures().collect();
    nodes.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
    for node in nodes {
        let pages = project
            .pages()
            .filter(|p| within(&node.tag, &p.structure))
            .count();
        let devices = project
            .devices()
            .filter(|d| within(&node.tag, &d.tag))
            .count();
        let kind = if node.enclosure {
            "Enclosure"
        } else if node.tag.location.is_empty() {
            "Plant"
        } else {
            "Location"
        };
        s.push(
            vec![
                Cell::text(node.tag.to_string()),
                Cell::text(node.description.clone()),
                Cell::text(kind),
                Cell::Int(pages as i64),
                Cell::Int(devices as i64),
            ],
            vec![node.id.into()],
        );
    }
    s
}

fn general_sheet(project: &Project) -> Section {
    let info = project.info();
    let p = &info.properties;
    let drawn = project
        .pages()
        .filter(|pg| !matches!(pg.kind, novo_core::PageKind::Generated { .. }))
        .count();
    let standard = if info.rules.standard.is_empty() {
        "IEC 60617".to_owned()
    } else {
        info.rules.standard.to_uppercase()
    };
    let number = |key: &str, unit: &str| {
        let v = prop(p, key);
        if v.is_empty() {
            v
        } else {
            format!("{v} {unit}")
        }
    };
    fields(
        "general",
        "General information",
        vec![
            ("Drawing standard", standard),
            (
                "Supply voltage",
                number(novo_checks::calc_view::inputs::SUPPLY_VOLTAGE_V, "V"),
            ),
            (
                "Supply short-circuit current",
                number(novo_checks::standards::keys::SUPPLY_SHORT_CIRCUIT_KA, "kA"),
            ),
            ("Control voltage", prop(p, keys::CONTROL)),
            ("Drawn pages", drawn.to_string()),
            ("Devices", project.devices().count().to_string()),
            ("Wires", project.wires().count().to_string()),
            ("Terminals", project.terminals().count().to_string()),
            ("Cables", project.cables().count().to_string()),
        ],
    )
}

fn project_sheet(project: &Project) -> Section {
    let info = project.info();
    let p = &info.properties;
    let with = |key: &str, unit: &str| {
        let v = prop(p, key);
        if v.is_empty() || unit.is_empty() {
            v
        } else {
            format!("{v} {unit}")
        }
    };
    fields(
        "project",
        "Project information",
        vec![
            ("Customer", prop(p, keys::CUSTOMER)),
            ("Site", prop(p, keys::SITE)),
            ("Order number", prop(p, keys::ORDER)),
            ("Safe working load", with(keys::CRANE_SWL_T, "t")),
            ("Span", with(keys::CRANE_SPAN_M, "m")),
            ("Lifting height", with(keys::CRANE_LIFT_M, "m")),
            ("Duty classification", prop(p, keys::CRANE_DUTY)),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::Document;

    #[test]
    fn five_sheets_and_dashes_for_blank_fields() {
        let doc = Document::default();
        let out = FrontMatter.generate(doc.project());
        let keys: Vec<_> = out.sections.iter().map(|s| s.key.as_str()).collect();
        assert_eq!(
            keys,
            ["title", "revisions", "structure", "general", "project"]
        );
        let title = out.section("title").unwrap();
        assert_eq!(title.text_rows()[2], ["Customer", "-"]);
    }

    #[test]
    fn fields_follow_the_project() {
        let mut doc = Document::default();
        doc.transact("info", |tx| {
            tx.set_info(|i| {
                i.name = "Crane 4".into();
                i.properties
                    .insert(keys::CUSTOMER.into(), Value::Text("Acme".into()));
            })
        })
        .unwrap();
        let out = FrontMatter.generate(doc.project());
        let title = out.section("title").unwrap().text_rows();
        assert_eq!(title[0], ["Project", "Crane 4"]);
        assert_eq!(title[2], ["Customer", "Acme"]);
    }
}
