//! NovoElectric application shell (owned by WS-01).
//!
//! Owns the native window, menus, panels, keyboard shortcuts and the command
//! palette, and wires the other crates together through two hooks:
//!
//! - [`ProjectBackend`]: owns the open [`novo_core::Document`] and opens and
//!   saves it. The project file (`novo-storage`) plugs in here.
//! - [`CanvasView`]: draws and edits the open page through the same document,
//!   so canvas edits share the undo history. The schematic editor
//!   (`novo-editor` on `novo-canvas`) plugs in here as [`EditorCanvas`].
//!
//! [`StorageBackend`] keeps projects in `.novo` files with autosave and crash
//! recovery. [`MemoryBackend`] and [`PlaceholderCanvas`] stand in for tests
//! and until the canvas is wired.

mod actions;
mod app;
mod backend;
mod canvas;
mod editor_canvas;
mod model_canvas;
mod palette;
mod settings;
mod theme;

pub use actions::{Action, RESERVED_SHORTCUTS};
pub use app::NovoApp;
pub use backend::{
    BackendError, MemoryBackend, OpenReport, ProjectBackend, RecoveryOffer, StorageBackend,
    new_document, page_labels,
};
pub use canvas::{CanvasView, OpenPage, PlaceholderCanvas};
pub use editor_canvas::EditorCanvas;
pub use model_canvas::ModelCanvas;

use eframe::egui;

/// Name used for the window title and the settings directory.
pub const APP_NAME: &str = "NovoElectric";

/// File extension of a NovoElectric project file, without the dot.
pub const PROJECT_EXTENSION: &str = novo_storage::PROJECT_EXTENSION;

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(novo_theme::brand::ICON_PNG_256).unwrap_or_default()
}

/// Opens the main window and runs until it is closed.
///
/// A project file passed as the first argument (as Windows does when a
/// `.novo` file is double-clicked) is opened on start.
pub fn run() -> eframe::Result {
    let startup_file = std::env::args_os().nth(1).map(|arg| {
        let path = std::path::PathBuf::from(arg);
        // absolute() rather than canonicalize(): no \\?\ prefix on Windows.
        std::path::absolute(&path).unwrap_or(path)
    });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("novoelectric")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([640.0, 400.0])
            .with_icon(window_icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |cc| {
            let canvas: Box<dyn CanvasView> = match cc.wgpu_render_state.as_ref() {
                Some(rs) => {
                    EditorCanvas::install(rs);
                    Box::new(EditorCanvas::default())
                }
                None => Box::new(PlaceholderCanvas::default()),
            };
            let backend = match data_dir() {
                Some(dir) => StorageBackend::with_untitled_dir(dir.join("untitled")),
                None => StorageBackend::default(),
            };
            let mut app = NovoApp::new(cc, Box::new(backend), canvas);
            if let Some(path) = startup_file {
                app.open_on_start(path);
            }
            Ok(Box::new(app))
        }),
    )
}

/// The per-user folder for data the app keeps for itself, such as unsaved
/// new projects: `%LOCALAPPDATA%\NovoElectric` on Windows. Never a
/// system-wide location, so no admin rights are needed.
fn data_dir() -> Option<std::path::PathBuf> {
    use std::path::PathBuf;
    let env = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        env("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local/share")))?
    };
    Some(base.join(APP_NAME))
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_decodes() {
        let icon = super::window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
