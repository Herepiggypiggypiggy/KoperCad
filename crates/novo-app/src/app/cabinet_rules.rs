//! Place rules and the rail and duct cut list in the Cabinet view.
//!
//! `Place all` follows the user's rules (`place-rules.txt` in the KoperCAD
//! folder under `%APPDATA%`, no admin rights; the standard rules when there is
//! none). The "Rules" window edits that file, the inspector says which rule
//! put a device where it is and which rules a hand edit broke, and the "Cut
//! list" window turns the rails and ducts into bars of stock to buy. The
//! numbers are practice values, not a standard: see
//! `docs/design/cabinet-layout.md` section 5b.1.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_core::{Project, StructureId};
use novo_layout::check::Violation;
use novo_layout::model::{ducts, load};
use novo_layout::plan::{Plan, ROW_GAP_MM};
use novo_layout::rules::{Explanation, RuleSet, Spec, place_all_around};
use novo_layout::state::CabinetLayout;
use novo_layout::stock::{CutPlan, Piece, Stock, optimise, rail_pieces};

use super::cabinet::Ask;
use super::cabinet_inspector::Facts;

/// File name inside the KoperCAD folder under `%APPDATA%`.
pub const RULES_FILE: &str = "place-rules.txt";

/// `%APPDATA%\KoperCAD\place-rules.txt` (the platform's data folder elsewhere).
pub fn rules_path() -> Option<PathBuf> {
    Some(
        novo_storage::profile::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(RULES_FILE),
    )
}

/// The rules in the file, and the reason when the file could not be used (the
/// standard rules are then used). A missing file is the standard rules.
pub fn load_rules(path: &Path) -> (RuleSet, Option<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (RuleSet::standard(), None);
    };
    match RuleSet::parse(&text) {
        Ok(rules) => (rules, None),
        Err(e) => (
            RuleSet::standard(),
            Some(format!("{RULES_FILE} {e}; the standard rules are used")),
        ),
    }
}

/// Writes the rule text to `path`, creating the folder.
pub fn save_rules(text: &str, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, text)
}

/// Rules, the last explanation and the two windows.
pub(super) struct RulesState {
    pub(super) rules: RuleSet,
    path: Option<PathBuf>,
    loaded: bool,
    /// Why the last `Place all` put things where it did.
    pub(super) explain: Option<Explanation>,
    pub(super) editor_open: bool,
    text: String,
    /// The rule text last written to (or read from) the file: what "Save"
    /// would not change.
    saved: String,
    /// Earlier rule sets, newest last, for Undo in the panel.
    history: Vec<RuleSet>,
    /// When the last edit was made, to fold a drag into one undo step.
    last_edit: f64,
    /// One line for the editor: what the last save or check did.
    notice: Option<String>,
    pub(super) cut_open: bool,
}

impl RulesState {
    pub(super) fn new() -> Self {
        Self {
            rules: RuleSet::standard(),
            path: rules_path(),
            loaded: false,
            explain: None,
            editor_open: false,
            text: String::new(),
            saved: String::new(),
            history: Vec::new(),
            last_edit: f64::NEG_INFINITY,
            notice: None,
            cut_open: false,
        }
    }

    /// Reads the rules file once, when first needed.
    pub(super) fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Some(path) = &self.path {
            let (rules, problem) = load_rules(path);
            self.rules = rules;
            self.notice = problem;
        }
        self.text = self.rules.to_text();
        self.saved = self.text.clone();
    }
}

