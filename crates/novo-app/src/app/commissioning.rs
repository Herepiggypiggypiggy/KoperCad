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
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use novo_canvas::marks::{Mark, MarkKind};
use novo_canvas::model::item_id;
use novo_commissioning::{
    CheckKind, CheckRecord, CheckState, CommissioningStore, FieldPackage, Progress, PunchItem,
    PunchState, Scope, Severity, Target, TargetInfo, progress, targets,
};
use novo_core::{PageId, Project};
use novo_pdf::{ReportFileFormat, ReportPdfOptions};
use novo_reports::CsvOptions;
use novo_theme::{Density, Palette, icons, space, widgets};
use novo_xref::Place;

use super::cabinet_bar::{note_ctx, note_rect};

use super::changes::shown_on;
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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Checks,
    Punch,
}

/// Commissioning mode's data and panel state.
pub(crate) struct CommissioningMode {
    pub(super) store: CommissioningStore,
    /// Who the decisions are recorded for.
    pub(super) who: String,
    tab: Tab,
    pub(super) selected: Option<Target>,
    /// The measured value and note for the next decision.
    value: String,
    note: String,
    punch_title: String,
    punch_severity: Severity,
    show_closed: bool,
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
    dirty: bool,
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
            open_pages: HashSet::new(),
            unfolded_for: None,
            large: false,
            armed: None,
            dirty: false,
        }
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
fn now() -> String {
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
fn place_of(project: &Project, info: &TargetInfo, current: Option<PageId>) -> Option<Place> {
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
    fn load_commissioning(&mut self) {
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
        for info in targets(project).iter().filter(|i| i.pages.contains(&page)) {
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
        marks
    }

    /// The Commissioning tab of the outline. Returns whether it should close.
    pub(super) fn commissioning_outline(&mut self, ui: &mut egui::Ui) -> bool {
        self.load_commissioning();
        let current = self.current_page;
        let mut go: Option<Place> = None;
        let mut export: Option<ReportFileFormat> = None;
        let mut field: Option<FieldAction> = None;
        let tablet = self.commissioning_tablet();
        {
            let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
            else {
                ui.weak("Open a project to commission it");
                return false;
            };
            let project = doc.project();
            mode.large = self.settings.large_buttons || tablet;
            touch_density(ui, mode.large);
            let mut tab = usize::from(mode.tab == Tab::Punch);
            let mut large = mode.large;
            ui.horizontal(|ui| {
                let row = ui.scope(|ui| widgets::tabs(ui, &mut tab, &["Checks", "Punch list"]));
                if row.inner {
                    mode.tab = [Tab::Checks, Tab::Punch][tab];
                }
                // The tabs are one widget; the Punch tab is the right half.
                let r = row.response.rect;
                let punch = egui::Rect::from_min_max(egui::pos2(r.center().x, r.top()), r.max);
                note_rect(ui.ctx(), "commissioning.punch", "Punch tab", punch, false);
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
            if let Some(page) = current {
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
        if let Some(place) = go {
            self.jump(place, true);
        }
        if let Some(format) = export {
            self.export_commissioning_report(format);
        }
        match field {
            Some(FieldAction::Export) => self.export_field_results(),
            Some(FieldAction::Import) => self.import_field_results(),
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
        let package = FieldPackage::new(&name, &mode.who, &now(), &mode.store, doc.project());
        let mut dialog = rfd::FileDialog::new()
            .set_title("Export field results")
            .set_file_name(format!("{name} field results.kcfield"))
            .add_filter("KoperCAD field results", &["kcfield"]);
        if let Some(dir) = projects_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(path) = dialog.save_file() else {
            return;
        };
        match write_replacing(&path, package.to_json().as_bytes()) {
            Ok(()) => self.message(Level::Info, format!("Exported {}", path.display())),
            Err(e) => self.message(Level::Error, format!("Could not export field results: {e}")),
        }
    }

    /// Merges a `.kcfield` file into this project's commissioning data.
    pub(super) fn import_field_results(&mut self) {
        let mut dialog = rfd::FileDialog::new()
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
        let text = format!(
            "Imported field results from {}: {}",
            package.exported_by,
            report.summary()
        );
        self.message(Level::Info, text);
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
        "By".to_owned()
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
    let all = targets(project);
    let pages = project.pages_in_order();
    // The next open target: the current page first, then the others in page order.
    let is_open = |mode: &CommissioningMode, i: &TargetInfo| {
        i.target
            .checks()
            .iter()
            .any(|&c| mode.store.state(i.target, c) == CheckState::Pending)
    };
    let next = all
        .iter()
        .find(|i| current.is_some_and(|p| i.pages.contains(&p)) && is_open(mode, i))
        .or_else(|| {
            pages.iter().find_map(|page| {
                all.iter()
                    .find(|i| i.pages.contains(&page.id) && is_open(mode, i))
            })
        })
        .cloned();
    ui.horizontal(|ui| {
        let next_open = widgets::primary_button(ui, "Next open")
            .on_hover_text("Select the next wire, terminal or device with an open check");
        note_ctx(
            ui.ctx(),
            "commissioning.next_open",
            "Next open button",
            &next_open,
        );
        if next_open.clicked() {
            match &next {
                Some(info) => {
                    mode.selected = Some(info.target);
                    *go = place_of(project, info, current);
                }
                None => mode.selected = None,
            }
        }
        if next.is_none() {
            ui.weak("Everything is decided");
        }
    });
    let p = Palette::of(ui);

    if let Some(info) = all.iter().find(|i| Some(i.target) == mode.selected) {
        ui.separator();
        ui.strong(&info.label);
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut mode.value)
                    .hint_text("Value (2.5 Nm, 0.2 ohm)")
                    .desired_width(130.0),
            );
            ui.add(
                egui::TextEdit::singleline(&mut mode.note)
                    .hint_text("Note")
                    .desired_width(f32::INFINITY),
            );
        });
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
                    mode.decide(info.target, check, CheckState::Failed);
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
                for info in on_page {
                    let state = target_state(&mode.store, info.target);
                    let decided = info
                        .target
                        .checks()
                        .iter()
                        .filter(|&&c| mode.store.state(info.target, c) != CheckState::Pending)
                        .count();
                    let row = widgets::row(ui, mode.selected == Some(info.target), |ui| {
                        ui.add_space(space::XL);
                        let (icon, color) = state_icon(p, state);
                        ui.label(egui::RichText::new(icon).color(color));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                                |ui| ui.add(egui::Label::new(&info.label).truncate()),
                            );
                        });
                    });
                    if row.clicked() {
                        mode.selected = Some(info.target);
                        *go = place_of(project, info, current);
                    }
                }
            }
        });
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
                        if button.clicked()
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
                });
                ui.add_space(space::S);
            }
        });
}
