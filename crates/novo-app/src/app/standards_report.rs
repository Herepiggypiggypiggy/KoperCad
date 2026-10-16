//! The standards compliance report as a document: a PDF (and CSV or Excel)
//! made from [`novo_checks::standards::document`], and an HTML page that
//! prints on A4. Both are written next to the project file; the content is
//! the same, so what a customer prints equals what the engineer saw.

use std::path::{Path, PathBuf};

use novo_checks::standards::document::{self, ComplianceDocument, Verdict};
use novo_reports::{Cell, Column, ReportOutput, Section};

use super::pdf::{replaceable_path, write_replacing};
use super::{Level, NovoApp, projects_dir};

/// Which file the report is written as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StandardsFormat {
    Pdf,
    Html,
    Excel,
}

/// `text` broken at spaces into lines of at most `width` characters. A word
/// longer than the line is cut.
pub(super) fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_owned();
        while word.chars().count() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let head: String = word.chars().take(width).collect();
            word = word.chars().skip(width).collect();
            lines.push(head);
        }
        let joined = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if joined > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Characters per line of the four wrapped columns of a requirement table,
/// for A4 landscape at the report text size.
const W_REQ: usize = 42;
const W_EVIDENCE: usize = 74;
const W_FIX: usize = 52;

fn evidence_lines(r: &document::Requirement) -> Vec<String> {
    let mut out = Vec::new();
    if r.verdict == Verdict::NotApplicable {
        return vec!["Nothing in the project falls under this.".to_owned()];
    }
    if r.checked > 0 {
        out.push(format!("{} checked", r.checked));
    }
    for e in r.evidence.iter().take(6) {
        out.extend(wrap(&format!("- {e}"), W_EVIDENCE));
    }
    if r.evidence.len() > 6 {
        out.push(format!("- and {} more", r.evidence.len() - 6));
    }
    for f in &r.findings {
        let page = f
            .page
            .as_ref()
            .map(|p| format!(" (page {p})"))
            .unwrap_or_default();
        let waived = f
            .reason
            .as_ref()
            .map(|x| format!(" WAIVED, reason: {x}"))
            .unwrap_or_default();
        out.extend(wrap(
            &format!("Finding: {}{page}{waived}", f.text),
            W_EVIDENCE,
        ));
    }
    for u in &r.not_checkable {
        out.extend(wrap(&format!("Not checkable: {u}"), W_EVIDENCE));
    }
    out
}

/// The document as a report: a summary, one table per standard, the basis
/// of the numbers, what is not covered, and a review block.
pub(super) fn report_output(doc: &ComplianceDocument) -> ReportOutput {
    let mut sections = Vec::new();
    let mut summary = Section::new(
        "summary",
        "Summary",
        vec![
            Column::new("standard", "Standard", 14.0),
            Column::new("pass", "Pass", 6.0).right(),
            Column::new("fail", "Fail", 6.0).right(),
            Column::new("partly", "Partly", 6.0).right(),
            Column::new("not_checkable", "Not checkable", 9.0).right(),
            Column::new("waived", "Waived", 6.0).right(),
            Column::new("na", "n/a", 6.0).right(),
        ],
    );
    for s in &doc.sections {
        summary.push(
            vec![
                Cell::text(s.set.short()),
                Cell::Int(s.count(Verdict::Pass) as i64),
                Cell::Int(s.count(Verdict::Fail) as i64),
                Cell::Int(s.count(Verdict::Partial) as i64),
                Cell::Int(s.count(Verdict::NotCheckable) as i64),
                Cell::Int(s.count(Verdict::Waived) as i64),
                Cell::Int(s.count(Verdict::NotApplicable) as i64),
            ],
            Vec::new(),
        );
    }
    sections.push(summary);
    let mut notice = Section::new(
        "notice",
        "What this report is",
        vec![Column::new("text", "Notice", 1.0)],
    );
    for line in wrap(doc.notice, 150) {
        notice.push(vec![Cell::text(line)], Vec::new());
    }
    for s in &doc.sections {
        let line = format!(
            "{}: clause pointers refer to {}.",
            s.set.short(),
            s.set.edition()
        );
        notice.push(vec![Cell::text(line)], Vec::new());
    }
    sections.push(notice);
    for s in &doc.sections {
        let mut t = Section::new(
            s.set.id(),
            s.set.short(),
            vec![
                Column::new("result", "Result", 14.0),
                Column::new("clause", "Clause", 7.0),
                Column::new("requirement", "Requirement", 27.0),
                Column::new("evidence", "Evidence and findings", 48.0),
                Column::new("fix", "Where to fix", 36.0),
            ],
        );
        for r in &s.requirements {
            let mut verdict = vec![r.verdict.label().to_owned()];
            if r.rests_on_planning_values()
                && matches!(r.verdict, Verdict::Pass | Verdict::Fail | Verdict::Partial)
            {
                verdict.push("planning value".to_owned());
            }
            let mut req = wrap(r.title, W_REQ);
            req.extend(wrap(r.requirement, W_REQ));
            let evidence = evidence_lines(r);
            let fix = if matches!(
                r.verdict,
                Verdict::Fail | Verdict::Partial | Verdict::NotCheckable
            ) {
                wrap(r.fix, W_FIX)
            } else {
                Vec::new()
            };
            let rows = req
                .len()
                .max(evidence.len())
                .max(fix.len())
                .max(verdict.len());
            let pick =
                |v: &Vec<String>, i: usize| v.get(i).cloned().map_or(Cell::Empty, Cell::text);
            for i in 0..rows {
                t.push(
                    vec![
                        pick(&verdict, i),
                        if i == 0 {
                            Cell::text(r.clause)
                        } else {
                            Cell::Empty
                        },
                        pick(&req, i),
                        pick(&evidence, i),
                        pick(&fix, i),
                    ],
                    Vec::new(),
                );
            }
        }
        sections.push(t);
    }
    let mut basis = Section::new(
        "basis",
        "Basis of the numbers",
        vec![
            Column::new("kind", "Kind", 10.0),
            Column::new("what", "What", 55.0),
            Column::new("used", "Used by", 30.0),
        ],
    );
    for b in &doc.basis {
        let what = wrap(b.what, 100);
        let used = wrap(&b.used_by.join(", "), 52);
        for i in 0..what.len().max(used.len()) {
            basis.push(
                vec![
                    if i == 0 {
                        Cell::text(b.kind.label())
                    } else {
                        Cell::Empty
                    },
                    what.get(i).cloned().map_or(Cell::Empty, Cell::text),
                    used.get(i).cloned().map_or(Cell::Empty, Cell::text),
                ],
                Vec::new(),
            );
        }
    }
    sections.push(basis);
    let mut scope = Section::new(
        "not_covered",
        "Not covered by this report",
        vec![Column::new("text", "Not covered", 1.0)],
    );
    for n in &doc.not_covered {
        for line in wrap(n, 150) {
            scope.push(vec![Cell::text(line)], Vec::new());
        }
    }
    sections.push(scope);
    let mut signoff = Section::new(
        "signoff",
        "Review",
        vec![
            Column::new("what", "", 10.0),
            Column::new("value", "", 30.0),
            Column::new("what2", "", 10.0),
            Column::new("value2", "", 30.0),
        ],
    );
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
        report: "standards".into(),
        title: format!("Standards: {}", doc.project_name),
        revision: 0,
        sections,
    }
}

