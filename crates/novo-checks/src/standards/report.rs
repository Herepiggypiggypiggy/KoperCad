//! The compliance report as text: lines for the PDF, Markdown and CSV.

use novo_core::Project;

use super::sccr::SccrSource;
use super::{ComplianceReport, RuleResult, RuleStatus, StandardSet, page_name};

/// The legal note printed at the top of every report.
pub const NOTICE: &str = "This report assists the engineer and does not certify compliance. The rules are short statements in our own words with clause pointers into the editions named; read the clauses in your own licensed copy of each standard, and confirm the values marked for verification. The engineer remains responsible for the design.";

/// A kA value without trailing zeros.
pub(crate) fn kilo(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

fn mark(status: RuleStatus) -> &'static str {
    match status {
        RuleStatus::Pass => "PASS",
        RuleStatus::Fail => "FAIL",
        RuleStatus::Partial => "PARTIAL",
        RuleStatus::NotEvaluated => "NOT EVALUATED",
        RuleStatus::NotApplicable => "n/a",
        RuleStatus::Waived => "WAIVED",
    }
}

fn page_suffix(project: &Project, page: Option<novo_core::PageId>) -> String {
    page_name(project, page)
        .map(|p| format!("  (page {p})"))
        .unwrap_or_default()
}

fn tally_text(r: &ComplianceReport, set: StandardSet) -> String {
    let t = r.tally(Some(set));
    let mut parts = vec![format!("{} pass", t.pass)];
    if t.fail > 0 {
        parts.push(format!("{} fail", t.fail));
    }
    if t.partial > 0 {
        parts.push(format!("{} partial", t.partial));
    }
    if t.not_evaluated > 0 {
        parts.push(format!("{} not evaluated", t.not_evaluated));
    }
    if t.waived > 0 {
        parts.push(format!("{} waived", t.waived));
    }
    if t.not_applicable > 0 {
        parts.push(format!("{} not applicable", t.not_applicable));
    }
    parts.join(", ")
}

/// The report as `(bold, text)` lines for a printed appendix. Empty when no
/// standard is selected.
pub fn lines(project: &Project, report: &ComplianceReport) -> Vec<(bool, String)> {
    if report.sets.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(bool, String)> = vec![
        (true, "Standards compliance".to_owned()),
        (false, NOTICE.to_owned()),
        (false, String::new()),
    ];
    for set in &report.sets {
        out.push((true, format!("{}: {}", set.short(), set.title())));
        out.push((false, format!("Edition: {}", set.edition())));
        out.push((false, tally_text(report, *set)));
        for r in report.of(*set) {
            out.push((
                false,
                format!(
                    "[{}] {} {}  ({})",
                    mark(r.status),
                    r.rule.clause,
                    r.rule.title,
                    r.rule.id
                ),
            ));
            for p in &r.open {
                out.push((
                    false,
                    format!("    {}{}", p.message, page_suffix(project, p.page)),
                ));
            }
            for w in &r.waived {
                out.push((
                    false,
                    format!(
                        "    Waived: {}{}",
                        w.problem.message,
                        page_suffix(project, w.problem.page)
                    ),
                ));
                out.push((false, format!("    Reason: {}", w.reason)));
            }
            for u in &r.unknown {
                out.push((false, format!("    Not evaluated: {}", u.why)));
            }
        }
        out.push((false, String::new()));
    }
    if let Some(sccr) = &report.sccr {
        out.push((true, "Short-circuit current rating".to_owned()));
        out.push((
            false,
            "Simplified UL 508A Supplement SB method: lowest component rating, raised behind a current-limiting device whose let-through is within the component's rating. Combination ratings and transformer secondaries are not modelled.".to_owned(),
        ));
        let available = sccr
            .available_ka
            .map_or("not stated".to_owned(), |a| format!("{} kA", kilo(a)));
        out.push((false, format!("Available at the supply: {available}")));
        out.push((
            false,
            match (sccr.panel_ka, sccr.complete) {
                (Some(p), true) => format!("Panel rating: {} kA", kilo(p)),
                (Some(p), false) => format!(
                    "Panel rating: not determined; at most {} kA (some components have no rating)",
                    kilo(p)
                ),
                (None, _) => "Panel rating: not determined".to_owned(),
            },
        ));
        for c in &sccr.components {
            let rating = c
                .rating_ka
                .map_or("unknown".to_owned(), |r| format!("{} kA", kilo(r)));
            let effective = match (c.effective_ka, c.rating_ka) {
                (Some(e), Some(r)) if (e - r).abs() > 1e-9 => {
                    format!(", counts as {} kA behind a current-limiting device", kilo(e))
                }
                _ => String::new(),
            };
            out.push((
                false,
                format!(
                    "    {} ({}): {rating} ({}){effective}",
                    c.tag,
                    c.kind,
                    c.source.label()
                ),
            ));
        }
        out.push((false, String::new()));
    }
    let verify: Vec<&RuleResult> = report
        .results
        .iter()
        .filter(|r| r.rule.verify.is_some())
        .collect();
    if !verify.is_empty() {
        out.push((true, "Values to confirm in your licensed copy".to_owned()));
        for r in verify {
            out.push((
                false,
                format!("{} {}: {}", r.rule.id, r.rule.clause, r.rule.verify.unwrap_or("")),
            ));
        }
    }
    out
}

