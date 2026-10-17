//! KoperCAD application shell (owned by WS-01).
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
mod command_field;
mod control;
pub mod diag;
mod dialogs;
mod editor_canvas;
mod gpu;
mod keys;
mod migrate;
mod model_canvas;
mod settings;
mod theme;

pub use actions::{Action, RESERVED_SHORTCUTS};
pub use app::NovoApp;
pub use backend::{
    BackendError, MemoryBackend, OpenReport, PageRow, ProjectBackend, RecoveryOffer,
    StorageBackend, new_document, page_labels, page_rows,
};
pub use canvas::{CanvasView, OpenPage, PlaceholderCanvas};
pub use editor_canvas::EditorCanvas;
pub use model_canvas::ModelCanvas;

use eframe::egui;

/// Name shown in the window title, menus and dialogs.
pub const APP_NAME: &str = "KoperCAD";

/// The maker, shown as "by KoperVonk" in the About box.
pub const APP_MAKER: &str = "KoperVonk";

/// Name of the per-user folders the app keeps data in: settings and the parts
/// catalog under `%APPDATA%`, new projects under `Documents`.
pub const DATA_DIR_NAME: &str = "KoperCAD";

/// The folder name from before the rename. Old data is moved from it (see the
/// `migrate` module); `%LOCALAPPDATA%\NovoElectric` also keeps this name, see
/// [`data_dir`].
pub const LEGACY_DATA_DIR_NAME: &str = "NovoElectric";

/// File extension of a KoperCAD project file, without the dot.
pub const PROJECT_EXTENSION: &str = novo_storage::PROJECT_EXTENSION;

/// Smallest window, in points; narrower and the top bar no longer fits. A
/// 1366 x 768 laptop at 200% scaling offers
/// 683 x 384 points, so the floor has to sit below that or the bottom of the
/// window (status bar) is pushed off the screen.
pub(crate) const MIN_WINDOW: [f32; 2] = [680.0, 320.0];

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(novo_theme::brand::ICON_PNG_256).unwrap_or_default()
}

/// Opens the main window and runs until it is closed.
///
/// A project file passed as the first argument (as Windows does when a
/// `.novo` file is double-clicked) is opened on start.
pub fn run() -> eframe::Result {
    let startup_file = std::env::args_os()
        .skip(1)
        .find(|a| !control::is_control_arg(a))
        .map(|arg| {
            let path = std::path::PathBuf::from(arg);
            // absolute() rather than canonicalize(): no \\?\ prefix on Windows.
            std::path::absolute(&path).unwrap_or(path)
        });
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id("kopercad")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size(MIN_WINDOW)
            .with_icon(window_icon()),
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: gpu::configuration(),
        ..Default::default()
    };
    // The first argument names eframe's settings folder; move the old one first.
    migrate::migrate_roaming_data();
    eframe::run_native(
        DATA_DIR_NAME,
        options,
        Box::new(move |cc| {
            if let Some(port) = control::requested_port() {
                control::install(&cc.egui_ctx, port);
            }
            let canvas: Box<dyn CanvasView> = match cc.wgpu_render_state.as_ref() {
                Some(rs) => {
                    let info = rs.adapter.get_info();
                    diag::set_gpu(format!(
                        "{} ({:?}, driver {} {})",
                        info.name, info.backend, info.driver, info.driver_info
                    ));
                    EditorCanvas::install(rs);
                    novo_view3d::widget::install(rs, 1);
                    Box::new(EditorCanvas::default())
                }
                None => {
                    diag::set_gpu("none (no graphics adapter)".to_owned());
                    Box::new(PlaceholderCanvas::default())
                }
            };
            let mut backend = match data_dir() {
                Some(dir) => StorageBackend::with_untitled_dir(dir.join("untitled")),
                None => StorageBackend::default(),
            };
            if let Some(dir) = recovery_dir() {
                backend = backend.with_recovery_dir(dir);
            }
            let mut app = NovoApp::new(cc, Box::new(backend), canvas);
            if let Some(path) = startup_file {
                app.open_on_start(path);
            }
            Ok(Box::new(app))
        }),
    )
}

/// The per-user folder for data the app keeps for itself, such as unsaved
/// new projects: `%LOCALAPPDATA%\NovoElectric` on Windows. It keeps the old name because
/// `scripts\run-latest.ps1` and `scripts\build-local.ps1` keep the experimental builds
/// there. Never a system-wide location, so no admin rights are needed.
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
    Some(base.join(LEGACY_DATA_DIR_NAME))
}

/// The per-user folder for the recovery files of saved projects:
/// `%LOCALAPPDATA%\KoperCAD\recovery` on Windows. Per user, never a system
/// location. (Before this folder existed recovery files sat beside the
/// project; those are still found.)
fn recovery_dir() -> Option<std::path::PathBuf> {
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
    Some(base.join(DATA_DIR_NAME).join("recovery"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn window_icon_decodes() {
        let icon = super::window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
    }
}
