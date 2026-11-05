//! Problem triangles in the app (docs/design/problem-triangles.md, lane T0b).
//!
//! The design checks (novo-checks) hand over [`Problem`]s; this module turns
//! each into a [`FlagDraft`], keeps them in one [`FlagStore`], draws a
//! triangle on every drawn anchor, and opens a popover when one is clicked.
//! The Issues list, the pill and F8 read the same problems, so the count is
//! one number.

use std::collections::HashMap;

use eframe::egui;
use novo_checks::{Problem, ProblemKind, Severity};
use novo_core::{AnyId, PageId};
use novo_flags::{Action, Anchor, Detail, FlagDraft, FlagId, FlagStore, Life, Source};

use super::NovoApp;

/// The command id of the "assign parts" fix.
pub(super) const FIX_ASSIGN_PARTS: &str = "parts.auto_assign";
/// The command id of the "usual pin names" fix.
pub(super) const FIX_PIN_NAMES: &str = "pin_names.fix";
/// The command id of the "preview the EMC suggestion" fix.
pub(super) const FIX_EMC_PREVIEW: &str = "emc.preview";

/// What the app holds about flags besides the store.
pub(super) struct FlagUi {
    pub(super) store: FlagStore,
    /// The problems revision the store was built from.
    synced: Option<u64>,
    /// Which problem (index in the shown list) each flag came from.
    index: HashMap<FlagId, usize>,
    /// The popover that is open, and where it was asked for.
    pub(super) open: Option<(FlagId, egui::Pos2)>,
    /// The EMC suggestion the open popover can preview (worked out once per
    /// popover, not per frame): the flag it was worked out for and the fix.
    emc_fix: Option<(FlagId, Option<novo_emc::Fix>)>,
    /// What Retry runs for each job flag (wiring lane).
    pub(super) retry: HashMap<FlagId, super::flags_wiring::JobRetry>,
    /// The transient flags of refused edits (lane T7).
    pub(super) edit: super::flags_edit::EditFlags,
    /// Failed jobs: what Retry and Open folder need (lane T12).
    pub(super) jobs: HashMap<FlagId, super::flags_jobs::JobInfo>,
    /// The failed jobs, oldest first.
    pub(super) job_order: Vec<FlagId>,
    /// The drafts each simulation window raised (lane T10), by window.
    pub(super) sim: std::collections::BTreeMap<&'static str, Vec<FlagDraft>>,
    /// The project revision and Place rules the Cabinet's own flags were built from.
    layout_synced: Option<(u64, novo_layout::rules::RuleSet)>,
}

impl Default for FlagUi {
    fn default() -> Self {
        Self {
            store: FlagStore::new(),
            synced: None,
            index: HashMap::new(),
            open: None,
            emc_fix: None,
            retry: HashMap::new(),
            edit: Default::default(),
            jobs: HashMap::new(),
            job_order: Vec::new(),
            sim: std::collections::BTreeMap::new(),
            layout_synced: None,
        }
    }
}

/// A title of at most 28 characters for a check kind.
pub(super) fn title(kind: ProblemKind) -> &'static str {
    match kind {
        ProblemKind::LoneInterruption => "Lone interruption point",
        ProblemKind::DuctOverlap => "Duct overlaps something",
        other => other.label(),
    }
}

/// The engine a check kind belongs to: the EMC rules, the safety function
/// calculation and the cable, protection and starter calculations get their
/// own source so the badge and the filters name them; every other check is
/// an Issues flag.
pub(super) fn source_of(kind: ProblemKind, key: &str) -> Source {
    match kind {
        ProblemKind::Emc => Source::Emc,
        ProblemKind::SafetyLevelNotMet | ProblemKind::SafetyBinding => Source::Safety,
        ProblemKind::CableRating
        | ProblemKind::ShortCircuit
        | ProblemKind::DisconnectionTime
        | ProblemKind::ThermalWithstand
        | ProblemKind::VoltageDrop
        | ProblemKind::MotorStarter
        | ProblemKind::ProtectiveConductor => Source::Calc,
        _ => Source::Issues(key.to_owned()),
    }
}