/// The report as Markdown.
pub fn to_markdown(project: &Project, report: &ComplianceReport) -> String {
    let mut md = String::new();
    md.push_str(&format!(
        "# Standards compliance: {}\n\n> {}\n\n",
        report.project_name,
        NOTICE
    ));
    for set in &report.sets {
        md.push_str(&format!(
            "## {}: {}\n\nEdition: {}. {}\n\n",
            set.short(),
            set.title(),
            set.edition(),
            tally_text(report, *set)
        ));
        md.push_str("| Result | Clause | Rule | Id |\n| --- | --- | --- | --- |\n");
        for r in report.of(*set) {
            md.push_str(&format!(
                "| {} | {} | {} | `{}` |\n",
                mark(r.status),
                r.rule.clause,
                r.rule.title,
                r.rule.id
            ));
        }
        md.push('\n');
        for r in report.of(*set) {
            if r.open.is_empty() && r.waived.is_empty() && r.unknown.is_empty() {
                continue;
            }
            md.push_str(&format!(
                "### {} {} (`{}`)\n\n_{}_\n\n",
                r.rule.clause, r.rule.title, r.rule.id, r.rule.requirement
            ));
            for p in &r.open {
                md.push_str(&format!(
                    "- **Fail:** {}{}\n",
                    p.message,
                    page_suffix(project, p.page)
                ));
            }
            for w in &r.waived {
                md.push_str(&format!(
                    "- **Waived:** {}{}. Reason: {}\n",
                    w.problem.message,
                    page_suffix(project, w.problem.page),
                    w.reason
                ));
            }
            for u in &r.unknown {
                md.push_str(&format!("- Not evaluated: {}\n", u.why));
            }
            md.push('\n');
        }
    }
    if let Some(sccr) = &report.sccr {
        md.push_str("## Short-circuit current rating\n\n");
        md.push_str(&format!(
            "Available at the supply: {}. Panel rating: {}.\n\n",
            sccr.available_ka
                .map_or("not stated".to_owned(), |a| format!("{} kA", kilo(a))),
            match (sccr.panel_ka, sccr.complete) {
                (Some(p), true) => format!("{} kA", kilo(p)),
                (Some(p), false) => format!("not determined, at most {} kA", kilo(p)),
                (None, _) => "not determined".to_owned(),
            }
        ));
        md.push_str("| Component | Type | Rating | Source | Counts as |\n| --- | --- | --- | --- | --- |\n");
        for c in &sccr.components {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                c.tag,
                c.kind,
                c.rating_ka.map_or("?".to_owned(), |r| format!("{} kA", kilo(r))),
                match c.source {
                    SccrSource::Unknown => "unknown",
                    other => other.label(),
                },
                c.effective_ka.map_or("?".to_owned(), |r| format!("{} kA", kilo(r)))
            ));
        }
        md.push('\n');
    }
    md
}

fn csv_cell(text: &str) -> String {
    if text.contains([',', '"', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The report as CSV, one row per finding, waiver or unevaluated item and one
/// per rule without any: `standard, rule, clause, title, result, kind,
/// detail, page`.
pub fn to_csv(project: &Project, report: &ComplianceReport) -> String {
    let mut csv = String::from("standard,rule,clause,title,result,kind,detail,page\n");
    let mut row = |r: &RuleResult, kind: &str, detail: &str, page: String| {
        csv.push_str(
            &[
                r.rule.set.short(),
                r.rule.id,
                r.rule.clause,
                r.rule.title,
                mark(r.status),
                kind,
                detail,
                &page,
            ]
            .iter()
            .map(|c| csv_cell(c))
            .collect::<Vec<_>>()
            .join(","),
        );
        csv.push('\n');
    };
    for r in &report.results {
        if r.open.is_empty() && r.waived.is_empty() && r.unknown.is_empty() {
            row(r, "", "", String::new());
        }
        for p in &r.open {
            row(r, "finding", &p.message, page_name(project, p.page).unwrap_or_default());
        }
        for w in &r.waived {
            row(
                r,
                "waived",
                &format!("{} (reason: {})", w.problem.message, w.reason),
                page_name(project, w.problem.page).unwrap_or_default(),
            );
        }
        for u in &r.unknown {
            row(r, "not evaluated", &u.why, String::new());
        }
    }
    csv
}
