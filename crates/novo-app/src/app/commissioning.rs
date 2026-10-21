//! Commissioning mode (docs/design/commissioning.md): ticking off wires,
//! terminals and devices at the machine, and the punch list.
//!
//! The data is novo-commissioning's [`CommissioningStore`], kept in the
//! project file's `commissioning` document and written after every change.
//! It is not a model edit: it is not undone and does not dirty the drawing.
//! The panel is the outline's Commissioning tab, with buttons sized for a
//! finger. On the drawing, passed objects get the "added" outline and failed
//! ones the error mark.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use novo_canvas::marks::{Mark, MarkKind};
use novo_canvas::model::item_id;
use novo_commissioning::{
    CheckKind, CheckRecord, CheckState, CommissioningStore, FieldPackage, Progress, PunchItem,
    PunchState, RedlineState, Scope, Severity, Signature, Target, TargetInfo, WalkOrder, WalkScope,
    WireKind, WirePlan, WireStep, next_open, next_open_with, progress, targets, walk_order_with,
};
use novo_core::{PageId, Project, WireId};
use novo_pdf::{ReportFileFormat, ReportPdfOptions};
use novo_reports::CsvOptions;
use novo_theme::{Density, Palette, icons, space, widgets};
use novo_xref::Place;

use super::cabinet_bar::{note_ctx, note_rect};
use super::commissioning_viewer::{self as viewer, PhotoViewer};

use super::changes::shown_on;
use super::commissioning_redlines::{RedlineAction, redlines_tab};
use super::commissioning_signature::{self as signature, SignDraft};
use super::pdf::{replaceable_path, write_replacing};
use super::revisions::iso_date;
use super::workspace::Workspace;
use super::{Level, NovoApp, OutlineMode, projects_dir};

/// How long the first touch of "Pass all open" waits for the second.
const ARM_SECONDS: f64 = 5.0;
/// Windows narrower than this (logical points) get the tablet layout in the
/// Commissioning workspace: the panel pinned and wider, large buttons on.
pub(super) const TABLET_MAX_WIDTH: f32 = 1200.0;
/// The panel's width in the tablet layout.
pub(super) const TABLET_PANEL_WIDTH: f32 = 380.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum FieldAction {
    Export,
    Import,
    PunchPdf,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Checks,
    Punch,
    Redlines,
}

/// Commissioning mode's data and panel state.
pub(crate) struct CommissioningMode {
    pub(super) store: CommissioningStore,
    /// Who the decisions are recorded for.
    pub(super) who: String,
    pub(super) tab: Tab,
    pub(super) selected: Option<Target>,
    /// The measured value and note for the next decision.
    value: String,
    pub(super) note: String,
    punch_title: String,
    punch_severity: Severity,
    show_closed: bool,
    pub(super) redline: super::commissioning_redlines::RedlineDraft,
    /// The pages of the checks list that are unfolded.
    open_pages: HashSet<PageId>,
    /// The page the list last unfolded for, so a page change unfolds it once.
    unfolded_for: Option<PageId>,
    /// Bigger buttons and text for a tablet.
    pub(super) large: bool,
    /// "Pass all open" was touched once for this target at this time (egui
    /// seconds); a second touch within `ARM_SECONDS` does it.
    armed: Option<(Target, f64)>,
    /// The store changed and is not yet kept in the project file.
    pub(super) dirty: bool,
    /// A punch item whose "Add photo" was touched; the picker opens after the panel.
    pub(super) photo_for: Option<u64>,
    /// The signature pad, open under the item being verified.
    pub(super) signing: Option<SignDraft>,
    /// The pictures of the punch items, shown small and large.
    pub(super) viewer: PhotoViewer,
    /// The wire figures of the walk (ends, ferrules, length), kept until the
    /// project changes: computing them routes every wire.
    plan: Option<(u64, Rc<WirePlan>)>,
    /// What "Next open" and Space visit: everything, or the wires only.
    pub(super) scope: WalkScope,
    /// The order the wires are walked in.
    pub(super) order: WalkOrder,
    /// A Fail waiting for its reason: the note is open for typing.
    fail_pending: Option<(Target, CheckKind)>,
    /// Put the cursor in the Note field on the next frame.
    focus_note: bool,
    /// A wire the walk moved to; the drawing selects it after the panel.
    pub(super) reveal_wire: Option<(PageId, WireId)>,
    /// A wire to show in the 3D view, asked for on the walk card.
    show_3d: Option<WireId>,
    /// The last big tick ended the walk; the panel says so.
    pub(super) finished: bool,
    /// What each big tick passed, so Back can take it back.
    walked: Vec<(Target, Vec<CheckKind>)>,
}

impl CommissioningMode {
    fn new(store: CommissioningStore) -> Self {
        let who = ["USERNAME", "USER"]
            .iter()
            .find_map(|k| std::env::var(k).ok())
            .unwrap_or_default();
        CommissioningMode {
            store,
            who,
            tab: Tab::Checks,
            selected: None,
            value: String::new(),
            note: String::new(),
            punch_title: String::new(),
            punch_severity: Severity::Minor,
            show_closed: false,
            redline: Default::default(),
            open_pages: HashSet::new(),
            unfolded_for: None,
            large: false,
            armed: None,
            dirty: false,
            photo_for: None,
            signing: None,
            viewer: Default::default(),
            plan: None,
            scope: WalkScope::Wires,
            order: WalkOrder::Number,
            fail_pending: None,
            focus_note: false,
            reveal_wire: None,
            show_3d: None,
            finished: false,
            walked: Vec::new(),
        }
    }

    /// The walk's wire figures for the project as it is now.
    fn plan(&mut self, project: &Project) -> Rc<WirePlan> {
        let revision = project.revision();
        match &self.plan {
            Some((r, plan)) if *r == revision => Rc::clone(plan),
            _ => {
                let plan = Rc::new(WirePlan::of(project));
                self.plan = Some((revision, Rc::clone(&plan)));
                plan
            }
        }
    }

    /// The next open target after `from` in the chosen walking order.
    fn next(
        &mut self,
        project: &Project,
        from: Option<Target>,
        current: Option<PageId>,
    ) -> Option<TargetInfo> {
        let plan = self.plan(project);
        next_open_with(
            project,
            &plan,
            &self.store,
            from,
            current,
            self.scope,
            self.order,
        )
    }

    /// Fail for a check: the first press opens the Note with the wire's ends
    /// in it so the reason can be typed, the second (or Enter) records it.
    /// A note typed beforehand records at once.
    fn fail(&mut self, target: Target, check: CheckKind, hint: &str) {
        if self.fail_pending == Some((target, check)) || !self.note.trim().is_empty() {
            self.fail_pending = None;
            self.decide(target, check, CheckState::Failed);
        } else {
            self.note = format!("{hint}: ");
            self.fail_pending = Some((target, check));
            self.focus_note = true;
        }
    }

    /// Moves the walk to `info`: selects it and asks the drawing to show it
    /// (a wire is selected there, so it is lit on its page).
    fn walk_to(
        &mut self,
        project: &Project,
        info: &TargetInfo,
        current: Option<PageId>,
        go: &mut Option<Place>,
    ) {
        self.selected = Some(info.target);
        *go = place_of(project, info, current);
        if let (Target::Wire(w), Some(place)) = (info.target, *go) {
            self.reveal_wire = Some((place.page, w));
        }
    }

