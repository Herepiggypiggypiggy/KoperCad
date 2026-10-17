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
        dirs::data_dir()?
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
        if !rules.rules.is_empty() {
            problems.push(
                "Surface rules need the inside size of the enclosure; every device stays on the plate"
                    .to_owned(),
            );
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
pub(super) fn annotate(
    facts: &mut Facts,
    state: &RulesState,
    judged: &Judgement,
    tag_of: &impl Fn(u64) -> String,
) {
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
        for v in judged
            .violations
            .iter()
            .filter(|v| v.devices().contains(&d.id))
        {
            d.problems.push(v.describe(&state.rules, &|id| tag_of(id)));
        }
    }
}

/// The floating "Place rules" window: the rule file as text, checked as it is
/// typed, saved on request.
pub(super) fn show_rules_window(ctx: &egui::Context, state: &mut RulesState) {
    if !state.editor_open {
        return;
    }
    state.ensure_loaded();
    let mut open = true;
    super::panel::dialog(ctx, "Place rules", [520.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            super::dialog_ui::hint(
                ui,
                "Where Place all puts each kind of device. One rule per line: \
                 letter codes, zone (top, middle, bottom), rail (own or shared), \
                 class (power, signal), and gap, above, below in mm. \
                 These are practice values, not a standard: change them to your own.",
            );
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::multiline(&mut state.text)
                    .code_editor()
                    .desired_rows(16)
                    .desired_width(f32::INFINITY),
            );
            let parsed = RuleSet::parse(&state.text);
            if let Err(e) = &parsed {
                ui.colored_label(ui.visuals().error_fg_color, e.to_string());
            }
            if let Some(n) = &state.notice {
                super::dialog_ui::hint(ui, n);
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = parsed.is_ok();
                let save = ui
                    .add_enabled_ui(ok, |ui| novo_theme::widgets::primary_button(ui, "Save"))
                    .inner;
                if save.clicked()
                    && let Ok(rules) = parsed
                {
                    state.notice = Some(match &state.path {
                        Some(path) => match save_rules(&state.text, path) {
                            Ok(()) => format!("Saved to {}", path.display()),
                            Err(e) => format!("Could not save: {e}"),
                        },
                        None => "There is no folder to save to; used for this session".into(),
                    });
                    state.rules = rules;
                }
                let standard = RuleSet::standard().to_text();
                if super::dialog_ui::confirm_button(
                    ui,
                    "standard_rules",
                    "Standard rules",
                    "Replace the text above with the standard rules?",
                    state.text != standard,
                ) {
                    state.text = standard;
                    state.notice = None;
                }
            });
        });
    state.editor_open = open;
}

/// The bars to buy for the rails and ducts of the plate.
pub(super) struct CutLists {
    pub rails: Option<(CutPlan, Vec<Piece>)>,
    /// One list per duct cross-section, with the duct numbers (1 up).
    pub ducts: Vec<(f64, f64, CutPlan)>,
}

pub(super) fn cut_lists(project: &Project, structure: StructureId) -> Option<CutLists> {
    let layout = load(project, structure)?;
    let stock = Stock::default();
    let rails = (!layout.rails.is_empty()).then(|| {
        let pieces = rail_pieces(&layout.rails, &layout.placements);
        (optimise(&pieces, stock), pieces)
    });
    let all = ducts(project, structure);
    let mut sections: Vec<(f64, f64, Vec<Piece>)> = Vec::new();
    for (n, d) in all.iter().enumerate() {
        let piece = Piece {
            id: n as u64 + 1,
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
        .open(&mut open)
        .show(ctx, |ui| {
            let Some(lists) = lists else {
                ui.weak("This panel has no mounting plate yet.");
                return;
            };
            ui.weak("Stock bars of 2000 mm, 2 mm saw cut. Rails are cut shorter than the plate only when that saves a bar.");
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
                let label = |id: u64| format!("Duct {id}");
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
