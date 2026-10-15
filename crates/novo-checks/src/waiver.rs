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
    project.dismissal(problem.kind.key(), &subjects(problem))
}

/// Waives a problem with a reason. Waiving it again replaces the reason.
pub fn waive(tx: &mut Tx<'_>, problem: &Problem, reason: &str) -> Result<DismissalId> {
    let id = tx.dismiss(problem.kind.key(), subjects(problem))?;
    tx.set_property(id, REASON, Some(Value::Text(reason.trim().to_owned())))?;
    Ok(id)
}

/// The problems that have not been waived.
pub fn check_open(project: &Project) -> Vec<Problem> {
    let mut problems = check(project);
    problems.retain(|p| waiver_of(project, p).is_none());
    problems
}

/// The waived problems with their reasons, in reading order.
pub fn waived_problems(project: &Project) -> Vec<Waived> {
    check(project)
        .into_iter()
        .filter_map(|problem| {
            let d = waiver_of(project, &problem)?;
            let reason = match d.properties.get(REASON) {
                Some(Value::Text(t)) => t.clone(),
                _ => String::new(),
            };
            Some(Waived {
                problem,
                reason,
                waiver: d.id,
            })
        })
        .collect()
}