    /// The big tick: passes every open check of the selected target and
    /// moves to the next open one. With nothing selected it starts the walk.
    /// Returns whether the walk is finished: everything in its scope is
    /// decided.
    pub(super) fn tick(
        &mut self,
        project: &Project,
        current: Option<PageId>,
        go: &mut Option<Place>,
    ) -> bool {
        let from = self.selected;
        if let Some(target) = from {
            let mut passed = Vec::new();
            for &check in target.checks() {
                if self.store.state(target, check) == CheckState::Pending {
                    self.decide(target, check, CheckState::Passed);
                    passed.push(check);
                }
            }
            if !passed.is_empty() {
                self.walked.push((target, passed));
            }
        }
        match self.next(project, from, current) {
            Some(info) => {
                self.walk_to(project, &info, current, go);
                false
            }
            None => {
                self.selected = None;
                true
            }
        }
    }

    /// Takes back the last big tick: its checks are pending again and the
    /// walk is on that target. Returns whether there was one.
    pub(super) fn back(
        &mut self,
        project: &Project,
        current: Option<PageId>,
        go: &mut Option<Place>,
    ) -> bool {
        let Some((target, checks)) = self.walked.pop() else {
            return false;
        };
        for check in checks {
            self.store.clear_check(target, check);
        }
        self.dirty = true;
        self.finished = false;
        if let Some(info) = targets(project).into_iter().find(|i| i.target == target) {
            self.walk_to(project, &info, current, go);
        }
        true
    }

    /// Decides one check for the person set, with the value and note typed.
    pub(super) fn decide(&mut self, target: Target, check: CheckKind, state: CheckState) {
        self.armed = None;
        let record = CheckRecord {
            target,
            check,
            state,
            by: self.who.clone(),
            at: now(),
            value: std::mem::take(&mut self.value),
            note: std::mem::take(&mut self.note),
        };
        if self.store.set_check(record).is_ok() {
            self.dirty = true;
        }
    }
}

/// Now as ISO 8601 text, UTC.
pub(super) fn now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let of_day = secs % 86_400;
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        iso_date((secs / 86_400) as i64),
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

/// The icon and colour of a check state: a copper-free traffic light that
/// always carries a shape, so it reads without colour.
fn state_icon(p: &Palette, state: CheckState) -> (&'static str, egui::Color32) {
    match state {
        CheckState::Pending => (icons::CIRCLE, p.text_tertiary),
        CheckState::Passed => (icons::CHECK_CIRCLE, p.success),
        CheckState::Failed => (icons::WARNING_OCTAGON, p.error),
        CheckState::NotApplicable => (icons::MINUS_CIRCLE, p.text_tertiary),
    }
}

/// One state for all the checks of a target: failed beats pending beats passed.
fn target_state(store: &CommissioningStore, target: Target) -> CheckState {
    let states: Vec<CheckState> = target
        .checks()
        .iter()
        .map(|&c| store.state(target, c))
        .collect();
    if states.contains(&CheckState::Failed) {
        CheckState::Failed
    } else if states.contains(&CheckState::Pending) {
        CheckState::Pending
    } else {
        CheckState::Passed
    }
}

/// Touch density (the Large switch) for this panel only: the controls are
/// 36 pt high and the text a step larger. Without it the panel uses the
/// controls of the app's density, so a list row is one control high.
fn touch_density(ui: &mut egui::Ui, large: bool) {
    if !large {
        return;
    }
    let m = Density::Touch.metrics();
    let style = ui.style_mut();
    style.spacing.interact_size.y = m.control_height;
    style.spacing.button_padding = egui::vec2(m.button_padding[0], m.button_padding[1]);
    if let Some(body) = style.text_styles.get_mut(&egui::TextStyle::Body) {
        body.size = m.font_body;
    }
    if let Some(button) = style.text_styles.get_mut(&egui::TextStyle::Button) {
        button.size = m.font_body;
    }
}

/// Passes every open check of the target in two touches: the first arms the
/// button with the count, the second does it; any other touch or a few
/// seconds disarm it. It sits under the checks, away from "Next open".
fn pass_all_open(ui: &mut egui::Ui, mode: &mut CommissioningMode, target: Target) {
    let open: Vec<CheckKind> = target
        .checks()
        .iter()
        .copied()
        .filter(|&c| mode.store.state(target, c) == CheckState::Pending)
        .collect();
    let now_s = ui.input(|i| i.time);
    if mode
        .armed
        .is_some_and(|(t, at)| t != target || open.is_empty() || now_s - at > ARM_SECONDS)
    {
        mode.armed = None;
    }
    if open.is_empty() {
        return;
    }
    ui.add_space(space::M);
    ui.horizontal(|ui| {
        if mode.armed.is_some() {
            if ui
                .button(format!("Pass {} open · Pass them", open.len()))
                .clicked()
            {
                for check in open {
                    mode.decide(target, check, CheckState::Passed);
                }
                mode.armed = None;
            } else if ui.button("Cancel").clicked() {
                mode.armed = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(ARM_SECONDS));
            }
        } else if ui.button(format!("Pass all {} open", open.len())).clicked() {
            mode.armed = Some((target, now_s));
        }
    });
}

/// A progress bar for one scope: "47 / 76 done" with the failures in red.
fn progress_bar(ui: &mut egui::Ui, title: &str, p: Progress) {
    let total = p.total.max(1) as f32;
    let mut detail = format!("{} / {}", p.done(), p.total);
    if p.failed > 0 {
        detail.push_str(&format!(" · {} failed", p.failed));
    }
    widgets::progress_bar(
        ui,
        title,
        &detail,
        p.done() as f32 / total,
        p.failed as f32 / total,
    );
}

/// Where a click on `info` takes the drawing: its page and, for a device or
/// terminal, a symbol that draws it there.
pub(super) fn place_of(
    project: &Project,
    info: &TargetInfo,
    current: Option<PageId>,
) -> Option<Place> {
    let page = current
        .filter(|p| info.pages.contains(p))
        .or_else(|| info.pages.first().copied())?;
    let symbols = match info.target {
        Target::Device(d) => project.symbols_of_device(d),
        Target::Terminal(t) => project.symbols_of_terminal(t),
        Target::Wire(_) => Vec::new(),
    };
    let symbol = symbols.iter().find(|s| s.page == page).map(|s| s.id);
    Some(Place { page, symbol })
}

impl NovoApp {
    /// The Commissioning workspace is showing, so its panel stays beside the
    /// drawing at any window width: the walk-through needs it all the time.
    pub(super) fn commissioning_pins_panel(&self) -> bool {
        self.is_open() && self.workspace() == Workspace::Commissioning
    }

    /// Commissioning on a window the size of a tablet.
    pub(super) fn commissioning_tablet(&self) -> bool {
        self.commissioning_pins_panel()
            && self.window_width > 0.0
            && self.window_width < TABLET_MAX_WIDTH
    }

    /// The `view.commissioning` command opens the Commissioning workspace:
    /// the mode is on, its panel in the outline's place. Leaving it drops the
    /// in-memory store, which is safe because every change is flushed to the
    /// project in the frame it is made.
    pub(super) fn enter_commissioning(&mut self) {
        self.load_commissioning();
        self.outline_mode = OutlineMode::Commissioning;
        if !self.outline_pinned() {
            self.outline_open = true;
        }
    }

    /// The Commissioning workspace is left: the mode is off.
    pub(super) fn leave_commissioning(&mut self) {
        self.wb.commissioning = None;
        if self.outline_mode == OutlineMode::Commissioning {
            self.outline_mode = OutlineMode::Pages;
        }
    }

