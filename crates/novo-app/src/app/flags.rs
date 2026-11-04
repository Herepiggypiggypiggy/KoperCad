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

/// What the app holds about flags besides the store.
pub(super) struct FlagUi {
    pub(super) store: FlagStore,
    /// The problems revision the store was built from.
    synced: Option<u64>,
    /// Which problem (index in the shown list) each flag came from.
    index: HashMap<FlagId, usize>,
    /// The popover that is open, and where it was asked for.
    pub(super) open: Option<(FlagId, egui::Pos2)>,
    /// Failed jobs: what Retry and Open folder need (lane T12).
    pub(super) jobs: HashMap<FlagId, super::flags_jobs::JobInfo>,
    /// The failed jobs, oldest first.
    pub(super) job_order: Vec<FlagId>,
    /// The drafts each simulation window raised (lane T10), by window.
    pub(super) sim: std::collections::BTreeMap<&'static str, Vec<FlagDraft>>,
}

impl Default for FlagUi {
    fn default() -> Self {
        Self {
            store: FlagStore::new(),
            synced: None,
            index: HashMap::new(),
            open: None,
            jobs: HashMap::new(),
            job_order: Vec::new(),
            sim: std::collections::BTreeMap::new(),
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
    let mut actions = vec![Action::Locate];
    match p.kind {
        ProblemKind::MissingPart => actions.push(Action::Fix(FIX_ASSIGN_PARTS.into())),
        ProblemKind::PinNames => actions.push(Action::Fix(FIX_PIN_NAMES.into())),
        _ => {}
    }
    actions.extend([Action::Waive, Action::Explain, Action::Copy]);
    Some(FlagDraft {
        severity,
        unchecked: is_unchecked(p.kind),
        source: Source::Issues(p.kind.key().to_owned()),
        rule: p.rule.unwrap_or_default().to_owned(),
        anchors: anchors(p),
        title: title(p.kind).to_owned(),
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
        let source = Source::Issues(String::new());
        if self.learn.quiet_checks() {
            if self.wb.flags.synced.take().is_some() {
                self.wb.flags.store.replace_source(&source, Vec::new());
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
        let built = drafts(problems);
        let flags = &mut self.wb.flags;
        flags.index.clear();
        for (i, d) in &built {
            let id = novo_flags::Flag::from_draft(d.clone()).id;
            flags.index.entry(id).or_insert(*i);
        }
        flags
            .store
            .replace_source(&source, built.into_iter().map(|(_, d)| d).collect());
        flags.synced = Some(revision);
    }

    /// The marks of the flags anchored on drawn objects of `page`.
    pub(super) fn flag_marks(&mut self, page: PageId) -> Vec<novo_canvas::Mark> {
        use novo_canvas::{Mark, model::item_id};
        self.sync_flags();
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let mut marks = Vec::new();
        for flag in self.wb.flags.store.list(false) {
            let hover = std::iter::once(flag.title.clone())
                .chain(flag.detail.0.iter().cloned())
                .collect::<Vec<_>>()
                .join(": ");
            for anchor in &flag.anchors {
                let id = match anchor {
                    Anchor::Symbol(raw) => raw
                        .parse()
                        .ok()
                        .map(novo_core::SymbolId)
                        .filter(|s| project.symbol(*s).is_some_and(|s| s.page == page))
                        .map(AnyId::Symbol),
                    Anchor::Wire(raw) => raw
                        .parse()
                        .ok()
                        .map(novo_core::WireId)
                        .filter(|w| project.wire(*w).is_some_and(|w| w.page == page))
                        .map(AnyId::Wire),
                    _ => None,
                };
                if let Some(id) = id {
                    marks.push(
                        Mark::on(mark_kind(flag), item_id(id))
                            .with_text(hover.clone())
                            .with_key(flag.id.0),
                    );
                }
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
        let mut chosen: Option<Action> = None;
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
                    });
                    for line in &flag.detail.0 {
                        ui.label(line);
                    }
                    ui.horizontal_wrapped(|ui| {
                        for action in &flag.actions {
                            let label = match action {
                                Action::Locate => "Locate",
                                Action::Fix(c) if c == FIX_ASSIGN_PARTS => "Assign parts",
                                Action::Fix(c) if c == FIX_PIN_NAMES => "Use the usual pin names",
                                Action::Fix(_) => "Fix",
                                Action::Waive => "Waive…",
                                Action::Retry => "Retry",
                                Action::Open(k) if k == super::flags_jobs::OPEN_FOLDER => {
                                    "Open folder"
                                }
                                Action::Open(k) if k == super::flags_jobs::DISMISS => "Dismiss",
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
        if close {
            self.wb.flags.open = None;
        }
    }

    fn run_flag_action(
        &mut self,
        flag: &novo_flags::Flag,
        index: Option<usize>,
        action: Action,
        ctx: &egui::Context,
    ) {
        if self.run_job_action(flag.id, &action) {
            return;
        }
        match action {
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
            _ => {}
        }
    }
}
