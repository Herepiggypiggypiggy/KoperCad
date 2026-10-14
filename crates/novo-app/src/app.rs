//! The main window: menus, panels, status bar, dialogs and action dispatch.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::actions::Action;
use novo_core::PageId;

use crate::backend::{ProjectBackend, page_labels};
use crate::canvas::{CanvasView, OpenPage};
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

    fn is_open(&self) -> bool {
        self.backend.document().is_some()
    }

    fn is_dirty(&self) -> bool {
        self.backend.document().is_some_and(|d| d.is_dirty())
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
            Ok(Some(changes)) => self.canvas.document_changed(doc, &changes),
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
            Ok(()) => {
                self.project_path = Some(path.to_owned());
                self.current_page = None;
                self.canvas.document_replaced();
                self.fix_current_page();
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
        self.canvas.document_replaced();
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
        self.fix_current_page();
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
            let page = self.current_page;
            let open = self
                .backend
                .document_mut()
                .zip(page)
                .map(|(document, page)| OpenPage { document, page });
            self.canvas.show(ui, open);
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
    use crate::backend::{BackendError, MemoryBackend, new_document};
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
        fn open(&mut self, _path: &Path) -> Result<(), BackendError> {
            self.document = Some(new_document("Opened")?);
            Ok(())
        }
        fn save(&mut self, _path: &Path) -> Result<(), BackendError> {
            if let Some(doc) = &mut self.document {
                doc.mark_saved();
            }
            Ok(())
        }
        fn close(&mut self) {
            self.document = None;
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