    /// Commissioning mode is showing, so the drawing is not edited: a change
    /// the canvas makes is taken back (see `after_canvas`).
    pub(super) fn commissioning_locks_edits(&self) -> bool {
        self.workspace() == Workspace::Commissioning
            || (self.wb.commissioning.is_some() && self.outline_mode == OutlineMode::Commissioning)
    }

    /// Each frame in the Commissioning workspace: its store is loaded (a
    /// project opened or created meanwhile dropped it) and its panel is the
    /// outline's tab. Picking another outline tab leaves the workspace for
    /// the schematic, keeping that tab.
    pub(super) fn sync_commissioning_workspace(&mut self) {
        if self.workspace() != Workspace::Commissioning || !self.is_open() {
            return;
        }
        if self.outline_mode != OutlineMode::Commissioning {
            let chosen = self.outline_mode;
            self.switch_workspace(Workspace::Schematic);
            self.outline_mode = chosen;
            return;
        }
        self.load_commissioning();
    }

    /// Reads the project's commissioning data when the mode has none yet.
    pub(super) fn load_commissioning(&mut self) {
        if self.wb.commissioning.is_some() || self.backend.document().is_none() {
            return;
        }
        let store = match self.backend.commissioning_json() {
            None => CommissioningStore::new(),
            Some(json) => match CommissioningStore::from_json(&json) {
                Ok(store) => store,
                Err(e) => {
                    self.message(
                        Level::Warning,
                        format!("Commissioning data not read, starting empty: {e}"),
                    );
                    CommissioningStore::new()
                }
            },
        };
        self.wb.commissioning = Some(CommissioningMode::new(store));
    }

    /// The marks of the mode for `page`: an outline on each passed object,
    /// the error mark on each failed one.
    pub(super) fn commissioning_marks(&mut self, page: PageId) -> Vec<Mark> {
        if self.outline_mode != OutlineMode::Commissioning {
            return Vec::new();
        }
        let (Some(mode), Some(doc)) = (self.wb.commissioning.as_ref(), self.backend.document())
        else {
            return Vec::new();
        };
        let project = doc.project();
        let mut marks = Vec::new();
        let infos = targets(project);
        for info in infos.iter().filter(|i| i.pages.contains(&page)) {
            let (kind, what) = match target_state(&mode.store, info.target) {
                CheckState::Failed => (MarkKind::Error, "✖ Failed"),
                CheckState::Passed => (MarkKind::Added, "✔ Passed"),
                _ => continue,
            };
            for id in shown_on(project, info.target.any_id(), page) {
                marks
                    .push(Mark::on(kind, item_id(id)).with_text(format!("{}: {what}", info.label)));
            }
        }
        // Open redlines show where the machine differs from the drawing.
        for redline in mode
            .store
            .redlines()
            .filter(|r| r.state == RedlineState::Open)
        {
            let Some(info) = infos
                .iter()
                .find(|i| i.target == redline.target && i.pages.contains(&page))
            else {
                continue;
            };
            for id in shown_on(project, info.target.any_id(), page) {
                marks.push(
                    Mark::on(MarkKind::Modified, item_id(id))
                        .with_text(format!("Redline: {}", redline.summary())),
                );
            }
        }
        marks
    }

