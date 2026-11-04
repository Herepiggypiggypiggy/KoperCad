//! "Check model" in the Parts window (lane STEP-HEALTH).
//!
//! Runs the model health report (`novo_interop::part3d::health`) on a
//! part's stored STEP file in the background and shows the answer: does it
//! read, what the reader leaves out, and the names and colours the file
//! carries. Nothing is changed or sent anywhere; "Copy report" puts the
//! text on the clipboard for a problem report.
//!
//! Where it is: Parts window > select a part with a STEP model > 3D model >
//! Check model.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;
use novo_interop::part3d::health::{Severity, StepHealth, check_step};
use novo_parts::PartKey;

enum Entry {
    Running(Receiver<Result<StepHealth, String>>),
    Done(Box<Result<StepHealth, String>>),
}

/// The report being shown, for one part.
#[derive(Default)]
pub(crate) struct HealthState {
    current: Option<(PartKey, Entry)>,
}

impl HealthState {
    /// Reads the STEP file at `step` for `key` in the background.
    pub(crate) fn start(&mut self, ctx: &egui::Context, key: PartKey, step: PathBuf) {
        let (tx, rx) = channel();
        let wake = ctx.clone();
        std::thread::spawn(move || {
            let result = std::fs::read(&step)
                .map_err(|e| format!("The STEP file cannot be read: {e}"))
                .map(|bytes| check_step(&bytes, &AtomicBool::new(false)));
            let _ = tx.send(result);
            wake.request_repaint();
        });
        self.current = Some((key, Entry::Running(rx)));
    }

    fn poll(&mut self) {
        if let Some((_, entry @ Entry::Running(_))) = self.current.as_mut() {
            let Entry::Running(rx) = entry else { return };
            if let Ok(done) = rx.try_recv() {
                *entry = Entry::Done(Box::new(done));
            }
        }
    }

    /// The finished report for `key`, if one is showing.
    #[cfg(test)]
    pub(crate) fn report(&self, key: &PartKey) -> Option<&StepHealth> {
        match self.current.as_ref()? {
            (k, Entry::Done(r)) if k == key => r.as_ref().as_ref().ok(),
            _ => None,
        }
    }
}

fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Info => "Note",
        Severity::Warning => "Warning",
        Severity::Problem => "Problem",
    }
}

/// The block under "3D model". Returns true when "Check model" was clicked.
pub(crate) fn block(
    ui: &mut egui::Ui,
    state: &mut HealthState,
    key: &PartKey,
    has_step: bool,
) -> bool {
    state.poll();
    let mut clicked = false;
    let running = matches!(&state.current, Some((k, Entry::Running(_))) if k == key);
    ui.add_enabled_ui(has_step && !running, |ui| {
        clicked = ui
            .small_button("Check model")
            .on_hover_text("Reads the STEP file and lists what KoperCAD cannot draw exactly, plus its names and colours")
            .clicked();
    });
    if running {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Checking the model...");
        });
    }
    match &state.current {
        Some((k, Entry::Done(r))) if k == key => match r.as_ref() {
            Err(why) => {
                ui.colored_label(ui.visuals().warn_fg_color, why);
            }
            Ok(h) => show(ui, h),
        },
        _ => {}
    }
    clicked
}

fn show(ui: &mut egui::Ui, h: &StepHealth) {
    ui.strong(h.headline());
    for f in &h.findings {
        let text = format!("{}: {}", severity_word(f.severity), f.text);
        match f.severity {
            Severity::Info => ui.weak(text),
            _ => ui.colored_label(ui.visuals().warn_fg_color, text),
        };
    }
    egui::CollapsingHeader::new("Details")
        .id_salt("parts_health_details")
        .show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(h.to_text()).monospace()).wrap());
            for s in &h.appearance.solids {
                ui.horizontal(|ui| {
                    if let Some(c) = s.rgb {
                        let (r, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        ui.painter()
                            .rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
                    }
                    ui.label(&s.name);
                });
            }
        });
    if ui.small_button("Copy report").clicked() {
        ui.ctx().copy_text(h.to_text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_ends_in_a_message_and_a_bad_file_in_a_problem() {
        let dir = std::env::temp_dir().join(format!("kc-health-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.step");
        std::fs::write(&bad, b"not a step file").unwrap();
        let key = PartKey::new("Acme", "1");
        let mut st = HealthState::default();
        st.start(&egui::Context::default(), key.clone(), bad);
        for _ in 0..200 {
            st.poll();
            if st.report(&key).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let h = st.report(&key).expect("report");
        assert_eq!(h.worst(), Severity::Problem);
        st.start(
            &egui::Context::default(),
            key.clone(),
            dir.join("missing.step"),
        );
        for _ in 0..200 {
            st.poll();
            if matches!(&st.current, Some((_, Entry::Done(_)))) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(st.report(&key).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
