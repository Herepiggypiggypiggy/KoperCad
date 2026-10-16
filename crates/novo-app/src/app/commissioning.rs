//! Commissioning mode (docs/design/commissioning.md): ticking off wires,
//! terminals and devices at the machine, and the punch list.
//!
//! The data is novo-commissioning's [`CommissioningStore`], kept in the
//! project file's `commissioning` document and written after every change.
//! It is not a model edit: it is not undone and does not dirty the drawing.
//! The panel is the outline's Commissioning tab, with buttons sized for a
//! finger. On the drawing, passed objects get the "added" outline and failed
//! ones the error mark.

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
use novo_xref::Place;

use super::cabinet_bar::note_ctx;

use super::changes::shown_on;
use super::pdf::{replaceable_path, write_replacing};
use super::revisions::iso_date;
use super::workspace::Workspace;
use super::{Level, NovoApp, OutlineMode, projects_dir};

/// Minimum height of the buttons: comfortable for a finger on a tablet.
const TOUCH: f32 = 44.0;
/// The same with Large on: for a tablet in a cabinet, gloves or a stylus.
const TOUCH_LARGE: f32 = 64.0;
/// How long the first touch of "Pass all open" waits for the second.
const ARM_SECONDS: f64 = 5.0;
/// Row and label text size with large buttons on.
const TEXT_LARGE: f32 = 18.0;

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
    /// Bigger buttons and text for a tablet.
    large: bool,
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

fn state_glyph(state: CheckState) -> &'static str {
    match state {
        CheckState::Pending => "○",
        CheckState::Passed => "✔",
        CheckState::Failed => "✖",
        CheckState::NotApplicable => "–",
    }
}

/// One glyph for all the checks of a target: failed beats pending beats passed.
fn target_glyph(store: &CommissioningStore, target: Target) -> &'static str {
    let states: Vec<CheckState> = target
        .checks()
        .iter()
        .map(|&c| store.state(target, c))
        .collect();
    if states.contains(&CheckState::Failed) {
        "✖"
    } else if states.contains(&CheckState::Pending) {
        "○"
    } else {
        "✔"
    }
}

/// The height of a touch target, for the Large switch.
fn touch(large: bool) -> f32 {
    if large { TOUCH_LARGE } else { TOUCH }
}

/// Row and label text, bigger with large buttons so a tablet reads it too.
fn sized(large: bool, text: String) -> egui::RichText {
    let text = egui::RichText::new(text);
    if large { text.size(TEXT_LARGE) } else { text }
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
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if mode.armed.is_some() {
            if big_button(
                ui,
                mode.large,
                &format!("Pass {} open · Pass them", open.len()),
            )
            .clicked()
            {
                for check in open {
                    mode.decide(target, check, CheckState::Passed);
                }
                mode.armed = None;
            } else if big_button(ui, mode.large, "Cancel").clicked() {
                mode.armed = None;
            } else {
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_secs_f64(ARM_SECONDS));
            }
        } else if big_button(ui, mode.large, &format!("Pass all {} open", open.len())).clicked() {
            mode.armed = Some((target, now_s));
        }
    });
}

fn big_button(ui: &mut egui::Ui, large: bool, text: &str) -> egui::Response {
    let text = if large {
        egui::RichText::new(text).size(18.0)
    } else {
        egui::RichText::new(text)
    };
    ui.add(egui::Button::new(text).min_size(egui::vec2(touch(large) * 1.5, touch(large))))
}

