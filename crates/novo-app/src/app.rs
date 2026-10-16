//! The main window, laid out as the Workbench of docs/DESIGN.md §3: a thin
//! top bar, the page outline (pinned or a flyout), the canvas with its HUD,
//! toasts for messages, the dialogs and action dispatch. There is no menu
//! bar and no status bar.

mod assembly;
mod background_panel;
mod cabinet;
mod cabinet_door;
mod cabinet_inspector;
mod cabinet_panel;
#[allow(dead_code)] // called from cabinet_show once the duct drawing PR is in
mod cabinet_routes;
mod cabinet_rules;
mod change_report;
mod changes;
mod checks;
mod commissioning;
mod compare;
mod dock;
mod drawers;
mod duplicate;
mod dxf;
mod example;
mod faults;
mod heat;
mod heat_run;
#[cfg(test)]
mod heat_run_tests;
mod inspector;
mod interop;
mod learn;
mod learn_ui;
mod link;
mod live;
mod live_anim;
mod macro_actions;
#[cfg(test)]
mod macro_actions_tests;
mod macro_browser;
#[cfg(test)]
mod macro_browser_tests;
#[cfg(test)]
mod macro_company_tests;
mod macro_thumb;
mod menu_bar;
mod numbering;
mod onboarding;
mod outline;
mod outline_tree;
mod pages;
mod parts;
mod parts_import;
mod parts_quality;
mod pdf;
mod plate_export;
mod plc;
mod plc_link;
mod plc_run;
mod potentials;
mod preferences;
mod project_settings;
mod protection;
mod reports;
mod reports_view;
mod revisions;
mod sheet_import;
mod sheets;
mod shortcuts_sheet;
mod simulate;
mod sizing;
mod standards;
mod status_bar;
mod terminals;
mod title_revisions;
mod userkeymap;
mod variants;
mod wheel;
mod wire_production;
mod workbench;
mod workspace;
mod xref;

#[cfg(test)]
mod assembly_pack_tests;
#[cfg(test)]
mod assembly_tests;
#[cfg(test)]
mod cabinet_routes_tests;
#[cfg(test)]
mod cabinet_rules_tests;
#[cfg(test)]
mod cabinet_tests;
#[cfg(test)]
mod changes_tests;
#[cfg(test)]
mod commissioning_tests;
#[cfg(test)]
mod compare_tests;
#[cfg(test)]
mod example_tests;
#[cfg(test)]
mod gate1_tests;
#[cfg(test)]
mod learn_tests;
#[cfg(test)]
mod numbering_tests;
#[cfg(test)]
mod onboarding_tests;
#[cfg(test)]
mod outline_tests;
#[cfg(test)]
mod protection_tests;
#[cfg(test)]
mod route_all_tests;
#[cfg(test)]
mod shortcuts_sheet_tests;
#[cfg(test)]
mod variants_tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;

use crate::actions::Action;

/// Hover text of the bulk-edit exports (the data to edit, not the reports).
const ASSEMBLY_HINT: &str = "The shop-floor print pack of every cabinet: build order (rails, ducts, devices, wires, labels), then prepare with the cut list, drill table, mount, wire, label and check, then the wire cutting list. Needs a cabinet with a mounting plate.";
const SHEETS_HINT: &str = "Devices, terminals, wires and cables to edit in Excel. Export only: bringing the edited file back comes with the import dialog. The generated reports are under Export all reports to Excel.";
use novo_core::{Aspect, Command, DeviceTag, ModelError, Outcome, PageId, PageKind};
use pages::PageView;

use crate::backend::{BackendError, PageRow, ProjectBackend, page_rows};
use crate::canvas::{CanvasView, OpenPage};
use crate::settings::Settings;
use crate::{APP_MAKER, APP_NAME, PROJECT_EXTENSION};
use novo_theme::widgets::primary_button;
pub(crate) use revisions::index_rule;
use workbench::Workbench;
pub(crate) use workbench::app_runs;

/// What the outline lists (docs/DESIGN.md §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OutlineMode {
    Pages,
    Issues,
    Changes,
    Commissioning,
}

