//! Help > Open log folder, Help > Copy problem report, and the card offered on
//! the start after a crash (REL-DIAG, see `crate::diag`). Local only: nothing
//! is sent anywhere.

use eframe::egui;

use super::{Level, NovoApp, parts_bulk, primary_button, userkeymap};

impl NovoApp {
    /// `help.open_log_folder`: the folder with the log and crash reports.
    pub(super) fn open_log_folder(&mut self) {
        let Some(dir) = crate::diag::log_dir() else {
            self.message(Level::Warning, "No log folder on this computer");
            return;
        };
        if cfg!(test) {
            self.message(Level::Info, format!("Log folder: {}", dir.display()));
            return;
        }
        match parts_bulk::open_folder(&dir) {
            Ok(()) => self.message(Level::Info, format!("Log folder: {}", dir.display())),
            Err(e) => self.message(
                Level::Warning,
                format!("Could not open the log folder: {e}"),
            ),
        }
    }

    /// `help.copy_problem_report`: version, build, system, graphics and the
    /// last log lines go to the clipboard, ready to paste into a message.
    pub(super) fn copy_problem_report(&mut self, ctx: &egui::Context) {
        ctx.copy_text(crate::diag::problem_report());
        self.message(
            Level::Info,
            "Problem report copied. Paste it into your message.",
        );
    }

    /// Asks whether to open the crash report the last run left behind.
    pub(super) fn crash_dialog(&mut self, ctx: &egui::Context) {
        // The recovery question comes first; two cards would stack.
        if self.backend.recovery_offer().is_some() {
            return;
        }
        let Some(report) = self.crash_report.clone() else {
            return;
        };
        let mut open_report = false;
        let mut open_folder = false;
        let mut copy = false;
        let mut close = false;
        egui::Modal::new(egui::Id::new("crash_report")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading("KoperCAD stopped unexpectedly");
            ui.label(
                "A report was written on this computer. Nothing was sent anywhere. \
                 Send it to the person who gave you this build.",
            );
            ui.weak(report.display().to_string());
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                open_report = primary_button(ui, "Open report").clicked();
                open_folder = ui.button("Open log folder").clicked();
                copy = ui.button("Copy problem report").clicked();
                close = ui.button("Dismiss").clicked();
            });
        });
        if open_report {
            match userkeymap::open_in_editor(&report) {
                Ok(()) => {}
                Err(e) => self.message(
                    Level::Warning,
                    format!("Open {} in a text editor ({e})", report.display()),
                ),
            }
            self.crash_report = None;
        }
        if open_folder {
            self.open_log_folder();
        }
        if copy {
            self.copy_problem_report(ctx);
        }
        if close {
            self.crash_report = None;
        }
    }

    /// Test hook: the report the next start would offer.
    #[cfg(test)]
    pub(super) fn offer_crash_report(&mut self, report: std::path::PathBuf) {
        self.crash_report = Some(report);
    }
}

#[cfg(test)]
mod tests {
    use super::super::gate1_tests::Drive;

    fn drive() -> (tempfile::TempDir, Drive) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        d.frame();
        (dir, d)
    }

    #[test]
    fn the_help_commands_are_run_by_the_app_and_sit_in_the_help_menu() {
        for id in ["help.open_log_folder", "help.copy_problem_report"] {
            assert!(super::super::workbench::app_runs(id), "{id}");
        }
    }

    #[test]
    fn copy_problem_report_puts_the_report_on_the_clipboard_and_says_so() {
        let (_dir, mut d) = drive();
        d.app.wb.queued.push("help.copy_problem_report");
        d.frame();
        assert!(
            d.app
                .messages()
                .last()
                .is_some_and(|m| m.text.contains("Problem report copied"))
        );
    }

    #[test]
    fn a_crash_report_from_the_last_run_blocks_the_window_until_answered() {
        let (dir, mut d) = drive();
        assert!(!d.app.modal_open());
        d.app.offer_crash_report(dir.path().join("crash-x.txt"));
        d.frame();
        assert!(d.app.modal_open());
        d.app.crash_report = None;
        d.frame();
        assert!(!d.app.modal_open());
    }
}
