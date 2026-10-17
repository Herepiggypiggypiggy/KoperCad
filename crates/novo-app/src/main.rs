//! KoperCAD desktop application shell (owned by WS-01).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    novo_app::diag::init();
    let result = novo_app::run();
    if let Err(e) = &result {
        // Release builds on Windows have no console, so say it in a dialog.
        log::error!("KoperCAD could not start: {e}");
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("KoperCAD could not start")
            .set_description(format!(
                "{e}\n\nKoperCAD needs a graphics driver with Vulkan, DirectX 12 or OpenGL support.{}",
                novo_app::diag::log_dir()
                    .map(|d| format!("\n\nThe log is in {}", d.display()))
                    .unwrap_or_default()
            ))
            .show();
    }
    result
}
