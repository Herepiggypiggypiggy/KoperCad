//! The arrange tools of the Cabinet (Autoplace on rail, Auto rails, Auto
//! ducts, Autogroup, Autodistance): they run the rules of
//! `novo_layout::arrange` on the selected devices (all of them when nothing
//! is selected) and show the result as a preview with the same Accept and
//! Cancel as Place all. Accepting is one undo step named after the tool.
//!
//! Where the user finds them: the Arrange entry of the Cabinet rail, Ctrl K
//! ("autodistance", "autogroup", ...), and the right-click menu of the plate.

use std::collections::{BTreeMap, HashSet};

use novo_core::{Project, StructureId};
use novo_layout::arrange::{self, Spacing};
use novo_layout::plan::{Candidate, Plan};
use novo_layout::regroup::{Goal, Inputs, Link, MAX_NET_DEVICES, regroup};
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
    "cabinet.arrange_group_wiring",
    "cabinet.arrange_group_heat",
    "cabinet.arrange_group_both",
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

/// The goal of an Autogroup-across-rows command.
fn regroup_goal(tool: &str) -> Option<Goal> {
    match tool {
        "cabinet.arrange_group_wiring" => Some(Goal::Wiring),
        "cabinet.arrange_group_heat" => Some(Goal::Heat),
        "cabinet.arrange_group_both" => Some(Goal::Both),
        _ => None,
    }
}

/// Which devices share wires, from the netlist: every net that touches two
/// or more devices of the enclosure counts for each pair of them, `1 / (k - 1)`
/// for a net of `k` devices. A net that touches more than
/// [`MAX_NET_DEVICES`] devices (PE, 0 V, a supply bar) says nothing about
/// which two belong together and is left out.
pub(super) fn wiring_links(project: &Project, devices: &HashSet<u64>) -> Vec<Link> {
    let netlist = project.netlist();
    let mut weights: BTreeMap<(u64, u64), f64> = BTreeMap::new();
    for net in netlist.nets() {
        let on: Vec<u64> = netlist
            .devices_on_net(project, net.id)
            .into_iter()
            .map(|d| d.raw())
            .filter(|d| devices.contains(d))
            .collect();
        if on.len() < 2 || on.len() > MAX_NET_DEVICES {
            continue;
        }
        let w = 1.0 / (on.len() - 1) as f64;
        for (i, &a) in on.iter().enumerate() {
            for &b in &on[i + 1..] {
                *weights.entry((a.min(b), a.max(b))).or_insert(0.0) += w;
            }
        }
    }
    weights
        .into_iter()
        .map(|((a, b), weight)| Link { a, b, weight })
        .collect()
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
    /// The plan of Autogroup by wiring and/or heat: reads the part data
    /// (losses, limits), the rule set and the netlist of the project.
    fn regroup_plan(
        &mut self,
        goal: Goal,
        structure: Option<StructureId>,
        ids: &[u64],
        layout: &CabinetLayout,
    ) -> Option<Plan> {
        let structure = structure?;
        self.cabinet.rules.ensure_loaded();
        let project = self.backend.document()?.project();
        let specs = novo_layout::rules::model::specs(project, structure);
        let on_plate: HashSet<u64> = layout.on_plate().iter().map(|p| p.id).collect();
        let links = if goal == Goal::Heat {
            Vec::new()
        } else {
            wiring_links(project, &on_plate)
        };
        let (mut plan, _) = regroup(
            layout,
            ids,
            goal,
            &Inputs {
                specs: &specs,
                links: &links,
                rules: &self.cabinet.rules.rules,
            },
        );
        plan.notes.push(
            "A search, not a proof: it never scores worse than the layout now, and Undo takes it \
             back."
                .into(),
        );
        Some(plan)
    }

    /// Runs an arrange tool and puts its plan up as the preview, or says why
    /// there is nothing to do.
    pub(super) fn arrange_preview(
        &mut self,
        tool: &str,
        structure: Option<StructureId>,
        ids: &[u64],
        layout: &CabinetLayout,
        cands: &[Candidate],
    ) {
        let plan = if let Some(goal) = regroup_goal(tool) {
            let Some(plan) = self.regroup_plan(goal, structure, ids, layout) else {
                return;
            };
            plan
        } else {
            let Some(plan) = plan_for(tool, ids, layout, cands) else {
                return;
            };
            plan
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
