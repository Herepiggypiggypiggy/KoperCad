//! KoperCAD desktop application shell (owned by WS-01).

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    novo_app::apply_profile_flag();
    novo_app::diag::init();
    // A native crash in the graphics driver leaves no report, so the program
    // leaves a marker while it starts. If the last start died, ask here, before
    // the window opens, whether to use safe graphics this time.
    novo_app::startup::begin(|failure| {
        let offer = !failure.was_safe_graphics;
        let result = rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Warning)
            .set_title("KoperCAD did not start last time")
            .set_description(failure.message())
            .set_buttons(if offer {
                rfd::MessageButtons::YesNo
            } else {
                rfd::MessageButtons::Ok
            })
            .show();
        if offer && result == rfd::MessageDialogResult::Yes {
            novo_app::startup::Answer::SafeGraphics
        } else {
            novo_app::startup::Answer::Normal
        }
    });
    let result = novo_app::run();
    // The program came back by itself (closed early, or an error it reports
    // below): that is not a start that died.
    novo_app::startup::ended();
    if let Err(e) = &result {
        // Release builds on Windows have no console, so say it in a dialog.
        log::error!("KoperCAD could not start: {e}");
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("KoperCAD could not start")
            .set_description(format!(
                "{e}\n\nKoperCAD needs a graphics driver with Vulkan, DirectX 12 or OpenGL support. \
                 Update the graphics driver, or on a remote desktop or virtual machine start KoperCAD \
                 on the local screen. If it still fails, start KoperCAD (safe graphics) from the Start menu, or run \
                 kopercad.exe --safe-graphics: it uses the software renderer, which is slower \
                 but needs no graphics driver.{}",
                novo_app::diag::log_dir()
                    .map(|d| format!("\n\nThe log is in {}", d.display()))
                    .unwrap_or_default()
            ))
            .show();
    }
    result
}