impl OutlineMode {
    fn label(self) -> &'static str {
        match self {
            OutlineMode::Pages => "Pages",
            OutlineMode::Issues => "Issues",
            OutlineMode::Changes => "Changes",
            OutlineMode::Commissioning => "Commissioning",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub level: Level,
    pub text: String,
    /// When it was raised; info toasts fade after [`TOAST_INFO`].
    pub at: Instant,
    /// Clicked away; warnings and errors stay until then.
    pub dismissed: bool,
}

/// How long an info toast stays up.
const TOAST_INFO: Duration = Duration::from_secs(5);
/// Most toasts shown at once; older ones wait in [`NovoApp::messages`].
const MAX_TOASTS: usize = 3;
/// Height of the top bar.
const TOP_BAR: f32 = 42.0;
/// Width of the outline, pinned or as a flyout.
const OUTLINE_WIDTH: f32 = 248.0;
/// Windows at least this wide pin the outline unless the user chose.
const PIN_OUTLINE_FROM: f32 = 1700.0;

/// An action that replaces or closes the open project. Nothing asks first:
/// the backend keeps the work (docs/DESIGN.md: no save prompts).
#[derive(Clone, Debug, PartialEq, Eq)]
enum Leaving {
    New,
    Open,
    OpenPath(PathBuf),
    Example,
    Practice,
    Close,
    Quit,
}

pub struct NovoApp {
    backend: Box<dyn ProjectBackend>,
    canvas: Box<dyn CanvasView>,
    settings: Settings,
    project_path: Option<PathBuf>,
    current_page: Option<PageId>,
    /// The hosted engines: command field, keys, cross references,
    /// terminals, problems and export.
    wb: Workbench,
    messages: Vec<Message>,
    show_about: bool,
    standards_open: bool,
    window_title: String,
    /// Opened on the first frame, so errors show in the window.
    open_on_start: Option<PathBuf>,
    /// The first frame has run its start-up work.
    started: bool,
    /// When to give the backend its next chance to autosave.
    next_tick: Instant,
    /// The last autosave failed; reported once until it works again.
    autosave_failing: bool,
    /// Focus mode (Ctrl+Shift+F) hides everything that is not pinned.
    focus_mode: bool,
    /// What the canvas area shows (docs/design/workspaces.md). The Cabinet
    /// is Ctrl+2.
    workspace: workspace::Workspace,
    cabinet: cabinet::CabinetState,
    reports_view: reports_view::ReportsView,
    /// The "Size the enclosure" panel and the Enclosures list.
    sizing: sizing::SizingState,
    protection: protection::ProtectionState,
    /// The outline flyout is showing over the canvas.
    outline_open: bool,
    /// What the outline lists.
    outline_mode: OutlineMode,
    /// The breadcrumb's page link this frame. A click on it toggles the
    /// flyout, so the flyout must not count it as a click elsewhere.
    breadcrumb_link: Option<egui::Response>,
    /// Text in the outline's page filter.
    page_filter: String,
    /// Whether the outline files pages by page group or by panel.
    page_view: PageView,
    /// The outline's tree: folds, selection, name boxes and its windows.
    ol: outline::OutlineState,
    /// Window width last frame, which decides whether the outline pins.
    window_width: f32,
    /// Free room between the top bar's left and right groups last frame;
    /// below zero they overlap. Read by the width test.
    top_bar_gap: f32,
    /// Width the menu names took last frame (`menu_bar`).
    menu_bar_width: f32,
    /// Lessons: the catalog, the running lesson and the person's progress.
    learn: learn::LearnState,
}

/// Most messages remembered; older ones are forgotten.
const MESSAGE_HISTORY: usize = 200;
/// How soon after a change a saved project is written to its file.
const WRITE_DELAY: Duration = Duration::from_millis(500);
/// How long to wait before retrying a failed autosave.
const AUTOSAVE_RETRY: Duration = Duration::from_secs(30);

impl NovoApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        backend: Box<dyn ProjectBackend>,
        canvas: Box<dyn CanvasView>,
    ) -> Self {
        let settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, Settings::STORAGE_KEY))
            .unwrap_or_default();
        novo_theme::apply(&cc.egui_ctx);
        let mut app = Self::with_settings(backend, canvas, settings);
        app.load_user_keymap();
        app.learn_init_paths();
        app
    }

    pub fn with_settings(
        backend: Box<dyn ProjectBackend>,
        canvas: Box<dyn CanvasView>,
        settings: Settings,
    ) -> Self {
        let wb = Workbench::with_recent(&settings.recent_actions);
        let mut app = Self {
            backend,
            canvas,
            settings,
            project_path: None,
            current_page: None,
            wb,
            messages: Vec::new(),
            show_about: false,
            standards_open: false,
            window_title: String::new(),
            open_on_start: None,
            started: false,
            next_tick: Instant::now(),
            autosave_failing: false,
            focus_mode: false,
            workspace: workspace::Workspace::Schematic,
            cabinet: cabinet::CabinetState::new(),
            reports_view: Default::default(),
            sizing: sizing::SizingState::new(),
            protection: protection::ProtectionState::new(),
            outline_open: false,
            outline_mode: OutlineMode::Pages,
            breadcrumb_link: None,
            page_filter: String::new(),
            page_view: PageView::Function,
            ol: outline::OutlineState::new(),
            window_width: 0.0,
            top_bar_gap: 0.0,
            menu_bar_width: 380.0,
            learn: learn::LearnState::new(),
        };
        app.restore_layout_at_start();
        app
    }

    /// Opens `path` when the window first appears.
    pub fn open_on_start(&mut self, path: PathBuf) {
        self.open_on_start = Some(path);
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    fn is_open(&self) -> bool {
        self.backend.document().is_some()
    }

    fn is_dirty(&self) -> bool {
        self.backend.is_dirty()
    }

    /// The project's name; a named file still called "Untitled" inside
    /// (saved before naming set the name) shows its file name.
    fn project_name(&self) -> Option<String> {
        self.backend.document().map(|d| {
            let name = &d.project().info().name;
            if !is_default_name(name) {
                return name.clone();
            }
            self.project_path
                .as_deref()
                .and_then(file_stem)
                .unwrap_or_else(|| "Untitled".to_owned())
        })
    }

    #[cfg(test)]
    fn pages(&self) -> Vec<(PageId, String)> {
        self.backend
            .document()
            .map(crate::backend::page_labels)
            .unwrap_or_default()
    }

    /// The project panel's rows: each page, and each sheet of a generated
    /// page, from the live reports.
    fn page_rows(&mut self) -> Vec<PageRow> {
        match self.backend.document() {
            Some(doc) => {
                let project = doc.project();
                page_rows(project, self.wb.reports.live(project))
            }
            None => Vec::new(),
        }
    }

    /// Keeps the shown page valid after undo, redo or a reload removed it.
    /// Without one, shows the first drawing page rather than the contents.
    fn fix_current_page(&mut self) {
        let pages: Vec<(PageId, bool)> = self
            .backend
            .document()
            .map(|doc| {
                doc.project()
                    .pages_in_order()
                    .into_iter()
                    .map(|p| (p.id, matches!(p.kind, PageKind::Generated { .. })))
                    .collect()
            })
            .unwrap_or_default();
        if !pages.iter().any(|(id, _)| Some(*id) == self.current_page) {
            self.current_page = pages
                .iter()
                .find(|(_, generated)| !generated)
                .or(pages.first())
                .map(|(id, _)| *id);
        }
    }

    fn undo_redo(&mut self, redo: bool) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = if redo { doc.redo() } else { doc.undo() };
        match result {
            Ok(Some(changes)) => {
                self.canvas.document_changed(doc, &changes);
                self.backend.record(&changes);
                self.reports_heard(&changes);
            }
            Ok(None) => {}
            Err(e) => {
                let what = if redo { "redo" } else { "undo" };
                self.message(Level::Error, format!("Could not {what}: {e}"));
            }
        }
        self.fix_current_page();
    }

    pub fn is_enabled(&self, action: Action) -> bool {
        let open = self.is_open();
        let doc = self.backend.document();
        match action {
            Action::SaveProject
            | Action::SaveProjectAs
            | Action::CloseProject
            | Action::ZoomToFit
            | Action::SizeEnclosure
            | Action::StandardsCheck
            | Action::EnclosureList
            | Action::Protection => open,
            Action::MotorStart => open,
            Action::ShowCabinet => self
                .workspace_unavailable(workspace::Workspace::Cabinet)
                .is_none(),
            Action::Undo => !self.commissioning_locks_edits() && doc.is_some_and(|d| d.can_undo()),
            Action::Redo => !self.commissioning_locks_edits() && doc.is_some_and(|d| d.can_redo()),
            _ => true,
        }
    }

    /// Runs an action as if the user had picked it.
    pub fn run(&mut self, ctx: &egui::Context, action: Action) {
        if !self.is_enabled(action) {
            return;
        }
        self.learn_command(action.id());
        if action != Action::CommandPalette {
            self.settings.remember_action(action.id());
        } else {
            self.tour_tick("commands");
        }
        match action {
            Action::NewProject => self.leave_then(ctx, Leaving::New),
            Action::OpenProject => self.leave_then(ctx, Leaving::Open),
            Action::OpenExample => self.leave_then(ctx, Leaving::Example),
            Action::TryFastKeys => self.leave_then(ctx, Leaving::Practice),
            Action::ShowWalkthrough => self.show_walkthrough(),
            Action::Learn => self.learn_run_command("help.learn", None),
            Action::SaveProject => {
                self.save();
            }
            Action::SaveProjectAs => {
                self.save_as();
            }
            Action::CloseProject => self.leave_then(ctx, Leaving::Close),
            Action::Quit => self.leave_then(ctx, Leaving::Quit),
            Action::Undo => self.undo_redo(false),
            Action::Redo => self.undo_redo(true),
            Action::ZoomToFit => self.canvas.zoom_to_fit(),
            Action::ShowSchematic => self.switch_workspace(workspace::Workspace::Schematic),
            Action::ShowCabinet => self.switch_workspace(workspace::Workspace::Cabinet),
            Action::SizeEnclosure => {
                self.switch_workspace(workspace::Workspace::Cabinet);
                self.sizing.panel_open = true;
            }
            Action::EnclosureList => {
                self.switch_workspace(workspace::Workspace::Cabinet);
                self.sizing.editor_open = true;
            }
            Action::StandardsCheck => self.standards_open = true,
            Action::Protection => self.protection.open = true,
            Action::MotorStart => self.open_motor_start(),
            Action::PinOutline => {
                let pinned = !self.outline_pinned();
                self.settings.outline_pinned = Some(pinned);
                self.outline_open = false;
            }
            Action::FocusMode => {
                self.focus_mode ^= true;
                self.outline_open = false;
            }
            Action::FullScreen => {
                let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
            }
            Action::CommandPalette => self.wb.field.open(novo_workflow::palette::Mode::All, None),
            Action::About => self.show_about = true,
        }
    }

    fn message(&mut self, level: Level, text: impl Into<String>) {
        let text = text.into();
        match level {
            Level::Info => log::info!("{text}"),
            Level::Warning => log::warn!("{text}"),
            Level::Error => log::error!("{text}"),
        }
        self.messages.push(Message {
            level,
            text,
            at: Instant::now(),
            dismissed: false,
        });
        if self.messages.len() > MESSAGE_HISTORY {
            let excess = self.messages.len() - MESSAGE_HISTORY;
            self.messages.drain(..excess);
        }
    }

    // ------------------------------------------------------------------
    // Project lifecycle

    fn leave_then(&mut self, ctx: &egui::Context, next: Leaving) {
        match next {
            Leaving::New => self.new_project(),
            Leaving::Open => {
                let mut dialog = rfd::FileDialog::new()
                    .set_title("Open project")
                    .add_filter("KoperCAD project", &[PROJECT_EXTENSION]);
                if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                if let Some(path) = dialog.pick_file() {
                    self.open_path(&path);
                }
            }
            Leaving::OpenPath(path) => self.open_path(&path),
            Leaving::Example => self.open_example(),
            Leaving::Practice => self.start_practice(),
            Leaving::Close => self.close_project(),
            Leaving::Quit => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn new_project(&mut self) {
        match self.backend.new_project("Untitled") {
            Ok(()) => {
                self.project_path = None;
                self.current_page = None;
                self.document_replaced();
                self.fix_current_page();
            }
            Err(e) => self.message(Level::Error, format!("Could not create a project: {e}")),
        }
    }

    fn open_path(&mut self, path: &Path) {
        match self.backend.open(path) {
            Ok(report) => {
                self.current_page = None;
                self.document_replaced();
                self.fix_current_page();
                if self.backend.is_untitled() {
                    self.project_path = None;
                } else {
                    self.project_path = Some(path.to_owned());
                    self.settings.remember_recent(path);
                    self.message(Level::Info, format!("Opened {}", path.display()));
                }
                if let Some(aside) = report.set_aside {
                    self.message(
                        Level::Warning,
                        format!(
                            "The unsaved work from the last session could not be read. It was kept as {}.",
                            aside.display()
                        ),
                    );
                }
            }
            Err(e) => {
                if is_not_found(&e) {
                    self.settings.forget_recent(path);
                }
                self.message(Level::Error, describe_error("open", path, &e));
            }
        }
    }

    /// Offers back an unsaved new project from a session that crashed.
    /// Returns true when it opened one.
    fn restore_untitled(&mut self) -> bool {
        match self.backend.restore_untitled() {
            Ok(true) => {
                self.project_path = None;
                self.current_page = None;
                self.document_replaced();
                self.fix_current_page();
                true
            }
            Ok(false) => false,
            Err(e) => {
                self.message(
                    Level::Warning,
                    format!("Could not check for unsaved work from the last session: {e}"),
                );
                false
            }
        }
    }

    /// Start-up: the file named on the command line, else unsaved work a
    /// crash left behind, else the last project, where it was left. There
    /// is no start screen.
    fn start(&mut self) {
        if let Some(path) = self.open_on_start.take() {
            self.open_path(&path);
        } else if !self.restore_untitled()
            && let Some(last) = self.settings.last_open.clone()
            && last.is_file()
        {
            self.open_path(&last);
        }
        self.restore_workspace_at_start();
    }

    /// Runs a model command on the open project the way the editor does:
    /// the canvas updates and the change reaches storage. For scripts and
    /// tests that drive the whole window. `None` when no project is open.
    pub fn execute(&mut self, command: Command) -> Option<Result<Outcome, ModelError>> {
        let doc = self.backend.document_mut()?;
        let outcome = match doc.execute(command) {
            Ok(outcome) => outcome,
            Err(e) => return Some(Err(e)),
        };
        self.canvas.document_changed(doc, &outcome.changes);
        self.backend.record(&outcome.changes);
        self.reports_heard(&outcome.changes);
        self.fix_current_page();
        Some(Ok(outcome))
    }

    /// The open project, if any.
    pub fn document(&self) -> Option<&novo_core::Document> {
        self.backend.document()
    }

    /// The page on the canvas.
    pub fn current_page(&self) -> Option<PageId> {
        self.current_page
    }

    /// A different project was opened, created or closed: nothing cached
    /// for the old one carries over.
    fn document_replaced(&mut self) {
        self.learn_project_changed();
        self.canvas.document_replaced();
        self.wb.reports = Default::default();
        self.wb.commissioning = None;
        self.wb.sheet = 0;
        self.add_generated_pages();
    }

    fn close_project(&mut self) {
        if let Err(e) = self.backend.close() {
            self.message(
                Level::Error,
                format!(
                    "Could not close the project: {e}. Your work is also kept in its recovery file."
                ),
            );
            return;
        }
        self.project_path = None;
        self.current_page = None;
        self.document_replaced();
    }

    /// Gives the backend its chance to autosave, and wakes the app up in
    /// time for the next one even when nothing else repaints.
    fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        // A saved project is written to its file shortly after each change.
        // While writing fails, wait for the slower retry instead.
        if self.project_path.is_some() && self.is_dirty() && !self.autosave_failing {
            self.next_tick = self.next_tick.min(now + WRITE_DELAY);
        }
        if now < self.next_tick {
            ctx.request_repaint_after(self.next_tick - now);
            return;
        }
        let wait = match self.backend.tick() {
            Ok(next) => {
                if self.autosave_failing {
                    self.autosave_failing = false;
                    self.message(Level::Info, "Saving works again");
                }
                next
            }
            Err(e) => {
                if !self.autosave_failing {
                    self.autosave_failing = true;
                    self.message(Level::Warning, format!("Could not save the project: {e}"));
                }
                Some(AUTOSAVE_RETRY)
            }
        };
        if let Some(wait) = wait {
            self.next_tick = now + wait;
            ctx.request_repaint_after(wait);
        }
    }

    /// Saves to the current file, asking for one if there is none.
    /// Returns true when the project ended up saved.
    fn save(&mut self) -> bool {
        match self.project_path.clone() {
            Some(path) => self.save_to(&path),
            None => self.save_as(),
        }
    }

    fn save_as(&mut self) -> bool {
        let name = self.project_name().unwrap_or_else(|| "Untitled".into());
        let mut dialog = rfd::FileDialog::new()
            .set_title("Save project")
            .add_filter("KoperCAD project", &[PROJECT_EXTENSION])
            .set_file_name(project_file_name(&name));
        // A project's first save goes to Documents\KoperCAD (DESIGN.md §3,
        // §6); later "Save as" starts next to the current file.
        let folder = match &self.project_path {
            Some(path) => path.parent().map(Path::to_owned),
            None => projects_dir().filter(|dir| std::fs::create_dir_all(dir).is_ok()),
        };
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
        match dialog.save_file() {
            Some(path) => self.name_and_save(&path),
            None => false,
        }
    }

    /// Naming a project sets its name and its file together (DESIGN.md §3):
    /// a project still called "Untitled", or named after its old file,
    /// takes the new file's name, then is saved there.
    fn name_and_save(&mut self, path: &Path) -> bool {
        let old_stem = self.project_path.as_deref().and_then(file_stem);
        if let (Some(stem), Some(doc)) = (file_stem(path), self.backend.document_mut()) {
            let current = doc.project().info().name.clone();
            if current != stem && (is_default_name(&current) || old_stem.as_ref() == Some(&current))
            {
                match doc.transact(format!("Name project {stem}"), |tx| {
                    tx.set_info(|info| info.name = stem.clone())
                }) {
                    Ok(((), changes)) => self.changed_outside_canvas(&changes),
                    Err(e) => self.message(Level::Error, format!("Could not name it: {e}")),
                }
            }
        }
        self.save_to(path)
    }

    fn save_to(&mut self, path: &Path) -> bool {
        match self.backend.save(path) {
            Ok(()) => {
                self.project_path = Some(path.to_owned());
                self.settings.remember_recent(path);
                self.message(Level::Info, format!("Saved {}", path.display()));
                true
            }
            Err(e) => {
                self.message(Level::Error, describe_error("save", path, &e));
                false
            }
        }
    }

    // ------------------------------------------------------------------
    // Frame

    /// Lays out one frame of the whole window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.started {
            self.started = true;
            self.start();
        }
        self.fix_current_page();
        self.tick(&ctx);
        self.settings.last_open = self.backend.file();
        self.update_title(&ctx);

        let mut picked: Vec<Action> = Vec::new();
        self.escape_popups(&ctx);
        self.space_pan(&ctx);
        self.space_released(&ctx);
        self.track_pointer(&ctx);
        if !self.modal_open() {
            self.outline_keys(&ctx);
        }
        let mut commands = if self.modal_open() {
            // The place key pressed again over its own wheel swaps it for
            // the part search; every other key waits for the wheel.
            if self.wheel_place_press(&ctx) {
                vec!["tool.place"]
            } else {
                Vec::new()
            }
        } else {
            self.key_commands(&ctx)
        };
        if self.workspace() != workspace::Workspace::Schematic {
            // The schematic's keys must not edit the hidden page: rotate
            // and delete go to the Cabinet, the rest keep only the view,
            // file and undo actions.
            let cabinet = self.workspace() == workspace::Workspace::Cabinet;
            for id in &commands {
                if cabinet && matches!(*id, "edit.rotate" | "edit.delete") {
                    self.cabinet.keys.push((*id).to_owned());
                }
            }
            commands.retain(|id| {
                Action::from_id(id).is_some()
                    || *id == "view.cabinet_table"
                    || id.starts_with("heat.")
            });
        }

        self.window_width = ctx.content_rect().width();
        let pinned = self.outline_pinned() && !self.focus_mode;
        if pinned {
            self.outline_open = false;
        }

        self.breadcrumb_link = None;
        egui::Panel::top("top_bar")
            .exact_size(TOP_BAR)
            .show(ui, |ui| self.top_bar(ui, &mut picked));
        self.status_bar(ui);
        let reports = self.workspace() == workspace::Workspace::Reports && self.is_open();
        if reports {
            egui::Panel::left("reports_list")
                .exact_size(OUTLINE_WIDTH)
                .resizable(false)
                .show(ui, |ui| self.reports_list(ui));
        } else if pinned && self.is_open() {
            egui::Panel::left("outline")
                .exact_size(OUTLINE_WIDTH)
                .resizable(false)
                .show(ui, |ui| self.outline(ui, &mut picked));
        }
        self.drawer(ui);
        let canvas_rect = egui::CentralPanel::no_frame()
            .show(ui, |ui| {
                if self.is_open() && self.workspace() == workspace::Workspace::Cabinet {
                    self.cabinet_show(ui);
                } else if reports {
                    self.reports_central(ui);
                } else if self.is_open() {
                    self.update_marks();
                    self.update_sheet();
                    self.update_underlay();
                    let open = self
                        .current_page
                        .and_then(|page| OpenPage::new(&mut *self.backend, page));
                    self.canvas.show(ui, open);
                    self.after_canvas();
                    self.after_canvas_frame();
                    self.wb.canvas_area = Some((ui.layer_id(), ui.min_rect()));
                } else {
                    self.start_panel(ui, &mut picked);
                }
            })
            .response
            .rect;

        if !self.focus_mode && !pinned && !reports && self.is_open() {
            self.outline_flyout(&ctx, canvas_rect, &mut picked);
        }
        if !self.focus_mode && self.workspace() == workspace::Workspace::Schematic {
            self.hud(&ctx, canvas_rect);
            self.sync_wiring_mode();
            self.sync_connect_mode();
            self.sync_catalog();
            self.tool_dock(&ctx, canvas_rect);
            self.inspector(&ctx, canvas_rect);
            self.live_view(&ctx, canvas_rect);
            self.plc_window(&ctx);
            self.tour_watch(&ctx);
            self.walkthrough_card(&ctx, canvas_rect);
            self.changes_card(&ctx, canvas_rect);
        }
        self.learn_watch(&ctx);
        self.learn_step(&ctx);
        self.learn_overlay(&ctx, canvas_rect);
        self.toasts(&ctx, canvas_rect);

        self.occurrences_popup(&ctx, canvas_rect);
        self.command_field(&ctx, canvas_rect);
        self.recovery_dialog(&ctx);
        self.about_dialog(&ctx);
        self.standards_window(&ctx);
        self.learn_window(&ctx);
        self.motor_start_window(&ctx);
        self.renumber_dialog(&ctx);
        self.plc_import_dialog(&ctx);
        self.preferences_dialog(&ctx);
        self.shortcuts_window(&ctx);
        self.background_panel(&ctx);
        self.parts_window(&ctx);
        self.project_settings_dialog(&ctx);
        self.macro_browser_window(&ctx);
        self.macro_save_dialog(&ctx);
        self.macro_place_window(&ctx);
        self.options_window(&ctx);
        self.generate_window(&ctx);
        self.issue_revision_dialog(&ctx);
        self.compare_window(&ctx);
        self.aml_import_dialog(&ctx);
        self.poll_dwg(&ctx);
        self.protection_window(&ctx);
        self.faults_window(&ctx);
        self.numbering_window(&ctx);
        self.page_form_dialog(&ctx);
        self.sheet_import_dialog(&ctx);
        self.outline_dialogs(&ctx);
        for id in std::mem::take(&mut self.wb.queued) {
            self.run_command(&ctx, id, None);
        }
        self.place_hold_tick(&ctx);
        self.place_wheel(&ctx);

        for action in picked {
            self.run(&ctx, action);
        }
        for command in commands {
            self.run_key_command(&ctx, command);
        }
        self.learn_end_frame();
    }

    /// Follows up on what the canvas did this frame: its edits reach the
    /// other views, and a page it asked for is opened.
    fn after_canvas(&mut self) {
        let changes = self.canvas.take_changes();
        if !changes.is_empty() && self.commissioning_locks_edits() {
            // Commissioning mode checks the drawing against the machine; a
            // stray touch must not move a wire. One undo takes it back.
            self.undo_redo(false);
            self.message(
                Level::Info,
                "Commissioning is read-only. Close it (Ctrl+K, Commissioning) to edit.",
            );
        } else if !changes.is_empty() {
            self.reports_heard(&changes);
            self.mark_earthed_terminals(&changes);
            self.fix_current_page();
        }
        if let Some(page) = self.canvas.take_page_request() {
            self.show_page(page);
        }
        let asked = self.canvas.take_commands();
        self.wb.queued.extend(asked);
        let cable = self
            .canvas
            .schematic()
            .and_then(|v| v.editor.take_cable_request());
        if let Some(wires) = cable {
            self.make_cable(&wires);
        }
    }

    /// A wire drawn to ground or a PE potential earths the terminals on
    /// its net (DESIGN.md §4, PE terminals). Runs only when a wire was
    /// added or reconnected, never on every edit, and joins the edit's undo
    /// step; a terminal toggled by hand is left alone.
    fn mark_earthed_terminals(&mut self, changes: &novo_core::ChangeSet) {
        if !changes
            .added
            .iter()
            .chain(&changes.modified)
            .any(|id| matches!(id, novo_core::AnyId::Wire(_)))
        {
            return;
        }
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let netlist = novo_core::Netlist::build(doc.project());
        if novo_terminals::earthed_terminals(doc.project(), &netlist).is_empty() {
            return;
        }
        if let Ok((_, extra)) = doc.amend_last("Mark PE terminals", novo_terminals::mark_earthed) {
            self.changed_outside_canvas(&extra);
        }
    }

    fn modal_open(&self) -> bool {
        self.wb.wheel.is_some() || self.modal_open_but_wheel()
    }

    fn modal_open_but_wheel(&self) -> bool {
        self.show_about
            || self.wb.renumber.is_some()
            || self.wb.page_form.is_some()
            || self.wb.sheet_import.is_some()
            || self.ol.dialog_open()
            || self.backend.recovery_offer().is_some()
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.project_name() {
            Some(name) => format!("{name}{} - {APP_NAME}", self.standard_suffix()),
            None => APP_NAME.to_owned(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    // ------------------------------------------------------------------
    // Workbench chrome

    /// Whether the outline takes space beside the canvas: the user's
    /// choice, otherwise only on wide windows.
    fn outline_pinned(&self) -> bool {
        self.settings
            .outline_pinned
            .unwrap_or(self.window_width >= PIN_OUTLINE_FROM)
    }

    /// Shows `page` on the canvas.
    fn show_page(&mut self, page: PageId) {
        if self.current_page != Some(page) {
            self.wb.sheet = 0;
        }
        self.current_page = Some(page);
        if self.wb.pending.is_some_and(|p| p.page != page) {
            self.wb.pending = None;
        }
        // A page is a schematic page: picking one leaves the Cabinet.
        self.switch_workspace(workspace::Workspace::Schematic);
    }

    /// Shows sheet `sheet` of a generated page (0 for any other page).
    pub(crate) fn show_sheet(&mut self, page: PageId, sheet: usize) {
        self.show_page(page);
        self.wb.sheet = sheet;
    }

    /// Autosave state for the top bar.
    fn saved_state(&self) -> &'static str {
        if !self.is_open() {
            ""
        } else if self.project_path.is_none() {
            "Not saved yet"
        } else if self.autosave_failing {
            "Not saved"
        } else if self.is_dirty() {
            "Saving"
        } else {
            "Saved"
        }
    }

    /// Mark (project menu), Schematic | Cabinet, breadcrumb, then "Search
    /// or run" and the saved state on the right.
    fn top_bar(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        let fit = menu_bar::TopBarFit::at(self.window_width);
        let focus = self.menu_key_pressed(ui.ctx());
        ui.horizontal_centered(|ui| {
            if self.menu_bar_wide() {
                self.mark(ui, false);
                self.menu_bar(ui, focus);
            } else {
                self.mark(ui, focus);
            }
            ui.separator();
            if let Some(workspace) = self.workspace_switcher(ui, fit.short_switcher) {
                self.wb.queued.push(workspace.command());
            }
            ui.separator();
            self.breadcrumb(ui, fit.name_chars);
            let left_end = ui.min_rect().right();

            let right = ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !self.status_bar_shown() {
                    ui.weak(self.saved_state());
                }
                self.issue_pill(ui);
                let shortcut = Action::CommandPalette
                    .shortcut()
                    .map(|s| ui.ctx().format_shortcut(&s))
                    .unwrap_or_default();
                let label = if fit.search.is_empty() {
                    shortcut.clone()
                } else {
                    fit.search.to_owned()
                };
                let mut search =
                    egui::Button::new(label).min_size(egui::vec2(fit.search_width, 0.0));
                if !fit.search.is_empty() && fit.search_width > 0.0 {
                    search = search.shortcut_text(shortcut.clone());
                }
                let search = ui
                    .add(search)
                    .on_hover_text(format!("Search or run ({shortcut})"));
                self.anchor_response("palette.button", &search, "Search or run");
                if search.clicked() {
                    picked.push(Action::CommandPalette);
                }
            });
            self.top_bar_gap = right.response.rect.left() - left_end;
        });
    }

    /// project / page. A click on the page opens the outline.
    fn breadcrumb(&mut self, ui: &mut egui::Ui, name_chars: usize) {
        let Some(name) = self.project_name() else {
            ui.weak("No project open");
            return;
        };
        let shown = if name.chars().count() > name_chars {
            let cut: String = name.chars().take(name_chars.saturating_sub(1)).collect();
            format!("{cut}…")
        } else {
            name.clone()
        };
        ui.label(shown).on_hover_text(name);
        let page = self.current_page.and_then(|id| {
            let sheet = self.wb.sheet;
            let rows = self.page_rows();
            let mine = rows.iter().filter(|r| r.page == id);
            // The shown sheet, or the page's last when it has fewer now.
            mine.clone()
                .find(|r| r.sheet == sheet)
                .or_else(|| mine.clone().next_back())
                .cloned()
        });
        if let Some(PageRow { label, .. }) = page {
            ui.weak("/");
            let link = ui.add(egui::Button::new(egui::RichText::new(label).strong()).frame(false));
            if link.clicked() && !self.outline_pinned() {
                self.outline_open ^= true;
            }
            self.breadcrumb_link = Some(link);
        }
        // A background that hides the wiring must show it exists.
        let background = self
            .page_properties()
            .and_then(novo_interop::underlay::stored_name)
            .map(str::to_owned);
        if let Some(file) = background {
            let shown = self.background_shown();
            let text = if shown {
                format!("DXF background: {file}")
            } else {
                format!("DXF background hidden: {file}")
            };
            let tip = if shown {
                "Click to hide"
            } else {
                "Click to show"
            };
            if ui
                .add(egui::Button::new(egui::RichText::new(text).weak()).frame(false))
                .on_hover_text(tip)
                .clicked()
            {
                self.toggle_background();
            }
        }
    }

    /// The outline's content: mode tabs, then the pages or the issues.
    /// Picking a page or an issue closes the flyout.
    fn outline(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            for mode in [
                OutlineMode::Pages,
                OutlineMode::Issues,
                OutlineMode::Changes,
                OutlineMode::Commissioning,
            ] {
                if ui
                    .selectable_label(self.outline_mode == mode, mode.label())
                    .clicked()
                {
                    self.outline_mode = mode;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let pinned = self.outline_pinned();
                let pin = egui::Button::selectable(pinned, "Pin");
                if ui
                    .add(pin)
                    .on_hover_text(if pinned {
                        "Unpin outline"
                    } else {
                        "Pin outline"
                    })
                    .clicked()
                {
                    picked.push(Action::PinOutline);
                }
            });
        });
        if self.outline_mode == OutlineMode::Issues {
            if self.issues_list(ui) {
                self.outline_open = false;
            }
            return;
        }
        if self.outline_mode == OutlineMode::Commissioning {
            if self.commissioning_outline(ui) {
                self.outline_open = false;
            }
            return;
        }
        if self.outline_mode == OutlineMode::Changes {
            if self.changes_outline(ui) {
                self.outline_open = false;
            }
            return;
        }
        if self.outline_pages(ui) {
            self.outline_open = false;
        }
    }

    /// Adds the contents page of a panel, which lists the panel's pages and
    /// follows it when it is renamed.
    fn add_panel_contents(&mut self, panel: &DeviceTag) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        match pages::panel_contents_command(project, panel) {
            Ok(command) => {
                if let Some(Err(e)) = self.execute(command) {
                    self.message(
                        Level::Error,
                        format!("Could not add the contents page: {e}"),
                    );
                }
            }
            Err(e) => self.message(Level::Warning, e),
        }
    }

    /// Renames a page group or panel everywhere its name is used, as one
    /// undo step, and says what it touched.
    fn rename_level(&mut self, aspect: Aspect, from: &DeviceTag, text: &str) {
        let command = match pages::rename_level_command(aspect, from, text) {
            Ok(Some(command)) => command,
            Ok(None) => return,
            Err(e) => return self.message(Level::Warning, e),
        };
        let (used, merges) = self
            .backend
            .document()
            .map(|d| {
                let project = d.project();
                (
                    pages::level_users(project, aspect, from),
                    pages::merges_with_existing(project, aspect, from, text),
                )
            })
            .unwrap_or_default();
        let merged = match (merges, aspect) {
            (false, _) => "",
            (true, Aspect::Function) => " (merges with the existing page group)",
            (true, _) => " (merges with the existing panel)",
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(Level::Info, format!("Renamed {from}{merged}: {used}")),
            Some(Err(e)) => self.message(Level::Error, format!("Could not rename {from}: {e}")),
            None => {}
        }
    }

    /// Puts the pages of a group and panel in the given order, as one undo
    /// step.
    fn reorder_pages(&mut self, pages: Vec<PageId>) {
        if let Some(Err(e)) = self.execute(Command::ReorderPages { pages }) {
            self.message(Level::Error, format!("Could not reorder the pages: {e}"));
        }
    }

    /// Below the pinning width the outline floats over the canvas, opened
    /// from the "Pages" button at its top left, so the drawing never moves.
    fn outline_flyout(
        &mut self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        picked: &mut Vec<Action>,
    ) {
        let toggle = egui::Area::new(egui::Id::new("outline_toggle"))
            .fixed_pos(canvas.left_top() + egui::vec2(8.0, 8.0))
            .show(ctx, |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                ui.add(egui::Button::selectable(
                    self.outline_open,
                    self.outline_mode.label(),
                ))
            })
            .inner;
        if toggle.clicked() {
            self.outline_open ^= true;
        }
        if !self.outline_open {
            return;
        }
        let top = canvas.top() + 8.0 + toggle.rect.height() + 4.0;
        let height = (canvas.bottom() - top - 8.0).max(120.0);
        let area = egui::Area::new(egui::Id::new("outline_flyout"))
            .fixed_pos(egui::pos2(canvas.left() + 8.0, top))
            .show(ctx, |ui| {
                let frame = egui::Frame::popup(ui.style());
                // The frame's margins and stroke come on top of the content.
                let chrome = frame.total_margin().sum();
                frame.show(ui, |ui| {
                    ui.set_width(OUTLINE_WIDTH - chrome.x);
                    ui.set_height(height - chrome.y);
                    self.outline(ui, picked);
                });
            });
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        // The "Pages" toggle and the breadcrumb's page link open and close
        // the flyout themselves; a click on either is not a click elsewhere.
        let opener = |r: &egui::Response| r.clicked() || r.hovered();
        // The issue pill opens it in Issues mode.
        let on_pill = ctx
            .pointer_interact_pos()
            .is_some_and(|p| self.wb.pill_rect.contains(p));
        let elsewhere = area.response.clicked_elsewhere()
            && !opener(&toggle)
            && !on_pill
            && !self.breadcrumb_link.as_ref().is_some_and(opener);
        if escape || elsewhere {
            self.outline_open = false;
        }
    }

    /// With no project open the canvas area offers the ways in: new, open
    /// and the recent projects.
    fn start_panel(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        let rect = ui.max_rect();
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let width = 320.0_f32.min(rect.width() - 32.0);
        let top = rect.top() + (rect.height() * 0.25).min(200.0);
        let column = egui::Rect::from_min_size(
            egui::pos2(rect.center().x - width / 2.0, top),
            egui::vec2(width, rect.bottom() - top),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
            ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                ui.weak("No project open");
                ui.add_space(8.0);
                if self.first_run() {
                    self.action_button(ui, Action::TryFastKeys, picked);
                    ui.weak("Two minutes: place, wire, cable, live view");
                    ui.add_space(8.0);
                }
                for a in [Action::NewProject, Action::OpenProject, Action::OpenExample] {
                    self.action_button(ui, a, picked);
                }
                if !self.first_run() {
                    self.action_button(ui, Action::TryFastKeys, picked);
                }
                self.learn_start_entry(ui);
                let unsaved = self.backend.unsaved_projects();
                if !unsaved.is_empty() {
                    ui.add_space(16.0);
                    ui.small("UNSAVED");
                    for (path, changed) in unsaved {
                        let row = ui.add(
                            egui::Button::new(format!("Untitled, {}", ago(changed))).frame(false),
                        );
                        if row.clicked() {
                            let ctx = ui.ctx().clone();
                            self.leave_then(&ctx, Leaving::OpenPath(path));
                        }
                    }
                }
                let recent = self.settings.recent_projects.clone();
                if !recent.is_empty() {
                    ui.add_space(16.0);
                    ui.small("RECENT");
                    for path in recent {
                        let name = path
                            .file_stem()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        let row = ui
                            .add(egui::Button::new(name).frame(false))
                            .on_hover_text(path.display().to_string());
                        if row.clicked() {
                            let ctx = ui.ctx().clone();
                            self.leave_then(&ctx, Leaving::OpenPath(path));
                        }
                    }
                }
            });
        });
    }

    /// Cursor position and friends, faint, at the canvas's top right. On
    /// the drawing, the step hint is in the tool dock instead.
    fn hud(&self, ctx: &egui::Context, canvas: egui::Rect) {
        // The status bar carries these values when it is on.
        if self.status_bar_shown() {
            return;
        }
        let text = match self.canvas.schematic_ref() {
            Some(view) => view
                .editor
                .cursor()
                .map(|p| format!("X {:.1} mm  Y {:.1} mm", p.x, p.y)),
            None => self.canvas.status_text(),
        };
        let Some(text) = text else {
            return;
        };
        egui::Area::new(egui::Id::new("hud"))
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(canvas.right_top() + egui::vec2(-12.0, 10.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.add(egui::Label::new(egui::RichText::new(text).monospace().weak()).extend());
            });
    }

    /// The latest messages, bottom left over the canvas. Info fades on its
    /// own; warnings and errors stay until clicked.
    fn toasts(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let now = Instant::now();
        let mut fade_in: Option<Duration> = None;
        let shown: Vec<usize> = self
            .messages
            .iter()
            .enumerate()
            .rev()
            .filter(|(_, m)| {
                if m.dismissed {
                    return false;
                }
                if m.level != Level::Info {
                    return true;
                }
                // The status bar shows info messages when it is on.
                if self.status_bar_shown() {
                    return false;
                }
                let left = TOAST_INFO.saturating_sub(now - m.at);
                if !left.is_zero() {
                    fade_in = Some(fade_in.map_or(left, |f| f.min(left)));
                }
                !left.is_zero()
            })
            .map(|(i, _)| i)
            .take(MAX_TOASTS)
            .collect();
        if let Some(wait) = fade_in {
            ctx.request_repaint_after(wait);
        }
        if shown.is_empty() {
            return;
        }
        let mut dismiss = Vec::new();
        egui::Area::new(egui::Id::new("toasts"))
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(canvas.left_bottom() + egui::vec2(12.0, -12.0))
            .show(ctx, |ui| {
                ui.set_max_width(420.0);
                for &i in shown.iter().rev() {
                    let m = &self.messages[i];
                    let color = match m.level {
                        Level::Info => ui.visuals().text_color(),
                        Level::Warning => ui.visuals().warn_fg_color,
                        Level::Error => ui.visuals().error_fg_color,
                    };
                    let frame = egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.add(egui::Label::new(egui::RichText::new(&m.text).color(color)).wrap());
                    });
                    let response = frame.response.interact(egui::Sense::click());
                    if response.on_hover_text("Click to dismiss").clicked() {
                        dismiss.push(i);
                    }
                }
            });
        for i in dismiss {
            self.messages[i].dismissed = true;
        }
    }

    fn action_button(&self, ui: &mut egui::Ui, action: Action, picked: &mut Vec<Action>) {
        let mut button = egui::Button::new(action.label());
        if let Some(s) = action.shortcut() {
            button = button.shortcut_text(ui.ctx().format_shortcut(&s));
        }
        if ui.add_enabled(self.is_enabled(action), button).clicked() {
            picked.push(action);
        }
    }

    // ------------------------------------------------------------------
    // Dialogs

    /// Asks whether to restore the unsaved work a crash left behind. The
    /// project as last saved stays shown until the user answers.
    fn recovery_dialog(&mut self, ctx: &egui::Context) {
        let Some(offer) = self.backend.recovery_offer() else {
            return;
        };
        let name = self.project_name().unwrap_or_default();
        let when = offer
            .autosaved_at
            .map(|t| format!(" from {}", ago(t)))
            .unwrap_or_default();
        let mut answer = None;
        egui::Modal::new(egui::Id::new("recovery")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("Restore unsaved work?");
            ui.label(format!(
                "KoperCAD did not close cleanly last time. \"{name}\" has unsaved work{when}."
            ));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Restore").clicked() {
                    answer = Some(true);
                }
                if ui.button("Discard").clicked() {
                    answer = Some(false);
                }
            });
        });
        // No implicit answer on Escape or a backdrop click: either choice
        // changes what is on disk, so it takes a deliberate click.
        if let Some(restore) = answer {
            self.resolve_recovery(restore);
        }
    }

    fn resolve_recovery(&mut self, restore: bool) {
        match self.backend.resolve_recovery(restore) {
            Ok(()) if restore => {
                self.document_replaced();
                self.fix_current_page();
                self.message(Level::Info, "Restored the unsaved work. Save to keep it.");
            }
            Ok(()) => {
                if !self.is_open() {
                    self.project_path = None;
                    self.current_page = None;
                    self.document_replaced();
                }
                self.message(Level::Info, "Discarded the unsaved work");
            }
            Err(e) => self.message(
                Level::Error,
                format!("Could not discard the unsaved work: {e}"),
            ),
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let modal = egui::Modal::new(egui::Id::new("about")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading(APP_NAME);
            ui.label(format!("by {}", APP_MAKER));
            ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
            ui.label(format!("Model API v{}", novo_core::MODEL_VERSION));
            ui.add_space(8.0);
            ui.button("Close").clicked()
        });
        if modal.inner || modal.should_close() {
            self.show_about = false;
        }
    }
}