    /// The Commissioning tab of the outline. Returns whether it should close.
    pub(super) fn commissioning_outline(&mut self, ui: &mut egui::Ui) -> bool {
        self.load_commissioning();
        let current = self.current_page;
        let mut go: Option<Place> = None;
        let mut export: Option<ReportFileFormat> = None;
        let mut field: Option<FieldAction> = None;
        let mut redline_action: Option<RedlineAction> = None;
        let tablet = self.commissioning_tablet();
        let photos_dir = self.field_photos_dir();
        {
            let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
            else {
                ui.weak("Open a project to commission it");
                return false;
            };
            let project = doc.project();
            mode.large = self.settings.large_buttons || tablet;
            mode.viewer.dir = photos_dir;
            touch_density(ui, mode.large);
            let mut tab = match mode.tab {
                Tab::Checks => 0,
                Tab::Punch => 1,
                Tab::Redlines => 2,
            };
            let mut large = mode.large;
            ui.horizontal(|ui| {
                let row = ui
                    .scope(|ui| widgets::tabs(ui, &mut tab, &["Checks", "Punch list", "Redlines"]));
                if row.inner {
                    mode.tab = [Tab::Checks, Tab::Punch, Tab::Redlines][tab];
                }
                // The tabs are one widget of equal thirds.
                let r = row.response.rect;
                let third = r.width() / 3.0;
                let part = |i: f32| {
                    egui::Rect::from_min_max(
                        egui::pos2(r.left() + third * i, r.top()),
                        egui::pos2(r.left() + third * (i + 1.0), r.bottom()),
                    )
                };
                note_rect(
                    ui.ctx(),
                    "commissioning.punch",
                    "Punch tab",
                    part(1.0),
                    row.inner && tab == 1,
                );
                note_rect(
                    ui.ctx(),
                    "commissioning.redlines",
                    "Redlines tab",
                    part(2.0),
                    row.inner && tab == 2,
                );
            });
            // The menu sits on its own row: beside the tabs it took their
            // room and cut the third one off in the narrow side panel.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (report, action) = header_menu(ui, mode, &mut large, tablet);
                    export = report;
                    field = action;
                });
            });
            if large != mode.large {
                mode.large = large;
                self.settings.large_buttons = large;
            }
            ui.add_space(space::M);
            progress_bar(
                ui,
                "Project",
                progress(project, &mode.store, Scope::Project),
            );
            // On a tablet the walk card needs the height: one bar is enough.
            let walking_on_tablet = tablet && mode.selected.is_some() && mode.tab == Tab::Checks;
            if let Some(page) = current
                && !walking_on_tablet
            {
                ui.add_space(space::S);
                progress_bar(
                    ui,
                    "This page",
                    progress(project, &mode.store, Scope::Page(page)),
                );
            }
            ui.add_space(space::M);
            match mode.tab {
                Tab::Checks => checks_tab(ui, mode, project, current, &mut go),
                Tab::Punch => punch_tab(ui, mode, project, current, &mut go),
                Tab::Redlines => {
                    redline_action = redlines_tab(ui, mode, project, current, &mut go);
                }
            }
            if mode.dirty {
                mode.dirty = false;
                let json = mode.store.to_json();
                self.backend.set_commissioning_json(&json);
            }
        }
        for a in super::cabinet_bar::take_noted(ui.ctx()) {
            self.anchor(a.id, a.rect, a.label);
            if a.clicked {
                self.learn_event(novo_learn::Event::Clicked(a.id.to_owned()));
            }
        }
        if let Some(mode) = self.wb.commissioning.as_mut() {
            viewer::window(ui.ctx(), &mut mode.viewer);
        }
        if let Some(place) = go {
            self.jump(place, true);
        }
        self.apply_walk_requests();
        if let Some(id) = self
            .wb
            .commissioning
            .as_mut()
            .and_then(|m| m.photo_for.take())
        {
            self.add_punch_photos(id);
        }
        if let Some(format) = export {
            self.export_commissioning_report(format);
        }
        match redline_action {
            Some(RedlineAction::Apply) => self.apply_redlines(false),
            Some(RedlineAction::ApplyAndIssue) => self.apply_redlines(true),
            None => {}
        }
        match field {
            Some(FieldAction::Export) => self.export_field_results(),
            Some(FieldAction::Import) => self.import_field_results(),
            Some(FieldAction::PunchPdf) => self.export_punch_list_pdf(),
            None => {}
        }
        false
    }

    /// Writes the field results (checks and punch items) to a `.kcfield`
    /// file the person picks, to carry to another machine.
    pub(super) fn export_field_results(&mut self) {
        let Some(name) = self.project_name() else {
            return;
        };
        self.load_commissioning();
        let (Some(mode), Some(doc)) = (self.wb.commissioning.as_ref(), self.backend.document())
        else {
            return;
        };
        let mut package = FieldPackage::new(&name, &mode.who, &now(), &mode.store, doc.project());
        self.pack_field_photos(&mut package);
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Export field results")
            .set_file_name(format!("{name} field results.kcfield"))
            .add_filter("KoperCAD field results", &["kcfield"]);
        if let Some(dir) = projects_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        let json = package.to_json();
        let size = if json.len() >= 5_000_000 {
            format!(" ({} MB with photos)", json.len() / 1_000_000)
        } else {
            String::new()
        };
        match write_replacing(&path, json.as_bytes()) {
            Ok(()) => self.message(Level::Info, format!("Exported {}{size}", path.display())),
            Err(e) => self.message(Level::Error, format!("Could not export field results: {e}")),
        }
    }

    /// Merges a `.kcfield` file into this project's commissioning data.
    pub(super) fn import_field_results(&mut self) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Import field results")
            .add_filter("KoperCAD field results", &["kcfield"]);
        if let Some(dir) = projects_dir() {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_field_results_from(&path);
        }
    }

    pub(super) fn import_field_results_from(&mut self, path: &Path) {
        self.load_commissioning();
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not read {}: {e}", path.display()),
                );
                return;
            }
        };
        let package = match FieldPackage::from_json(&text) {
            Ok(package) => package,
            Err(e) => {
                self.message(Level::Error, e.to_string());
                return;
            }
        };
        let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
        else {
            return;
        };
        let report = package.merge_into(&mut mode.store, doc.project());
        if report.changed() {
            mode.dirty = true;
        }
        self.unpack_field_photos(&package);
        let text = format!(
            "Imported field results from {}: {}",
            package.exported_by,
            report.summary()
        );
        self.message(Level::Info, text);
    }

    /// Does what the walk asked of the drawing and the views: selects the
    /// wire it moved to, or shows a wire in 3D.
    fn apply_walk_requests(&mut self) {
        let Some(mode) = self.wb.commissioning.as_mut() else {
            return;
        };
        let reveal = mode.reveal_wire.take();
        let show_3d = mode.show_3d.take();
        if let Some((page, wire)) = reveal {
            self.wb.pending_item = Some((page, novo_core::AnyId::Wire(wire)));
        }
        if let Some(wire) = show_3d {
            self.show_wire_in_3d(wire);
        }
    }

    /// Opens the 3D view with `wire` selected. The commissioning data is
    /// kept in the project in the frame it changes, so leaving is safe.
    fn show_wire_in_3d(&mut self, wire: WireId) {
        if let Some(mode) = self.wb.commissioning.as_mut()
            && mode.dirty
        {
            mode.dirty = false;
            let json = mode.store.to_json();
            self.backend.set_commissioning_json(&json);
        }
        self.switch_workspace(Workspace::View3d);
        if self.workspace() == Workspace::View3d {
            self.view3d.view.state.selection =
                std::collections::BTreeSet::from([novo_view3d::ItemId::new(
                    novo_view3d::ItemKind::Wire,
                    wire.raw(),
                )]);
        }
    }

    /// A tap of Space in the Commissioning workspace: the big tick, or the
    /// start of the walk when nothing is selected yet. Returns whether it
    /// was taken (otherwise Space opens the command field as everywhere).
    pub(super) fn commissioning_space(&mut self) -> bool {
        if !self.commissioning_pins_panel() {
            return false;
        }
        self.load_commissioning();
        let current = self.current_page;
        let mut go = None;
        let finished = {
            let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
            else {
                return false;
            };
            let finished = mode.tick(doc.project(), current, &mut go);
            mode.finished = finished;
            finished
        };
        if let Some(place) = go {
            self.jump(place, true);
        }
        self.apply_walk_requests();
        if finished {
            self.message(Level::Info, "Everything in the walk is decided");
        }
        true
    }

    /// The `export.punch_list` command: the open punch items as a PDF.
    pub(super) fn export_punch_list_pdf(&mut self) {
        let Some(name) = self.project_name() else {
            return;
        };
        self.load_commissioning();
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let Some(path) = report_path_for(
            "punch list",
            file.as_deref(),
            projects_dir().as_deref(),
            &name,
            "pdf",
        ) else {
            self.message(Level::Error, "Could not find the Documents folder");
            return;
        };
        let Some(mode) = self.wb.commissioning.as_ref() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let input = novo_commissioning::to_report_input(
            project,
            &mode.store,
            novo_reports::commissioning::default_sign_offs(&mode.who),
        );
        let report = novo_reports::commissioning::punch_list_report(&input);
        let options = ReportPdfOptions {
            title: format!("{name}, punch list"),
            footer_right: now().chars().take(10).collect(),
            language: self.report_translator(),
            ..ReportPdfOptions::default()
        };
        let result = novo_pdf::report_bytes(&report, ReportFileFormat::Pdf, &options)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_replacing(&path, &bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.message_file(format!("Exported {}", path.display()), &path);
                self.wb.last_export = Some(path);
            }
            Err(e) => self.message(
                Level::Error,
                format!("Could not export the punch list: {e}"),
            ),
        }
    }

    /// The `export.commissioning` command: the report as a PDF.
    pub(super) fn export_commissioning_pdf(&mut self) {
        self.export_commissioning_report(ReportFileFormat::Pdf);
    }

    /// Writes the commissioning report next to the project file (see
    /// [`report_path`]) and says where it went. Returns the path.
    pub(super) fn export_commissioning_report(
        &mut self,
        format: ReportFileFormat,
    ) -> Option<PathBuf> {
        let name = self.project_name()?;
        let ext = match format {
            ReportFileFormat::Pdf => "pdf",
            ReportFileFormat::Csv(_) => "csv",
            ReportFileFormat::Xlsx => "xlsx",
        };
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let Some(path) = report_path(file.as_deref(), projects_dir().as_deref(), &name, ext) else {
            self.message(Level::Error, "Could not find the Documents folder");
            return None;
        };
        self.write_commissioning_report(format, &path)
    }

    /// Writes the commissioning report to `path`.
    pub(super) fn write_commissioning_report(
        &mut self,
        format: ReportFileFormat,
        path: &Path,
    ) -> Option<PathBuf> {
        let name = self.project_name()?;
        self.load_commissioning();
        let mode = self.wb.commissioning.as_ref()?;
        let project = self.backend.document()?.project();
        let input = novo_commissioning::to_report_input(
            project,
            &mode.store,
            novo_reports::commissioning::default_sign_offs(&mode.who),
        );
        let report = novo_reports::commissioning::commissioning_report(&input);
        let options = ReportPdfOptions {
            title: format!("{name}, commissioning"),
            footer_right: now().chars().take(10).collect(),
            language: self.report_translator(),
            ..ReportPdfOptions::default()
        };
        let result = novo_pdf::report_bytes(&report, format, &options)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_replacing(path, &bytes).map_err(|e| e.to_string()));
        match result {
            Ok(()) => {
                self.message_file(format!("Exported {}", path.display()), path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export the report: {e}"));
                None
            }
        }
    }
}