/// The label of a flag's generic Open button: its home window's label, else plain "Open"
/// (a commissioning punch flag opens the punch list).
pub(super) fn open_label(flag: &novo_flags::Flag) -> &'static str {
    home_of(flag).map_or("Open", |(_, label)| label)
}

/// The window that holds the whole picture behind a finding: its command id
/// and the button label.
pub(super) fn home_of(flag: &novo_flags::Flag) -> Option<(&'static str, &'static str)> {
    match &flag.source {
        Source::Emc => Some(("check.emc", "Open EMC check")),
        Source::Safety => Some(("check.safety", "Open safety functions")),
        Source::Calc => Some(("check.calculations", "Open calculations")),
        Source::Issues(k) if k == ProblemKind::Standard.key() => {
            Some(("check.standards", "Open standards report"))
        }
        _ => None,
    }
}

/// The standing caveat of a source, shown behind the "?" in the popover.
/// These findings are design aids; none of them is a compliance statement.
pub(super) fn caveat(flag: &novo_flags::Flag) -> Option<String> {
    match &flag.source {
        Source::Emc => Some(format!(
            "{} Rules on what is drawn, geometric indices and closed-form estimates; clause numbers are unverified. Never a compliance statement.",
            novo_emc::DESIGN_AID
        )),
        Source::Safety => Some(novo_safety::DISCLAIMER.to_owned()),
        Source::Calc => Some(
            "Design aid, not a certified calculation. Planning tables are used unless a licensed copy of the standard is loaded.".to_owned(),
        ),
        Source::Issues(k) if k == ProblemKind::Standard.key() => Some(
            "Design aid, not compliance. The rules follow the selected standard as the program states them; clause numbers are unverified.".to_owned(),
        ),
        _ => None,
    }
}

/// A title of at most 28 characters for an EMC rule.
pub(super) fn emc_title(rule: &str) -> &'static str {
    match rule {
        "EMC-01" => "Classes too close",
        "EMC-02" => "Parallel run too long",
        "EMC-04" => "Motor cable shield",
        "EMC-05" => "Shield pigtail too long",
        "EMC-07" => "No drive input filter",
        "EMC-09" => "Loop area large",
        "EMC-11" => "Plate bonding",
        "EMC-12" => "Door and hinge crossing",
        "EMC-14" => "Signal cable leaves cabinet",
        "EMC-17" => "Sensitive device near drive",
        "EMC-19" => "Field at sensitive device",
        "EMC-20" => "Induced voltage in a run",
        "EMC-21" => "Motor cable leakage",
        "EMC-22" => "Box resonance, drive inside",
        _ => "EMC finding",
    }
}

/// Kinds that say "could not judge" (missing data) rather than "wrong".
pub(super) fn is_unchecked(kind: ProblemKind) -> bool {
    matches!(
        kind,
        ProblemKind::DepthUnknown | ProblemKind::GuessedWire | ProblemKind::UnsizedDuct
    )
}

/// The anchors of a problem: its drawn objects, else the spot on its page,
/// else the project.
fn anchors(p: &Problem) -> Vec<Anchor> {
    let mut out: Vec<Anchor> =
        p.at.iter()
            .filter_map(|id| {
                let raw = id.raw().to_string();
                Some(match id {
                    AnyId::Symbol(_) => Anchor::Symbol(raw),
                    AnyId::Wire(_) => Anchor::Wire(raw),
                    AnyId::Device(_) => Anchor::Device(raw),
                    AnyId::Terminal(_) => Anchor::Terminal(raw),
                    AnyId::Cable(_) => Anchor::Cable(raw),
                    AnyId::Rail(_) => Anchor::Rail(raw),
                    // A plate (its enclosure): the frame of the plate.
                    AnyId::Structure(_) => Anchor::Plate(raw, [0, 0]),
                    _ => return None,
                })
            })
            .collect();
    if out.is_empty() {
        out.push(match (p.page, p.position) {
            (Some(page), Some(at)) => Anchor::Page(
                page.0.to_string(),
                [at.x.round() as i64, at.y.round() as i64],
            ),
            _ => Anchor::Project,
        });
    }
    out
}

