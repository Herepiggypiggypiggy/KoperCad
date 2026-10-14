//! NovoElectric application shell (owned by WS-01).
//!
//! Owns the native window, menus, panels, keyboard shortcuts and the command
//! palette, and wires the other crates together through two hooks:
//!
//! - [`ProjectBackend`]: owns the open [`novo_core::Document`] and opens and
//!   saves it. The project file (`novo-storage`) plugs in here.
//! - [`CanvasView`]: draws and edits the open page through the same document,
//!   so canvas edits share the undo history. The canvas (`novo-canvas`) plugs
//!   in here.
//!
//! Until those land, [`MemoryBackend`] and [`PlaceholderCanvas`] keep the
//! shell usable on its own.

mod actions;
mod app;
mod backend;
mod canvas;
mod palette;
mod settings;

pub use actions::Action;
pub use app::NovoApp;
pub use backend::{BackendError, MemoryBackend, ProjectBackend, new_document, page_labels};
pub use canvas::{CanvasView, OpenPage, PlaceholderCanvas};

use eframe::egui;

/// Name used for the window title and the settings directory.
pub const APP_NAME: &str = "NovoElectric";

/// File extension of a NovoElectric project file, without the dot.
pub const PROJECT_EXTENSION: &str = "novo";

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../../../assets/icon/novoelectric.png"))
        .unwrap_or_default()
}

/// Opens the main window and runs until it is closed.
///
/// A project file passed as the first argument (as Windows does when a
/// `.novo` file is double-clicked) is opened on start.
pub fn run() -> eframe::Result {
    let startup_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);
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
        Box::new(|cc| {
            let mut app = NovoApp::new(
                cc,
                Box::new(MemoryBackend::default()),
                Box::new(PlaceholderCanvas::default()),
            );
            if let Some(path) = startup_file {
                app.open_on_start(path);
            }
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_decodes() {
        let icon = super::window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
