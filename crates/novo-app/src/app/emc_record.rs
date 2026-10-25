//! The EMC design record: what the EMC rules found, what the bench has to do
//! about it, which rules ran and what rests on assumptions, as a PDF or an
//! Excel file next to the project. A design aid, never a test result: the
//! same content the EMC check window shows, made printable.

use std::path::{Path, PathBuf};

use novo_emc::{Report, Severity, Status, Subject};
use novo_reports::{Cell, Column, ReportOutput, Section};

use super::pdf::{replaceable_path, write_replacing};
use super::standards_report::wrap;
use super::{Level, NovoApp, projects_dir};

/// Which file the record is written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EmcRecordFormat {
    Pdf,
    Excel,
}

const W_TEXT: usize = 78;
const W_TODO: usize = 52;
const W_SHORT: usize = 40;
/// Wires and devices listed by risk score.
const TOP_SCORES: usize = 10;

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "Error",
        Severity::Warning => "Warning",
        Severity::Info => "Note",
    }
}

fn status_word(s: Status) -> &'static str {
    match s {
        Status::NoFinding => "No finding",
        Status::Finding => "Finding",
        Status::NotApplicable => "Not applicable",
        Status::NotEvaluated => "Not evaluated",
    }
}

/// `text` broken into lines for a printed page, or kept whole for a
/// spreadsheet cell (one row per finding, so it can be filtered and sorted).
fn lines(text: &str, width: usize, wrapped: bool) -> Vec<String> {
    if wrapped {
        wrap(text, width)
    } else {
        vec![text.to_owned()]
    }
}

/// `rows` of wrapped columns as table rows: a row per line of the tallest.
fn push_wrapped(section: &mut Section, columns: &[Vec<String>]) {
    let rows = columns.iter().map(Vec::len).max().unwrap_or(0).max(1);
    for i in 0..rows {
        let cells = columns
            .iter()
            .map(|c| c.get(i).cloned().map_or(Cell::Empty, Cell::text))
            .collect();
        section.push(cells, Vec::new());
    }
}

/// What the bench does about a finding: the fixes the program offers, worded
/// as requirements for the build, or the plain statement that none is
/// automatic.
fn to_do(f: &novo_emc::Finding, wrapped: bool) -> Vec<String> {
    if f.fixes.is_empty() {
        return lines(
            "By hand: the program offers no automatic fix. Read the reason and decide on the drawing or in the cabinet.",
            W_TODO,
            wrapped,
        );
    }
    if !wrapped {
        return vec![
            f.fixes
                .iter()
                .map(|fix| fix.hint())
                .collect::<Vec<_>>()
                .join("; "),
        ];
    }
    f.fixes
        .iter()
        .flat_map(|fix| wrap(&format!("- {}", fix.hint()), W_TODO))
        .collect()
}