/// One flag draft for a problem; `None` for information.
pub(super) fn draft(p: &Problem) -> Option<FlagDraft> {
    let severity = match p.severity {
        Severity::Error => novo_flags::Severity::Red,
        Severity::Warning => novo_flags::Severity::Yellow,
        Severity::Info => return None,
    };
    let source = source_of(p.kind, p.kind.key());
    let mut actions = vec![Action::Locate];
    match p.kind {
        ProblemKind::MissingPart => actions.push(Action::Fix(FIX_ASSIGN_PARTS.into())),
        ProblemKind::PinNames => actions.push(Action::Fix(FIX_PIN_NAMES.into())),
        ProblemKind::Emc => actions.push(Action::Fix(FIX_EMC_PREVIEW.into())),
        _ => {}
    }
    // The window with the whole picture (scores, assumptions, tables).
    match source {
        Source::Emc => actions.push(Action::Open("check.emc".into())),
        Source::Safety => actions.push(Action::Open("check.safety".into())),
        Source::Calc => actions.push(Action::Open("check.calculations".into())),
        Source::Issues(_) if p.kind == ProblemKind::Standard => {
            actions.push(Action::Open("check.standards".into()));
        }
        _ => {}
    }
    actions.extend([Action::Waive, Action::Explain, Action::Copy]);
    Some(FlagDraft {
        severity,
        unchecked: is_unchecked(p.kind),
        source,
        rule: p.rule.unwrap_or_default().to_owned(),
        anchors: anchors(p),
        title: match (p.kind, p.rule) {
            (ProblemKind::Emc, Some(rule)) => emc_title(rule).to_owned(),
            _ => title(p.kind).to_owned(),
        },
        detail: Detail::new([p.message.clone()]),
        actions,
        waiver_key: Some(p.waiver_key()),
        life: Life::Live,
    })
}

/// Every flag draft of a problem list, each with the index of its problem.
pub(super) fn drafts(problems: &[Problem]) -> Vec<(usize, FlagDraft)> {
    problems
        .iter()
        .enumerate()
        .filter_map(|(i, p)| draft(p).map(|d| (i, d)))
        .collect()
}

/// The mark a flag draws on the canvas.
pub(super) fn mark_kind(flag: &novo_flags::Flag) -> novo_canvas::MarkKind {
    use novo_canvas::MarkKind;
    match (flag.severity, flag.unchecked) {
        (novo_flags::Severity::Red, _) => MarkKind::Error,
        (novo_flags::Severity::Yellow, true) => MarkKind::Unchecked,
        (novo_flags::Severity::Yellow, false) => MarkKind::Warning,
    }
}

impl NovoApp {
    /// Brings the flag store up to the shown problems. Cheap when nothing
    /// changed: one revision compare.
    pub(super) fn sync_flags(&mut self) {
        self.sync_issue_flags();
        self.sync_layout_flags();
    }