/// Where the commissioning report goes: `<project> commissioning.<ext>`
/// beside the project file, or in the Documents folder for a project with no
/// file yet. A PDF KoperCAD wrote is replaced; anything else that is already
/// there stays and the report takes the next free `<name> 2.<ext>`.
pub(super) fn report_path(
    file: Option<&Path>,
    dir: Option<&Path>,
    name: &str,
    ext: &str,
) -> Option<PathBuf> {
    report_path_for("commissioning", file, dir, name, ext)
}

/// [`report_path`] for a report called `kind` (`<project> <kind>.<ext>`).
pub(super) fn report_path_for(
    kind: &str,
    file: Option<&Path>,
    dir: Option<&Path>,
    name: &str,
    ext: &str,
) -> Option<PathBuf> {
    let base = match file {
        Some(file) => {
            let stem = file.file_stem()?.to_string_lossy().into_owned();
            file.with_file_name(format!("{stem} {kind}.{ext}"))
        }
        None => {
            let stem = Path::new(&super::project_file_name(name))
                .with_extension("")
                .to_string_lossy()
                .into_owned();
            dir?.join(format!("{stem} {kind}.{ext}"))
        }
    };
    if ext == "pdf" {
        return Some(replaceable_path(&base));
    }
    let stem = base.file_stem()?.to_string_lossy().into_owned();
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            base.clone()
        } else {
            base.with_file_name(format!("{stem} {n}.{ext}"))
        };
        if !candidate.exists() {
            return Some(candidate);
        }
        n += 1;
    }
}

/// The one menu in the panel header: who is deciding, the report, and the
/// Large switch. The button shows the name, so the person sees who the next
/// decision is recorded for without opening it. Returns the report to write.
fn header_menu(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    large: &mut bool,
    tablet: bool,
) -> (Option<ReportFileFormat>, Option<FieldAction>) {
    let mut export = None;
    let mut field = None;
    let name = if mode.who.trim().is_empty() {
        "Your name".to_owned()
    } else {
        format!("By {}", mode.who.trim())
    };
    let menu = ui.menu_button(format!("{name}  {}", icons::DOTS_THREE), |ui| {
        ui.set_width(220.0);
        widgets::section_label(ui, "By");
        let who = ui.add(egui::TextEdit::singleline(&mut mode.who).desired_width(f32::INFINITY));
        note_ctx(ui.ctx(), "commissioning.who", "By menu", &who);
        ui.add_space(space::S);
        widgets::section_label(ui, "Report");
        for (label, format) in [
            ("Write PDF report", ReportFileFormat::Pdf),
            (
                "Write CSV report",
                ReportFileFormat::Csv(CsvOptions::default()),
            ),
        ] {
            let item = widgets::menu_item(ui, Some(icons::FILE_TEXT), label, None, true)
                .on_hover_text("Write the commissioning report next to the project file");
            if matches!(format, ReportFileFormat::Pdf) {
                note_ctx(
                    ui.ctx(),
                    "commissioning.report",
                    "Write PDF report item",
                    &item,
                );
            }
            if item.clicked() {
                export = Some(format);
                ui.close();
            }
        }
        let punch = widgets::menu_item(
            ui,
            Some(icons::FILE_TEXT),
            "Write punch list PDF",
            None,
            true,
        )
        .on_hover_text("Only the open punch items, to carry round the cabinet");
        if punch.clicked() {
            field = Some(FieldAction::PunchPdf);
            ui.close();
        }
        ui.add_space(space::S);
        widgets::section_label(ui, "Field results");
        for (label, action, hint, id, name) in [
            (
                "Export field results",
                FieldAction::Export,
                "Save your ticks and punch items as a file to carry to the desk (no cloud)",
                "commissioning.field_export",
                "Export field results item",
            ),
            (
                "Import field results",
                FieldAction::Import,
                "Merge a field results file into this project; newer decisions win",
                "commissioning.field_import",
                "Import field results item",
            ),
        ] {
            let item = widgets::menu_item(ui, Some(icons::FILE_TEXT), label, None, true)
                .on_hover_text(hint);
            note_ctx(ui.ctx(), id, name, &item);
            if item.clicked() {
                field = Some(action);
                ui.close();
            }
        }
        ui.add_space(space::S);
        // The tablet layout always has them on.
        ui.add_enabled(!tablet, egui::Checkbox::new(large, "Large buttons"));
    });
    // The button stands in for the field and the PDF item until it is open.
    if menu.inner.is_none() {
        note_ctx(ui.ctx(), "commissioning.who", "By menu", &menu.response);
        note_ctx(
            ui.ctx(),
            "commissioning.report",
            "Write PDF report item",
            &menu.response,
        );
        note_ctx(
            ui.ctx(),
            "commissioning.field_export",
            "Export field results item",
            &menu.response,
        );
        note_ctx(
            ui.ctx(),
            "commissioning.field_import",
            "Import field results item",
            &menu.response,
        );
    }
    (export, field)
}