impl eframe::App for NovoApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, Settings::STORAGE_KEY, &self.settings);
    }

    fn on_exit(&mut self) {
        // A clean close deletes the recovery file, so the next start does
        // not offer to restore work that was deliberately not saved.
        if let Err(e) = self.backend.close() {
            log::warn!("The project did not close cleanly: {e}");
        }
    }
}

fn is_not_found(e: &BackendError) -> bool {
    matches!(
        e,
        BackendError::Storage(novo_storage::StorageError::Io(io))
            if io.kind() == std::io::ErrorKind::NotFound
    )
}

/// One line for the status bar. Storage errors already name the file and
/// say what is wrong in plain words.
fn describe_error(verb: &str, path: &Path, e: &BackendError) -> String {
    use novo_storage::StorageError as S;
    match e {
        BackendError::Storage(
            S::Locked(_) | S::NotAProject(_) | S::NewerFormat { .. } | S::Corrupt { .. },
        ) => format!("Could not {verb} the project: {e}"),
        _ if is_not_found(e) => format!(
            "Could not {verb} {}: the file does not exist",
            path.display()
        ),
        _ => format!("Could not {verb} {}: {e}", path.display()),
    }
}

/// "3 minutes ago" style text for a past moment.
fn ago(t: SystemTime) -> String {
    let secs = SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    match secs {
        0..60 => "less than a minute ago".to_owned(),
        60..120 => "1 minute ago".to_owned(),
        120..3600 => format!("{} minutes ago", secs / 60),
        3600..7200 => "1 hour ago".to_owned(),
        7200..86400 => format!("{} hours ago", secs / 3600),
        _ => format!("{} days ago", secs / 86400),
    }
}

