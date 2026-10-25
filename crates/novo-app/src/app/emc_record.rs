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
fn to_do(f: &novo_emc::Finding) -> Vec<String> {
    if f.fixes.is_empty() {
        return wrap(
            "By hand: the program offers no automatic fix. Read the reason and decide on the drawing or in the cabinet.",
            W_TODO,
        );
    }
    f.fixes
        .iter()
        .flat_map(|fix| wrap(&format!("- {}", fix.hint()), W_TODO))
        .collect()
}

/// The record as a report. `name_of` gives a wire or device its tag.
pub(super) fn record_output(
    project_name: &str,
    report: &Report,
    name_of: &dyn Fn(Subject) -> String,
) -> ReportOutput {
    let mut sections = Vec::new();

    let mut summary = Section::new(
        "summary",
        "EMC design record",
        vec![
            Column::new("what", "What", 30.0),
            Column::new("count", "Count", 8.0).right(),
        ],
    );
    let count = |s: Severity| report.findings.iter().filter(|f| f.severity == s).count();
    for (label, n) in [
        ("Errors", count(Severity::Error)),
        ("Warnings", count(Severity::Warning)),
        ("Notes", count(Severity::Info)),
        ("Rules that ran", report.rules.len()),
    ] {
        summary.push(vec![Cell::text(label), Cell::Int(n as i64)], Vec::new());
    }
    sections.push(summary);

    let mut notice = Section::new(
        "notice",
        "What this is",
        vec![Column::new("text", "Read this first", 120.0)],
    )
    .flowing();
    for line in wrap(novo_emc::DESIGN_AID, 110)
        .into_iter()
        .chain(wrap(
            "The findings are rules on what is drawn, geometric indices and closed-form estimates with planning distances from common practice. Clause numbers are unverified. A list with no finding is not a statement that the design is fine: see Not checked below. Compliance is shown by measurement on the finished machine.",
            110,
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
                wrap(&f.message, W_TEXT),
                to_do(f),
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
        let reasons: Vec<String> = s
            .reasons
            .iter()
            .flat_map(|r| wrap(&format!("+{} {} {}", r.points, r.rule, r.text), 100))
            .collect();
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
        "Rules that ran",
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
                wrap(rule.title, W_SHORT),
                vec![format!(
                    "{} ({})",
                    status_word(r.status),
                    rule.level.label().to_lowercase()
                )],
                wrap(rule.source, 70),
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
        for (i, line) in wrap(t, 110).into_iter().enumerate() {
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
        base
    })
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
    pub(super) fn emc_record_output(&self, name: &str) -> Option<ReportOutput> {
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
        Some(record_output(name, &report, &tag))
    }

    /// Writes the record to `path`.
    pub(super) fn write_emc_record(
        &mut self,
        format: EmcRecordFormat,
        path: &Path,
    ) -> Option<PathBuf> {
        let name = self.project_name()?;
        let output = self.emc_record_output(&name)?;
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
    fn paths_sit_beside_the_project() {
        let p = record_path(Some(Path::new("/x/Press.novo")), None, "Press", "pdf").unwrap();
        assert!(p.to_string_lossy().contains("Press EMC record"));
    }

    #[test]
    fn an_empty_report_says_no_finding_and_still_lists_what_is_not_checked() {
        let out = record_output("Press", &Report::default(), &|_| String::new());
        let ids: Vec<_> = out.sections.iter().map(|s| s.key.as_str()).collect();
        assert!(
            ids.contains(&"findings") && ids.contains(&"not_checked") && ids.contains(&"signoff")
        );
        assert!(out.title.contains("Press"));
    }
}