fn checks_tab(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    project: &Project,
    current: Option<PageId>,
    go: &mut Option<Place>,
) {
    if let Some((target, _)) = mode.fail_pending
        && Some(target) != mode.selected
    {
        // The walk moved on before the reason was given.
        mode.fail_pending = None;
        mode.note.clear();
    }
    let all = targets(project);
    let pages = project.pages_in_order();
    let plan = mode.plan(project);
    let next = mode.next(project, mode.selected, current);
    ui.horizontal(|ui| {
        let next_open_button = widgets::primary_button(ui, "Next open")
            .on_hover_text("Go to the next wire, terminal or device with an open check");
        note_ctx(
            ui.ctx(),
            "commissioning.next_open",
            "Next open button",
            &next_open_button,
        );
        if next_open_button.clicked() {
            match &next {
                Some(info) => mode.walk_to(project, info, current, go),
                None => mode.selected = None,
            }
        }
        let mut wires_only = mode.scope == WalkScope::Wires;
        if ui
            .checkbox(&mut wires_only, "Wires only")
            .on_hover_text("Walk the wires one by one; leave terminals and devices for later")
            .changed()
        {
            mode.scope = if wires_only {
                WalkScope::Wires
            } else {
                WalkScope::All
            };
        }
        let mut by_place = mode.order == WalkOrder::Place;
        if ui
            .checkbox(&mut by_place, "By place")
            .on_hover_text("Walk the wires by the device or strip they start on, the way the hand moves at the cabinet")
            .changed()
        {
            mode.order = if by_place {
                WalkOrder::Place
            } else {
                WalkOrder::Number
            };
        }
        if next.is_none() || mode.finished {
            let wires_done = mode.scope == WalkScope::Wires
                && next_open(project, &mode.store, None, None, WalkScope::All).is_some();
            ui.weak(if wires_done {
                "Wires done. Untick Wires only for terminals and devices"
            } else {
                "Everything is decided. Check the Punch list"
            });
        }
    });
    let p = Palette::of(ui);

    if let Some(info) = all.iter().find(|i| Some(i.target) == mode.selected) {
        ui.separator();
        let step = match info.target {
            Target::Wire(w) => plan.step(w),
            _ => None,
        };
        // The card keeps to part of the panel and scrolls inside itself, so
        // the list below always has room (on a tablet it filled the column).
        let card_height = (ui.available_height() - 70.0).max(200.0);
        egui::ScrollArea::vertical()
            .id_salt("walk_card")
            .max_height(card_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                walk_card(ui, mode, project, info, step, current, go);
                ui.add_space(space::S);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut mode.value)
                            .hint_text("Value")
                            .desired_width(80.0),
                    );
                    let note = ui.add(
                        egui::TextEdit::singleline(&mut mode.note)
                            .hint_text("Note")
                            .desired_width(f32::INFINITY),
                    );
                    if std::mem::take(&mut mode.focus_note) {
                        note.request_focus();
                        // Cursor after the prefilled ends, ready to type the reason.
                        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), note.id) {
                            let end = egui::text::CCursor::new(mode.note.chars().count());
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(end)));
                            state.store(ui.ctx(), note.id);
                        }
                    }
                    if note.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && let Some((target, check)) = mode.fail_pending
                    {
                        mode.fail_pending = None;
                        mode.decide(target, check, CheckState::Failed);
                    }
                });
                if let Some((_, check)) = mode.fail_pending {
                    ui.weak(format!(
                        "Type why {} failed, then Enter or Fail again",
                        check.label().to_lowercase()
                    ));
                }
                let fail_hint = match step {
                    Some(s) if !s.from.is_empty() || !s.to.is_empty() => {
                        format!("{} to {}", s.from, s.to)
                    }
                    _ => info.label.clone(),
                };
                walk_keys(ui.ctx(), mode, project, info, &fail_hint, current, go);
                let first_check = info.target.checks().first().copied();
                for &check in info.target.checks() {
                    let first = Some(check) == first_check;
                    let state = mode.store.state(info.target, check);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = space::S;
                        let (icon, color) = state_icon(p, state);
                        ui.label(egui::RichText::new(icon).color(color));
                        ui.label(check.label());
                        let pass = ui.button("Pass");
                        let fail = ui.button("Fail");
                        if first {
                            note_ctx(ui.ctx(), "commissioning.pass", "Pass button", &pass);
                            note_ctx(ui.ctx(), "commissioning.fail", "Fail button", &fail);
                        }
                        if pass.clicked() {
                            mode.decide(info.target, check, CheckState::Passed);
                        }
                        if fail.clicked() {
                            mode.fail(info.target, check, &fail_hint);
                        }
                        if ui.button("N/A").clicked() {
                            mode.decide(info.target, check, CheckState::NotApplicable);
                        }
                        if state != CheckState::Pending && ui.button("Reset").clicked() {
                            mode.store.clear_check(info.target, check);
                            mode.dirty = true;
                        }
                    });
                    // A failure raises a punch item by itself; say so and link to it.
                    if state == CheckState::Failed {
                        let item = mode
                            .store
                            .punch_items()
                            .find(|p| {
                                p.target == Some(info.target)
                                    && p.check == Some(check)
                                    && !p.state.is_closed()
                            })
                            .map(|p| (p.title.clone(), severity_label(p.severity)));
                        if let Some((title, severity)) = item {
                            ui.horizontal_wrapped(|ui| {
                                ui.weak(format!("Punch item raised: {title} ({severity})"));
                                if ui.small_button("Open punch list").clicked() {
                                    mode.tab = Tab::Punch;
                                }
                            });
                        }
                    }
                    if let Some(r) = mode.store.record(info.target, check)
                        && !r.by.is_empty()
                    {
                        let detail = [r.value.as_str(), r.note.as_str()]
                            .iter()
                            .filter(|t| !t.is_empty())
                            .copied()
                            .collect::<Vec<_>>()
                            .join(" · ");
                        ui.weak(format!("{}, {} {detail}", r.by, r.at));
                    }
                }
                pass_all_open(ui, mode, info.target);
            });
        ui.separator();
    }

    if current != mode.unfolded_for {
        mode.unfolded_for = current;
        mode.open_pages.extend(current);
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for page in &pages {
                let on_page: Vec<&TargetInfo> =
                    all.iter().filter(|i| i.pages.contains(&page.id)).collect();
                if on_page.is_empty() {
                    continue;
                }
                let open = mode.open_pages.contains(&page.id);
                let done = progress(project, &mode.store, Scope::Page(page.id));
                let name = if page.title.is_empty() {
                    page.name.clone()
                } else {
                    format!("{} · {}", page.name, page.title)
                };
                let head = widgets::row(ui, false, |ui| {
                    ui.label(
                        egui::RichText::new(if open {
                            icons::CARET_DOWN
                        } else {
                            icons::CARET_RIGHT
                        })
                        .color(p.text_tertiary),
                    );
                    widgets::section_label(ui, &name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let color = if done.failed > 0 {
                            p.error
                        } else {
                            p.text_secondary
                        };
                        ui.label(
                            egui::RichText::new(format!("{}/{}", done.done(), done.total))
                                .monospace()
                                .color(color),
                        );
                    });
                });
                if head.clicked() {
                    if open {
                        mode.open_pages.remove(&page.id);
                    } else {
                        mode.open_pages.insert(page.id);
                    }
                }
                if !open {
                    continue;
                }
                let mut on_page = on_page;
                // Plain wires, then cable cores, each by number, then the
                // terminals and devices: a number drawn as both a wire and
                // a core sits in two groups instead of looking like a repeat.
                on_page.sort_by(|a, b| {
                    let (ga, gb) = (target_group(a.target, &plan), target_group(b.target, &plan));
                    ga.cmp(&gb).then_with(|| match (a.target, b.target) {
                        (Target::Wire(_), Target::Wire(_)) => {
                            novo_core::natural_cmp(&a.label, &b.label)
                        }
                        _ => std::cmp::Ordering::Equal,
                    })
                });
                let mut group = None;
                for info in on_page {
                    let kind = target_group(info.target, &plan);
                    if group != Some(kind) {
                        group = Some(kind);
                        ui.horizontal(|ui| {
                            ui.add_space(space::XL);
                            ui.label(
                                egui::RichText::new(kind.heading())
                                    .small()
                                    .color(p.text_tertiary),
                            );
                        });
                    }
                    let state = target_state(&mode.store, info.target);
                    let decided = info
                        .target
                        .checks()
                        .iter()
                        .filter(|&&c| mode.store.state(info.target, c) != CheckState::Pending)
                        .count();
                    let step = match info.target {
                        Target::Wire(w) => plan.step(w),
                        _ => None,
                    };
                    let height = ui.spacing().interact_size.y + 2.0;
                    // On a tablet a wire row has a second line with its ends and
                    // is tall enough for a finger; at the desk it stays one
                    // line so the list shows as many wires as it can.
                    let two_lines = step.is_some() && mode.large;
                    let height = if two_lines { height + 24.0 } else { height };
                    let row = widgets::row_of_height(
                        ui,
                        mode.selected == Some(info.target),
                        height,
                        |ui| {
                            ui.add_space(space::XL);
                            let (icon, color) = state_icon(p, state);
                            ui.label(egui::RichText::new(icon).color(color));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{decided}/{}",
                                            info.target.checks().len()
                                        ))
                                        .monospace()
                                        .color(p.text_tertiary),
                                    );
                                    ui.with_layout(
                                        egui::Layout::left_to_right(egui::Align::Center),
                                        |ui| match step {
                                            Some(step) => wire_row_text(ui, p, step, two_lines),
                                            None => {
                                                ui.add(egui::Label::new(&info.label).truncate());
                                            }
                                        },
                                    );
                                },
                            );
                        },
                    );
                    if row.clicked() {
                        mode.walk_to(project, info, current, go);
                    }
                }
            }
        });
}

