//! The standards compliance report as a document: a PDF (and CSV or Excel)
//! made from [`novo_checks::standards::document`], and an HTML page that
//! prints on A4. Both are written next to the project file; the content is
//! the same, so what a customer prints equals what the engineer saw.

use std::path::{Path, PathBuf};

use novo_checks::standards::design::{DesignInput, HeatFact, HeatTone};
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

/// Characters per line of the design detail column.
const W_DETAIL: usize = 112;

/// The design checks: one summary table, then one block per check with what
/// it asks, where its data comes from, what it assumes and what it found.
fn design_sections(doc: &ComplianceDocument) -> Vec<Section> {
    let mut summary = Section::new(
        "design_summary",
        "Design checks",
        vec![
            Column::new("check", "Check", 26.0),
            Column::new("result", "Result", 14.0),
            Column::new("found", "Found", 34.0),
            Column::new("rests", "Rests on", 40.0),
        ],
    );
    for r in &doc.design {
        let found = wrap(&document::found_text(r), 56);
        let rests = wrap(r.family.reliance.label(), 64);
        for i in 0..found.len().max(rests.len()).max(1) {
            summary.push(
                vec![
                    if i == 0 {
                        Cell::text(r.family.title)
                    } else {
                        Cell::Empty
                    },
                    if i == 0 {
                        Cell::text(r.verdict.label())
                    } else {
                        Cell::Empty
                    },
                    found.get(i).cloned().map_or(Cell::Empty, Cell::text),
                    rests.get(i).cloned().map_or(Cell::Empty, Cell::text),
                ],
                Vec::new(),
            );
        }
    }
    let mut detail = Section::new(
        "design_detail",
        "Design checks in detail",
        vec![
            Column::new("check", "Check", 22.0),
            Column::new("result", "Result", 14.0),
            Column::new("what", "", 14.0),
            Column::new("text", "", 80.0),
        ],
    );
    for r in &doc.design {
        let mut lines: Vec<(&str, String)> = vec![
            ("Asks", r.family.what.to_owned()),
            ("Data from", r.family.source.to_owned()),
            ("Assumes", r.family.assumptions.to_owned()),
        ];
        for e in &r.evidence {
            lines.push(("Evidence", e.clone()));
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
            lines.push(("Finding", format!("{}{page}{waived}", f.text)));
        }
        if r.more > 0 {
            lines.push((
                "Finding",
                format!("and {} more: see the Issues list", r.more),
            ));
        }
        for u in &r.not_checkable {
            lines.push(("Not checked", u.clone()));
        }
        if matches!(
            r.verdict,
            Verdict::Fail | Verdict::Partial | Verdict::NotCheckable
        ) {
            lines.push(("Where to fix", r.family.fix.to_owned()));
        }
        let mut first = true;
        for (label, text) in lines {
            for (i, line) in wrap(&text, W_DETAIL).into_iter().enumerate() {
                detail.push(
                    vec![
                        if first {
                            Cell::text(r.family.title)
                        } else {
                            Cell::Empty
                        },
                        if first {
                            Cell::text(r.verdict.label())
                        } else {
                            Cell::Empty
                        },
                        if i == 0 {
                            Cell::text(label)
                        } else {
                            Cell::Empty
                        },
                        Cell::text(line),
                    ],
                    Vec::new(),
                );
                first = false;
            }
        }
    }
    vec![summary, detail]
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
    let mut limits = Section::new(
        "limits",
        "Read this first",
        vec![Column::new("text", "Limits of this report", 1.0)],
    );
    for l in &doc.limits {
        for (i, line) in wrap(l, 146).into_iter().enumerate() {
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
    sections.extend(design_sections(doc));
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
    /// The Heat result the 3D view holds, as the report states it. `None`
    /// when no run has finished in this session.
    fn heat_fact(&self) -> Option<HeatFact> {
        use super::heat_verdict::{Tone, verdict};
        let sol = self.cabinet.heat.solution()?;
        let project = self.backend.document()?.project();
        let tag = |raw: u64| {
            project
                .get::<novo_core::Device>(novo_core::DeviceId::from(raw))
                .and_then(|d| d.tag.device_code().map(str::to_owned))
                .unwrap_or_else(|| format!("device {raw}"))
        };
        let v = verdict(&sol.snapshot, &tag)?;
        let tone = match v.tone {
            Tone::Fine => HeatTone::Fine,
            Tone::Tight => HeatTone::Tight,
            Tone::TooHot => HeatTone::TooHot,
            Tone::Unknown => HeatTone::Unknown,
        };
        let sentence = match &v.device {
            Some((_, device)) => format!("{}{device}{}", v.lead, v.tail),
            None => format!("{}{}", v.lead, v.tail),
        };
        Some(HeatFact {
            tone,
            sentence,
            out_of_date: self.cabinet.heat.live.is_stale(),
        })
    }

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
            let heat = self.heat_fact();
            let project = self.backend.document()?.project();
            let report = novo_checks::standards::compliance(project);
            let input = DesignInput {
                open: novo_reports::issues::open(project),
                waived: novo_reports::issues::waived(project),
                heat,
            };
            document::build_with(project, &report, &input)
        };
        let bytes = match format {
            StandardsFormat::Pdf => {
                let options = novo_pdf::ReportPdfOptions {
                    title: format!("{name}, standards compliance"),
                    footer_right: "Subset of the standard, planning values; not a certification"
                        .to_owned(),
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
                self.message_file(format!("Exported {}", path.display()), path);
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