/// The record as a report. `name_of` gives a wire or device its tag. With
/// `wrapped` the long texts are broken into lines for a printed page; without
/// it each finding is one row for a spreadsheet. `date` and `version` say
/// when and by which program the record was made.
pub(super) fn record_output(
    project_name: &str,
    report: &Report,
    name_of: &dyn Fn(Subject) -> String,
    date: &str,
    version: &str,
    wrapped: bool,
) -> ReportOutput {
    let mut sections = Vec::new();

    let mut summary = Section::new(
        "summary",
        "EMC design record",
        vec![
            Column::new("what", "What", 30.0),
            Column::new("count", "Value", 20.0).right(),
        ],
    );
    let evaluated = report
        .rules
        .iter()
        .filter(|r| matches!(r.status, Status::NoFinding | Status::Finding))
        .count();
    let count = |s: Severity| report.findings.iter().filter(|f| f.severity == s).count();
    for (label, n) in [
        ("Errors", count(Severity::Error)),
        ("Warnings", count(Severity::Warning)),
        ("Notes", count(Severity::Info)),
        ("Rules in the check", report.rules.len()),
        (
            "Rules that were evaluated (the rest: not applicable or no data)",
            evaluated,
        ),
    ] {
        summary.push(vec![Cell::text(label), Cell::Int(n as i64)], Vec::new());
    }
    summary.push(vec![Cell::text("Made on"), Cell::text(date)], Vec::new());
    summary.push(
        vec![
            Cell::text("Program"),
            Cell::text(format!("KoperCAD {version}")),
        ],
        Vec::new(),
    );
    sections.push(summary);

    let mut notice = Section::new(
        "notice",
        "What this is",
        vec![Column::new("text", "Read this first", 120.0)],
    )
    .flowing();
    for line in lines(novo_emc::DESIGN_AID, 110, wrapped)
        .into_iter()
        .chain(lines(
            "The findings are rules on what is drawn, geometric indices and closed-form estimates with planning distances from common practice. Clause numbers are unverified. A list with no finding is not a statement that the design is fine: see Not checked below. Compliance is shown by measurement on the finished machine.",
            110,
            wrapped,
        ))
    {
        notice.push(vec![Cell::text(line)], Vec::new());
    }
    sections.push(notice);

    let mut findings = Section::new(
        "findings",
        "Findings and what to do at the bench",
        vec![
            Column::new("rule", "Rule", 8.0),
            Column::new("severity", "Severity", 9.0),
            Column::new("kind", "Kind", 8.0),
            Column::new("reason", "Reason", 60.0),
            Column::new("todo", "What to do", 40.0),
        ],
    )
    .flowing();
    let mut ordered: Vec<_> = report.findings.iter().collect();
    ordered.sort_by_key(|f| (f.severity, f.rule));
    for f in ordered {
        push_wrapped(
            &mut findings,
            &[
                vec![f.rule.to_owned()],
                vec![severity_word(f.severity).to_owned()],
                vec![f.level.label().to_owned()],
                lines(&f.message, W_TEXT, wrapped),
                to_do(f, wrapped),
            ],
        );
    }
    if report.findings.is_empty() {
        findings.push(
            vec![
                Cell::text("-"),
                Cell::Empty,
                Cell::Empty,
                Cell::text("No finding."),
                Cell::Empty,
            ],
            Vec::new(),
        );
    }
    sections.push(findings);

    let mut scores = Section::new(
        "scores",
        "Highest risk scores",
        vec![
            Column::new("name", "Wire or device", 24.0),
            Column::new("points", "Points", 8.0).right(),
            Column::new("reasons", "Reasons", 90.0),
        ],
    )
    .flowing();
    let mut all: Vec<_> = report.runs.iter().chain(&report.devices).collect();
    all.sort_by_key(|s| std::cmp::Reverse(s.points));
    for s in all.into_iter().filter(|s| s.points > 0).take(TOP_SCORES) {
        let reasons: Vec<String> = if wrapped {
            s.reasons
                .iter()
                .flat_map(|r| wrap(&format!("+{} {} {}", r.points, r.rule, r.text), 100))
                .collect()
        } else {
            vec![
                s.reasons
                    .iter()
                    .map(|r| format!("+{} {} {}", r.points, r.rule, r.text))
                    .collect::<Vec<_>>()
                    .join("; "),
            ]
        };
        push_wrapped(
            &mut scores,
            &[
                vec![name_of(s.subject)],
                vec![format!("{} of 100", s.points)],
                reasons,
            ],
        );
    }
    scores.push(
        vec![
            Cell::text("The score orders wires for attention; it is not a measurement."),
            Cell::Empty,
            Cell::Empty,
        ],
        Vec::new(),
    );
    sections.push(scores);

    let mut rules = Section::new(
        "rules",
        "Rules in the check",
        vec![
            Column::new("rule", "Rule", 8.0),
            Column::new("title", "Title", 36.0),
            Column::new("result", "Result", 14.0),
            Column::new("source", "Source (unverified)", 60.0),
        ],
    )
    .flowing();
    for r in &report.rules {
        let rule = novo_emc::rules::rule(r.rule);
        push_wrapped(
            &mut rules,
            &[
                vec![r.rule.to_owned()],
                lines(rule.title, W_SHORT, wrapped),
                vec![format!(
                    "{} ({})",
                    status_word(r.status),
                    rule.level.label().to_lowercase()
                )],
                lines(rule.source, 70, wrapped),
            ],
        );
    }
    sections.push(rules);

    let mut assumptions = Section::new(
        "assumptions",
        "Assumptions the numbers rest on",
        vec![
            Column::new("name", "Setting", 40.0),
            Column::new("value", "Value", 24.0),
            Column::new("origin", "Origin", 40.0),
        ],
    )
    .flowing();
    for a in &report.assumptions {
        assumptions.push(
            vec![
                Cell::text(a.name),
                Cell::text(a.value.clone()),
                Cell::text(if a.user_set {
                    "set in the project"
                } else {
                    "program default"
                }),
            ],
            Vec::new(),
        );
    }
    sections.push(assumptions);

    let mut limits = Section::new(
        "not_checked",
        "Not checked",
        vec![Column::new("text", "What the rules do not cover", 120.0)],
    )
    .flowing();
    for t in novo_emc::NOT_CHECKED {
        for (i, line) in lines(t, 110, wrapped).into_iter().enumerate() {
            limits.push(
                vec![Cell::text(if i == 0 {
                    format!("- {line}")
                } else {
                    format!("  {line}")
                })],
                Vec::new(),
            );
        }
    }
    sections.push(limits);

    let mut signoff = Section::new(
        "signoff",
        "Review",
        vec![
            Column::new("label", "", 20.0),
            Column::new("value", "", 40.0),
            Column::new("label2", "", 20.0),
            Column::new("value2", "", 40.0),
        ],
    )
    .flowing();
    signoff.push(
        vec![
            Cell::text("Reviewed by"),
            Cell::Empty,
            Cell::text("Date"),
            Cell::Empty,
        ],
        Vec::new(),
    );
    signoff.push(
        vec![
            Cell::text("Signature"),
            Cell::Empty,
            Cell::text("Role"),
            Cell::Empty,
        ],
        Vec::new(),
    );
    sections.push(signoff);

    ReportOutput {
        report: "emc_record".into(),
        title: project_name.to_owned(),
        revision: 0,
        sections,
    }
}