    fn sync_issue_flags(&mut self) {
        let engines = [
            Source::Issues(String::new()),
            Source::Emc,
            Source::Safety,
            Source::Calc,
        ];
        if self.learn.quiet_checks() {
            if self.wb.flags.synced.take().is_some() {
                for engine in &engines {
                    self.wb.flags.store.replace_source(engine, Vec::new());
                }
                self.wb.flags.index.clear();
            }
            return;
        }
        let Some((revision, problems)) = self.wb.problems.as_ref() else {
            return;
        };
        if self.wb.flags.synced == Some(*revision) {
            return;
        }
        let revision = *revision;
        let mut built = drafts(problems);
        if let Some(doc) = self.backend.document() {
            // Wire-end checks sit on the pin the wire lands on.
            for (i, d) in &mut built {
                let pins = super::flags_wiring::pin_anchors(doc.project(), &problems[*i]);
                if !pins.is_empty() {
                    d.anchors = pins;
                }
            }
        }
        let flags = &mut self.wb.flags;
        flags.index.clear();
        for (i, d) in &built {
            let id = novo_flags::Flag::from_draft(d.clone()).id;
            flags.index.entry(id).or_insert(*i);
        }
        // One replace per engine: a source only ever replaces its own flags.
        for engine in &engines {
            let mine: Vec<FlagDraft> = built
                .iter()
                .filter(|(_, d)| d.source.key().split(':').next() == engine.key().split(':').next())
                .map(|(_, d)| d.clone())
                .collect();
            flags.store.replace_source(engine, mine);
        }
        flags.synced = Some(revision);
    }

    /// The Cabinet's own flags (broken Place rules, covered wiring openings):
    /// rebuilt when the project or the rules change.
    fn sync_layout_flags(&mut self) {
        if self.learn.quiet_checks() {
            if self.wb.flags.layout_synced.take().is_some() {
                self.wb
                    .flags
                    .store
                    .replace_source(&Source::Layout, Vec::new());
            }
            return;
        }
        let Some(revision) = self.backend.document().map(|d| d.project().revision()) else {
            return;
        };
        self.cabinet.rules.ensure_loaded();
        if self
            .wb
            .flags
            .layout_synced
            .as_ref()
            .is_some_and(|(r, set)| *r == revision && *set == self.cabinet.rules.rules)
        {
            return;
        }
        let rules = self.cabinet.rules.rules.clone();
        let Some(doc) = self.backend.document() else {
            return;
        };
        let built = super::cabinet_flags::layout_drafts(doc.project(), &rules);
        self.wb.flags.store.replace_source(&Source::Layout, built);
        self.wb.flags.layout_synced = Some((revision, rules));
    }

    /// The marks of the flags anchored on drawn objects of `page`.
    pub(super) fn flag_marks(&mut self, page: PageId) -> Vec<novo_canvas::Mark> {
        use novo_canvas::Mark;
        self.sync_flags();
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let mut marks = Vec::new();
        for flag in self.wb.flags.store.list(false) {
            // The reason shows in the popover only: no second line on the canvas.
            for target in super::flags_edit::mark_targets(project, flag, page) {
                marks.push(Mark {
                    kind: mark_kind(flag),
                    target,
                    text: String::new(),
                    key: flag.id.0,
                });
            }
        }
        marks
    }

    /// A click on a triangle on the canvas opens its popover.
    pub(super) fn flag_click(&mut self) {
        let Some((keys, at)) = self.canvas.schematic().and_then(|v| v.take_mark_click()) else {
            return;
        };
        let store = &self.wb.flags.store;
        if let Some(id) = keys
            .into_iter()
            .map(FlagId)
            .find(|id| store.get(*id).is_some())
        {
            self.wb.flags.open = Some((id, at));
        }
    }