/// The devices not yet placed, sorted by the structure's surface rules
/// (`surface_rules` property): `plate` stays here, `others` belong on the
/// door or another surface. Without an enclosure there is no other surface,
/// so everything stays on the plate and the problems say so.
pub(super) fn surface_split(
    project: &Project,
    structure: StructureId,
    layout: &CabinetLayout,
) -> novo_layout::surface_rules::Split {
    use novo_layout::surface_rules::{Split, split};
    let specs: Vec<Spec> = novo_layout::rules::model::specs(project, structure)
        .into_iter()
        .filter(|s| !layout.placements.iter().any(|p| p.id == s.id))
        .collect();
    let (rules, mut problems) = novo_layout::rules::model::surface_rules(project, structure);
    let Some(enclosure) = layout.enclosure.as_ref() else {
        if !rules.standard && !rules.rules.is_empty() {
            problems.push(
                "Surface rules need the inside size of the enclosure; every device stays on the plate"
                    .to_owned(),
            );
        } else if rules.standard {
            let wanting = specs
                .iter()
                .filter(|s| {
                    s.surface
                        .as_deref()
                        .or_else(|| rules.surface_for(&s.kind))
                        .is_some_and(|name| name != novo_layout::surfaces::MAIN_PLATE)
                })
                .count();
            if wanting > 0 {
                problems.push(format!(
                    "{wanting} pilot {} (push buttons, lamps, meters) belong on the door; \
                     {} stay on the plate until the enclosure has an inside size and a door (Cabinet, Door)",
                    if wanting == 1 { "device" } else { "devices" },
                    if wanting == 1 { "it" } else { "they" },
                ));
            }
        }
        return Split {
            plate: specs,
            others: Vec::new(),
            problems,
        };
    };
    let (surfaces, _) = novo_layout::surfaces::Surfaces::from_enclosure(
        enclosure,
        &layout.plate,
        &novo_layout::model::extra_surfaces(project, structure),
    );
    let mut out = split(&specs, &rules, &surfaces);
    problems.append(&mut out.problems);
    out.problems = problems;
    out
}

/// Plans for the devices the surface rules send to other surfaces, one per
/// surface, in the order of `split.others`. A surface the enclosure does
/// not have gets no plan.
pub(super) fn surface_plans(
    project: &Project,
    structure: StructureId,
    layout: &CabinetLayout,
    split: &novo_layout::surface_rules::Split,
) -> Vec<(String, Plan)> {
    let Some(enclosure) = layout.enclosure.as_ref() else {
        return Vec::new();
    };
    let (surfaces, _) = novo_layout::surfaces::Surfaces::from_enclosure(
        enclosure,
        &layout.plate,
        &novo_layout::model::extra_surfaces(project, structure),
    );
    split
        .others
        .iter()
        .filter_map(|(name, specs)| {
            novo_layout::surface_rules::plan_surface(&surfaces, name, specs)
                .map(|plan| (name.clone(), plan))
        })
        .collect()
}

/// The plan for the devices not yet placed, by the rules, below what is
/// already on the plate; with the reason for every decision.
pub(super) fn plan(
    project: &Project,
    structure: StructureId,
    layout: &CabinetLayout,
    rules: &RuleSet,
) -> (Plan, Explanation) {
    let specs = surface_split(project, structure, layout).plate;
    let top = layout
        .placements
        .iter()
        .map(|p| p.footprint().bottom() + ROW_GAP_MM)
        .fold(0.0, f64::max);
    // Ducts already on the plate are kept clear, not planned over.
    let ducts: Vec<_> = layout.ducts.iter().map(|d| d.footprint()).collect();
    place_all_around(
        &layout.plate,
        &specs,
        rules,
        layout.rails.iter().map(|r| r.id + 1).max().unwrap_or(1),
        top,
        &ducts,
    )
}

/// Largest side offered, mm.
pub(super) const MAX_PLATE_SIDE_MM: f64 = 2500.0;