/// `<project> EMC record.<ext>` beside the project file, or in the Documents
/// folder for a project with no file yet.
fn record_path(file: Option<&Path>, dir: Option<&Path>, name: &str, ext: &str) -> Option<PathBuf> {
    let base = match file {
        Some(file) => {
            let stem = file.file_stem()?.to_string_lossy().into_owned();
            file.with_file_name(format!("{stem} EMC record.{ext}"))
        }
        None => {
            let stem = Path::new(&super::project_file_name(name))
                .with_extension("")
                .to_string_lossy()
                .into_owned();
            dir?.join(format!("{stem} EMC record.{ext}"))
        }
    };
    Some(if ext == "pdf" {
        replaceable_path(&base)
    } else {
        // A workbook may have been filled in (the review block): never replace one.
        first_unused(&base)
    })
}

/// `path` itself when nothing is there, else the first free `<stem> 2.<ext>`,
/// `<stem> 3.<ext>`...
fn first_unused(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    (1..)
        .map(|n| {
            if n == 1 {
                path.to_owned()
            } else {
                path.with_file_name(format!("{stem} {n}.{ext}"))
            }
        })
        .find(|p| !p.exists())
        .unwrap_or_else(|| path.to_owned())
}

impl NovoApp {
    /// Writes the EMC record next to the project and says where it went.
    pub(super) fn export_emc_record(&mut self, format: EmcRecordFormat) -> Option<PathBuf> {
        let name = self.project_name()?;
        let ext = match format {
            EmcRecordFormat::Pdf => "pdf",
            EmcRecordFormat::Excel => "xlsx",
        };
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let Some(path) = record_path(file.as_deref(), projects_dir().as_deref(), &name, ext) else {
            self.message(Level::Error, "Could not find the Documents folder");
            return None;
        };
        self.write_emc_record(format, &path)
    }

