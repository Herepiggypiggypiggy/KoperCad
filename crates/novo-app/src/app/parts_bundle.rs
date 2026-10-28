//! Export and import of a parts library bundle, in the Parts window
//! (Parts > More > Export library... and Import library...).
//!
//! A bundle moves your parts to another computer or to a colleague
//! (`novo_parts::bundle`). Sharing is limited to the parts you wrote
//! yourself, and the files of makers go only into a backup for your own use,
//! which asks you to confirm that it is yours before they are stored. The
//! work (reading and writing hundreds of megabytes of STEP files) runs on a
//! worker thread, so the window keeps painting.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui;
use novo_parts::{
    Applied, Bundle, BundleMode, Catalog, CatalogPart, Choices, ExportOptions, ExportReport,
    LocalLibrary, Preview, export_bundle,
};

/// What the export form asks.
#[derive(Debug, Clone, Default)]
struct ExportForm {
    /// A backup for yourself (everything) instead of a bundle to share.
    backup: bool,
    /// Put CAD files in.
    files: bool,
    /// For a shared bundle with files: the user says they are theirs to share.
    mine: bool,
}

/// A bundle that was opened and is waiting for the user's decision.
struct Opened {
    path: PathBuf,
    bundle: Bundle,
    preview: Preview,
    replace: bool,
    confirmed: bool,
}

/// What a finished import hands back.
struct ImportDone {
    applied: Applied,
    /// The bundle's parts that are now in the catalog, to be saved.
    changed: Vec<CatalogPart>,
}

enum Job {
    Exporting(Receiver<Result<ExportReport, String>>, PathBuf),
    Opening(Receiver<Result<Opened, String>>),
    Importing(Receiver<Result<ImportDone, String>>),
}

/// "1 part", "2 parts".
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What the dialog needs to know about the window.
pub(super) struct Env<'a> {
    pub catalog: &'a Catalog,
    pub library: Option<&'a LocalLibrary>,
    pub wake: Option<&'a egui::Context>,
    pub today: String,
}

#[derive(Default)]
pub(super) struct BundleUi {
    form: Option<ExportForm>,
    opened: Option<Opened>,
    job: Option<Job>,
    /// What the last export or import did, until the user closes it.
    result: Vec<String>,
}

fn spawn<T: Send + 'static>(
    wake: Option<&egui::Context>,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<Receiver<T>> {
    let (tx, rx) = channel();
    let wake = wake.cloned();
    std::thread::Builder::new()
        .name("parts-bundle".into())
        .spawn(move || {
            let _ = tx.send(work());
            if let Some(ctx) = wake {
                ctx.request_repaint();
            }
        })
        .ok()
        .map(|_| rx)
}

impl BundleUi {
    /// Whether anything of the dialog is showing.
    pub(super) fn visible(&self) -> bool {
        self.form.is_some()
            || self.opened.is_some()
            || self.job.is_some()
            || !self.result.is_empty()
    }

    /// Adds a line to what the dialog says.
    pub(super) fn note(&mut self, line: String) {
        self.result.push(line);
    }

    /// Opens the export form.
    pub(super) fn start_export(&mut self) {
        if self.job.is_none() {
            self.result.clear();
            self.opened = None;
            self.form = Some(ExportForm::default());
        }
    }

    /// Asks for a bundle and opens it in the background.
    pub(super) fn start_import(&mut self, env: &Env) {
        if self.job.is_some() {
            return;
        }
        let Some(path) = crate::dialogs::FileDialog::new()
            .set_title("Library bundle")
            .add_filter("KoperCAD library bundle", &["kcpl"])
            .pick_file()
        else {
            return;
        };
        self.open_path(path, env);
    }

    /// Opens the bundle at `path` in the background.
    pub(super) fn open_path(&mut self, path: PathBuf, env: &Env) {
        self.result.clear();
        self.form = None;
        self.opened = None;
        let catalog = env.catalog.clone();
        let library = env.library.cloned();
        self.job = spawn(env.wake, move || {
            let bundle = Bundle::open(&path).map_err(|e| e.to_string())?;
            let preview = bundle.preview(&catalog, library.as_ref());
            Ok(Opened {
                path,
                bundle,
                preview,
                replace: false,
                confirmed: false,
            })
        })
        .map(Job::Opening);
        if self.job.is_none() {
            self.result
                .push("The bundle could not be opened: no thread could be started.".into());
        }
    }