/// The smallest plate, in the proportions of the current one, on which
/// `Place all` puts every device by the rules. Tried by planning on bigger
/// and bigger copies of the layout, so it is what Place all will really do
/// and not a sum of areas. `None` when the plate already holds everything or
/// no plate up to 2.5 m a side does (a device the rules never place).
pub(super) fn plate_that_holds(
    project: &Project,
    structure: StructureId,
    layout: &CabinetLayout,
    rules: &RuleSet,
) -> Option<novo_core::MountingPlate> {
    if plan(project, structure, layout, rules)
        .0
        .unplaced
        .is_empty()
    {
        return None;
    }
    let round = |side: f64| (side / 50.0).ceil() * 50.0;
    let mut trial = layout.clone();
    let mut last = (layout.plate.width, layout.plate.height);
    for step in 1..=40 {
        let scale = 1.0 + 0.05 * f64::from(step);
        let size = (
            round(layout.plate.width * scale),
            round(layout.plate.height * scale),
        );
        if size == last {
            continue;
        }
        if size.0.max(size.1) > MAX_PLATE_SIDE_MM {
            return None;
        }
        last = size;
        trial.plate.width = size.0;
        trial.plate.height = size.1;
        if plan(project, structure, &trial, rules)
            .0
            .unplaced
            .is_empty()
        {
            return Some(novo_core::MountingPlate {
                width: size.0,
                height: size.1,
                margin: layout.plate.margin,
            });
        }
    }
    None
}

/// What the rules say about the devices on the plate, ready for the
/// inspector.
pub(super) struct Judgement {
    pub violations: Vec<Violation>,
    pub specs: Vec<Spec>,
}

pub(super) fn judge(project: &Project, structure: StructureId, rules: &RuleSet) -> Judgement {
    let specs = novo_layout::rules::model::specs(project, structure);
    let violations = match load(project, structure) {
        Some(layout) => novo_layout::check::check(
            &layout.placements,
            &ducts(project, structure),
            &specs,
            rules,
        ),
        None => Vec::new(),
    };
    Judgement { violations, specs }
}

/// Adds, to every device the inspector shows, the rule that applies to it,
/// why `Place all` put it there, and the rules it breaks now.
pub(super) fn annotate(facts: &mut Facts, state: &RulesState, judged: &Judgement) {
    let Facts::Devices(devices) = facts else {
        return;
    };
    for d in devices {
        if let Some(spec) = judged.specs.iter().find(|s| s.id == d.id) {
            let (index, rule) = state.rules.rule_for_kind(&spec.kind);
            d.rule = Some(match index {
                Some(i) => format!("rule {}", state.rules.rule_text(i)),
                None => format!("no rule for {}, so the middle zone", rule.zone.as_str()),
            });
        }
        d.why = state
            .explain
            .as_ref()
            .filter(|e| e.decision(d.id).is_some())
            .map(|e| e.describe(d.id, &state.rules));
    }
}

