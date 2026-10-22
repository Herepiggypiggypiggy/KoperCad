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
//! [`StorageBackend`] keeps projects in `.kopercad` files (older `.novo` files still open) with autosave and crash
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

/// Every extension the Open dialog lists: `.kopercad` and the older `.novo`.
pub const PROJECT_EXTENSIONS: [&str; 2] = novo_storage::PROJECT_EXTENSIONS;

/// Smallest window, in points; narrower and the top bar no longer fits. A
/// 1366 x 768 laptop at 200% scaling offers
/// 683 x 384 points, so the floor has to sit below that or the bottom of the
/// window (status bar) is pushed off the screen.
/// The floor assumes a screen of at least 683 points; on a smaller one
/// (1280 x 720 at 200%) the window is wider than the screen, a trade for
/// keeping the top bar whole.
pub(crate) const MIN_WINDOW: [f32; 2] = [680.0, 320.0];

fn window_icon() -> egui::IconData {
    eframe::icon_data::from_png_bytes(novo_theme::brand::ICON_PNG_256).unwrap_or_default()
}

/// Reads the command line (without the program name): the `--profile-dir
/// <folder>` (or `--profile-dir=<folder>`) flag and the project file to open.
/// The control-mode flags are skipped here; `control` reads them.
fn command_line(
    args: impl Iterator<Item = std::ffi::OsString>,
) -> (Option<std::path::PathBuf>, Option<std::path::PathBuf>) {
    let mut profile = None;
    let mut file = None;
    let mut args = args;
    while let Some(arg) = args.next() {
        if arg == "--profile-dir" {
            profile = args.next().map(std::path::PathBuf::from);
        } else if let Some(dir) = arg.to_str().and_then(|a| a.strip_prefix("--profile-dir=")) {
            profile = Some(std::path::PathBuf::from(dir));
        } else if !control::is_control_arg(&arg) && file.is_none() {
            file = Some(std::path::PathBuf::from(arg));
        }
    }
    (profile.filter(|p| !p.as_os_str().is_empty()), file)
}

/// Applies the `--profile-dir` flag. `main` calls it before [`diag::init`], so the
/// log and crash files go to the profile folder too; [`run`] repeats it (the
/// first call wins).
pub fn apply_profile_flag() {
    if let (Some(dir), _) = command_line(std::env::args_os().skip(1)) {
        novo_storage::profile::set_root(dir);
    }
}

/// Opens the main window and runs until it is closed.
///
/// A project file passed as the first argument (as Windows does when a
/// `.novo` file is double-clicked) is opened on start.
pub fn run() -> eframe::Result {
    let (profile, startup_file) = command_line(std::env::args_os().skip(1));
    if let Some(dir) = profile {
        novo_storage::profile::set_root(dir);
    }
    let startup_file = startup_file.map(|path| {
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
        // eframe keeps the window settings (`app.ron`) under the user's data
        // folder; with a profile folder they go there too.
        persistence_path: novo_storage::profile::root()
            .map(|r| r.join("roaming").join(DATA_DIR_NAME).join("app.ron")),
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
            diag::startup_done();
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
    let base = if let Some(root) = novo_storage::profile::root() {
        root.join("local")
    } else if cfg!(windows) {
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
    let base = if let Some(root) = novo_storage::profile::root() {
        root.join("local")
    } else if cfg!(windows) {
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

#[cfg(test)]
mod command_line_tests {
    use super::command_line;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn parse(args: &[&str]) -> (Option<PathBuf>, Option<PathBuf>) {
        command_line(args.iter().map(OsString::from))
    }

    #[test]
    fn the_profile_flag_takes_its_value_and_is_not_a_project_file() {
        assert_eq!(
            parse(&["--profile-dir", "C:\\clean", "plant.novo"]),
            (Some("C:\\clean".into()), Some("plant.novo".into()))
        );
        assert_eq!(
            parse(&["--profile-dir=clean", "--control"]),
            (Some("clean".into()), None)
        );
        assert_eq!(parse(&["plant.novo"]), (None, Some("plant.novo".into())));
        assert_eq!(parse(&["--profile-dir="]), (None, None));
    }
}
