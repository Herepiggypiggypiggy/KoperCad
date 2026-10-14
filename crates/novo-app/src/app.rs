//! The main window: menus, panels, status bar, dialogs and action dispatch.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;

use crate::actions::Action;
use novo_core::PageId;

use crate::backend::{BackendError, ProjectBackend, page_labels};
use crate::canvas::{CanvasView, OpenPage};
use crate::palette::{Entry, Palette, Target};
use crate::settings::{Settings, Theme};
use crate::{APP_NAME, PROJECT_EXTENSION};

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
}

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
        let app = Self::with_settings(backend, canvas, settings);
        cc.egui_ctx.set_theme(app.settings.theme.preference());
        app
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
            Action::ToggleProjectPanel => {
                self.settings.show_project_panel ^= true;
            }
            Action::TogglePropertiesPanel => {
                self.settings.show_properties_panel ^= true;
            }
            Action::ToggleMessagesPanel => {
                self.settings.show_messages_panel ^= true;
            }
            Action::CommandPalette => self.palette.open(),
            Action::ThemeSystem => self.set_theme(ctx, Theme::System),
            Action::ThemeLight => self.set_theme(ctx, Theme::Light),
            Action::ThemeDark => self.set_theme(ctx, Theme::Dark),
            Action::About => self.show_about = true,
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, theme: Theme) {
        self.settings.theme = theme;
        ctx.set_theme(theme.preference());
    }

    fn message(&mut self, level: Level, text: impl Into<String>) {
        let text = text.into();
        match level {
            Level::Info => log::info!("{text}"),
            Level::Warning => log::warn!("{text}"),
            Level::Error => log::error!("{text}"),
        }
        self.messages.push(Message { level, text });
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
    fn restore_untitled(&mut self) {
        match self.backend.restore_untitled() {
            Ok(true) => {
                self.project_path = None;
                self.current_page = None;
                self.canvas.document_replaced();
                self.fix_current_page();
            }
            Ok(false) => {}
            Err(e) => self.message(
                Level::Warning,
                format!("Could not check for unsaved work from the last session: {e}"),
            ),
        }
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
            match self.open_on_start.take() {
                Some(path) => self.open_path(&path),
                None => self.restore_untitled(),
            }
        }
        self.fix_current_page();
        self.tick(&ctx);
        self.handle_close_request(&ctx);
        self.update_title(&ctx);

        let mut picked: Vec<Action> = Vec::new();
        if !self.modal_open() {
            picked.extend(self.consume_shortcuts(&ctx));
        }

        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui, &mut picked));
        egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui));
        if self.settings.show_messages_panel {
            egui::Panel::bottom("messages")
                .resizable(true)
                .default_size(140.0)
                .show(ui, |ui| self.messages_panel(ui));
        }
        if self.settings.show_project_panel {
            egui::Panel::left("project")
                .resizable(true)
                .default_size(220.0)
                .show(ui, |ui| self.project_panel(ui));
        }
        if self.settings.show_properties_panel {
            egui::Panel::right("properties")
                .resizable(true)
                .default_size(260.0)
                .show(ui, properties_panel);
        }
        egui::CentralPanel::no_frame().show(ui, |ui| {
            let open = self
                .current_page
                .and_then(|page| OpenPage::new(&mut *self.backend, page));
            self.canvas.show(ui, open);
        });

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
    // Panels

    fn menu_bar(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                for a in [Action::NewProject, Action::OpenProject] {
                    self.action_button(ui, a, picked);
                }
                ui.add_enabled_ui(!self.settings.recent_projects.is_empty(), |ui| {
                    ui.menu_button("Open recent", |ui| {
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
                self.action_button(ui, Action::Quit, picked);
            });
            ui.menu_button("Edit", |ui| {
                let doc = self.backend.document();
                let undo = doc
                    .and_then(|d| d.undo_label())
                    .map(|l| format!("Undo {l}"));
                let redo = doc
                    .and_then(|d| d.redo_label())
                    .map(|l| format!("Redo {l}"));
                self.labeled_button(ui, Action::Undo, undo, picked);
                self.labeled_button(ui, Action::Redo, redo, picked);
            });
            ui.menu_button("View", |ui| {
                self.action_button(ui, Action::CommandPalette, picked);
                self.action_button(ui, Action::ZoomToFit, picked);
                ui.separator();
                for (a, on) in [
                    (Action::ToggleProjectPanel, self.settings.show_project_panel),
                    (
                        Action::TogglePropertiesPanel,
                        self.settings.show_properties_panel,
                    ),
                    (
                        Action::ToggleMessagesPanel,
                        self.settings.show_messages_panel,
                    ),
                ] {
                    self.toggle_button(ui, a, on, picked);
                }
                ui.separator();
                for (a, theme) in [
                    (Action::ThemeSystem, Theme::System),
                    (Action::ThemeLight, Theme::Light),
                    (Action::ThemeDark, Theme::Dark),
                ] {
                    self.toggle_button(ui, a, self.settings.theme == theme, picked);
                }
            });
            ui.menu_button("Help", |ui| {
                self.action_button(ui, Action::About, picked);
            });
        });
    }

    fn action_button(&self, ui: &mut egui::Ui, action: Action, picked: &mut Vec<Action>) {
        self.labeled_button(ui, action, None, picked);
    }

    fn labeled_button(
        &self,
        ui: &mut egui::Ui,
        action: Action,
        label: Option<String>,
        picked: &mut Vec<Action>,
    ) {
        let label = label.unwrap_or_else(|| action.label().to_owned());
        let mut button = egui::Button::new(label);
        if let Some(s) = action.shortcut() {
            button = button.shortcut_text(ui.ctx().format_shortcut(&s));
        }
        if ui.add_enabled(self.is_enabled(action), button).clicked() {
            picked.push(action);
        }
    }

    fn toggle_button(&self, ui: &mut egui::Ui, action: Action, on: bool, picked: &mut Vec<Action>) {
        let mut button = egui::Button::selectable(on, action.label());
        if let Some(s) = action.shortcut() {
            button = button.shortcut_text(ui.ctx().format_shortcut(&s));
        }
        if ui.add(button).clicked() {
            picked.push(action);
        }
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match self.messages.last() {
                Some(m) if m.level == Level::Error => {
                    ui.colored_label(ui.visuals().error_fg_color, &m.text);
                }
                Some(m) if m.level == Level::Warning => {
                    ui.colored_label(ui.visuals().warn_fg_color, &m.text);
                }
                Some(m) => {
                    ui.label(&m.text);
                }
                None => {
                    ui.label("Ready");
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(text) = self.canvas.status_text() {
                    ui.monospace(text);
                }
            });
        });
    }

    fn project_panel(&mut self, ui: &mut egui::Ui) {
        ui.set_min_width(ui.available_width());
        ui.strong("Project");
        ui.separator();
        let Some(name) = self.project_name() else {
            ui.weak("No project open");
            return;
        };
        ui.heading(name);
        if let Some(path) = &self.project_path {
            ui.weak(path.display().to_string());
        } else {
            ui.weak("Not saved yet");
        }
        ui.separator();
        ui.strong("Pages");
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, label) in self.pages() {
                let selected = self.current_page == Some(id);
                if ui.selectable_label(selected, label).clicked() {
                    self.current_page = Some(id);
                }
            }
        });
    }

    fn messages_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Messages");
            if ui.small_button("Clear").clicked() {
                self.messages.clear();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .auto_shrink(false)
            .show(ui, |ui| {
                for m in &self.messages {
                    let color = match m.level {
                        Level::Info => ui.visuals().text_color(),
                        Level::Warning => ui.visuals().warn_fg_color,
                        Level::Error => ui.visuals().error_fg_color,
                    };
                    ui.colored_label(color, &m.text);
                }
            });
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
                if ui.button("Save").clicked() {
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
                if ui.button("Restore").clicked() {
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

fn properties_panel(ui: &mut egui::Ui) {
    ui.set_min_width(ui.available_width());
    ui.strong("Properties");
    ui.separator();
    ui.weak("Nothing selected");
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
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, Default::default());
        app.run(&ctx, Action::NewProject);
        frame(&ctx, &mut app, Default::default());
        assert!(app.current_page.is_some());
    }

    #[test]
    fn ctrl_n_creates_a_project() {
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert!(app.is_open());
    }

    #[test]
    fn ctrl_k_opens_the_palette_and_blocks_other_shortcuts() {
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        frame(&ctx, &mut app, key(egui::Key::K, egui::Modifiers::COMMAND));
        assert!(app.palette.is_open());
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert!(!app.is_open());
    }

    #[test]
    fn save_without_a_project_is_ignored() {
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::SaveProject);
        assert!(app.messages().is_empty());
    }

    #[test]
    fn closing_a_dirty_project_asks_first() {
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        app.run(&ctx, Action::NewProject);
        app.run(&ctx, Action::CloseProject);
        assert!(app.unsaved_prompt.is_none());
        assert!(!app.is_open());
    }

    #[test]
    fn placeholder_open_reports_an_error() {
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        app.finish_discarding(&ctx, Discarding::OpenPath(PathBuf::from("x.novo")));
        assert_eq!(app.messages().last().map(|m| m.level), Some(Level::Error));
        assert!(!app.is_open());
    }

    #[test]
    fn startup_file_is_opened_on_first_frame() {
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();
        let mut fired = Vec::new();
        let mut output = ctx.run_ui(key(key_, modifiers), |ui| {
            fired = app.consume_shortcuts(ui.ctx());
        });
        output.textures_delta.clear();
        fired
    }

    #[test]
    fn bare_shortcuts_ignore_extra_modifiers() {
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();

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
        let ctx = egui::Context::default();

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
        let ctx = egui::Context::default();
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
        let ctx = egui::Context::default();
        let mut first = storage_app();
        first.run(&ctx, Action::NewProject);
        assert!(first.save_to(&path));

        let mut second = storage_app();
        second.open_path(&path);
        assert!(!second.is_open());
        let text = &second.messages().last().unwrap().text;
        assert!(text.contains("already open"), "{text}");
    }

    #[test]
    fn panel_toggles_flip_settings() {
        let ctx = egui::Context::default();
        let mut app = app_with(MemoryBackend::default());
        let before = app.settings().show_messages_panel;
        app.run(&ctx, Action::ToggleMessagesPanel);
        assert_ne!(app.settings().show_messages_panel, before);
        frame(&ctx, &mut app, Default::default());
    }
}