/// The Place rules panel: the rules as cards in plain words, dockable at the
/// right. Changes apply to Place all at once and can be undone; "Save" keeps
/// them as the person's defaults. The file as text is under "Edit as text".
pub(super) fn show_rules_window(
    ctx: &egui::Context,
    state: &mut RulesState,
    canvas: egui::Rect,
    surface: Option<(StructureId, &novo_layout::surface_rules::SurfaceRules)>,
) -> Option<super::cabinet::Ask> {
    if !state.editor_open {
        return None;
    }
    let mut ask = None;
    state.ensure_loaded();
    let now = ctx.input(|i| i.time);
    let before = state.rules.clone();
    let mut text_edit = false;
    super::panel::Panel::new(
        "place_rules",
        "Place rules",
        canvas,
        super::panel::Side::Right,
        380.0,
    )
    .closable()
    .dockable()
    .fixed_open()
    .scrolls()
    .height(canvas.height())
    .show(ctx, |ui| {
        ui.set_max_width(356.0);
        if let Some(n) = &state.notice {
            super::dialog_ui::hint(ui, n);
        }
        let dirty = state.rules.to_text() != state.saved;
        ui.horizontal(|ui| {
            let save =
                ui.add_enabled_ui(dirty, |ui| novo_theme::widgets::primary_button(ui, "Save"));
            if !dirty {
                save.response
                    .on_disabled_hover_text("Nothing to save: the rules are already saved");
            }
            if save.inner.clicked() {
                let text = state.rules.to_text();
                state.notice = Some(match &state.path {
                    Some(path) => match save_rules(&text, path) {
                        Ok(()) => {
                            state.saved = text;
                            format!("Saved to {}", path.display())
                        }
                        Err(e) => format!("Could not save: {e}"),
                    },
                    None => "There is no folder to save to; used for this session".into(),
                });
            }
            let can_undo = !state.history.is_empty();
            if ui
                .add_enabled(can_undo, egui::Button::new("Undo"))
                .on_hover_text("Go back one change in this panel")
                .on_disabled_hover_text("Nothing to undo yet in this panel")
                .clicked()
                && let Some(previous) = state.history.pop()
            {
                state.rules = previous;
                state.text = state.rules.to_text();
                state.last_edit = f64::NEG_INFINITY;
            }
            let standard = RuleSet::standard();
            if super::dialog_ui::confirm_button(
                ui,
                "standard_rules",
                "KoperCAD's default rules",
                "Replace all rules with KoperCAD's default rules?",
                state.rules != standard,
            ) {
                state.history.push(state.rules.clone());
                state.rules = standard;
                state.text = state.rules.to_text();
                state.notice = None;
            }
        });
        ui.weak(if dirty {
            "Applies now. Not saved as your default yet."
        } else {
            "Saved as your default."
        });
        ui.separator();
        let (changed, surface_edit) =
            super::cabinet_rules_panel::show(ui, &mut state.rules, surface.map(|(_, r)| r));
        if changed {
            state.text = state.rules.to_text();
        }
        if let (Some((s, _)), Some(text)) = (surface, surface_edit) {
            ask = Some(super::cabinet::Ask::SetSurfaceRules(s, text));
        }
        egui::CollapsingHeader::new("Edit as text")
            .default_open(false)
            .show(ui, |ui| {
                dialog_ui_hint(ui);
                let r = ui.add(
                    egui::TextEdit::multiline(&mut state.text)
                        .code_editor()
                        .desired_rows(10)
                        .desired_width(f32::INFINITY),
                );
                match RuleSet::parse(&state.text) {
                    Ok(rules) => {
                        if r.changed() {
                            state.rules = rules;
                            text_edit = true;
                        }
                    }
                    Err(e) => {
                        ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                    }
                }
            });
    });
    if state.rules != before && state.history.last() != Some(&before) {
        // A drag changes the rules every frame: one undo step per burst.
        if now - state.last_edit > 0.8 {
            state.history.push(before);
            if state.history.len() > 50 {
                state.history.remove(0);
            }
        }
        state.last_edit = now;
    }
    if text_edit {
        state.notice = None;
    }
    if super::panel::take_closed(ctx, "place_rules") {
        state.editor_open = false;
    }
    ask
}

fn dialog_ui_hint(ui: &mut egui::Ui) {
    super::dialog_ui::hint(
        ui,
        "The same rules as a file: one per line. Letter codes, then zone=, rail=, class=, \
         gap=, above=, below= in mm. `heat on` / `heat off`. Edits here apply once the text reads.",
    );
}

/// The bars to buy for the rails and ducts of the plate.
pub(super) struct CutLists {
    pub rails: Option<(CutPlan, Vec<Piece>)>,
    /// One list per duct cross-section; the pieces are the duct rail ids.
    pub ducts: Vec<(f64, f64, CutPlan)>,
    /// The short name of every duct (`D1`): the name the wire production
    /// report and the bench sheets print as a wire's track.
    pub duct_names: std::collections::BTreeMap<u64, String>,
}

