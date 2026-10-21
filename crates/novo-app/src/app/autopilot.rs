//! Autopilot window: Edit > Autopilot, and the "Autopilot" button in the
//! Bench output's "Before you print" list.
//!
//! Shows what the program would fill in (sizes, colours, wire numbers), each
//! with the reason from the calculations, so the person accepts row by row or
//! all at once. Nothing is written until then, and what is written is one
//! undo step. The engine is `novo_reports::autopilot`.

use std::collections::BTreeSet;

use eframe::egui;
use novo_checks::calc::tables::TableSet;
use novo_numbering::{Mode, Reason, Scheme, Scope};
use novo_reports::autopilot::{self, Autopilot, Basis, Change, Proposal, Ready};
use novo_reports::{FerruleStandard, wire_cut_jobs_with};

use super::{Level, NovoApp};

/// The window's state: the plan being reviewed and which rows are ticked.
pub(super) struct AutopilotState {
    plan: Autopilot,
    ready: Ready,
    ticked: BTreeSet<usize>,
}

/// The rows ticked when the list opens: everything but the assumed ones,
/// which a person has to tick on purpose.
pub(super) fn default_ticked(plan: &Autopilot) -> BTreeSet<usize> {
    plan.proposals
        .iter()
        .filter(|p| p.basis != Basis::Assumed)
        .map(|p| p.id)
        .collect()
}

/// Builds the plan for the project: sizes and colours from the calculations,
/// wire numbers from the project's numbering scheme.
pub(super) fn build(project: &novo_core::Project, has_plate: bool) -> (Autopilot, Ready) {
    let standard = FerruleStandard::of(project);
    let jobs = wire_cut_jobs_with(project, &standard);
    let mut plan = autopilot::plan(project, &TableSet::builtin(), &jobs);
    let scheme = Scheme::load(project);
    let numbers = novo_numbering::plan(project, &scheme, Mode::Fill, &Scope::Project);
    let extra: Vec<Proposal> = numbers
        .changed_groups()
        .filter_map(|g| {
            let new = g.new.clone()?;
            let potential = g
                .potential
                .as_deref()
                .map(|p| format!(" on {p}"))
                .unwrap_or_default();
            let why = match g.reason {
                Reason::New => format!(
                    "no number yet: the next free number of your {} scheme",
                    scheme.kind.label().to_lowercase()
                ),
                Reason::Healed => {
                    "the wires of this net carry different numbers: they follow the rest".to_owned()
                }
                Reason::Replaced => "the old number was used twice or is off the scheme".to_owned(),
                _ => "follows your numbering scheme".to_owned(),
            };
            Some(Proposal {
                id: 0,
                wires: g.wires.clone(),
                label: format!("{}{potential}", g.at),
                change: Change::Number(new),
                reason: why,
                basis: Basis::Rule,
            })
        })
        .collect();
    plan.extend(extra);
    let ready = autopilot::ready(&jobs, has_plate);
    (plan, ready)
}

impl NovoApp {
    /// Opens the Autopilot review list on the current drawing.
    pub(super) fn open_autopilot(&mut self) {
        let has_plate = self.view3d_has_cabinet();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let (plan, ready) = build(project, has_plate);
        let ticked = default_ticked(&plan);
        self.wb.autopilot = Some(AutopilotState {
            plan,
            ready,
            ticked,
        });
    }

    /// Writes the proposals with these ids as one undo step.
    pub(super) fn apply_autopilot(&mut self, plan: &Autopilot, ids: Option<&BTreeSet<usize>>) {
        let mut counts = (0, 0, 0, 0, 0);
        let unverified = plan.proposals.iter().any(|p| {
            ids.is_none_or(|set| set.contains(&p.id))
                && matches!(p.change, Change::Size(_))
                && matches!(p.basis, Basis::Planning | Basis::Assumed)
        });
        if !self.edit_project("Autopilot: complete the wires", |tx| {
            counts = plan.apply(tx, ids)?;
            Ok(())
        }) {
            return;
        }
        let (sized, coloured, numbered, cabled, entries) = counts;
        let mut parts = Vec::new();
        if sized > 0 {
            parts.push(if unverified {
                format!("{sized} sizes (planning values, check them)")
            } else {
                format!("{sized} sizes")
            });
        }
        if coloured > 0 {
            parts.push(format!("{coloured} colours"));
        }
        if numbered > 0 {
            parts.push(format!("{numbered} wire numbers"));
        }
        if cabled > 0 {
            parts.push(format!("{cabled} cables"));
        }
        if entries > 0 {
            parts.push(if entries == 1 {
                "1 cabinet entry point".to_owned()
            } else {
                format!("{entries} cabinet entry points")
            });
        }
        let text = if parts.is_empty() {
            "Autopilot had nothing to fill in".to_owned()
        } else {
            format!(
                "Autopilot filled in {} (Undo takes them back in one step)",
                parts.join(", ")
            )
        };
        self.message(Level::Info, text);
    }

