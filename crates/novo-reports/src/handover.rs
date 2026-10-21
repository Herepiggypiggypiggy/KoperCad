//! The handover package's own pages: the contents page with the honest
//! "what is exact, what is an estimate" table, and the revision table.
//!
//! The package itself (PDF drawing set, lists, commissioning record, ...) is
//! written by the app from the exports that already exist; this module only
//! decides what the two summary pages say, from the project and from facts
//! the app passes in (which files were written, how many design problems are
//! open, how far commissioning is). It is pure and tested.

use novo_checks::Summary;
use novo_core::Project;

use crate::table::{Cell, Column, ReportOutput, Section};
use crate::{BillOfMaterials, Report, is_estimate, wire_cut_jobs};

/// Report id of the contents page.
pub const CONTENTS_ID: &str = "handover";
/// Report id of the revision table.
pub const REVISIONS_ID: &str = "revisions";

/// Whether a file of the package was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Written,
    /// Not written, with the reason in plain words.
    Left(String),
}

/// One line of the contents page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandoverFile {
    pub name: String,
    pub what: String,
    pub status: FileStatus,
}

/// How far commissioning is, from the commissioning store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CommissioningFacts {
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    /// Punch items that still need work or verification.
    pub open_punch: usize,
    /// Of those, the ones that block hand-over.
    pub blocking_punch: usize,
}

/// What the app knows that the project alone does not tell.
#[derive(Debug, Clone, Default)]
pub struct HandoverInput {
    /// `2026-10-21`.
    pub date: String,
    pub files: Vec<HandoverFile>,
    /// The design checks as they stand; `None` when they were not run.
    pub problems: Option<Summary>,
    pub commissioning: Option<CommissioningFacts>,
    /// Pages that differ from the latest issued revision.
    pub changed_pages: usize,
    /// Short sentences for a Notes table, for example about files an
    /// earlier package left in the folder.
    pub notes: Vec<String>,
}

/// How far a statement in the package can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    /// Counted or read straight from the drawing.
    Exact,
    /// A result the program works out from the model and the figures the
    /// designer stated (routed lengths, checks it runs). Not measured.
    Calculated,
    /// What the project itself states, for example that a revision is issued.
    Stated,
    /// A model or planning figure: check it before relying on it.
    Estimate,
    /// Something is missing, so the figure is not complete.
    Incomplete,
    /// Written down by a person on site; the program does not verify it.
    Recorded,
    /// Not part of this package.
    NotIncluded,
}

impl Truth {
    pub fn text(self) -> &'static str {
        match self {
            Truth::Exact => "Exact",
            Truth::Calculated => "Calculated",
            Truth::Stated => "Stated",
            Truth::Estimate => "Estimate",
            Truth::Incomplete => "Incomplete",
            Truth::Recorded => "Recorded on site",
            Truth::NotIncluded => "Not included",
        }
    }
}

/// One row of the "exact or estimated" table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TruthRow {
    pub item: String,
    pub truth: Truth,
    pub detail: String,
}