    /// The record of the open project as a report.
    pub(super) fn emc_record_output(&self, name: &str, wrapped: bool) -> Option<ReportOutput> {
        let project = self.backend.document()?.project();
        let netlist = project.netlist();
        let input = novo_emc::project::input_of(project, &netlist);
        let field = novo_emc::project::field_setup_of(project, &netlist, &input);
        let report = novo_emc::analyse_with_field(&input, Some(&field));
        let tag = |s: Subject| {
            match s {
                Subject::Run(id) => input
                    .runs
                    .iter()
                    .find(|r| r.id == id)
                    .map(|r| r.name.clone()),
                Subject::Device(id) => input
                    .devices
                    .iter()
                    .find(|d| d.id == id)
                    .map(|d| d.tag.clone()),
                _ => None,
            }
            .unwrap_or_else(|| "(unnamed)".to_owned())
        };
        Some(record_output(
            name,
            &report,
            &tag,
            &super::revisions::today(),
            env!("CARGO_PKG_VERSION"),
            wrapped,
        ))
    }

    /// Writes the record to `path`.
    pub(super) fn write_emc_record(
        &mut self,
        format: EmcRecordFormat,
        path: &Path,
    ) -> Option<PathBuf> {
        let name = self.project_name()?;
        let output = self.emc_record_output(&name, format == EmcRecordFormat::Pdf)?;
        let bytes = match format {
            EmcRecordFormat::Pdf => {
                let options = novo_pdf::ReportPdfOptions {
                    title: format!("{name}, EMC design record"),
                    footer_right: "Design aid, planning values; not a test result".to_owned(),
                    language: self.report_translator(),
                    ..Default::default()
                };
                novo_pdf::export_report_pdf(&[&output], &options)
            }
            EmcRecordFormat::Excel => {
                match novo_reports::xlsx_bytes(&[&novo_reports::i18n::translate_output(
                    &output,
                    &self.report_translator(),
                )]) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        self.message(Level::Error, format!("Could not write the record: {e}"));
                        return None;
                    }
                }
            }
        };
        match write_replacing(path, &bytes) {
            Ok(()) => {
                self.message_file(format!("Exported {}", path.display()), path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not write the record: {e}"));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_excel_record_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("Press EMC record.xlsx");
        assert_eq!(first_unused(&base), base);
        std::fs::write(&base, b"signed").unwrap();
        let second = dir.path().join("Press EMC record 2.xlsx");
        assert_eq!(first_unused(&base), second);
        std::fs::write(&second, b"x").unwrap();
        assert_eq!(
            first_unused(&base),
            dir.path().join("Press EMC record 3.xlsx")
        );
    }

    #[test]
    fn paths_sit_beside_the_project() {
        let p = record_path(Some(Path::new("/x/Press.novo")), None, "Press", "pdf").unwrap();
        assert!(p.to_string_lossy().contains("Press EMC record"));
    }

    #[test]
    fn an_empty_report_says_no_finding_and_still_lists_what_is_not_checked() {
        let out = record_output(
            "Press",
            &Report::default(),
            &|_| String::new(),
            "2026-10-25",
            "0.1.0",
            true,
        );
        let ids: Vec<_> = out.sections.iter().map(|s| s.key.as_str()).collect();
        assert!(
            ids.contains(&"findings") && ids.contains(&"not_checked") && ids.contains(&"signoff")
        );
        assert!(out.title.contains("Press"));
    }
}