    pub(super) fn autopilot_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.wb.autopilot.take() else {
            return;
        };
        let has_plate = self.view3d_has_cabinet();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        if state.plan.revision() != project.revision() {
            // The drawing changed under the list (an undo, an edit): plan again.
            let (plan, ready) = build(project, has_plate);
            state.ticked = default_ticked(&plan);
            state.plan = plan;
            state.ready = ready;
        }
        let mut open = true;
        let mut apply: Option<(BTreeSet<usize>, bool)> = None;
        super::panel::dialog(ctx, "Autopilot", [620.0, 640.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                body(ui, &mut state, &mut apply);
            });
        if let Some((ids, finish)) = apply {
            // Accept all is the end of the job: close, so Bench output behind
            // it shows the new meter. Accepting some rows keeps the list.
            if finish {
                open = false;
            }
            let plan = state.plan.clone();
            self.apply_autopilot(&plan, Some(&ids));
            // The list is re-planned on the next frame, from the new drawing.
        }
        if open {
            self.wb.autopilot = Some(state);
        }
    }
}

/// The meter used by the Bench output and this window.
pub(super) fn meter(ui: &mut egui::Ui, ready: Ready) {
    let fraction = ready.done as f32 / ready.total.max(1) as f32;
    novo_theme::widgets::progress_bar(
        ui,
        "Wires ready to build",
        &format!("{} %", ready.percent()),
        fraction,
        0.0,
    )
    .on_hover_text(
        "Per wire: a number, a cross-section, a colour and a routed length; plus a mounting plate. Estimated lengths do not count.",
    );
}

fn body(
    ui: &mut egui::Ui,
    state: &mut AutopilotState,
    apply: &mut Option<(BTreeSet<usize>, bool)>,
) {
    let pal = crate::theme::ui_palette();
    meter(ui, state.ready);
    ui.add_space(6.0);
    let total = state.plan.proposals.len();
    if total == 0 {
        novo_theme::widgets::banner(
            ui,
            novo_theme::widgets::Level::Success,
            "Nothing left to fill in: every wire has a size, a colour and a number",
            |_| {},
        );
    } else {
        ui.label(format!(
            "{} sizes, {} colours, {} wire numbers, {} cables and {} cabinet entry points can be filled in from the drawing. Nothing is written until you accept; one Undo takes it all back.",
            state.plan.count("Size"),
            state.plan.count("Colour"),
            state.plan.count("Number"),
            state.plan.count("Cable"),
            state.plan.count("Entry point"),
        ));
    }
    if state.plan.planning_values && state.plan.count("Size") > 0 {
        ui.add_space(4.0);
        novo_theme::widgets::banner(
            ui,
            novo_theme::widgets::Level::Warning,
            "Sizes use planning values: the built-in ampacity tables are not checked against the standard. Read the clauses in your licensed copy before relying on a number.",
            |_| {},
        );
    }
    if state
        .plan
        .proposals
        .iter()
        .any(|p| p.reason.contains("IEC "))
    {
        ui.add_space(4.0);
        novo_theme::widgets::banner(
            ui,
            novo_theme::widgets::Level::Info,
            "Unverified: the IEC clause numbers in the reasons are not checked against a licensed copy. Check them in your copy of the standard.",
            |_| {},
        );
    }
    for line in &state.plan.left_to_you {
        ui.add_space(4.0);
        novo_theme::widgets::banner(ui, novo_theme::widgets::Level::Info, line, |_| {});
    }
    if !state.plan.unresolved.is_empty() {
        ui.add_space(4.0);
        novo_theme::widgets::section_label(ui, "Needs your decision");
        for line in &state.plan.unresolved {
            ui.label(line);
        }
    }
    if total > 0 {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let ticked = state.ticked.len();
            let sure = default_ticked(&state.plan);
            let label = if sure.len() < total {
                format!("Accept all except assumed ({})", sure.len())
            } else {
                format!("Accept all ({total})")
            };
            let go = ui
                .add_enabled_ui(!sure.is_empty(), |ui| {
                    novo_theme::widgets::primary_button(ui, &label).clicked()
                })
                .inner;
            if go {
                *apply = Some((sure, true));
            }
            if ui
                .add_enabled(
                    ticked > 0,
                    egui::Button::new(format!("Accept ticked ({ticked})")),
                )
                .clicked()
            {
                *apply = Some((state.ticked.clone(), false));
            }
            if ui.small_button("Tick none").clicked() {
                state.ticked.clear();
            }
            if ui.small_button("Tick all").clicked() {
                state.ticked = state.plan.proposals.iter().map(|p| p.id).collect();
            }
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for p in &state.plan.proposals {
                let mut on = state.ticked.contains(&p.id);
                ui.horizontal_wrapped(|ui| {
                    if ui.checkbox(&mut on, "").changed() {
                        if on {
                            state.ticked.insert(p.id);
                        } else {
                            state.ticked.remove(&p.id);
                        }
                    }
                    ui.strong(format!("{}  {}", p.change.kind(), p.change.value()));
                    ui.label(egui::RichText::new(&p.label).color(pal.text_secondary));
                    let tone = match p.basis {
                        Basis::Rule | Basis::Calculated => pal.success,
                        Basis::Planning | Basis::Assumed => pal.warning,
                    };
                    ui.label(egui::RichText::new(p.basis.label()).small().color(tone));
                });
                ui.indent(("autopilot-reason", p.id), |ui| {
                    ui.label(
                        egui::RichText::new(&p.reason)
                            .small()
                            .color(pal.text_secondary),
                    );
                });
                ui.add_space(2.0);
            }
        });
    }
}