fn row(item: &str, truth: Truth, detail: impl Into<String>) -> TruthRow {
    TruthRow {
        item: item.to_owned(),
        truth,
        detail: detail.into(),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What can and cannot be relied on in this project's package, one row per
/// subject. The order is the order of the printed table.
pub fn truth_rows(project: &Project, input: &HandoverInput) -> Vec<TruthRow> {
    let mut rows = Vec::new();

    let pages = project.pages().count();
    let devices = project.devices().count();
    rows.push(row(
        "Drawing, device and terminal lists",
        Truth::Exact,
        format!(
            "Read from the drawing: {}, {}.",
            plural(pages, "page", "pages"),
            plural(devices, "device", "devices")
        ),
    ));

    let missing = BillOfMaterials
        .generate(project)
        .section("without_parts")
        .map_or(0, |s| s.rows.len());
    rows.push(if missing == 0 {
        row(
            "Parts list",
            Truth::Exact,
            "Every device and cable has a part.",
        )
    } else {
        row(
            "Parts list",
            Truth::Incomplete,
            format!(
                "{} no part, so the list leaves them out: see \"without parts\" in the parts list.",
                plural(missing, "device or cable has", "devices and cables have")
            ),
        )
    });

    let jobs = wire_cut_jobs(project);
    if jobs.is_empty() {
        rows.push(row(
            "Wire lengths",
            Truth::NotIncluded,
            "No connected wires yet.",
        ));
    } else {
        let mut bases: std::collections::BTreeMap<&str, usize> = Default::default();
        for j in &jobs {
            *bases.entry(j.basis.as_str()).or_default() += 1;
        }
        let guessed = jobs.iter().filter(|j| is_estimate(&j.basis)).count();
        let present = bases
            .iter()
            .map(|(b, n)| format!("{n} {b}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut notes = String::new();
        if bases.contains_key("routed, no depth") {
            notes.push_str(
                " Wires \"routed, no depth\" are too short by the rise to the terminals.",
            );
        }
        if bases.contains_key("door, no gap") {
            notes.push_str(" Door wires leave out the gap to the door and the hinge loop.");
        }
        rows.push(if guessed == 0 {
            row(
                "Wire lengths",
                Truth::Calculated,
                format!(
                    "{} by basis: {present}. Routed through the model's ducts or taken from a stated \
                     cable length: calculated, not measured on the plate.{notes}",
                    plural(jobs.len(), "wire", "wires")
                ),
            )
        } else {
            row(
                "Wire lengths",
                Truth::Estimate,
                format!(
                    "{guessed} of {} are estimates. By basis: {present}. See the Basis column of the wire \
                     list. Check the estimated ones on the plate.{notes}",
                    plural(jobs.len(), "wire", "wires")
                ),
            )
        });
    }

    rows.push(match input.problems {
        None => row(
            "Design checks",
            Truth::NotIncluded,
            "The checks were not run for this package.",
        ),
        Some(s) if s.is_clean() => row(
            "Design checks",
            Truth::Calculated,
            "No open problems in the checks the program runs. This is not a design review.",
        ),
        Some(s) => row(
            "Design checks",
            Truth::Incomplete,
            format!(
                "{} remain open. The drawing set carries a Design checks page with them.",
                s.pill_text()
            ),
        ),
    });

    let functions = novo_checks::safety::functions(project);
    if !functions.is_empty() {
        let rated = functions
            .iter()
            .filter(|f| novo_checks::safety::rate(project, f).met() == Some(true))
            .count();
        rows.push(row(
            "Safety functions",
            Truth::Estimate,
            format!(
                "{} of {} meet the required level in the calculation aid. PFHd for categories 2 to 4 is a \
                 closed-form model, not the standard's table: not a certification.",
                rated,
                plural(functions.len(), "function", "functions")
            ),
        ));
    }

    if !novo_checks::standards::compliance(project).sets.is_empty() {
        rows.push(row(
            "Standards report",
            Truth::Estimate,
            "A subset of each standard with planning values (for example cable ampacity). Not a certification.",
        ));
    }

    rows.push(row(
        "Heat",
        Truth::NotIncluded,
        "Heat results are a simulation model; they are not part of this package. Run Heat in the 3D view to see them.",
    ));

    rows.push(match input.commissioning {
        None => row(
            "Commissioning record",
            Truth::NotIncluded,
            "No commissioning has started.",
        ),
        Some(c) if c.total == 0 => row(
            "Commissioning record",
            Truth::NotIncluded,
            "There is nothing to check yet.",
        ),
        Some(c) => row(
            "Commissioning record",
            Truth::Recorded,
            format!(
                "{} of {} checks done, {} failed; {} open ({} blocking). \
                 These are people's entries on site; the program does not measure.",
                c.done,
                c.total,
                c.failed,
                plural(c.open_punch, "punch item", "punch items"),
                c.blocking_punch
            ),
        ),
    });

    let locked = project.pages().filter(|p| p.locked.is_some()).count();
    rows.push(match project.latest_revision() {
        None => row(
            "Issued state",
            Truth::Incomplete,
            "No revision has been issued, so no page is locked and nothing is recorded as the state handed over.",
        ),
        Some(r) => {
            let loose = pages - locked.min(pages);
            if loose == 0 && input.changed_pages == 0 {
                row(
                    "Issued state",
                    Truth::Stated,
                    format!(
                        "The drawing set is frozen at revision {}: every page is locked and unchanged since. \
                         Whether the machine was built to it is recorded on site, not known to the program.",
                        r.index
                    ),
                )
            } else {
                row(
                    "Issued state",
                    Truth::Incomplete,
                    format!(
                        "Latest revision {}: {} not locked, {} changed since it was issued.",
                        r.index,
                        plural(loose, "page is", "pages are"),
                        plural(input.changed_pages, "page", "pages")
                    ),
                )
            }
        }
    });

    rows
}

/// The contents page: what is in the package, then what to rely on.
pub fn handover_summary(project: &Project, input: &HandoverInput) -> ReportOutput {
    let name = project.info().name.clone();
    let issue = match project.latest_revision() {
        Some(r) => format!("revision {}", r.index),
        None => "no revision issued".to_owned(),
    };
    let mut files = Section::new(
        "files",
        &format!("Handover package, {name}, {issue}, {}", input.date),
        vec![
            Column::new("file", "File", 30.0),
            Column::new("what", "What it is", 50.0),
            Column::new("state", "State", 20.0),
        ],
    );
    for f in &input.files {
        let state = match &f.status {
            FileStatus::Written => "Included".to_owned(),
            FileStatus::Left(why) => format!("Left out: {why}"),
        };
        files.push(
            vec![Cell::text(&f.name), Cell::text(&f.what), Cell::text(state)],
            Vec::new(),
        );
    }
    let mut truth = Section::new(
        "truth",
        "What to rely on: exact or estimated",
        vec![
            Column::new("item", "Item", 24.0),
            Column::new("truth", "Standing", 14.0),
            Column::new("detail", "Detail", 62.0),
        ],
    );
    for r in truth_rows(project, input) {
        truth.push(
            vec![
                Cell::text(r.item),
                Cell::text(r.truth.text()),
                Cell::text(r.detail),
            ],
            Vec::new(),
        );
    }
    let mut sections = vec![files, truth];
    if !input.notes.is_empty() {
        let mut notes = Section::new("notes", "Notes", vec![Column::new("note", "Note", 100.0)]);
        for n in &input.notes {
            notes.push(vec![Cell::text(n)], Vec::new());
        }
        sections.push(notes);
    }
    ReportOutput {
        report: CONTENTS_ID.to_owned(),
        title: format!("{name}: handover package, {issue}"),
        revision: project.revision(),
        sections,
    }
}

/// The issued revisions, oldest first: index, date, who issued it and what
/// changed.
pub fn revision_table(project: &Project) -> ReportOutput {
    let mut section = Section::new(
        "revisions",
        "Revisions",
        vec![
            Column::new("index", "Index", 8.0),
            Column::new("date", "Date", 14.0),
            Column::new("author", "Issued by", 18.0),
            Column::new("description", "Description", 60.0),
        ],
    );
    for r in project.revisions() {
        let text = if r.note().is_empty() {
            r.description.clone()
        } else {
            format!("{}. {}", r.description, r.note())
        };
        section.push(
            vec![
                Cell::text(&r.index),
                Cell::text(&r.date),
                Cell::text(&r.author),
                Cell::text(text),
            ],
            Vec::new(),
        );
    }
    if section.rows.is_empty() {
        section.push(
            vec![
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                Cell::text("No revision has been issued yet."),
            ],
            Vec::new(),
        );
    }
    ReportOutput {
        report: REVISIONS_ID.to_owned(),
        title: format!("{}: revisions", project.info().name),
        revision: project.revision(),
        sections: vec![section],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::Document;

    #[test]
    fn an_empty_project_says_so_and_does_not_claim_exact_wires() {
        let doc = Document::default();
        let rows = truth_rows(doc.project(), &HandoverInput::default());
        let find = |item: &str| rows.iter().find(|r| r.item == item).unwrap();
        assert_eq!(find("Wire lengths").truth, Truth::NotIncluded);
        assert_eq!(find("Issued state").truth, Truth::Incomplete);
        assert_eq!(find("Heat").truth, Truth::NotIncluded);
        assert_eq!(find("Design checks").truth, Truth::NotIncluded);
        assert_eq!(find("Commissioning record").truth, Truth::NotIncluded);
    }

    #[test]
    fn the_revision_table_lists_each_issue_or_says_none() {
        let doc = Document::default();
        let table = revision_table(doc.project());
        assert!(table.sections[0].text_rows()[0][3].contains("No revision"));
    }

    #[test]
    fn the_contents_page_names_left_out_files_with_the_reason() {
        let doc = Document::default();
        let input = HandoverInput {
            date: "2026-10-21".into(),
            files: vec![
                HandoverFile {
                    name: "1 Drawings.pdf".into(),
                    what: "Drawing set".into(),
                    status: FileStatus::Written,
                },
                HandoverFile {
                    name: "6 Safety.pdf".into(),
                    what: "Safety functions".into(),
                    status: FileStatus::Left("no safety functions".into()),
                },
            ],
            ..HandoverInput::default()
        };
        let out = handover_summary(doc.project(), &input);
        let files = out.section("files").unwrap().text_rows();
        assert_eq!(files[0][2], "Included");
        assert_eq!(files[1][2], "Left out: no safety functions");
        assert!(out.section("truth").unwrap().rows.len() >= 5);
    }

    #[test]
    fn wire_lengths_are_never_called_exact() {
        let doc = Document::default();
        let rows = truth_rows(doc.project(), &HandoverInput::default());
        assert!(
            rows.iter()
                .all(|r| r.item != "Wire lengths" || r.truth != Truth::Exact)
        );
        assert!(
            rows.iter()
                .all(|r| r.item != "Issued state" || r.truth != Truth::Exact)
        );
    }

    #[test]
    fn open_commissioning_is_recorded_not_exact() {
        let doc = Document::default();
        let input = HandoverInput {
            commissioning: Some(CommissioningFacts {
                total: 10,
                done: 7,
                failed: 1,
                open_punch: 2,
                blocking_punch: 1,
            }),
            ..HandoverInput::default()
        };
        let rows = truth_rows(doc.project(), &input);
        let c = rows
            .iter()
            .find(|r| r.item == "Commissioning record")
            .unwrap();
        assert_eq!(c.truth, Truth::Recorded);
        assert!(c.detail.contains("7 of 10"));
        assert!(c.detail.contains("1 blocking"));
    }
}