pub(super) fn cut_lists(project: &Project, structure: StructureId) -> Option<CutLists> {
    let layout = load(project, structure)?;
    let stock = Stock::default();
    let rails = (!layout.rails.is_empty()).then(|| {
        let pieces = rail_pieces(&layout.rails, &layout.placements);
        (optimise(&pieces, stock), pieces)
    });
    let all = ducts(project, structure);
    // `ducts` lists the plate's duct rails in model order: the same order here.
    let rail_ids: Vec<u64> = project
        .all::<novo_core::Rail>()
        .filter(|r| r.structure == structure && r.kind == novo_core::RailKind::Duct)
        .map(|r| r.id.raw())
        .collect();
    let mut sections: Vec<(f64, f64, Vec<Piece>)> = Vec::new();
    for (n, d) in all.iter().enumerate() {
        let piece = Piece {
            id: rail_ids.get(n).copied().unwrap_or(n as u64 + 1),
            min: d.length,
            max: d.length,
        };
        match sections
            .iter_mut()
            .find(|(w, h, _)| *w == d.width && *h == d.height)
        {
            Some((_, _, pieces)) => pieces.push(piece),
            None => sections.push((d.width, d.height, vec![piece])),
        }
    }
    Some(CutLists {
        rails,
        ducts: sections
            .into_iter()
            .map(|(w, h, pieces)| (w, h, optimise(&pieces, stock)))
            .collect(),
        duct_names: novo_reports::duct_names(project),
    })
}

fn summary(ui: &mut egui::Ui, plan: &CutPlan) {
    ui.label(format!(
        "{} bar{} of {:.0} mm; {:.0} mm to keep, {:.0} mm scrap",
        plan.bars_needed(),
        if plan.bars_needed() == 1 { "" } else { "s" },
        plan.stock.length,
        plan.reusable(),
        plan.scrap()
    ));
    if plan.budget_hit {
        ui.weak("The search stopped early; fewer bars may be possible.");
    }
}

/// The floating "Rails and ducts" cut list. Returns the rail lengths to set
/// when the person asks to trim the rails.
pub(super) fn show_cut_window(
    ctx: &egui::Context,
    state: &mut RulesState,
    lists: Option<&CutLists>,
) -> Option<Ask> {
    if !state.cut_open {
        return None;
    }
    let mut open = true;
    let mut ask = None;
    super::panel::dialog(ctx, "Cut list: rails and ducts", [460.0, 0.0])
        .open(super::panel::esc(ctx, &mut open))
        .show(ctx, |ui| {
            let Some(lists) = lists else {
                ui.weak("This panel has no mounting plate yet.");
                return;
            };
            super::dialog_ui::explained(ui, "Stock bars", "Stock bars of 2000 mm, 2 mm saw cut. Rails are cut shorter than the plate only when that saves a bar.");
            ui.add_space(4.0);
            if let Some((plan, pieces)) = &lists.rails {
                ui.strong("DIN rail");
                summary(ui, plan);
                let label = |id: u64| format!("Rail {id}");
                for (n, bar) in plan.bars.iter().enumerate() {
                    ui.label(format!("{}. {}", n + 1, bar.line(&plan.stock, &label)));
                }
                let changes: Vec<(u64, f64)> = plan
                    .lengths
                    .iter()
                    .filter(|(id, len)| {
                        pieces
                            .iter()
                            .find(|p| p.id == *id)
                            .is_some_and(|p| (p.max - len).abs() > 0.5)
                    })
                    .copied()
                    .collect();
                if !changes.is_empty()
                    && ui
                        .button(format!("Cut {} rails to these lengths", changes.len()))
                        .on_hover_text("Shortens the rails on the plate; one undo step")
                        .clicked()
                {
                    ask = Some(Ask::SetRailLengths(changes));
                }
                ui.add_space(6.0);
            }
            for (w, h, plan) in &lists.ducts {
                ui.strong(format!("Duct {w:.0} x {h:.0}"));
                summary(ui, plan);
                let label = |id: u64| {
                    lists
                        .duct_names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| "duct ?".to_owned())
                };
                for (n, bar) in plan.bars.iter().enumerate() {
                    ui.label(format!("{}. {}", n + 1, bar.line(&plan.stock, &label)));
                }
                ui.add_space(6.0);
            }
            if lists.rails.is_none() && lists.ducts.is_empty() {
                ui.weak("No rails or ducts on the plate yet. Place all adds them.");
            }
        });
    state.cut_open = open;
    ask
}
