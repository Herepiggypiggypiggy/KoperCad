//! The project's issue list as every route reads it: the design checks of
//! `novo-checks` plus the wires that break the project's house wire rules,
//! which only this crate can read. The app (Issues list, marks, PDF report,
//! Ask Claude), `kopercad-cli check` and any other consumer use these
//! functions so they all show the same problems, waivers included.

use novo_checks::{Problem, ProblemKind, Waived};
use novo_core::{AnyId, Project};

use crate::autopilot::rule_breaks;

/// Every problem, waived or not: the design checks and the house-rule wire
/// breaks (under House rules only, see [`rule_breaks`]).
pub fn all(project: &Project) -> Vec<Problem> {
    let mut problems = novo_checks::check(project);
    problems.extend(rule_breaks(project).into_iter().map(|b| {
        let mut problem = Problem::new(ProblemKind::WireRule, vec![AnyId::Wire(b.wire)], b.message);
        problem.page = project.wire(b.wire).map(|w| w.page);
        problem
    }));
    problems
}

/// The problems that have not been waived.
pub fn open(project: &Project) -> Vec<Problem> {
    novo_checks::split_waived(project, all(project)).0
}

/// The waived problems with their reasons.
pub fn waived(project: &Project) -> Vec<Waived> {
    novo_checks::split_waived(project, all(project)).1
}

/// The issue list as lines of text for a printed report; see
/// [`novo_checks::report_lines`].
pub fn report_lines(project: &Project) -> Vec<(bool, String)> {
    novo_checks::report_lines_of(project, all(project))
}
