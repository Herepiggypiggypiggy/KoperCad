//! The main window: menus, panels, status bar, dialogs and action dispatch.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::actions::Action;
use crate::backend::{PageId, ProjectBackend};
use crate::canvas::CanvasView;
use crate::palette::Palette;
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
}

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

    pub fn is_enabled(&self, action: Action) -> bool {
        let open = self.backend.is_open();
        match action {
            Action::SaveProject
            | Action::SaveProjectAs
            | Action::CloseProject
            | Action::ZoomToFit => open,
            Action::Undo => open && self.backend.can_undo(),
            Action::Redo => open && self.backend.can_redo(),
            _ => true,
        }
    }

    /// Runs an action as if the user had picked it.
    pub fn run(&mut self, ctx: &egui::Context, action: Action) {
        if !self.is_enabled(action) {
            return;
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
            Action::Undo => self.backend.undo(),
            Action::Redo => self.backend.redo(),
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
        if self.backend.is_open() && self.backend.is_dirty() {
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
                self.current_page = self.backend.pages().first().map(|p| p.id);
            }
            Err(e) => self.message(Level::Error, format!("Could not create a project: {e}")),
        }
    }

    fn open_path(&mut self, path: &Path) {
        match self.backend.open(path) {
            Ok(()) => {
                self.project_path = Some(path.to_owned());
                self.current_page = self.backend.pages().first().map(|p| p.id);
                self.settings.remember_recent(path);
                self.message(Level::Info, format!("Opened {}", path.display()));
            }
            Err(e) => {
                if matches!(e, crate::BackendError::Io(ref io) if io.kind() == std::io::ErrorKind::NotFound)
                {
                    self.settings.forget_recent(path);
                }
                self.message(
                    Level::Error,
                    format!("Could not open {}: {e}", path.display()),
                );
            }
        }
    }

    fn close_project(&mut self) {
        self.backend.close();
        self.project_path = None;
        self.current_page = None;
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
        let name = self
            .backend
            .project_name()
            .unwrap_or_else(|| "Untitled".into());
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
                self.message(
                    Level::Error,
                    format!("Could not save {}: {e}", path.display()),
                );
                false
            }
        }
    }

    // ------------------------------------------------------------------
    // Frame

    /// Lays out one frame of the whole window.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if let Some(path) = self.open_on_start.take() {
            self.open_path(&path);
        }
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
            let page = if self.backend.is_open() {
                self.current_page
            } else {
                None
            };
            self.canvas.show(ui, page);
        });

        let enabled: Vec<Action> = Action::ALL
            .iter()
            .copied()
            .filter(|&a| self.is_enabled(a))
            .collect();
        if let Some(action) = self.palette.show(&ctx, |a| enabled.contains(&a)) {
            picked.push(action);
        }
        self.unsaved_dialog(&ctx);
        self.about_dialog(&ctx);

        for action in picked {
            self.run(&ctx, action);
        }
    }

    fn modal_open(&self) -> bool {
        self.palette.is_open() || self.unsaved_prompt.is_some() || self.show_about
    }

    fn consume_shortcuts(&self, ctx: &egui::Context) -> Vec<Action> {
        let mut fired = Vec::new();
        for (action, shortcut) in Action::by_shortcut_specificity() {
            if ctx.input_mut(|i| i.consume_shortcut(&shortcut)) {
                fired.push(action);
            }
        }
        fired
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        let requested = ctx.input(|i| i.viewport().close_requested());
        if requested && !self.quitting && self.backend.is_open() && self.backend.is_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.unsaved_prompt = Some(Discarding::Quit);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let title = match self.backend.project_name() {
            Some(name) => {
                let dirty = if self.backend.is_dirty() { "*" } else { "" };
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
                self.action_button(ui, Action::Undo, picked);
                self.action_button(ui, Action::Redo, picked);
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
        let mut button = egui::Button::new(action.label());
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
        let Some(name) = self.backend.project_name() else {
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
            for page in self.backend.pages() {
                let selected = self.current_page == Some(page.id);
                if ui.selectable_label(selected, &page.title).clicked() {
                    self.current_page = Some(page.id);
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
        let name = self.backend.project_name().unwrap_or_default();
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
    use crate::backend::{BackendError, PageInfo, PlaceholderBackend};
    use crate::canvas::PlaceholderCanvas;

    /// A backend whose project always has unsaved changes.
    #[derive(Default)]
    struct DirtyBackend {
        open: bool,
    }

    impl ProjectBackend for DirtyBackend {
        fn new_project(&mut self, _name: &str) -> Result<(), BackendError> {
            self.open = true;
            Ok(())
        }
        fn open(&mut self, _path: &Path) -> Result<(), BackendError> {
            self.open = true;
            Ok(())
        }
        fn save(&mut self, _path: &Path) -> Result<(), BackendError> {
            Ok(())
        }
        fn close(&mut self) {
            self.open = false;
        }
        fn is_open(&self) -> bool {
            self.open
        }
        fn project_name(&self) -> Option<String> {
            self.open.then(|| "Test".to_owned())
        }
        fn is_dirty(&self) -> bool {
            self.open
        }
        fn pages(&self) -> Vec<PageInfo> {
            Vec::new()
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
        let mut app = app_with(PlaceholderBackend::default());
        frame(&ctx, &mut app, Default::default());
        app.run(&ctx, Action::NewProject);
        frame(&ctx, &mut app, Default::default());
        assert_eq!(app.current_page, Some(PageId(1)));
    }

    #[test]
    fn ctrl_n_creates_a_project() {
        let ctx = egui::Context::default();
        let mut app = app_with(PlaceholderBackend::default());
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert!(app.backend.is_open());
    }

    #[test]
    fn ctrl_k_opens_the_palette_and_blocks_other_shortcuts() {
        let ctx = egui::Context::default();
        let mut app = app_with(PlaceholderBackend::default());
        frame(&ctx, &mut app, key(egui::Key::K, egui::Modifiers::COMMAND));
        assert!(app.palette.is_open());
        frame(&ctx, &mut app, key(egui::Key::N, egui::Modifiers::COMMAND));
        assert!(!app.backend.is_open());
    }

    #[test]
    fn save_without_a_project_is_ignored() {
        let ctx = egui::Context::default();
        let mut app = app_with(PlaceholderBackend::default());
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
        assert!(app.backend.is_open());

        app.answer_unsaved(&ctx, Discarding::Close, UnsavedAnswer::Cancel);
        assert!(app.backend.is_open());

        app.run(&ctx, Action::CloseProject);
        app.answer_unsaved(&ctx, Discarding::Close, UnsavedAnswer::Discard);
        assert!(!app.backend.is_open());
        assert!(app.unsaved_prompt.is_none());
    }

    #[test]
    fn closing_a_clean_project_does_not_ask() {
        let ctx = egui::Context::default();
        let mut app = app_with(PlaceholderBackend::default());
        app.run(&ctx, Action::NewProject);
        app.run(&ctx, Action::CloseProject);
        assert!(app.unsaved_prompt.is_none());
        assert!(!app.backend.is_open());
    }

    #[test]
    fn placeholder_open_reports_an_error() {
        let ctx = egui::Context::default();
        let mut app = app_with(PlaceholderBackend::default());
        app.finish_discarding(&ctx, Discarding::OpenPath(PathBuf::from("x.novo")));
        assert_eq!(app.messages().last().map(|m| m.level), Some(Level::Error));
        assert!(!app.backend.is_open());
    }

    #[test]
    fn startup_file_is_opened_on_first_frame() {
        let ctx = egui::Context::default();
        let mut app = app_with(DirtyBackend::default());
        app.open_on_start(PathBuf::from("plant.novo"));
        frame(&ctx, &mut app, Default::default());
        assert!(app.backend.is_open());
        assert_eq!(app.project_path, Some(PathBuf::from("plant.novo")));
        assert_eq!(
            app.settings().recent_projects,
            vec![PathBuf::from("plant.novo")]
        );
    }

    #[test]
    fn panel_toggles_flip_settings() {
        let ctx = egui::Context::default();
        let mut app = app_with(PlaceholderBackend::default());
        let before = app.settings().show_messages_panel;
        app.run(&ctx, Action::ToggleMessagesPanel);
        assert_ne!(app.settings().show_messages_panel, before);
        frame(&ctx, &mut app, Default::default());
    }
}