    /// Collects a finished job. The parts an import added are returned for the
    /// window to merge into its catalog and save.
    pub(super) fn poll(&mut self) -> Vec<CatalogPart> {
        let Some(job) = self.job.take() else {
            return Vec::new();
        };
        match job {
            Job::Exporting(rx, out) => match rx.try_recv() {
                Err(TryRecvError::Empty) => self.job = Some(Job::Exporting(rx, out)),
                Err(TryRecvError::Disconnected) => self
                    .result
                    .push("The export stopped unexpectedly; no file was written.".into()),
                Ok(Err(e)) => self.result.push(format!("Not exported: {e}")),
                Ok(Ok(r)) => {
                    self.result.push(format!(
                        "Wrote {}: {} parts, {} files.",
                        out.display(),
                        r.parts,
                        r.files
                    ));
                    if r.listed_only > 0 {
                        self.result.push(format!(
                            "{} parts from makers' catalogs are listed by order number only.",
                            r.listed_only
                        ));
                    }
                    self.result.extend(r.notes);
                }
            },
            Job::Opening(rx) => match rx.try_recv() {
                Err(TryRecvError::Empty) => self.job = Some(Job::Opening(rx)),
                Err(TryRecvError::Disconnected) => self
                    .result
                    .push("The bundle could not be read: the reader stopped unexpectedly.".into()),
                Ok(Err(e)) => self.result.push(e),
                Ok(Ok(opened)) => self.opened = Some(opened),
            },
            Job::Importing(rx) => match rx.try_recv() {
                Err(TryRecvError::Empty) => self.job = Some(Job::Importing(rx)),
                Err(TryRecvError::Disconnected) => self
                    .result
                    .push("The import stopped unexpectedly; check your catalog.".into()),
                Ok(Err(e)) => self.result.push(format!("Not imported: {e}")),
                Ok(Ok(done)) => {
                    let a = &done.applied;
                    self.result.push(format!(
                        "Added {}, replaced {}, kept {} of yours that differ, {} the same; {} stored.",
                        count(a.added, "part", "parts"),
                        a.replaced,
                        a.kept,
                        a.same,
                        count(a.files_stored, "file", "files"),
                    ));
                    self.result.extend(a.notes.iter().cloned());
                    return done.changed;
                }
            },
        }
        Vec::new()
    }