/// Where new projects are saved: `<Documents>\KoperCAD` (or the old
/// `NovoElectric` folder if that is where the projects already are). Documents comes
/// from the known-folder lookup, because company PCs often redirect it to
/// OneDrive; it is always inside the user's profile.
fn projects_dir() -> Option<PathBuf> {
    dirs::document_dir().map(|docs| crate::migrate::projects_dir_in(&docs))
}

/// `<name>.novo`, with the characters Windows forbids in file names replaced.
fn project_file_name(name: &str) -> String {
    let name: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let name = name.trim_end_matches(['.', ' ']);
    let name = if name.is_empty() { "Untitled" } else { name };
    format!("{name}.{PROJECT_EXTENSION}")
}

/// The name a project has before anyone names it.
fn is_default_name(name: &str) -> bool {
    name.trim().is_empty() || name == "Untitled"
}

/// A file's name without its extension.
fn file_stem(path: &Path) -> Option<String> {
    path.file_stem().map(|s| s.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{MemoryBackend, OpenReport, StorageBackend, new_document};
    use crate::canvas::PlaceholderCanvas;
    use novo_core::Document;

    /// Opens and saves "files" in memory; `new_project` leaves an unsaved
    /// edit so the unsaved-changes flow can be tested.
    #[derive(Default)]
    struct DirtyBackend {
        document: Option<Document>,
    }

    impl ProjectBackend for DirtyBackend {
        fn new_project(&mut self, name: &str) -> Result<(), BackendError> {
            let mut doc = new_document(name)?;
            doc.transact("Add page", |tx| tx.add_page("2"))?;
            self.document = Some(doc);
            Ok(())
        }
        fn open(&mut self, _path: &Path) -> Result<OpenReport, BackendError> {
            self.document = Some(new_document("Opened")?);
            Ok(OpenReport::default())
        }
        fn save(&mut self, _path: &Path) -> Result<(), BackendError> {
            if let Some(doc) = &mut self.document {
                doc.mark_saved();
            }
            Ok(())
        }
        fn close(&mut self) -> Result<(), BackendError> {
            self.document = None;
            Ok(())
        }
        fn document(&self) -> Option<&Document> {
            self.document.as_ref()
        }
        fn document_mut(&mut self) -> Option<&mut Document> {
            self.document.as_mut()
        }
    }

    /// A context with the app's theme and fonts, as `NovoApp::new` sets up.
    fn themed_ctx() -> egui::Context {
        let ctx = egui::Context::default();
        novo_theme::apply(&ctx);
        ctx
    }

    fn app_with(backend: impl ProjectBackend + 'static) -> NovoApp {
        NovoApp::with_settings(
            Box::new(backend),
            Box::new(PlaceholderCanvas::default()),
            Settings::default(),
        )
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::RawInput {
        egui::RawInput {
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        }
    }

    fn frame(ctx: &egui::Context, app: &mut NovoApp, input: egui::RawInput) {
        let mut output = ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
    }

    #[test]
    fn renders_with_and_without_a_project() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, Default::default());
        app.run(&ctx, Action::NewProject);
        frame(&ctx, &mut app, Default::default());
        assert!(app.current_page.is_some());
    }

    #[test]
    fn ctrl_n_creates_a_project() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert!(app.is_open());
    }

    #[test]
    fn ctrl_k_opens_the_command_field() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, key(egui::Key::K, egui::Modifiers::COMMAND));
        assert!(app.wb.field.is_open());
        frame(
            &ctx,
            &mut app,
            key(egui::Key::Escape, egui::Modifiers::NONE),
        );
        assert!(!app.wb.field.is_open());
    }

    #[test]
    fn save_without_a_project_is_ignored() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::SaveProject);
        assert!(app.messages().is_empty());
    }

    #[test]
    fn project_file_names_are_safe_on_windows() {
        assert_eq!(project_file_name("Pump station"), "Pump station.novo");
        assert_eq!(project_file_name("A/B: C?"), "A_B_ C_.novo");
        assert_eq!(project_file_name(" . "), "Untitled.novo");
    }

    #[test]
    fn new_projects_are_saved_under_documents() {
        if let Some(dir) = projects_dir() {
            assert!(
                dir.ends_with(crate::DATA_DIR_NAME) || dir.ends_with(crate::LEGACY_DATA_DIR_NAME)
            );
            assert!(dir.parent().is_some_and(|p| p.is_absolute()));
        }
    }

    #[test]
    fn closing_a_changed_project_does_not_ask() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.run(&ctx, Action::NewProject);
        assert!(app.is_dirty());
        app.run(&ctx, Action::CloseProject);
        assert!(!app.is_open());
        assert!(!app.modal_open());
    }

    #[test]
    fn placeholder_open_reports_an_error() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.leave_then(&ctx, Leaving::OpenPath(PathBuf::from("x.novo")));
        assert_eq!(app.messages().last().map(|m| m.level), Some(Level::Error));
        assert!(!app.is_open());
    }

    #[test]
    fn startup_file_is_opened_on_first_frame() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.open_on_start(PathBuf::from("plant.novo"));
        frame(&ctx, &mut app, Default::default());
        assert!(app.is_open());
        assert_eq!(app.project_path, Some(PathBuf::from("plant.novo")));
        assert_eq!(
            app.settings().recent_projects,
            vec![PathBuf::from("plant.novo")]
        );
    }

    #[test]
    fn undo_keeps_the_shown_page_valid() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.run(&ctx, Action::NewProject);
        let second = app.pages()[1].0;
        app.current_page = Some(second);
        assert!(app.is_enabled(Action::Undo));
        app.run(&ctx, Action::Undo);
        assert_eq!(app.pages().len(), 1);
        assert_ne!(app.current_page, Some(second));
        assert!(app.current_page.is_some());
        assert!(!app.is_dirty());
        assert!(app.is_enabled(Action::Redo));
    }

    #[test]
    fn saving_clears_the_dirty_flag() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.run(&ctx, Action::NewProject);
        assert!(app.is_dirty());
        assert!(app.save_to(Path::new("plant.novo")));
        assert!(!app.is_dirty());
        frame(&ctx, &mut app, Default::default());
        assert_eq!(app.window_title, "plant - KoperCAD", "named after its file");
    }

    /// Which commands a single key press would run.
    fn fired_by(app: &NovoApp, key_: egui::Key, modifiers: egui::Modifiers) -> Vec<&'static str> {
        let ctx = themed_ctx();
        let mut fired = Vec::new();
        let mut output = ctx.run_ui(key(key_, modifiers), |ui| {
            fired = app.key_commands(ui.ctx());
        });
        output.textures_delta.clear();
        fired
    }

    #[test]
    fn bare_shortcuts_ignore_extra_modifiers() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        assert_eq!(
            fired_by(&app, egui::Key::Home, egui::Modifiers::NONE),
            vec!["view.fit"]
        );
        assert!(fired_by(&app, egui::Key::Home, egui::Modifiers::SHIFT).is_empty());
    }

    #[test]
    fn disabled_actions_leave_their_keys_alone() {
        let app = app_with(MemoryBackend::default());
        assert!(fired_by(&app, egui::Key::S, egui::Modifiers::COMMAND).is_empty());
    }

    #[test]
    fn ctrl_y_redoes() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.run(&ctx, Action::NewProject);
        app.run(&ctx, Action::Undo);
        assert_eq!(
            fired_by(&app, egui::Key::Y, egui::Modifiers::COMMAND),
            vec!["edit.redo"]
        );
        assert_eq!(
            fired_by(
                &app,
                egui::Key::Z,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
            ),
            vec!["edit.redo"]
        );
    }

    fn storage_app() -> NovoApp {
        app_with(StorageBackend::default())
    }

    fn page_count(app: &NovoApp) -> usize {
        app.pages().len()
    }

    #[test]
    fn edits_and_undo_reach_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let ctx = themed_ctx();

        let mut app = storage_app();
        app.run(&ctx, Action::NewProject);
        let page = app.current_page.unwrap();
        let mut open = OpenPage::new(&mut *app.backend, page).unwrap();
        open.transact("Add page", |tx| tx.add_page("2")).unwrap();
        open.transact("Add page", |tx| tx.add_page("3")).unwrap();
        assert!(app.is_dirty());
        assert!(app.save_to(&path));
        assert!(!app.is_dirty());

        app.run(&ctx, Action::Undo);
        assert!(app.is_dirty());
        assert!(app.save_to(&path));
        app.run(&ctx, Action::CloseProject);
        assert!(!app.is_open());

        let mut reopened = storage_app();
        reopened.open_path(&path);
        assert_eq!(page_count(&reopened), 2, "{:?}", reopened.messages());
    }

    #[test]
    fn a_crash_offers_the_unsaved_work_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        crate::backend::tests::crash_after_autosave(&path);
        let ctx = themed_ctx();

        let mut app = storage_app();
        app.open_path(&path);
        assert_eq!(page_count(&app), 2);
        assert!(app.modal_open(), "the recovery question blocks the window");
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert_eq!(page_count(&app), 2, "shortcuts wait for the answer");

        app.resolve_recovery(true);
        assert!(!app.modal_open());
        assert_eq!(page_count(&app), 3);
        assert!(app.is_dirty(), "restored work still needs a save");
        frame(&ctx, &mut app, Default::default());
        assert_eq!(app.window_title, "Plant - KoperCAD");
    }

    #[test]
    fn an_unsaved_new_project_is_offered_back_after_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        crate::backend::tests::crash_untitled(dir.path());
        let ctx = themed_ctx();
        let mut app = app_with(StorageBackend::with_untitled_dir(dir.path()));
        frame(&ctx, &mut app, Default::default());
        assert!(app.is_open());
        assert!(app.modal_open());
        app.resolve_recovery(true);
        assert_eq!(page_count(&app), 2);
        assert_eq!(app.project_path, None);
        assert!(app.is_dirty());

        // Discarding instead leaves no project open.
        let dir = tempfile::tempdir().unwrap();
        crate::backend::tests::crash_untitled(dir.path());
        let mut app = app_with(StorageBackend::with_untitled_dir(dir.path()));
        frame(&ctx, &mut app, Default::default());
        app.resolve_recovery(false);
        assert!(!app.is_open());
    }

    #[test]
    fn opening_a_missing_recent_file_forgets_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("gone.novo");
        let mut app = storage_app();
        app.settings.remember_recent(&path);
        app.open_path(&path);
        assert!(app.settings().recent_projects.is_empty());
        let last = app.messages().last().unwrap();
        assert_eq!(last.level, Level::Error);
        assert!(last.text.contains("does not exist"), "{}", last.text);
    }

    #[test]
    fn a_file_open_elsewhere_is_reported_plainly() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let ctx = themed_ctx();
        let mut first = storage_app();
        first.run(&ctx, Action::NewProject);
        assert!(first.save_to(&path));

        let mut second = storage_app();
        second.open_path(&path);
        assert!(!second.is_open());
        let text = &second.messages().last().unwrap().text;
        assert!(text.contains("already open"), "{text}");
    }

    fn sized(width: f32) -> egui::RawInput {
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, 800.0),
            )),
            ..Default::default()
        }
    }

    #[test]
    fn naming_a_project_sets_its_name_and_its_file_together() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = themed_ctx();
        let mut app = app_with(crate::backend::StorageBackend::with_untitled_dir(
            dir.path().join("untitled"),
        ));
        app.run(&ctx, Action::NewProject);
        assert_eq!(app.project_name().as_deref(), Some("Untitled"));

        let plant = dir.path().join("Plant.novo");
        assert!(app.name_and_save(&plant));
        let name = |app: &NovoApp| {
            app.backend
                .document()
                .unwrap()
                .project()
                .info()
                .name
                .clone()
        };
        assert_eq!(name(&app), "Plant");
        assert_eq!(app.project_name().as_deref(), Some("Plant"));

        // Saved under another name, a project named after its file follows.
        let mill = dir.path().join("Mill.novo");
        assert!(app.name_and_save(&mill));
        assert_eq!(name(&app), "Mill");

        // A name the user chose is kept whatever the file is called.
        app.backend
            .document_mut()
            .unwrap()
            .transact("Rename", |tx| {
                tx.set_info(|i| i.name = "Water works".into())
            })
            .unwrap();
        assert!(app.name_and_save(&dir.path().join("copy.novo")));
        assert_eq!(name(&app), "Water works");

        // A file saved before naming set the name shows its file name.
        app.backend
            .document_mut()
            .unwrap()
            .transact("Reset", |tx| tx.set_info(|i| i.name = "Untitled".into()))
            .unwrap();
        assert_eq!(app.project_name().as_deref(), Some("copy"));
    }

    #[test]
    fn the_outline_pins_on_wide_windows_unless_chosen() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        frame(&ctx, &mut app, sized(1366.0));
        assert!(!app.outline_pinned());
        frame(&ctx, &mut app, sized(1920.0));
        assert!(app.outline_pinned());

        app.run(&ctx, Action::PinOutline);
        assert_eq!(app.settings().outline_pinned, Some(false));
        frame(&ctx, &mut app, sized(1920.0));
        assert!(!app.outline_pinned());
    }

    fn click_at(pos: egui::Pos2) -> egui::RawInput {
        let mut input = sized(1366.0);
        input.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ];
        input
    }

    #[test]
    fn the_breadcrumb_page_opens_the_flyout_and_a_canvas_click_closes_it() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        frame(&ctx, &mut app, sized(1366.0));
        assert!(!app.outline_pinned());
        let link = app.breadcrumb_link.as_ref().expect("page link").rect;

        frame(&ctx, &mut app, click_at(link.center()));
        assert!(app.outline_open, "the breadcrumb click opens the flyout");
        frame(&ctx, &mut app, sized(1366.0));
        assert!(app.outline_open, "and it stays open the next frame");

        // A click on the canvas, away from the flyout, closes it.
        frame(&ctx, &mut app, click_at(egui::pos2(1000.0, 500.0)));
        frame(&ctx, &mut app, sized(1366.0));
        assert!(!app.outline_open);
    }

    #[test]
    fn the_issue_pill_opens_the_outline_in_issues_mode() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        frame(&ctx, &mut app, sized(1366.0));
        assert!(!app.outline_pinned());
        let pill = app.wb.pill_rect;
        assert!(pill.is_positive(), "the pill is drawn");

        frame(&ctx, &mut app, click_at(pill.center()));
        frame(&ctx, &mut app, sized(1366.0));
        assert!(app.outline_open, "the pill opens the flyout");
        assert_eq!(app.outline_mode, OutlineMode::Issues);

        // A second click on the pill closes it again.
        frame(&ctx, &mut app, click_at(pill.center()));
        frame(&ctx, &mut app, sized(1366.0));
        assert!(!app.outline_open);

        // F8-style requests (check.problems) open it, never close it.
        app.show_issues(false);
        assert!(app.outline_open);
        app.show_issues(false);
        assert!(app.outline_open, "without toggle it stays open");
    }

    #[test]
    fn focus_mode_toggles_and_closes_the_flyout() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        app.outline_open = true;
        frame(
            &ctx,
            &mut app,
            key(
                egui::Key::F,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
            ),
        );
        assert!(app.focus_mode);
        assert!(!app.outline_open);
        app.run(&ctx, Action::FocusMode);
        assert!(!app.focus_mode);
    }

    #[test]
    fn live_potentials_toggle_from_the_command_and_survive_frames() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        assert!(!app.wb.live.on);
        app.run_command(&ctx, "sim.live", None);
        assert!(app.wb.live.on);
        frame(&ctx, &mut app, sized(1366.0));
        app.run_command(&ctx, "sim.live", None);
        assert!(!app.wb.live.on);
    }

    #[test]
    fn cabinet_needs_an_open_project() {
        let app = app_with(MemoryBackend::default());
        assert!(!app.is_enabled(Action::ShowCabinet));
        assert!(app.is_enabled(Action::ShowSchematic));
    }

    #[test]
    fn info_toasts_fade_and_errors_stay_until_dismissed() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.message(Level::Info, "Saved");
        app.message(Level::Error, "Could not save");
        app.messages[0].at -= TOAST_INFO;
        app.messages[1].at -= TOAST_INFO;
        frame(&ctx, &mut app, Default::default());
        let open = ctx.memory(|m| m.areas().visible_layer_ids());
        assert!(open.iter().any(|l| l.id == egui::Id::new("toasts")));
        app.messages[1].dismissed = true;
        frame(&ctx, &mut app, Default::default());
        frame(&ctx, &mut app, Default::default());
        let open = ctx.memory(|m| m.areas().visible_layer_ids());
        assert!(!open.iter().any(|l| l.id == egui::Id::new("toasts")));
    }

    #[test]
    fn the_saved_state_follows_the_project() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        assert_eq!(app.saved_state(), "");
        app.run(&ctx, Action::NewProject);
        assert_eq!(app.saved_state(), "Not saved yet");
        assert!(app.save_to(Path::new("plant.novo")));
        assert_eq!(app.saved_state(), "Saved");
        app.run(&ctx, Action::Undo);
        assert_eq!(app.saved_state(), "Saving");
    }

    #[test]
    fn start_up_reopens_the_last_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let ctx = themed_ctx();
        let mut first = storage_app();
        first.run(&ctx, Action::NewProject);
        assert!(first.save_to(&path));
        first.run(&ctx, Action::CloseProject);

        let mut next = app_with(StorageBackend::default());
        next.settings.last_open = Some(path.clone());
        frame(&ctx, &mut next, Default::default());
        assert_eq!(next.project_path, Some(path));

        // A last project that is gone leaves the start panel, quietly.
        let mut gone = storage_app();
        gone.settings.remember_recent(&dir.path().join("gone.novo"));
        frame(&ctx, &mut gone, Default::default());
        assert!(!gone.is_open());
        assert!(gone.messages().is_empty());
    }

    #[test]
    fn commands_run_through_the_window_reach_storage() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let ctx = themed_ctx();
        let mut app = storage_app();
        let add = || Command::AddPage {
            name: "2".into(),
            title: String::new(),
            kind: Default::default(),
            format: Default::default(),
            structure: Default::default(),
        };
        assert!(app.execute(add()).is_none());
        app.run(&ctx, Action::NewProject);
        assert!(app.save_to(&path));
        app.execute(add()).unwrap().unwrap();
        assert_eq!(page_count(&app), 2);
        assert!(app.save_to(&path));
        app.run(&ctx, Action::CloseProject);

        let mut reopened = storage_app();
        reopened.open_path(&path);
        assert_eq!(page_count(&reopened), 2);
    }
}
