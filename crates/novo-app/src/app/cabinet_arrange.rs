//! The arrange tools of the Cabinet (Autoplace on rail, Auto rails, Auto
//! ducts, Autogroup, Autodistance): they run the rules of
//! `novo_layout::arrange` on the selected devices (all of them when nothing
//! is selected) and show the result as a preview with the same Accept and
//! Cancel as Place all. Accepting is one undo step named after the tool.
//!
//! Where the user finds them: the Arrange entry of the Cabinet rail, Ctrl K
//! ("autodistance", "autogroup", ...), and the right-click menu of the plate.

use novo_layout::arrange::{self, Spacing};
use novo_layout::plan::{Candidate, Plan};
use novo_layout::state::CabinetLayout;

use super::NovoApp;
use crate::app::Level;

/// The command id as a static string, for the tools this module runs.
pub(super) fn tool_id(id: &str) -> Option<&'static str> {
    TOOLS.iter().copied().find(|t| *t == id)
}

/// The arrange commands, in the order the flyout lists them.
pub(super) const TOOLS: &[&str] = &[
    "cabinet.arrange_onto_rail",
    "cabinet.arrange_rails",
    "cabinet.arrange_ducts",
    "cabinet.arrange_group",
    "cabinet.arrange_tight",
    "cabinet.arrange_even",
];

/// The plan an arrange command makes for `ids` (empty: everything).
pub(super) fn plan_for(
    tool: &str,
    ids: &[u64],
    layout: &CabinetLayout,
    cands: &[Candidate],
) -> Option<Plan> {
    let keys = cands.iter().map(|c| (c.id, c.sort_key.clone())).collect();
    Some(match tool {
        "cabinet.arrange_onto_rail" => arrange::onto_rails(layout, ids, cands),
        "cabinet.arrange_rails" => arrange::make_rails(layout, ids),
        "cabinet.arrange_ducts" => arrange::add_ducts(layout),
        "cabinet.arrange_group" => arrange::group(layout, ids, &keys),
        "cabinet.arrange_tight" => arrange::distribute(layout, ids, Spacing::Tight),
        "cabinet.arrange_even" => arrange::distribute(layout, ids, Spacing::Even),
        _ => return None,
    })
}

/// True when the plan changes something.
fn changes(plan: &Plan) -> bool {
    !plan.moves.is_empty() || !plan.rails.is_empty() || !plan.ducts.is_empty()
}

/// The preview banner of an arrange plan; `None` for a Place all plan.
pub(super) fn banner(plan: &Plan) -> Option<String> {
    let title = plan.title.as_deref()?;
    let n = plan.moves.len();
    let mut parts = Vec::new();
    if n > 0 {
        parts.push(format!(
            "{n} {} would move or mount",
            if n == 1 { "device" } else { "devices" }
        ));
    }
    if !plan.rails.is_empty() {
        parts.push(format!("{} rail(s) would be added", plan.rails.len()));
    }
    if !plan.ducts.is_empty() {
        parts.push(format!("{} duct(s) would be added", plan.ducts.len()));
    }
    let mut text = format!(
        "Preview ({title}): {}. Nothing is stored until you accept; Undo takes it back in one step. \
         A planning aid by simple rules, not a clearance check or a heat calculation.",
        parts.join(", ")
    );
    for note in &plan.notes {
        text.push(' ');
        text.push_str(note);
    }
    Some(text)
}

impl NovoApp {
    /// Runs an arrange tool and puts its plan up as the preview, or says why
    /// there is nothing to do.
    pub(super) fn arrange_preview(
        &mut self,
        tool: &str,
        ids: &[u64],
        layout: &CabinetLayout,
        cands: &[Candidate],
    ) {
        let Some(plan) = plan_for(tool, ids, layout, cands) else {
            return;
        };
        if changes(&plan) {
            self.cabinet.preview = Some(plan);
            return;
        }
        let text = if plan.notes.is_empty() {
            "Nothing to change: the devices already stand as this tool would put them.".to_owned()
        } else {
            plan.notes.join(" ")
        };
        self.message(Level::Info, text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_a_command_the_app_runs() {
        for id in TOOLS {
            assert!(novo_workflow::commands::is_known(id), "{id}");
            assert!(super::super::workbench::app_runs(id), "{id}");
            assert_eq!(tool_id(id), Some(*id));
        }
    }
}