    /// Draws the dialog.
    pub(super) fn show(&mut self, ui: &mut egui::Ui, env: &Env) {
        if !self.visible() {
            return;
        }
        let pal = crate::theme::ui_palette();
        let mut export_now = false;
        let mut cancel = false;
        let mut import_now = false;
        ui.group(|ui| {
            match &self.job {
                Some(Job::Exporting(..)) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Writing the bundle\u{2026}");
                    });
                }
                Some(Job::Opening(_)) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Reading the bundle\u{2026}");
                    });
                }
                Some(Job::Importing(_)) => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Adding the parts and files\u{2026}");
                    });
                }
                None => {}
            }
            if let Some(form) = self.form.as_mut() {
                ui.strong("Export library");
                ui.weak(
                    "Your parts in one file, to move to another computer or to give to a colleague.",
                );
                ui.radio_value(&mut form.backup, false, "Share: only the parts I wrote myself");
                ui.radio_value(
                    &mut form.backup,
                    true,
                    "Backup for me: everything, with the files from makers' sites",
                );
                if form.backup {
                    form.files = true;
                    ui.colored_label(
                        pal.warning,
                        "A backup may hold files from makers that are not meant to be passed on. Keep it for yourself.",
                    );
                } else {
                    ui.add_enabled(
                        true,
                        egui::Checkbox::new(&mut form.files, "Include the CAD files of those parts"),
                    );
                    if form.files {
                        ui.checkbox(
                            &mut form.mine,
                            "These files are mine to share (I made them, or the maker allows passing them on)",
                        );
                    }
                    ui.weak(
                        "Parts from a maker's catalog are not included; only their order numbers are listed, so the receiver can import them from the maker.",
                    );
                }
                let ready = form.backup || !form.files || form.mine;
                ui.horizontal(|ui| {
                    export_now = ui
                        .add_enabled(ready && self.job.is_none(), egui::Button::new("Export\u{2026}"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            }
            if let Some(o) = self.opened.as_mut() {
                ui.strong(format!(
                    "Library bundle {}",
                    o.path.file_name().map_or_else(String::new, |n| n
                        .to_string_lossy()
                        .into_owned())
                ));
                let m = &o.bundle.manifest;
                ui.weak(format!("Made {} by KoperCAD {}.", m.created, m.app));
                ui.label(m.notice.clone());
                let p = &o.preview;
                ui.label(format!(
                    "{} new {}, {} the same, {} different from yours; {} ({} new).",
                    p.new_parts.len(),
                    if p.new_parts.len() == 1 { "part" } else { "parts" },
                    p.same,
                    p.different.len(),
                    count(p.files, "file", "files"),
                    p.files_new
                ));
                if !m.needs.is_empty() {
                    ui.weak(format!(
                        "{} the sender used {} not included; import {} from the maker.",
                        count(m.needs.len(), "part", "parts"),
                        if m.needs.len() == 1 { "is" } else { "are" },
                        if m.needs.len() == 1 { "it" } else { "them" },
                    ));
                }
                if !p.different.is_empty() {
                    ui.checkbox(
                        &mut o.replace,
                        "Replace my parts that differ (otherwise mine are kept)",
                    );
                }
                let needs_yes = o.bundle.needs_confirmation();
                if needs_yes {
                    ui.colored_label(
                        pal.warning,
                        "This backup holds files from makers' sites.",
                    );
                    ui.checkbox(&mut o.confirmed, "It is my own backup");
                }
                ui.horizontal(|ui| {
                    import_now = ui
                        .add_enabled(
                            (!needs_yes || o.confirmed) && self.job.is_none(),
                            egui::Button::new("Import"),
                        )
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            }
            for line in &self.result {
                ui.label(line);
            }
            if !self.result.is_empty()
                && self.form.is_none()
                && self.opened.is_none()
                && self.job.is_none()
                && ui.button("Close").clicked()
            {
                cancel = true;
            }
        });
        if cancel && self.job.is_none() {
            self.form = None;
            self.opened = None;
            self.result.clear();
        }
        if export_now {
            self.export(env);
        }
        if import_now {
            self.import(env);
        }
    }

    fn export(&mut self, env: &Env) {
        let Some(form) = self.form.clone() else {
            return;
        };
        let name = format!(
            "parts-{}-{}.kcpl",
            if form.backup { "backup" } else { "share" },
            env.today
        );
        let Some(out) = crate::dialogs::FileDialog::new()
            .set_title("Save the library bundle")
            .set_file_name(name)
            .add_filter("KoperCAD library bundle", &["kcpl"])
            .save_file()
        else {
            return;
        };
        let catalog = env.catalog.clone();
        let library = env.library.cloned();
        let opts = ExportOptions {
            mode: if form.backup {
                BundleMode::Backup
            } else {
                BundleMode::Share
            },
            include_files: form.files,
            files_are_shareable: form.mine,
            today: env.today.clone(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
        };
        let target = out.clone();
        let rx = spawn(env.wake, move || {
            export_bundle(&catalog, library.as_ref(), &opts, &target).map_err(|e| e.to_string())
        });
        match rx {
            Some(rx) => {
                self.job = Some(Job::Exporting(rx, out));
                self.form = None;
            }
            None => self
                .result
                .push("Not exported: no thread could be started.".into()),
        }
    }

    fn import(&mut self, env: &Env) {
        let Some(mut o) = self.opened.take() else {
            return;
        };
        let choices = Choices {
            replace_different: o.replace,
            confirmed_own_backup: o.confirmed,
        };
        let mut catalog = env.catalog.clone();
        let library = env.library.cloned();
        let rx = spawn(env.wake, move || {
            let applied = o
                .bundle
                .apply(&mut catalog, library.as_ref(), &choices)
                .map_err(|e| e.to_string())?;
            // The bundle's parts as they are in the catalog now: new ones,
            // and replaced ones. A kept part of the user's is not touched.
            let changed = o
                .bundle
                .parts()
                .iter()
                .filter(|p| catalog.get(&p.key()) == Some(*p))
                .cloned()
                .collect();
            Ok(ImportDone { applied, changed })
        });
        match rx {
            Some(rx) => self.job = Some(Job::Importing(rx)),
            None => self
                .result
                .push("Not imported: no thread could be started.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_parts::{CatalogPart, ImportFlow, NewSource, Provenance};

    fn wait(ui: &mut BundleUi) -> Vec<CatalogPart> {
        let started = std::time::Instant::now();
        loop {
            let changed = ui.poll();
            if !ui.job.is_some() {
                return changed;
            }
            assert!(
                started.elapsed() < std::time::Duration::from_secs(20),
                "the job did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn a_backup_is_opened_in_the_background_and_imported_after_the_confirmation() {
        let dir = tempfile::tempdir().unwrap();
        let src = LocalLibrary::at(dir.path().join("src"));
        let mut catalog = Catalog::new();
        let mut part = CatalogPart::new("Schneider Electric", "LC1D09BD");
        part.provenance = Some(Provenance {
            source: "import:bmecat".into(),
            ..Default::default()
        });
        src.store(
            &NewSource {
                manufacturer: &part.manufacturer,
                order_number: &part.order_number,
                file_name: "m.stp",
                url: None,
                flow: ImportFlow::Manual,
                imported: "2026-10-28",
                terms_url: None,
            },
            b"STEP",
        )
        .unwrap();
        catalog.upsert(part.clone());
        let bundle = dir.path().join("b.kcpl");
        export_bundle(
            &catalog,
            Some(&src),
            &ExportOptions {
                mode: BundleMode::Backup,
                include_files: true,
                files_are_shareable: false,
                today: "2026-10-28".into(),
                app_version: "t".into(),
            },
            &bundle,
        )
        .unwrap();

        let target = LocalLibrary::at(dir.path().join("dst"));
        let empty = Catalog::new();
        let env = Env {
            catalog: &empty,
            library: Some(&target),
            wake: None,
            today: "2026-10-28".into(),
        };
        let mut ui = BundleUi::default();
        ui.open_path(bundle.clone(), &env);
        assert!(
            ui.visible() && ui.job.is_some(),
            "the bundle opens off the window thread"
        );
        assert!(wait(&mut ui).is_empty());
        let opened = ui.opened.as_mut().expect("the bundle was read");
        assert!(opened.bundle.needs_confirmation());
        assert_eq!(opened.preview.new_parts.len(), 1);

        // Without the confirmation nothing is stored.
        ui.import(&env);
        let changed = wait(&mut ui);
        assert!(changed.is_empty());
        assert!(
            ui.result.iter().any(|l| l.contains("own backup")),
            "{:?}",
            ui.result
        );
        assert!(target.records().unwrap().is_empty());

        ui.result.clear();
        ui.open_path(bundle, &env);
        wait(&mut ui);
        ui.opened.as_mut().unwrap().confirmed = true;
        ui.import(&env);
        let changed = wait(&mut ui);
        assert_eq!(changed.len(), 1);
        assert!(target.has(&part.key()));
        assert!(
            ui.result
                .iter()
                .any(|l| l.contains("Added 1 part,") && l.contains("1 file stored")),
            "{:?}",
            ui.result
        );
    }

    #[test]
    fn a_file_that_is_not_a_bundle_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.kcpl");
        std::fs::write(&path, b"text").unwrap();
        let empty = Catalog::new();
        let env = Env {
            catalog: &empty,
            library: None,
            wake: None,
            today: "2026-10-28".into(),
        };
        let mut ui = BundleUi::default();
        ui.open_path(path, &env);
        wait(&mut ui);
        assert!(ui.opened.is_none());
        assert!(
            ui.result
                .iter()
                .any(|l| l.contains("not a KoperCAD library bundle")),
            "{:?}",
            ui.result
        );
    }
}
