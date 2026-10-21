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
use novo_core::Value;
use novo_numbering::{Mode, Reason, Scheme, Scope};
use novo_reports::autopilot::{self, Autopilot, Basis, Change, Group, Proposal, Ready};
use novo_reports::wire_rules::{self, ColourClass, Preset, WireRules};
use novo_reports::{FerruleStandard, wire_cut_jobs_with};

use super::{Level, NovoApp};

/// The window's state: the plan being reviewed and which rows are ticked.
pub(super) struct AutopilotState {
    plan: Autopilot,
    ready: Ready,
    ticked: BTreeSet<usize>,
    /// The plan grouped by protective device.
    groups: Vec<Group>,
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
        let groups = autopilot::groups(project, &plan);
        self.wb.autopilot = Some(AutopilotState {
            plan,
            ready,
            ticked,
            groups,
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
            state.groups = autopilot::groups(project, &plan);
            state.plan = plan;
            state.ready = ready;
        }
        let mut open = true;
        let mut apply: Option<(BTreeSet<usize>, bool)> = None;
        let mut rule_edits: Vec<(String, Option<Value>)> = Vec::new();
        super::panel::dialog(ctx, "Autopilot", [620.0, 640.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                rules_section(ui, &WireRules::of(project), &mut rule_edits);
                body(ui, &mut state, &mut apply);
            });
        if !rule_edits.is_empty() {
            self.edit_project("Wire rules", |tx| {
                for (key, value) in &rule_edits {
                    novo_checks::standards::set_info_property(tx, key, value.clone())?;
                }
                Ok(())
            });
        }
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
    .on_hover_text(ready.explanation());
}

fn body(
    ui: &mut egui::Ui,
    state: &mut AutopilotState,
    apply: &mut Option<(BTreeSet<usize>, bool)>,
) {
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
            "{} sizes, {} colours, {} wire numbers, {} cables and {} can be filled in from the drawing. Nothing is written until you accept; one Undo takes it all back.",
            state.plan.count("Size"),
            state.plan.count("Colour"),
            state.plan.count("Number"),
            state.plan.count("Cable"),
            match state.plan.count("Entry point") {
                1 => "1 cabinet entry point".to_owned(),
                n => format!("{n} cabinet entry points"),
            },
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
        novo_theme::widgets::banner_wrapped(ui, novo_theme::widgets::Level::Info, line);
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
        egui::ScrollArea::vertical()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                let groups = state.groups.clone();
                for g in &groups {
                    group_view(ui, state, g, apply);
                }
            });
    }
}