fn progress_bar(ui: &mut egui::Ui, title: &str, p: Progress) {
    let fraction = (p.percent() / 100.0) as f32;
    let mut right = format!("{}/{}", p.done(), p.total);
    if p.failed > 0 {
        right.push_str(&format!(" · {} failed", p.failed));
    }
    let palette = novo_theme::Palette::of(ui);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(title).color(palette.text_secondary));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(right)
                    .monospace()
                    .color(palette.text_primary),
            );
        });
    });
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    ui.painter().rect_filled(rect, 3, palette.sunken);
    let mut done = rect;
    done.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    ui.painter().rect_filled(done, 3, palette.success);
    ui.add_space(4.0);
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
            let (kind, what) = match target_glyph(&mode.store, info.target) {
                "✖" => (MarkKind::Error, "✖ Failed"),
                "✔" => (MarkKind::Added, "✔ Passed"),
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
        {
            let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
            else {
                ui.weak("Open a project to commission it");
                return false;
            };
            let project = doc.project();
            mode.large = self.settings.large_buttons;
            ui.horizontal(|ui| {
                for (tab, label) in [(Tab::Checks, "Checks"), (Tab::Punch, "Punch list")] {
                    let chip = novo_theme::widgets::chip(ui, label, mode.tab == tab);
                    if tab == Tab::Punch {
                        note_ctx(ui.ctx(), "commissioning.punch", "Punch tab", &chip);
                    }
                    if chip.clicked() {
                        mode.tab = tab;
                    }
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.add_sized(
                    [64.0, 22.0],
                    egui::Label::new(egui::RichText::new("Signed by").weak()),
                );
                let who = ui.add(
                    egui::TextEdit::singleline(&mut mode.who)
                        .hint_text("Name for the sign-off")
                        .desired_width(f32::INFINITY),
                );
                note_ctx(ui.ctx(), "commissioning.who", "By field", &who);
            });
            ui.add_space(6.0);
            progress_bar(
                ui,
                "Project",
                progress(project, &mode.store, Scope::Project),
            );
            if let Some(page) = current {
                progress_bar(
                    ui,
                    "This page",
                    progress(project, &mode.store, Scope::Page(page)),
                );
            }
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.add_sized(
                    [64.0, 22.0],
                    egui::Label::new(egui::RichText::new("Report").weak()),
                );
                for (label, format) in [
                    ("PDF", ReportFileFormat::Pdf),
                    ("CSV", ReportFileFormat::Csv(CsvOptions::default())),
                ] {
                    let button = ui
                        .add(egui::Button::new(label).min_size(egui::vec2(52.0, 24.0)))
                        .on_hover_text("Write the commissioning report next to the project file");
                    if label == "PDF" {
                        note_ctx(
                            ui.ctx(),
                            "commissioning.report",
                            "Report PDF button",
                            &button,
                        );
                    }
                    if button.clicked() {
                        export = Some(format);
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.add_sized(
                    [64.0, 22.0],
                    egui::Label::new(egui::RichText::new("Field").weak()),
                );
                for (label, action, hint) in [
                    (
                        "Export",
                        FieldAction::Export,
                        "Save your ticks and punch items as a file to carry to the desk (no cloud)",
                    ),
                    (
                        "Import",
                        FieldAction::Import,
                        "Merge a field results file into this project; newer decisions win",
                    ),
                ] {
                    let button = ui
                        .add(egui::Button::new(label).min_size(egui::vec2(60.0, 24.0)))
                        .on_hover_text(hint);
                    note_ctx(
                        ui.ctx(),
                        if action == FieldAction::Export {
                            "commissioning.field_export"
                        } else {
                            "commissioning.field_import"
                        },
                        if action == FieldAction::Export {
                            "Field results Export button"
                        } else {
                            "Field results Import button"
                        },
                        &button,
                    );
                    if button.clicked() {
                        field = Some(action);
                    }
                }
            });
            if ui.checkbox(&mut mode.large, "Large buttons").changed() {
                self.settings.large_buttons = mode.large;
            }
            ui.add_space(4.0);
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
                self.message(Level::Info, format!("Exported {}", path.display()));
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
    let base = match file {
        Some(file) => {
            let stem = file.file_stem()?.to_string_lossy().into_owned();
            file.with_file_name(format!("{stem} commissioning.{ext}"))
        }
        None => {
            let stem = Path::new(&super::project_file_name(name))
                .with_extension("")
                .to_string_lossy()
                .into_owned();
            dir?.join(format!("{stem} commissioning.{ext}"))
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

fn checks_tab(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    project: &Project,
    current: Option<PageId>,
    go: &mut Option<Place>,
) {
    let Some(page) = current else {
        ui.weak("Open a page to check what is drawn on it");
        return;
    };
    let on_page: Vec<TargetInfo> = targets(project)
        .into_iter()
        .filter(|i| i.pages.contains(&page))
        .collect();
    let next = on_page
        .iter()
        .find(|i| {
            i.target
                .checks()
                .iter()
                .any(|&c| mode.store.state(i.target, c) == CheckState::Pending)
        })
        .cloned();
    ui.horizontal(|ui| {
        let next_open = big_button(ui, mode.large, "Next open")
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
            ui.weak("Everything on this page is decided");
        }
    });

    if let Some(info) = on_page.iter().find(|i| Some(i.target) == mode.selected) {
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
                ui.label(sized(
                    mode.large,
                    format!("{} {}", state_glyph(state), check.label()),
                ));
                let pass = big_button(ui, mode.large, "Pass");
                let fail = big_button(ui, mode.large, "Fail");
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
                if big_button(ui, mode.large, "N/A").clicked() {
                    mode.decide(info.target, check, CheckState::NotApplicable);
                }
                if state != CheckState::Pending && big_button(ui, mode.large, "Reset").clicked() {
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

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for info in &on_page {
                let glyph = target_glyph(&mode.store, info.target);
                let tint = match glyph {
                    "✖" => novo_theme::tokens::ERROR,
                    "✔" => novo_theme::tokens::SUCCESS,
                    _ => novo_theme::tokens::TEXT_MUTED,
                };
                let size = if mode.large { TEXT_LARGE } else { 13.0 };
                let font = egui::FontId::proportional(size);
                let mut text = egui::text::LayoutJob::default();
                text.append(
                    &format!("{glyph}  "),
                    0.0,
                    egui::TextFormat::simple(font.clone(), tint),
                );
                text.append(
                    &info.label,
                    0.0,
                    egui::TextFormat::simple(font, ui.visuals().text_color()),
                );
                let row = egui::Button::selectable(mode.selected == Some(info.target), text)
                    .min_size(egui::vec2(ui.available_width(), touch(mode.large) * 0.7));
                if ui.add(row).clicked() {
                    mode.selected = Some(info.target);
                    *go = place_of(project, info, current);
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
        if ui
            .add_enabled(
                can_add,
                egui::Button::new("Add")
                    .min_size(egui::vec2(touch(mode.large) * 1.5, touch(mode.large))),
            )
            .clicked()
        {
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
            for item in &items {
                let info = item.target.and_then(|t| all.iter().find(|i| i.target == t));
                let label = info.map_or(String::new(), |i| format!(" · {}", i.label));
                let text = format!(
                    "{} · {:?} · {}{label}",
                    severity_label(item.severity),
                    item.state,
                    item.title
                );
                if ui.selectable_label(false, text).clicked()
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
                        let button = big_button(ui, mode.large, label);
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
                        && big_button(ui, mode.large, "Won't fix").clicked()
                        && mode
                            .store
                            .set_punch_state(item.id, PunchState::WontFix, &mode.who, &now())
                            .is_ok()
                    {
                        mode.dirty = true;
                    }
                });
                ui.separator();
            }
        });
}
