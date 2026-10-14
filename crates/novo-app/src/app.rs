//! The main window, laid out as the Workbench of docs/DESIGN.md §3: a thin
//! top bar, the page outline (pinned or a flyout), the canvas with its HUD,
//! toasts for messages, the dialogs and action dispatch. There is no menu
//! bar and no status bar.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;

use crate::actions::Action;
use novo_core::{Command, ModelError, Outcome, PageId};

use crate::backend::{BackendError, ProjectBackend, page_labels};
use crate::canvas::{CanvasView, OpenPage};
use crate::palette::{Entry, Palette, Target};
use crate::settings::Settings;
use crate::{APP_NAME, PROJECT_EXTENSION};
use novo_theme::widgets::{primary_button, section_label, segment};

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
/// Pages listed under "Recent" in the outline.
const MAX_RECENT_PAGES: usize = 5;

/// An action that throws away the open project, so it first asks about
/// unsaved changes.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Discarding {
    New,
    Open,
    OpenPath(PathBuf),
    Close,
    Quit,
}

pub struct NovoApp {
    backend: Box<dyn ProjectBackend>,
    canvas: Box<dyn CanvasView>,
    settings: Settings,
    project_path: Option<PathBuf>,
    current_page: Option<PageId>,
    palette: Palette,
    messages: Vec<Message>,
    /// Waiting for the user to answer "save changes?" before doing this.
    unsaved_prompt: Option<Discarding>,
    show_about: bool,
    /// Set once the user has agreed to quit, so the next close request passes.
    quitting: bool,
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
    /// The outline flyout is showing over the canvas.
    outline_open: bool,
    /// Text in the outline's page filter.
    page_filter: String,
    /// Pages visited this session, newest first.
    recent_pages: Vec<PageId>,
    /// Window width last frame, which decides whether the outline pins.
    window_width: f32,
}

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
        Self::with_settings(backend, canvas, settings)
    }

    pub fn with_settings(
        backend: Box<dyn ProjectBackend>,
        canvas: Box<dyn CanvasView>,
        settings: Settings,
    ) -> Self {
        Self {
            backend,
            canvas,
            settings,
            project_path: None,
            current_page: None,
            palette: Palette::default(),
            messages: Vec::new(),
            unsaved_prompt: None,
            show_about: false,
            quitting: false,
            window_title: String::new(),
            open_on_start: None,
            started: false,
            next_tick: Instant::now(),
            autosave_failing: false,
            focus_mode: false,
            outline_open: false,
            page_filter: String::new(),
            recent_pages: Vec::new(),
            window_width: 0.0,
        }
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

    fn project_name(&self) -> Option<String> {
        self.backend.document().map(|d| {
            let name = &d.project().info().name;
            if name.is_empty() {
                "Untitled".to_owned()
            } else {
                name.clone()
            }
        })
    }

    fn pages(&self) -> Vec<(PageId, String)> {
        self.backend.document().map(page_labels).unwrap_or_default()
    }

    /// Keeps the shown page valid after undo, redo or a reload removed it.
    fn fix_current_page(&mut self) {
        let pages = self.pages();
        self.recent_pages
            .retain(|r| pages.iter().any(|(id, _)| id == r));
        if !pages.iter().any(|(id, _)| Some(*id) == self.current_page) {
            self.current_page = pages.first().map(|(id, _)| *id);
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
            | Action::ZoomToFit => open,
            Action::ShowCabinet => false,
            Action::Undo => doc.is_some_and(|d| d.can_undo()),
            Action::Redo => doc.is_some_and(|d| d.can_redo()),
            _ => true,
        }
    }

    /// Runs an action as if the user had picked it.
    pub fn run(&mut self, ctx: &egui::Context, action: Action) {
        if !self.is_enabled(action) {
            return;
        }
        if action != Action::CommandPalette {
            self.settings.remember_action(action.id());
        }
        match action {
            Action::NewProject => self.discard_then(ctx, Discarding::New),
            Action::OpenProject => self.discard_then(ctx, Discarding::Open),
            Action::SaveProject => {
                self.save();
            }
            Action::SaveProjectAs => {
                self.save_as();
            }
            Action::CloseProject => self.discard_then(ctx, Discarding::Close),
            Action::Quit => self.discard_then(ctx, Discarding::Quit),
            Action::Undo => self.undo_redo(false),
            Action::Redo => self.undo_redo(true),
            Action::ZoomToFit => self.canvas.zoom_to_fit(),
            // The only canvas so far; Cabinet stays disabled until it exists.
            Action::ShowSchematic | Action::ShowCabinet => {}
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
            Action::CommandPalette => self.palette.open(),
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
    }

    // ------------------------------------------------------------------
    // Project lifecycle

    fn discard_then(&mut self, ctx: &egui::Context, next: Discarding) {
        if self.is_dirty() {
            self.unsaved_prompt = Some(next);
        } else {
            self.finish_discarding(ctx, next);
        }
    }

    fn finish_discarding(&mut self, ctx: &egui::Context, next: Discarding) {
        match next {
            Discarding::New => self.new_project(),
            Discarding::Open => {
                let dialog = rfd::FileDialog::new()
                    .set_title("Open project")
                    .add_filter("NovoElectric project", &[PROJECT_EXTENSION]);
                if let Some(path) = dialog.pick_file() {
                    self.open_path(&path);
                }
            }
            Discarding::OpenPath(path) => self.open_path(&path),
            Discarding::Close => self.close_project(),
            Discarding::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    fn new_project(&mut self) {
        match self.backend.new_project("Untitled") {
            Ok(()) => {
                self.project_path = None;
                self.current_page = None;
                self.canvas.document_replaced();
                self.fix_current_page();
            }
            Err(e) => self.message(Level::Error, format!("Could not create a project: {e}")),
        }
    }

    fn open_path(&mut self, path: &Path) {
        match self.backend.open(path) {
            Ok(report) => {
                self.project_path = Some(path.to_owned());
                self.current_page = None;
                self.canvas.document_replaced();
                self.fix_current_page();
                self.settings.remember_recent(path);
                self.message(Level::Info, format!("Opened {}", path.display()));
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
                self.canvas.document_replaced();
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
            && let Some(last) = self.settings.recent_projects.first().cloned()
            && last.is_file()
        {
            self.open_path(&last);
        }
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

    fn close_project(&mut self) {
        if let Err(e) = self.backend.close() {
            self.message(
                Level::Warning,
                format!("The project did not close cleanly: {e}"),
            );
        }
        self.project_path = None;
        self.current_page = None;
        self.canvas.document_replaced();
    }

    /// Gives the backend its chance to autosave, and wakes the app up in
    /// time for the next one even when nothing else repaints.
    fn tick(&mut self, ctx: &egui::Context) {
        let now = Instant::now();
        if now < self.next_tick {
            ctx.request_repaint_after(self.next_tick - now);
            return;
        }
        let wait = match self.backend.tick() {
            Ok(next) => {
                if self.autosave_failing {
                    self.autosave_failing = false;
                    self.message(Level::Info, "Autosave works again");
                }
                next
            }
            Err(e) => {
                if !self.autosave_failing {
                    self.autosave_failing = true;
                    self.message(Level::Warning, format!("Autosave failed: {e}"));
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
        let dialog = rfd::FileDialog::new()
            .set_title("Save project")
            .add_filter("NovoElectric project", &[PROJECT_EXTENSION])
            .set_file_name(format!("{name}.{PROJECT_EXTENSION}"));
        match dialog.save_file() {
            Some(path) => self.save_to(&path),
            None => false,
        }
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
        self.handle_close_request(&ctx);
        self.update_title(&ctx);

        let mut picked: Vec<Action> = Vec::new();
        if !self.modal_open() {
            picked.extend(self.consume_shortcuts(&ctx));
        }

        self.window_width = ctx.content_rect().width();
        let pinned = self.outline_pinned() && !self.focus_mode;
        if pinned {
            self.outline_open = false;
        }

        egui::Panel::top("top_bar")
            .exact_size(TOP_BAR)
            .show(ui, |ui| self.top_bar(ui, &mut picked));
        if pinned && self.is_open() {
            egui::Panel::left("outline")
                .exact_size(OUTLINE_WIDTH)
                .resizable(false)
                .show(ui, |ui| self.outline(ui, &mut picked));
        }
        let canvas_rect = egui::CentralPanel::no_frame()
            .show(ui, |ui| {
                if self.is_open() {
                    let open = self
                        .current_page
                        .and_then(|page| OpenPage::new(&mut *self.backend, page));
                    self.canvas.show(ui, open);
                } else {
                    self.start_panel(ui, &mut picked);
                }
            })
            .response
            .rect;

        if !self.focus_mode {
            if !pinned && self.is_open() {
                self.outline_flyout(&ctx, canvas_rect, &mut picked);
            }
            self.hud(&ctx, canvas_rect);
        }
        self.toasts(&ctx, canvas_rect);

        if self.palette.is_open() {
            let entries = self.palette_entries();
            let recent: Vec<Target> = self
                .settings
                .recent_actions
                .iter()
                .filter_map(|id| Action::from_id(id))
                .map(Target::Action)
                .collect();
            match self.palette.show(&ctx, &entries, &recent) {
                Some(Target::Action(action)) => picked.push(action),
                None => {}
            }
        }
        self.recovery_dialog(&ctx);
        self.unsaved_dialog(&ctx);
        self.about_dialog(&ctx);

        for action in picked {
            self.run(&ctx, action);
        }
    }

    /// What the command palette searches: every enabled action for now;
    /// pages, devices and symbols join later as new `Target`s.
    fn palette_entries(&self) -> Vec<Entry> {
        Action::ALL
            .iter()
            .copied()
            .filter(|&a| a != Action::CommandPalette && self.is_enabled(a))
            .map(Entry::action)
            .collect()
    }

    fn modal_open(&self) -> bool {
        self.palette.is_open()
            || self.unsaved_prompt.is_some()
            || self.show_about
            || self.backend.recovery_offer().is_some()
    }

    /// Takes the key presses that trigger enabled actions, leaving the rest
    /// (and anything a focused text field needs) to the widgets.
    fn consume_shortcuts(&self, ctx: &egui::Context) -> Vec<Action> {
        let typing = ctx.egui_wants_keyboard_input();
        let mut fired = Vec::new();
        for (action, shortcut) in Action::by_shortcut_specificity() {
            if !self.is_enabled(action) || (typing && action.yields_to_text_input(&shortcut)) {
                continue;
            }
            let hit = ctx.input_mut(|i| {
                if shortcut.modifiers.is_none() {
                    consume_bare_key(i, shortcut.logical_key)
                } else {
                    i.consume_shortcut(&shortcut)
                }
            });
            if hit {
                fired.push(action);
            }
        }
        fired
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        let requested = ctx.input(|i| i.viewport().close_requested());
        if requested && !self.quitting && self.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.unsaved_prompt = Some(Discarding::Quit);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.project_name() {
            Some(name) => {
                let dirty = if self.is_dirty() { "*" } else { "" };
                format!("{name}{dirty} - {APP_NAME}")
            }
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

    /// Shows `page` on the canvas and puts it first under "Recent".
    fn show_page(&mut self, page: PageId) {
        self.current_page = Some(page);
        self.recent_pages.retain(|&p| p != page);
        self.recent_pages.insert(0, page);
        self.recent_pages.truncate(MAX_RECENT_PAGES);
    }

    /// Autosave state for the top bar.
    fn saved_state(&self) -> &'static str {
        if !self.is_open() {
            ""
        } else if self.project_path.is_none() {
            "Not saved yet"
        } else if self.is_dirty() {
            "Unsaved changes"
        } else {
            "Saved"
        }
    }

    /// Mark (project menu), Schematic | Cabinet, breadcrumb, then "Search
    /// or run" and the saved state on the right.
    fn top_bar(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        ui.horizontal_centered(|ui| {
            ui.menu_button(egui::RichText::new(APP_NAME).strong(), |ui| {
                self.project_menu(ui, picked)
            });
            ui.separator();
            for (action, label) in [
                (Action::ShowSchematic, "Schematic"),
                (Action::ShowCabinet, "Cabinet"),
            ] {
                let selected = action == Action::ShowSchematic;
                let response = ui
                    .add_enabled_ui(self.is_enabled(action), |ui| {
                        segment(ui, label, selected)
                    })
                    .inner;
                let response = if action == Action::ShowCabinet {
                    response.on_disabled_hover_text("The cabinet canvas comes later")
                } else {
                    response
                };
                if response.clicked() {
                    picked.push(action);
                }
            }
            ui.separator();
            self.breadcrumb(ui);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(self.saved_state());
                let shortcut = Action::CommandPalette
                    .shortcut()
                    .map(|s| ui.ctx().format_shortcut(&s))
                    .unwrap_or_default();
                let search = egui::Button::new("Search or run")
                    .shortcut_text(shortcut)
                    .min_size(egui::vec2(240.0, 0.0));
                if ui.add(search).clicked() {
                    picked.push(Action::CommandPalette);
                }
            });
        });
    }

    /// The short project menu behind the mark. Everything in it is also in
    /// the command palette.
    fn project_menu(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        for a in [Action::NewProject, Action::OpenProject] {
            self.action_button(ui, a, picked);
        }
        ui.add_enabled_ui(!self.settings.recent_projects.is_empty(), |ui| {
            ui.menu_button("Recent", |ui| {
                for path in self.settings.recent_projects.clone() {
                    if ui.button(path.display().to_string()).clicked() {
                        let ctx = ui.ctx().clone();
                        self.discard_then(&ctx, Discarding::OpenPath(path));
                    }
                }
            });
        });
        ui.separator();
        for a in [
            Action::SaveProject,
            Action::SaveProjectAs,
            Action::CloseProject,
        ] {
            self.action_button(ui, a, picked);
        }
        ui.separator();
        for a in [Action::About, Action::Quit] {
            self.action_button(ui, a, picked);
        }
    }

    /// project / page. A click on the page opens the outline.
    fn breadcrumb(&mut self, ui: &mut egui::Ui) {
        let Some(name) = self.project_name() else {
            ui.weak("No project open");
            return;
        };
        ui.label(name);
        let page = self
            .current_page
            .and_then(|id| self.pages().into_iter().find(|(p, _)| *p == id));
        if let Some((_, label)) = page {
            ui.weak("/");
            let link = ui.add(egui::Button::new(egui::RichText::new(label).strong()).frame(false));
            if link.clicked() && !self.outline_pinned() {
                self.outline_open ^= true;
            }
        }
    }

    /// The outline's content: mode tabs, then the page list. Picking a page
    /// closes the flyout.
    fn outline(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            let _ = ui.selectable_label(true, "Pages");
            for mode in ["Issues", "Changes"] {
                ui.add_enabled(false, egui::Button::selectable(false, mode))
                    .on_disabled_hover_text("Comes later");
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
        ui.add(
            egui::TextEdit::singleline(&mut self.page_filter)
                .hint_text("Filter pages")
                .desired_width(f32::INFINITY),
        );
        ui.add_space(4.0);

        let pages = self.pages();
        let filter = self.page_filter.to_lowercase();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if filter.is_empty() {
                    let recent: Vec<_> = self
                        .recent_pages
                        .iter()
                        .filter_map(|id| pages.iter().find(|(p, _)| p == id))
                        .collect();
                    if !recent.is_empty() {
                        section_label(ui, "Recent");
                        for (id, label) in recent {
                            if self.page_row(ui, *id, label) {
                                chosen = Some(*id);
                            }
                        }
                        ui.add_space(6.0);
                    }
                }
                section_label(ui, "All pages");
                let mut any = false;
                for (id, label) in &pages {
                    if filter.is_empty() || label.to_lowercase().contains(&filter) {
                        any = true;
                        if self.page_row(ui, *id, label) {
                            chosen = Some(*id);
                        }
                    }
                }
                if !any {
                    ui.weak("No page matches");
                }
            });
        if let Some(page) = chosen {
            self.show_page(page);
            self.outline_open = false;
        }
    }

    fn page_row(&self, ui: &mut egui::Ui, id: PageId, label: &str) -> bool {
        let selected = self.current_page == Some(id);
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            ui.add(egui::Button::selectable(selected, label).min_size(egui::vec2(0.0, 22.0)))
        })
        .inner
        .clicked()
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
                ui.add(egui::Button::selectable(self.outline_open, "Pages"))
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
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(OUTLINE_WIDTH);
                    ui.set_height(height);
                    self.outline(ui, picked);
                });
            });
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let elsewhere = area.response.clicked_elsewhere() && !toggle.clicked() && !toggle.hovered();
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
                for a in [Action::NewProject, Action::OpenProject] {
                    self.action_button(ui, a, picked);
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
                            self.discard_then(&ctx, Discarding::OpenPath(path));
                        }
                    }
                }
            });
        });
    }

    /// Cursor position and friends, faint, at the canvas's top right.
    fn hud(&self, ctx: &egui::Context, canvas: egui::Rect) {
        let Some(text) = self.canvas.status_text() else {
            return;
        };
        egui::Area::new(egui::Id::new("hud"))
            .pivot(egui::Align2::RIGHT_TOP)
            .fixed_pos(canvas.right_top() + egui::vec2(-12.0, 10.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.label(egui::RichText::new(text).monospace().weak());
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

    fn unsaved_dialog(&mut self, ctx: &egui::Context) {
        let Some(next) = self.unsaved_prompt.clone() else {
            return;
        };
        let name = self.project_name().unwrap_or_default();
        let mut answer = None;
        let modal = egui::Modal::new(egui::Id::new("unsaved_changes")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.heading("Unsaved changes");
            ui.label(format!("Save the changes to \"{name}\" first?"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if primary_button(ui, "Save").clicked() {
                    answer = Some(UnsavedAnswer::Save);
                }
                if ui.button("Don't save").clicked() {
                    answer = Some(UnsavedAnswer::Discard);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(UnsavedAnswer::Cancel);
                }
            });
        });
        if answer.is_none() && modal.should_close() {
            answer = Some(UnsavedAnswer::Cancel);
        }
        if let Some(answer) = answer {
            self.answer_unsaved(ctx, next, answer);
        }
    }

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
                "NovoElectric did not close cleanly last time. \"{name}\" has unsaved work{when}."
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
                self.canvas.document_replaced();
                self.fix_current_page();
                self.message(Level::Info, "Restored the unsaved work. Save to keep it.");
            }
            Ok(()) => {
                if !self.is_open() {
                    self.project_path = None;
                    self.current_page = None;
                    self.canvas.document_replaced();
                }
                self.message(Level::Info, "Discarded the unsaved work");
            }
            Err(e) => self.message(
                Level::Error,
                format!("Could not discard the unsaved work: {e}"),
            ),
        }
    }

    fn answer_unsaved(&mut self, ctx: &egui::Context, next: Discarding, answer: UnsavedAnswer) {
        self.unsaved_prompt = None;
        match answer {
            UnsavedAnswer::Save => {
                if self.save() {
                    self.finish_discarding(ctx, next);
                }
            }
            UnsavedAnswer::Discard => self.finish_discarding(ctx, next),
            UnsavedAnswer::Cancel => {}
        }
    }

    fn about_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let modal = egui::Modal::new(egui::Id::new("about")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading(APP_NAME);
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

#[derive(Clone, Copy, Debug)]
enum UnsavedAnswer {
    Save,
    Discard,
    Cancel,
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

/// Consumes a press of `key` with no modifiers held. egui's own matching is
/// loose, so a bare Home would also fire on Shift+Home.
fn consume_bare_key(input: &mut egui::InputState, key: egui::Key) -> bool {
    let before = input.events.len();
    input.events.retain(|event| {
        !matches!(
            event,
            egui::Event::Key { key: k, pressed: true, modifiers, .. }
                if *k == key && modifiers.is_none()
        )
    });
    input.events.len() != before
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
    fn ctrl_k_opens_the_palette_and_blocks_other_shortcuts() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, key(egui::Key::K, egui::Modifiers::COMMAND));
        assert!(app.palette.is_open());
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert!(!app.is_open());
    }

    #[test]
    fn save_without_a_project_is_ignored() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::SaveProject);
        assert!(app.messages().is_empty());
    }

    #[test]
    fn closing_a_dirty_project_asks_first() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.run(&ctx, Action::NewProject);
        app.run(&ctx, Action::CloseProject);
        assert_eq!(app.unsaved_prompt, Some(Discarding::Close));
        assert!(app.is_open());

        app.answer_unsaved(&ctx, Discarding::Close, UnsavedAnswer::Cancel);
        assert!(app.is_open());

        app.run(&ctx, Action::CloseProject);
        app.answer_unsaved(&ctx, Discarding::Close, UnsavedAnswer::Discard);
        assert!(!app.is_open());
        assert!(app.unsaved_prompt.is_none());
    }

    #[test]
    fn closing_a_clean_project_does_not_ask() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        app.run(&ctx, Action::CloseProject);
        assert!(app.unsaved_prompt.is_none());
        assert!(!app.is_open());
    }

    #[test]
    fn placeholder_open_reports_an_error() {
        let ctx = themed_ctx();
        let mut app = app_with(MemoryBackend::default());
        app.finish_discarding(&ctx, Discarding::OpenPath(PathBuf::from("x.novo")));
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
        assert_eq!(app.window_title, "Untitled - NovoElectric");
    }

    /// Which actions a single key press would trigger.
    fn fired_by(app: &NovoApp, key_: egui::Key, modifiers: egui::Modifiers) -> Vec<Action> {
        let ctx = themed_ctx();
        let mut fired = Vec::new();
        let mut output = ctx.run_ui(key(key_, modifiers), |ui| {
            fired = app.consume_shortcuts(ui.ctx());
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
            vec![Action::ZoomToFit]
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
            vec![Action::Redo]
        );
        assert_eq!(
            fired_by(
                &app,
                egui::Key::Z,
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
            ),
            vec![Action::Redo]
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
        assert_eq!(app.window_title, "Plant* - NovoElectric");
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
    fn cabinet_waits_for_its_canvas() {
        let app = app_with(MemoryBackend::default());
        assert!(!app.is_enabled(Action::ShowCabinet));
        assert!(app.is_enabled(Action::ShowSchematic));
    }

    #[test]
    fn visited_pages_lead_the_recent_list_and_vanish_with_their_page() {
        let ctx = themed_ctx();
        let mut app = app_with(DirtyBackend::default());
        app.run(&ctx, Action::NewProject);
        let (one, two) = (app.pages()[0].0, app.pages()[1].0);
        app.show_page(one);
        app.show_page(two);
        app.show_page(one);
        assert_eq!(app.recent_pages, vec![one, two]);
        app.run(&ctx, Action::Undo);
        assert_eq!(app.recent_pages, vec![one]);
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
        assert_eq!(app.saved_state(), "Unsaved changes");
    }

    #[test]
    fn start_up_reopens_the_last_project() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plant.novo");
        let ctx = egui::Context::default();
        let mut first = storage_app();
        first.run(&ctx, Action::NewProject);
        assert!(first.save_to(&path));
        first.run(&ctx, Action::CloseProject);

        let mut next = app_with(StorageBackend::default());
        next.settings = first.settings().clone();
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
        let ctx = egui::Context::default();
        let mut app = storage_app();
        let add = || Command::AddPage {
            name: "2".into(),
            title: String::new(),
            kind: Default::default(),
            format: Default::default(),
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
