//! Waivers: a problem the designer accepts, with the reason why. A waiver is
//! a [`Dismissal`] named after the problem's kind and about the objects it
//! marks, with the reason in its `reason` property. It goes away with any of
//! those objects, and with it the waiver; if the problem changes (another
//! object is involved) it is a new problem and is shown again.

use novo_core::{AnyId, Dismissal, DismissalId, Project, Result, Tx, Value};

use crate::{Problem, check};

/// The property holding a waiver's reason.
pub const REASON: &str = "reason";

/// A problem together with the reason it was waived.
#[derive(Debug, Clone, PartialEq)]
pub struct Waived {
    pub problem: Problem,
    pub reason: String,
    pub waiver: DismissalId,
}

fn subjects(problem: &Problem) -> Vec<AnyId> {
    let mut at = problem.at.clone();
    at.sort();
    at.dedup();
    at
}

/// The waiver of a problem, if it has one.
pub fn waiver_of<'a>(project: &'a Project, problem: &Problem) -> Option<&'a Dismissal> {
    project.dismissal(&problem.waiver_key(), &subjects(problem))
}

/// Waives a problem with a reason. Waiving it again replaces the reason.
pub fn waive(tx: &mut Tx<'_>, problem: &Problem, reason: &str) -> Result<DismissalId> {
    let id = tx.dismiss(problem.waiver_key(), subjects(problem))?;
    tx.set_property(id, REASON, Some(Value::Text(reason.trim().to_owned())))?;
    Ok(id)
}

/// Splits checked problems into the open ones and the waived ones. For an
/// application that adds problems found outside this crate (their input lives
/// elsewhere) to the list from [`check`](crate::check).
pub fn split_waived(project: &Project, problems: Vec<Problem>) -> (Vec<Problem>, Vec<Waived>) {
    let mut open = Vec::new();
    let mut waived = Vec::new();
    for problem in problems {
        let Some(d) = waiver_of(project, &problem) else {
            open.push(problem);
            continue;
        };
        let reason = match d.properties.get(REASON) {
            Some(Value::Text(t)) => t.clone(),
            _ => String::new(),
        };
        waived.push(Waived {
            reason,
            waiver: d.id,
            problem,
        });
    }
    (open, waived)
}

/// The problems that have not been waived.
pub fn check_open(project: &Project) -> Vec<Problem> {
    split_waived(project, check(project)).0
}

/// The waived problems with their reasons, in reading order.
pub fn waived_problems(project: &Project) -> Vec<Waived> {
    split_waived(project, check(project)).1
}

/// The design checks as lines of text for a printed report: a headline, the
/// open problems by severity with their page, then the waived ones with
/// their reasons. `(bold, text)` per line. Always says something, so a
/// clean project prints "No issues".
pub fn report_lines(project: &Project) -> Vec<(bool, String)> {
    report_lines_of(project, check(project))
}

/// [`report_lines`] for an already checked list of problems.
pub fn report_lines_of(project: &Project, problems: Vec<Problem>) -> Vec<(bool, String)> {
    let page_of = |p: &Problem| {
        p.page
            .and_then(|id| project.page(id))
            .map(|page| format!("  (page {})", page.name))
            .unwrap_or_default()
    };
    let (open, waived) = split_waived(project, problems);
    let mut lines = vec![
        (true, "Design checks".to_owned()),
        (false, crate::Summary::of(&open).pill_text()),
    ];
    for severity in [
        crate::Severity::Error,
        crate::Severity::Warning,
        crate::Severity::Info,
    ] {
        let of: Vec<&Problem> = open.iter().filter(|p| p.severity == severity).collect();
        if of.is_empty() {
            continue;
        }
        lines.push((false, String::new()));
        let name = match severity {
            crate::Severity::Error => "Errors",
            crate::Severity::Warning => "Warnings",
            crate::Severity::Info => "Information",
        };
        lines.push((true, format!("{name} ({})", of.len())));
        for p in of {
            lines.push((
                false,
                format!("{}: {}{}", p.kind.label(), p.message, page_of(p)),
            ));
        }
    }
    if !waived.is_empty() {
        lines.push((false, String::new()));
        lines.push((true, format!("Waived ({})", waived.len())));
        for w in &waived {
            lines.push((
                false,
                format!(
                    "{}: {}{}",
                    w.problem.kind.label(),
                    w.problem.message,
                    page_of(&w.problem)
                ),
            ));
            lines.push((false, format!("    Reason: {}", w.reason)));
        }
    }
    lines
}
