//! "Check model" in the Parts window.
//!
//! Runs the STEP model health report (`novo_interop::part3d::health`) on a
//! part's stored STEP file in the background and shows the answer: whether it
//! reads, what the reader approximated or left out, and the name and colour
//! the file carries. Nothing is changed or sent anywhere; "Copy report" puts
//! the text on the clipboard for a problem report.
//!
//! Where it is: Parts window > select a part with a STEP model > 3D model >
//! Check model.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use eframe::egui;
use novo_interop::part3d::health::{StepHealth, Verdict, health_of_file};
use novo_parts::PartKey;

enum Entry {
    Running(Receiver<StepHealth>),
    Done(Box<StepHealth>),
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
            let _ = tx.send(health_of_file(&step));
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
            (k, Entry::Done(h)) if k == key => Some(h),
            _ => None,
        }
    }
}

/// The report as plain text for the clipboard.
pub(crate) fn report_text(h: &StepHealth) -> String {
    let mut t = format!("{}\n", h.summary());
    if let Some(s) = &h.schema {
        t.push_str(&format!("Schema: {s}\n"));
    }
    if let Some(o) = &h.originator {
        t.push_str(&format!("Written by: {o}\n"));
    }
    t.push_str(&format!(
        "{} entities of {} kinds; {} placements, {} triangles; {} faces drawn flat, {} skipped\n",
        h.entities, h.kinds, h.placements, h.triangles, h.approximated_faces, h.skipped_faces
    ));
    if let Some(n) = h.style.part_name() {
        t.push_str(&format!("Name in the file: {n}\n"));
    }
    if let Some(c) = h.style.dominant {
        t.push_str(&format!(
            "Colour in the file: {}\n",
            novo_interop::part3d::style::hex(c)
        ));
    }
    for n in &h.notes {
        t.push_str(&format!("{n}\n"));
    }
    t
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
            .on_hover_text("Reads the STEP file and lists what KoperCAD cannot draw exactly")
            .clicked();
    });
    if running {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Checking the model...");
        });
    }
    if let Some((k, Entry::Done(h))) = &state.current
        && k == key
    {
        let line = h.summary();
        if h.verdict == Verdict::Clean {
            ui.label(line);
        } else {
            ui.colored_label(ui.visuals().warn_fg_color, line);
        }
        for n in &h.notes {
            ui.weak(n);
        }
        if ui.small_button("Copy report").clicked() {
            ui.ctx().copy_text(report_text(h));
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wait(st: &mut HealthState, key: &PartKey) {
        for _ in 0..300 {
            st.poll();
            if st.report(key).is_some() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("no report");
    }

    #[test]
    fn a_bad_or_missing_file_is_a_report_not_a_crash() {
        let dir = std::env::temp_dir().join(format!("kc-health-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bad = dir.join("bad.step");
        std::fs::write(&bad, b"not a step file").unwrap();
        let key = PartKey::new("Acme", "1");
        let ctx = egui::Context::default();
        let mut st = HealthState::default();
        st.start(&ctx, key.clone(), bad);
        wait(&mut st, &key);
        let h = st.report(&key).unwrap();
        assert_eq!(h.verdict, Verdict::Unreadable);
        assert!(report_text(h).contains("Cannot be read"));
        st.start(&ctx, key.clone(), dir.join("missing.step"));
        wait(&mut st, &key);
        assert_eq!(st.report(&key).unwrap().verdict, Verdict::Unreadable);
        let _ = std::fs::remove_dir_all(dir);
    }
}
