//! NovoElectric desktop application shell (owned by WS-01).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let result = novo_app::run();
    if let Err(e) = &result {
        // Release builds on Windows have no console, so say it in a dialog.
        log::error!("NovoElectric could not start: {e}");
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("NovoElectric could not start")
            .set_description(format!(
                "{e}\n\nNovoElectric needs a graphics driver with Vulkan, DirectX 12 or OpenGL support."
            ))
            .show();
    }
    result
}