/// One protective device (or "Cabinet") with its proposals.
fn group_view(
    ui: &mut egui::Ui,
    state: &mut AutopilotState,
    g: &Group,
    apply: &mut Option<(BTreeSet<usize>, bool)>,
) {
    let pal = crate::theme::ui_palette();
    let ticked = g.ids.iter().filter(|i| state.ticked.contains(i)).count();
    let header = format!(
        "{}  ·  {} {}  ·  {}",
        g.title,
        g.ids.len(),
        if g.ids.len() == 1 {
            "change"
        } else {
            "changes"
        },
        g.summary
    );
    egui::CollapsingHeader::new(egui::RichText::new(header).strong())
        .id_salt(("autopilot-group", &g.title))
        .default_open(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        ticked > 0,
                        egui::Button::new(format!("Accept ticked in this group ({ticked})")),
                    )
                    .clicked()
                {
                    *apply = Some((
                        g.ids
                            .iter()
                            .copied()
                            .filter(|i| state.ticked.contains(i))
                            .collect(),
                        false,
                    ));
                }
                if ui.small_button("Tick group").clicked() {
                    state.ticked.extend(g.ids.iter().copied());
                }
                if ui.small_button("Untick group").clicked() {
                    for i in &g.ids {
                        state.ticked.remove(i);
                    }
                }
            });
            for p in state
                .plan
                .proposals
                .iter()
                .filter(|p| g.ids.contains(&p.id))
            {
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

/// What a blank minimum means: the program's own smallest size.
fn default_min(control: bool) -> String {
    if control {
        "program default (0.75 mm²)".to_owned()
    } else {
        "program default (1.5 mm²)".to_owned()
    }
}

/// Smallest sections a house rule can name, mm².
const MIN_CHOICES: [f64; 6] = [0.5, 0.75, 1.0, 1.5, 2.5, 4.0];

/// The "Wire rules" part of the window: which colour and minimum-size rules
/// the Autopilot follows in this project. Stored in the project, one undo
/// step per change.
fn rules_section(ui: &mut egui::Ui, rules: &WireRules, edits: &mut Vec<(String, Option<Value>)>) {
    let title = format!("Wire rules: {}", rules.preset.label());
    egui::CollapsingHeader::new(egui::RichText::new(title).strong())
        .id_salt("autopilot-wire-rules")
        .default_open(false)
        .show(ui, |ui| {
            ui.label("Colours and smallest sections the Autopilot proposes. IEC 60204-1 is the default; House rules let this project use its own. Rules only change what the Autopilot proposes from now on; wires that already have a colour or size are not touched, and the ones that break the house rules are listed in Issues. A wire's class (power, control, neutral, ...) is worked out from its circuit; a wire whose class is a guess is not reported, and protective earth and PEN colours are not checked. Ampacity sizing stays on planning values either way.");
            ui.horizontal(|ui| {
                for preset in [Preset::Iec60204, Preset::House] {
                    if ui
                        .radio(rules.preset == preset, preset.label())
                        .clicked()
                        && rules.preset != preset
                    {
                        let text = if preset == Preset::House { "house" } else { "iec60204" };
                        edits.push((
                            wire_rules::PRESET.to_owned(),
                            Some(Value::Text(text.to_owned())),
                        ));
                    }
                }
            });
            if rules.preset != Preset::House {
                return;
            }
            ui.weak("A class left on \"IEC 60204-1\" keeps the colour of the standard. Protective earth and PEN are always green-yellow (IEC 60204-1, IEC 60445): a house rule cannot change them.");
            egui::Grid::new("autopilot-house-colours")
                .num_columns(4)
                .show(ui, |ui| {
                    for (n, class) in ColourClass::EDITABLE.into_iter().enumerate() {
                        ui.label(class.label());
                        let current = rules.house_colour(class).map(str::to_owned);
                        let mut pick = current.clone();
                        egui::ComboBox::from_id_salt(("house-colour", class.key()))
                            .selected_text(pick.clone().unwrap_or_else(|| "IEC 60204-1".into()))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut pick, None, "IEC 60204-1");
                                for code in wire_rules::CODES {
                                    ui.selectable_value(&mut pick, Some(code.to_owned()), code);
                                }
                            });
                        if pick != current {
                            edits.push((class.key(), pick.map(Value::Text)));
                        }
                        if n % 2 == 1 {
                            ui.end_row();
                        }
                    }
                    for (label, key, control) in [
                        ("Smallest power wire", wire_rules::MIN_POWER_MM2, false),
                        ("Smallest control wire", wire_rules::MIN_CONTROL_MM2, true),
                    ] {
                        ui.label(label);
                        let current = rules.min_mm2(control);
                        let mut pick = current;
                        egui::ComboBox::from_id_salt(("house-min", key))
                            .selected_text(match pick {
                                Some(v) => format!("{v} mm²"),
                                None => default_min(control),
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut pick, None, default_min(control));
                                for v in MIN_CHOICES {
                                    ui.selectable_value(&mut pick, Some(v), format!("{v} mm²"));
                                }
                            });
                        if pick != current {
                            edits.push((key.to_owned(), pick.map(Value::Float)));
                        }
                    }
                    ui.end_row();
                });
        });
}