/// The text of a wire's row: the number and what the wire is, and under it
/// where it goes.
fn wire_row_text(ui: &mut egui::Ui, p: &Palette, step: &WireStep, two_lines: bool) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.horizontal(|ui| {
            let number = if step.number.is_empty() {
                "no number"
            } else {
                &step.number
            };
            ui.add(egui::Label::new(egui::RichText::new(number).strong()).truncate());
            ui.label(
                egui::RichText::new(step.kind.short())
                    .small()
                    .color(p.text_tertiary),
            );
        });
        if !two_lines {
            return;
        }
        let ends = if step.from.is_empty() && step.to.is_empty() {
            "open ends".to_owned()
        } else {
            format!("{}  \u{2192}  {}", step.from, step.to)
        };
        ui.add(
            egui::Label::new(egui::RichText::new(ends).small().color(p.text_secondary)).truncate(),
        );
    });
}

/// The big card of the walk: what the selected target is and where it goes,
/// and the tick. A wire shows its number, what it is (wire or cable core),
/// both ends, the ferrule text, length, size and colour; a terminal or
/// device shows its tag. Space does the same as the big button.
fn walk_card(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    project: &Project,
    info: &TargetInfo,
    step: Option<&WireStep>,
    current: Option<PageId>,
    go: &mut Option<Place>,
) {
    let p = Palette::of(ui);
    let open = info
        .target
        .checks()
        .iter()
        .filter(|&&c| mode.store.state(info.target, c) == CheckState::Pending)
        .count();
    let plan = mode.plan(project);
    let order = walk_order_with(project, &plan, mode.scope, mode.order);
    if let Some(at) = order.iter().position(|i| i.target == info.target) {
        ui.label(
            egui::RichText::new(format!("{} of {}", at + 1, order.len()))
                .small()
                .color(p.text_tertiary),
        );
    }
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(if let Some(s) = step.filter(|s| !s.number.is_empty()) {
                s.number.clone()
            } else {
                info.label.clone()
            })
            .size(26.0)
            .strong()
            .color(p.text_strong),
        );
        if let Some(step) = step {
            let (text, color) = match &step.kind {
                WireKind::Plain => ("wire".to_owned(), p.text_secondary),
                WireKind::Core { .. } => (step.kind.short(), p.accent_hover),
            };
            ui.label(egui::RichText::new(text).color(color));
        } else {
            ui.label(egui::RichText::new(target_group_name(info.target)).color(p.text_secondary));
        }
    });
    if let Some(step) = step {
        let end =
            |ui: &mut egui::Ui, caption: &str, designation: &str, ferrule: &Option<String>| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(caption).small().color(p.text_tertiary));
                    let text = if designation.is_empty() {
                        "open end"
                    } else {
                        designation
                    };
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(text)
                                .size(17.0)
                                .monospace()
                                .color(p.text_primary),
                        )
                        .truncate(),
                    );
                    let (text, color) = match ferrule {
                        Some(f) => (format!("ferrule {f}"), p.text_secondary),
                        None => ("no ferrule".to_owned(), p.text_tertiary),
                    };
                    ui.label(egui::RichText::new(text).small().color(color));
                });
            };
        end(ui, "FROM", &step.from, &step.ferrule_from);
        end(ui, "TO  ", &step.to, &step.ferrule_to);
        let mut facts = Vec::new();
        if let Some(mm) = step.length_mm {
            facts.push(format!("cut length {mm:.0} mm"));
        }
        if let Some(s) = step.cross_section_mm2 {
            facts.push(format!("{s} mm\u{b2}"));
        }
        if !step.colour.is_empty() {
            facts.push(step.colour.clone());
        }
        if !facts.is_empty() {
            ui.label(egui::RichText::new(facts.join("  ·  ")).color(p.text_secondary));
        }
        let hint = step.same_number_hint();
        if !hint.is_empty() {
            ui.label(egui::RichText::new(hint).small().color(p.text_tertiary));
        }
    } else {
        let pages = info
            .pages
            .iter()
            .filter_map(|&pg| project.page(pg).map(|pg| pg.name.clone()))
            .collect::<Vec<_>>()
            .join(", ");
        if !pages.is_empty() {
            ui.label(egui::RichText::new(format!("Page {pages}")).color(p.text_secondary));
        }
    }
    ui.add_space(space::S);
    let label = if open == 0 {
        "All decided".to_owned()
    } else {
        let names: Vec<&str> = info
            .target
            .checks()
            .iter()
            .filter(|&&c| mode.store.state(info.target, c) == CheckState::Pending)
            .map(|c| c.label())
            .collect();
        format!("Pass {} (Space)", names.join(" + ").to_lowercase())
    };
    let tick = ui.add_enabled(
        open > 0,
        egui::Button::new(
            egui::RichText::new(label)
                .size(18.0)
                .strong()
                .color(p.on_accent),
        )
        .fill(p.accent)
        .stroke(egui::Stroke::NONE)
        .min_size(egui::vec2(ui.available_width(), 52.0)),
    );
    let vouch = info
        .target
        .checks()
        .iter()
        .filter(|&&c| mode.store.state(info.target, c) == CheckState::Pending)
        .map(|c| c.label())
        .collect::<Vec<_>>()
        .join(" and ");
    if tick
        .on_hover_text(format!(
            "Records {vouch} as passed by {} and goes to the next one",
            if mode.who.trim().is_empty() {
                "you"
            } else {
                mode.who.trim()
            }
        ))
        .clicked()
    {
        let finished = mode.tick(project, current, go);
        mode.finished = finished;
    }
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                !mode.walked.is_empty(),
                egui::Button::new("Back").min_size(egui::vec2(0.0, 40.0)),
            )
            .on_hover_text("Take back the last pass")
            .clicked()
        {
            mode.back(project, current, go);
        }
        if ui
            .add(egui::Button::new("Skip").min_size(egui::vec2(0.0, 40.0)))
            .on_hover_text("Go to the next one without deciding this one")
            .clicked()
            && let Some(next) = mode.next(project, mode.selected, current)
        {
            mode.walk_to(project, &next, current, go);
        }
        if let Target::Wire(w) = info.target
            && ui
                .add(
                    egui::Button::new(format!("{} 3D", icons::canvas::VIEW_3D))
                        .min_size(egui::vec2(0.0, 40.0)),
                )
                .on_hover_text("Show this wire in the 3D view")
                .clicked()
        {
            mode.show_3d = Some(w);
        }
    });
    ui.weak("F fail · N n/a · Backspace back");
}

/// F fails, N marks not applicable (the first open check of the target in
/// the card) and Backspace takes back the last pass. Only while no text
/// field has the keyboard.
fn walk_keys(
    ctx: &egui::Context,
    mode: &mut CommissioningMode,
    project: &Project,
    info: &TargetInfo,
    fail_hint: &str,
    current: Option<PageId>,
    go: &mut Option<Place>,
) {
    if ctx.egui_wants_keyboard_input() {
        return;
    }
    let open = info
        .target
        .checks()
        .iter()
        .copied()
        .find(|&c| mode.store.state(info.target, c) == CheckState::Pending);
    let (fail, na, back) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::F),
            i.consume_key(egui::Modifiers::NONE, egui::Key::N),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace),
        )
    });
    if let Some(check) = open {
        if fail {
            mode.fail(info.target, check, fail_hint);
        } else if na {
            mode.decide(info.target, check, CheckState::NotApplicable);
        }
    }
    if back {
        mode.back(project, current, go);
    }
}