/// `<project> standards.<ext>` beside the project file, or in the Documents
/// folder for a project with no file yet.
fn report_path(file: Option<&Path>, dir: Option<&Path>, name: &str, ext: &str) -> Option<PathBuf> {
    let base = match file {
        Some(file) => {
            let stem = file.file_stem()?.to_string_lossy().into_owned();
            file.with_file_name(format!("{stem} standards.{ext}"))
        }
        None => {
            let stem = Path::new(&super::project_file_name(name))
                .with_extension("")
                .to_string_lossy()
                .into_owned();
            dir?.join(format!("{stem} standards.{ext}"))
        }
    };
    if ext == "pdf" {
        Some(replaceable_path(&base))
    } else {
        Some(base)
    }
}

impl NovoApp {
    /// Writes the compliance report next to the project and says where it
    /// went. A PDF KoperCAD wrote is replaced; an HTML page is always ours.
    pub(super) fn export_standards_report(&mut self, format: StandardsFormat) -> Option<PathBuf> {
        let name = self.project_name()?;
        let ext = match format {
            StandardsFormat::Pdf => "pdf",
            StandardsFormat::Html => "html",
            StandardsFormat::Excel => "xlsx",
        };
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let Some(path) = report_path(file.as_deref(), projects_dir().as_deref(), &name, ext) else {
            self.message(Level::Error, "Could not find the Documents folder");
            return None;
        };
        self.write_standards_report(format, &path)
    }

    /// Writes the report to `path`.
    pub(super) fn write_standards_report(
        &mut self,
        format: StandardsFormat,
        path: &Path,
    ) -> Option<PathBuf> {
        let name = self.project_name()?;
        let doc = {
            let project = self.backend.document()?.project();
            let report = novo_checks::standards::compliance(project);
            if report.sets.is_empty() {
                self.message(
                    Level::Info,
                    "Choose a standard in Standards check first: the report covers the standards you select",
                );
                return None;
            }
            document::build(project, &report)
        };
        let bytes = match format {
            StandardsFormat::Pdf => {
                let options = novo_pdf::ReportPdfOptions {
                    title: format!("{name}, standards compliance"),
                    footer_right: "Assists the engineer; not a certificate".to_owned(),
                    language: self.report_translator(),
                    ..Default::default()
                };
                novo_pdf::export_report_pdf(&[&report_output(&doc)], &options)
            }
            StandardsFormat::Html => document::to_html(&doc, "").into_bytes(),
            StandardsFormat::Excel => {
                match novo_reports::xlsx_bytes(&[&novo_reports::i18n::translate_output(
                    &report_output(&doc),
                    &self.report_translator(),
                )]) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        self.message(Level::Error, format!("Could not write the report: {e}"));
                        return None;
                    }
                }
            }
        };
        match write_replacing(path, &bytes) {
            Ok(()) => {
                self.message(Level::Info, format!("Exported {}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not write the report: {e}"));
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_keeps_words_and_cuts_long_ones() {
        assert_eq!(wrap("aa bb cc", 5), vec!["aa bb", "cc"]);
        assert_eq!(wrap("abcdefgh", 3), vec!["abc", "def", "gh"]);
        assert!(wrap("", 10).is_empty());
    }

    #[test]
    fn paths_sit_beside_the_project() {
        let p = report_path(Some(Path::new("/x/Press.novo")), None, "Press", "html").unwrap();
        assert_eq!(p, Path::new("/x/Press standards.html"));
    }
}