    /// The popover of the flag asked for: title, source badge, the numbers,
    /// and the actions. Esc or a click elsewhere closes it.
    pub(super) fn flag_popover(&mut self, ctx: &egui::Context) {
        let Some((id, at)) = self.wb.flags.open else {
            return;
        };
        let Some(flag) = self.wb.flags.store.get(id).cloned() else {
            self.wb.flags.open = None;
            return;
        };
        let pal = crate::theme::ui_palette();
        let color = match flag.severity {
            novo_flags::Severity::Red => pal.error,
            novo_flags::Severity::Yellow => pal.warning,
        };
        let index = self.wb.flags.index.get(&id).copied();
        if flag.source == Source::Emc
            && self.wb.flags.emc_fix.as_ref().is_none_or(|(f, _)| *f != id)
        {
            let rule = index.and_then(|i| self.shown_problems().get(i).and_then(|p| p.rule));
            let fix = self.emc_fix_of(&flag, rule);
            self.wb.flags.emc_fix = Some((id, fix));
        }
        let emc_fix = self
            .wb
            .flags
            .emc_fix
            .as_ref()
            .and_then(|(f, x)| (*f == id).then(|| x.clone()))
            .flatten();
        let mut chosen: Option<Action> = None;
        let mut dismiss = false;
        let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let area = egui::Area::new(egui::Id::new("flag_popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(at + egui::vec2(8.0, 8.0))
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(320.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(novo_theme::icons::status::WARNING)
                                .color(color)
                                .strong(),
                        )
                        .on_hover_text(flag.accessible_name());
                        ui.label(egui::RichText::new(&flag.title).strong());
                        ui.weak(flag.source.badge());
                        if let Some(text) = caveat(&flag) {
                            novo_theme::widgets::help_mark(ui, &text);
                        }
                    });
                    novo_theme::widgets::help_scroll(ui, |ui| {
                        for line in &flag.detail.0 {
                            ui.label(line);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        for action in &flag.actions {
                            let label = match action {
                                Action::Locate => "Locate",
                                Action::Fix(c) if c == FIX_ASSIGN_PARTS => "Assign parts",
                                Action::Fix(c) if c == FIX_PIN_NAMES => "Use the usual pin names",
                                Action::Fix(c) if c == FIX_EMC_PREVIEW && emc_fix.is_some() => {
                                    "Preview the fix"
                                }
                                Action::Fix(c) if c == FIX_EMC_PREVIEW => continue,
                                a if super::flags_shell::shell_command(a).is_some() => {
                                    super::flags_shell::shell_command(a).map_or("Fix", |c| c.1)
                                }
                                Action::Open(k) if k == super::flags_jobs::OPEN_FOLDER => {
                                    "Open folder"
                                }
                                Action::Open(k) if k == super::flags_jobs::DISMISS => "Dismiss",
                                Action::Open(_) => open_label(&flag),
                                Action::Fix(_) => "Fix",
                                Action::Waive => "Waive…",
                                Action::Retry => "Retry",
                                Action::Copy if flag.source == Source::Job => "Copy details",
                                Action::Copy => "Copy",
                                Action::Explain if self.settings.ai.enabled => {
                                    "Explain with Claude"
                                }
                                _ => continue,
                            };
                            if ui.button(label).clicked() {
                                chosen = Some(action.clone());
                            }
                        }
                        if flag.life == Life::Until
                            && flag.source == Source::Shell
                            && ui.button("Dismiss").clicked()
                        {
                            dismiss = true;
                        }
                    });
                });
            });
        let clicked_away = ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| !area.response.rect.contains(p));
        // The click that opened it lands in the same frame it is asked for.
        let just_opened = ctx.input(|i| i.pointer.any_released())
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| p.distance(at) < 2.0);
        if clicked_away && !just_opened {
            close = true;
        }
        if let Some(action) = chosen {
            close = true;
            self.run_flag_action(&flag, index, action, ctx);
        }
        if dismiss {
            close = true;
            self.wb.flags.store.clear(id);
        }
        if close {
            self.wb.flags.open = None;
            self.wb.flags.emc_fix = None;
        }
    }

    fn run_flag_action(
        &mut self,
        flag: &novo_flags::Flag,
        index: Option<usize>,
        action: Action,
        ctx: &egui::Context,
    ) {
        // Retry of a commissioning job is run here; Dismiss and the rest are the job path's.
        if super::commissioning_flags::is_commissioning_job(flag) && action == Action::Retry {
            self.retry_commissioning_job(flag);
            return;
        }
        if self.run_job_action(flag.id, &action) {
            return;
        }
        if flag.source == Source::Commissioning {
            match action {
                Action::Locate => self.locate_commissioning_flag(flag),
                Action::Open(panel) => self.open_commissioning_panel(&panel),
                Action::Copy => {
                    ctx.copy_text(format!("{}\n{}", flag.title, flag.detail.0.join("\n")))
                }
                _ => {}
            }
            return;
        }
        match action {
            Action::Locate if super::cabinet_flags::is_plate_flag(flag) => {
                self.locate_on_plate(flag);
            }
            Action::Locate => match index {
                Some(i) => self.go_problem(i),
                // In 3D the parts are selected there; elsewhere the device is
                // shown on its page.
                None if self.workspace() == super::workspace::Workspace::View3d => {
                    self.locate_flag_in_3d(flag)
                }
                None => self.locate_flag(flag),
            },
            Action::Fix(cmd) if cmd == FIX_ASSIGN_PARTS => self.wb.queued.push("parts.auto_assign"),
            Action::Fix(cmd) if cmd == FIX_PIN_NAMES => {
                if let Some(i) = index {
                    self.fix_pin_names(i);
                }
            }
            Action::Fix(cmd) if cmd == FIX_EMC_PREVIEW => {
                if let Some((_, Some(fix))) = self.wb.flags.emc_fix.clone() {
                    self.emc.open = true;
                    self.emc_preview(&fix);
                }
            }
            Action::Open(_) => {
                if let Some((command, _)) = home_of(flag) {
                    self.wb.queued.push(command);
                }
            }
            Action::Waive => {
                if let Some(i) = index {
                    self.wb.waiving = Some((i, String::new()));
                    self.show_issues(false);
                }
            }
            Action::Explain => {
                if let Some(i) = index {
                    self.explain_problem(i);
                }
            }
            Action::Copy => ctx.copy_text(flag.detail.0.join("\n")),
            other => {
                if let Some((command, _)) = super::flags_shell::shell_command(&other) {
                    self.wb.queued.push(command);
                }
            }
        }
    }

    /// Shows a plate flag where it is: the Cabinet on the right plate, its
    /// devices selected and in view.
    pub(super) fn locate_on_plate(&mut self, flag: &novo_flags::Flag) {
        let (plate, devices) = super::cabinet_flags::targets(flag);
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let structure = plate
            .map(novo_core::StructureId::from)
            .or_else(|| {
                devices
                    .first()
                    .and_then(|d| super::cabinet_flags::plate_of(project, *d))
            })
            .and_then(|id| {
                project
                    .structures()
                    .find(|s| s.id == id)
                    .map(|s| s.tag.clone())
            });
        let items: Vec<_> = devices
            .iter()
            .map(|d| novo_canvas::cabinet::box_item(*d))
            .collect();
        if let Some(tag) = structure
            && self.cabinet.location.as_ref() != Some(&tag)
        {
            self.cabinet.location = Some(tag);
            self.cabinet.preview = None;
            self.cabinet.surface_preview.clear();
            self.cabinet.result = None;
        }
        self.cabinet.table = false;
        self.cabinet.surface = None;
        self.cabinet.view.selection.apply(
            items.iter().copied(),
            novo_canvas::selection::SelectOp::Replace,
        );
        self.switch_workspace(super::workspace::Workspace::Cabinet);
        if !items.is_empty() {
            self.cabinet.view.reveal(items);
        }
    }

    /// Red and yellow counts of the Cabinet's own flags (broken Place rules,
    /// covered wiring openings): the part of the one count the design checks
    /// do not hold.
    pub(super) fn layout_counts(&self) -> (usize, usize) {
        let mut red = 0;
        let mut yellow = 0;
        for f in self.wb.flags.store.list(false) {
            if f.source == Source::Layout {
                match f.severity {
                    novo_flags::Severity::Red => red += 1,
                    novo_flags::Severity::Yellow => yellow += 1,
                }
            }
        }
        (red, yellow)
    }
}