/// The sub-heading a target sits under in the checks list, so a bare wire
/// number reads as a wire, and a core of a cable is not mistaken for a
/// repeat of the wire with the same number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Group {
    Wires,
    CableCores,
    Terminals,
    Devices,
}

impl Group {
    fn heading(self) -> &'static str {
        match self {
            Group::Wires => "Wires",
            Group::CableCores => "Cable cores",
            Group::Terminals => "Terminals",
            Group::Devices => "Devices",
        }
    }
}

fn target_group(target: Target, plan: &WirePlan) -> Group {
    match target {
        Target::Wire(w) => match plan.step(w).map(|s| &s.kind) {
            Some(WireKind::Core { .. }) => Group::CableCores,
            _ => Group::Wires,
        },
        Target::Terminal(_) => Group::Terminals,
        Target::Device(_) => Group::Devices,
    }
}

fn target_group_name(target: Target) -> &'static str {
    match target {
        Target::Wire(_) => "wire",
        Target::Terminal(_) => "terminal",
        Target::Device(_) => "device",
    }
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Minor => "Minor",
        Severity::Major => "Major",
        Severity::Blocking => "Blocking",
    }
}

fn punch_tab(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    project: &Project,
    current: Option<PageId>,
    go: &mut Option<Place>,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut mode.punch_title)
                .hint_text("New punch item")
                .desired_width(f32::INFINITY),
        );
    });
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("punch_severity")
            .selected_text(severity_label(mode.punch_severity))
            .show_ui(ui, |ui| {
                for s in [Severity::Minor, Severity::Major, Severity::Blocking] {
                    ui.selectable_value(&mut mode.punch_severity, s, severity_label(s));
                }
            });
        let can_add = !mode.punch_title.trim().is_empty();
        if ui.add_enabled(can_add, egui::Button::new("Add")).clicked() {
            mode.store.add_punch(PunchItem {
                id: 0,
                title: std::mem::take(&mut mode.punch_title).trim().to_owned(),
                note: String::new(),
                severity: mode.punch_severity,
                state: PunchState::Open,
                target: mode.selected,
                check: None,
                raised_by: mode.who.clone(),
                raised_at: now(),
                closed_by: None,
                closed_at: None,
                photos: Vec::new(),
                signature: None,
            });
            mode.dirty = true;
        }
        ui.checkbox(&mut mode.show_closed, "Show closed");
    });
    let all = targets(project);
    let items: Vec<PunchItem> = mode
        .store
        .punch_items()
        .filter(|p| mode.show_closed || !p.state.is_closed())
        .cloned()
        .collect();
    if items.is_empty() {
        ui.weak("No open punch items");
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let p = Palette::of(ui);
            for item in &items {
                let info = item.target.and_then(|t| all.iter().find(|i| i.target == t));
                let (icon, color) = match item.severity {
                    Severity::Blocking => (icons::WARNING_OCTAGON, p.error),
                    Severity::Major => (icons::WARNING, p.warning),
                    Severity::Minor => (icons::CIRCLE, p.text_tertiary),
                };
                let row = widgets::row(ui, false, |ui| {
                    ui.label(egui::RichText::new(icon).color(color));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("{:?}", item.state))
                                .size(12.0)
                                .color(p.text_secondary),
                        );
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            let label = info.map_or(String::new(), |i| format!(" · {}", i.label));
                            ui.add(
                                egui::Label::new(format!(
                                    "{} · {}{label}",
                                    severity_label(item.severity),
                                    item.title
                                ))
                                .truncate(),
                            )
                        });
                    });
                })
                .on_hover_text(severity_label(item.severity));
                if row.clicked()
                    && let Some(info) = info
                {
                    mode.selected = Some(info.target);
                    *go = place_of(project, info, current);
                }
                ui.horizontal_wrapped(|ui| {
                    let moves: &[(&str, PunchState)] = match item.state {
                        PunchState::Open => &[("Fixed", PunchState::Fixed)],
                        PunchState::Fixed => &[
                            ("Verified", PunchState::Verified),
                            ("Reopen", PunchState::Open),
                        ],
                        PunchState::Verified | PunchState::WontFix => {
                            &[("Reopen", PunchState::Open)]
                        }
                    };
                    for &(label, state) in moves {
                        let button = ui.button(label);
                        if label == "Fixed" || label == "Verified" {
                            note_ctx(ui.ctx(), "commissioning.resolve", "Resolve button", &button);
                        }
                        if button.clicked() && state == PunchState::Verified {
                            // Verifying is signed: the pad opens under the item.
                            mode.signing = Some(SignDraft::new(item.id, &mode.who));
                        } else if button.clicked()
                            && mode
                                .store
                                .set_punch_state(item.id, state, &mode.who, &now())
                                .is_ok()
                        {
                            mode.dirty = true;
                        }
                    }
                    if !item.state.is_closed()
                        && item.state != PunchState::Fixed
                        && ui.button("Won't fix").clicked()
                        && mode
                            .store
                            .set_punch_state(item.id, PunchState::WontFix, &mode.who, &now())
                            .is_ok()
                    {
                        mode.dirty = true;
                    }
                    let photo = ui.button("Add photo").on_hover_text(
                        "Attach pictures of the issue; they travel in the field results file",
                    );
                    note_ctx(ui.ctx(), "commissioning.photo", "Add photo button", &photo);
                    if photo.clicked() {
                        mode.photo_for = Some(item.id);
                    }
                });
                if mode.signing.as_ref().is_some_and(|d| d.item == item.id) {
                    signing_area(ui, mode, item.id);
                }
                if let Some(sig) = &item.signature {
                    ui.weak(format!("Signed by {} · {}", sig.name, sig.at));
                    signature::preview(ui, sig);
                }
                if !item.photos.is_empty() {
                    viewer::strip(ui, &mut mode.viewer, &item.photos);
                }
                ui.add_space(space::S);
            }
        });
}

/// The pad, the name and the buttons that finish or drop a sign-off.
fn signing_area(ui: &mut egui::Ui, mode: &mut CommissioningMode, id: u64) {
    let Some(draft) = mode.signing.as_mut() else {
        return;
    };
    ui.add(
        egui::TextEdit::singleline(&mut draft.name)
            .hint_text("Name of the person verifying")
            .desired_width(f32::INFINITY),
    );
    signature::pad(ui, draft);
    let (mut sign, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        let can = !draft.name.trim().is_empty();
        let button = ui.add_enabled(can, egui::Button::new("Sign and verify"));
        note_ctx(
            ui.ctx(),
            "commissioning.sign",
            "Sign and verify button",
            &button,
        );
        sign = button.clicked();
        if ui
            .add_enabled(!draft.is_blank(), egui::Button::new("Clear"))
            .clicked()
        {
            draft.clear();
        }
        cancel = ui.button("Cancel").clicked();
    });
    if sign {
        let signed = Signature::from_strokes(&draft.name, &now(), draft.strokes());
        if mode.store.sign_off(id, signed).is_ok() {
            mode.dirty = true;
        }
        mode.signing = None;
    } else if cancel {
        mode.signing = None;
    }
}
