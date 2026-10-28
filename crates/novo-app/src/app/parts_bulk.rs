//! Parts commands that used to be engine only: the bulk import of a folder
//! of maker catalog files (`parts.import_folder`, what `kopercad-cli parts`
//! does), and filling devices with parts (`parts.auto_assign`,
//! `parts.clear`, `parts.remove_unused`).
//!
//! Everything lives under the user's profile (`%APPDATA%\KoperCAD`): the
//! catalog, its inbox and the library. No admin rights, no maker site is
//! contacted (docs/PARTS-BULK-IMPORT.md).

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_core::DeviceId;
use novo_parts::{
    Catalog, CatalogPart, Confidence, InboxOptions, InboxReport, assign_choices, clear_parts,
    default_inbox, import_inbox, prepare_inbox, propose_parts, remove_unused_parts, unused_parts,
};

use super::part_picker::selected_devices;
use super::revisions::today;
use super::{Level, NovoApp};

/// The result of a bulk import, shown until closed.
pub(crate) struct BulkReport {
    pub(super) folder: PathBuf,
    pub(super) text: String,
    pub(super) rejected: usize,
}

/// One part a device could get, as the dialog lists it.
struct AssignOption {
    part: CatalogPart,
    confidence: Confidence,
    reason: String,
}

/// One device in the "Assign parts to all devices" dialog.
struct AssignRow {
    device: DeviceId,
    tag: String,
    /// Best first (see `propose_parts`), at most [`MAX_OPTIONS`].
    options: Vec<AssignOption>,
    /// The option that will be assigned; `None` skips the device.
    chosen: Option<usize>,
}

/// Most parts one device's list offers; the Parts window has the rest.
const MAX_OPTIONS: usize = 25;

/// Proposals waiting for the person's choices.
pub(crate) struct AutoAssignPreview {
    rows: Vec<AssignRow>,
    /// Tags of devices for which no catalog part fits at all.
    none: Vec<String>,
}

impl AutoAssignPreview {
    fn chosen_count(&self) -> usize {
        self.rows.iter().filter(|r| r.chosen.is_some()).count()
    }

    /// What the dialog will do: the part chosen for each device.
    fn choices(&self) -> Vec<(DeviceId, &CatalogPart)> {
        self.rows
            .iter()
            .filter_map(|r| Some((r.device, &r.options[r.chosen?].part)))
            .collect()
    }
}

/// Opens a folder in the system file manager (per user, no elevation).
pub(crate) fn open_folder(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open {}: {e}", path.display()))
}

/// The option a row takes when its tick is set: the sure one, else the first.
fn best_sure(row: &AssignRow) -> Option<usize> {
    let sure = |c: Confidence| c != Confidence::Unsure;
    let mut sure_options = row
        .options
        .iter()
        .enumerate()
        .filter(|(_, o)| sure(o.confidence));
    match (sure_options.next(), sure_options.next()) {
        (Some((at, _)), None) => Some(at),
        _ => None,
    }
}

fn confidence_colour(p: &novo_theme::Palette, c: Confidence) -> egui::Color32 {
    match c {
        Confidence::Certain => p.success,
        Confidence::Likely => p.text_secondary,
        Confidence::Unsure => p.warning,
    }
}

/// `-T1, -T2 and 4 more`.
fn tag_list(tags: &[String]) -> String {
    const SHOWN: usize = 8;
    if tags.len() <= SHOWN {
        tags.join(", ")
    } else {
        format!(
            "{} and {} more",
            tags[..SHOWN].join(", "),
            tags.len() - SHOWN
        )
    }
}

/// `dir/name`, or `dir/stem (2).ext`, `(3)`... when that file exists.
pub(super) fn free_name(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    let as_path = Path::new(name);
    let stem = as_path.file_stem().unwrap_or(name).to_string_lossy();
    let ext = as_path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("an unused number exists")
}

/// Creates the inbox when missing and imports it.
pub(super) fn prepare_and_import_inbox(app: &mut NovoApp, inbox: &Path) -> Result<(), String> {
    prepare_inbox(inbox).map_err(|e| format!("Parts inbox: {e}"))?;
    app.bulk_import_from(inbox, None);
    Ok(())
}

/// The one-line summary of a run, for the status messages.
pub(crate) fn summary_line(report: &InboxReport) -> String {
    let rejected = report.rejected.len();
    let mut s = format!(
        "Parts import: {} read, {} new, catalog now {} parts",
        report.total_read(),
        report.total_added(),
        report.catalog_size
    );
    if rejected > 0 {
        s.push_str(&format!(", {rejected} file(s) with no parts"));
    }
    s
}

impl NovoApp {
    /// The command `parts.import_folder`: asks for a folder of catalog files
    /// (BMEcat, CSV, ZIP) and imports it. The picker opens in the default
    /// inbox, which is created with a folder per maker first.
    pub(super) fn import_parts_folder(&mut self) {
        let inbox = default_inbox();
        if let Some(dir) = &inbox
            && let Err(e) = prepare_inbox(dir)
        {
            self.message(Level::Warning, format!("Parts inbox: {e}"));
        }
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Folder with catalog files (BMEcat, CSV, ZIP)");
        if let Some(dir) = inbox.filter(|d| d.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(folder) = dialog.pick_folder() {
            self.bulk_import_from(&folder, None);
        }
    }

    /// Imports every catalog file under `folder` into the catalog at
    /// `catalog_path` (default: the user's catalog) and shows the result.
    pub(crate) fn bulk_import_from(&mut self, folder: &Path, catalog_path: Option<&Path>) -> bool {
        let opts = InboxOptions {
            today: today(),
            dry_run: false,
        };
        if folder.is_file() {
            let name = folder
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.message(
                Level::Error,
                format!(
                    "Parts import: {name} is a file, not a folder. To import one CSV or Excel file use File > Import parts > Import parts from a table."
                ),
            );
            return false;
        }
        match import_inbox(folder, catalog_path, &opts) {
            Ok(report) => {
                self.message(Level::Info, summary_line(&report));
                self.wb.parts_bulk = Some(BulkReport {
                    folder: folder.to_owned(),
                    text: report.render(),
                    rejected: report.rejected.len(),
                });
                if catalog_path.is_none() {
                    self.reload_parts_catalog();
                }
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("Parts import: {e}"));
                false
            }
        }
    }

    /// Copies catalog files into the inbox and imports it. Used by the
    /// Parts window button and by a file dropped with no part selected.
    pub(super) fn import_catalog_files(&mut self, files: &[PathBuf]) {
        let Some(inbox) = default_inbox() else {
            self.message(Level::Error, "No profile folder for the parts inbox");
            return;
        };
        if let Err(e) = prepare_inbox(&inbox) {
            self.message(Level::Error, format!("Parts inbox: {e}"));
            return;
        }
        for file in files {
            let Some(name) = file.file_name() else {
                continue;
            };
            // A file inside the inbox stays where it is.
            if file.starts_with(&inbox) {
                continue;
            }
            // The file name picks the maker (recipes match the name), so the
            // file goes in the inbox root.
            // A file already there with that name is kept: the copy gets a
            // number (`parts (2).csv`).
            let target = free_name(&inbox, name);
            if let Err(e) = std::fs::copy(file, &target) {
                self.message(
                    Level::Error,
                    format!("Could not copy {}: {e}", file.display()),
                );
            }
        }
        self.bulk_import_from(&inbox, None);
    }

    /// Reloads the open Parts window's catalog from disk.
    fn reload_parts_catalog(&mut self) {
        if let Some(win) = self.wb.parts.as_mut() {
            match Catalog::load_default() {
                Ok(c) => win.set_catalog(c),
                Err(e) => self.message(Level::Error, format!("Parts catalog: {e}")),
            }
        }
    }

    pub(super) fn parts_bulk_dialog(&mut self, ctx: &egui::Context) {
        let Some(report) = &self.wb.parts_bulk else {
            return;
        };
        let mut close = false;
        let mut open_inbox = false;
        let modal = egui::Modal::new(egui::Id::new("parts_bulk")).show(ctx, |ui| {
            ui.set_width(640.0);
            super::dialog_ui::title(ui, "Parts import", "");
            ui.weak(format!("Folder: {}", report.folder.display()));
            if report.rejected > 0 {
                ui.colored_label(
                    crate::theme::ui_palette().warning,
                    format!(
                        "{} file(s) held no parts, see the list below",
                        report.rejected
                    ),
                );
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(&report.text).monospace()).wrap());
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                close = ui.button("Close").clicked();
                open_inbox = ui.button("Open inbox folder").clicked();
            });
        });
        if open_inbox
            && let Some(dir) = default_inbox()
            && let Err(e) = open_folder(&dir)
        {
            self.message(Level::Error, e);
        }
        if close || modal.should_close() {
            self.wb.parts_bulk = None;
        }
    }

    /// The command `parts.auto_assign`: works out the proposals and shows
    /// them; nothing changes until Accept all.
    pub(super) fn start_auto_assign(&mut self) {
        let catalog = match Catalog::load_default() {
            Ok(c) => c,
            Err(e) => {
                self.message(Level::Error, format!("Parts catalog: {e}"));
                Catalog::starter()
            }
        };
        self.start_auto_assign_with(&catalog);
    }

    pub(crate) fn start_auto_assign_with(&mut self, catalog: &Catalog) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            self.message(Level::Info, "Open a project to assign parts");
            return;
        };
        let proposals = propose_parts(catalog, project, None);
        let rows: Vec<AssignRow> = proposals
            .iter()
            .filter_map(|p| {
                let device = project.device(p.device)?;
                let chosen = p
                    .default_choice()
                    .and_then(|best| {
                        p.candidates
                            .iter()
                            .position(|c| std::ptr::eq(c.part, best.part))
                    })
                    .filter(|&at| at < MAX_OPTIONS);
                Some(AssignRow {
                    device: p.device,
                    tag: device.tag.to_string(),
                    options: p
                        .candidates
                        .iter()
                        .take(MAX_OPTIONS)
                        .map(|c| AssignOption {
                            part: c.part.clone(),
                            confidence: c.confidence,
                            reason: c.reason.clone(),
                        })
                        .collect(),
                    chosen,
                })
            })
            .collect();
        // `propose_parts` never returns a proposal with no candidates, so a
        // device without a proposal is one with no fitting catalog part.
        let mut none: Vec<&novo_core::Device> = project
            .devices()
            .filter(|d| {
                d.parts.is_empty()
                    && d.kind != novo_core::DeviceKind::TerminalStrip
                    && !proposals.iter().any(|p| p.device == d.id)
            })
            .collect();
        none.sort_by(|a, b| novo_core::natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
        let none: Vec<String> = none.iter().map(|d| d.tag.to_string()).collect();
        if rows.is_empty() && none.is_empty() {
            self.message(Level::Info, "Every device already has a part");
            return;
        }
        self.wb.auto_assign = Some(AutoAssignPreview { rows, none });
    }

    /// Applies the previewed choices as one undo step.
    pub(crate) fn confirm_auto_assign(&mut self) -> bool {
        let Some(preview) = self.wb.auto_assign.take() else {
            return false;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return false;
        };
        let count = preview.chosen_count();
        let Some(command) = assign_choices(project, &preview.choices()) else {
            self.message(Level::Info, "No part chosen, nothing changed");
            return false;
        };
        match self.execute(command) {
            Some(Ok(_)) => {
                self.message(Level::Info, format!("Assigned parts to {count} device(s)"));
                true
            }
            Some(Err(e)) => {
                self.message(Level::Error, format!("Could not assign parts: {e}"));
                false
            }
            None => false,
        }
    }

    pub(super) fn auto_assign_dialog(&mut self, ctx: &egui::Context) {
        let Some(preview) = &mut self.wb.auto_assign else {
            return;
        };
        let palette = crate::theme::ui_palette();
        let (mut go, mut cancel) = (false, false);
        let modal = egui::Modal::new(egui::Id::new("auto_assign")).show(ctx, |ui| {
            ui.set_width(780.0);
            super::dialog_ui::title(ui, "Assign parts to all devices", "");
            ui.weak(
                "Tick the devices that get a part and pick the part. Only parts that are \
                 sure are ticked; a part of another kind is never offered.",
            );
            ui.add_space(4.0);
            if preview.rows.is_empty() {
                ui.label("No part in the catalog fits any of the devices without a part.");
            }
            ui.horizontal(|ui| {
                if !preview.rows.is_empty() && ui.small_button("Tick sure parts").clicked() {
                    for row in &mut preview.rows {
                        row.chosen = best_sure(row);
                    }
                }
                if !preview.rows.is_empty() && ui.small_button("Untick all").clicked() {
                    for row in &mut preview.rows {
                        row.chosen = None;
                    }
                }
            });
            egui::ScrollArea::vertical()
                .max_height(340.0)
                .show(ui, |ui| {
                    egui::Grid::new("auto_assign_rows")
                        .num_columns(5)
                        .spacing([10.0, 6.0])
                        .striped(true)
                        .show(ui, |ui| {
                            if !preview.rows.is_empty() {
                                for h in ["", "Device", "Part", "Match", "Why"] {
                                    ui.label(egui::RichText::new(h).strong());
                                }
                                ui.end_row();
                            }
                            for row in &mut preview.rows {
                                let mut on = row.chosen.is_some();
                                if ui.checkbox(&mut on, "").changed() {
                                    row.chosen = on.then(|| best_sure(row).unwrap_or(0));
                                }
                                ui.monospace(&row.tag);
                                let shown = row.chosen.unwrap_or(0);
                                let label = |o: &AssignOption| {
                                    format!("{} {}", o.part.manufacturer, o.part.order_number)
                                };
                                egui::ComboBox::from_id_salt(("auto_assign_part", row.device))
                                    .width(230.0)
                                    .selected_text(label(&row.options[shown]))
                                    .show_ui(ui, |ui| {
                                        for (at, o) in row.options.iter().enumerate() {
                                            let text = format!(
                                                "{}  ({}) {}",
                                                label(o),
                                                o.confidence.label(),
                                                o.part.description
                                            );
                                            if ui
                                                .selectable_label(row.chosen == Some(at), text)
                                                .clicked()
                                            {
                                                row.chosen = Some(at);
                                            }
                                        }
                                    });
                                let o = &row.options[shown];
                                ui.colored_label(
                                    confidence_colour(palette, o.confidence),
                                    o.confidence.label(),
                                );
                                ui.add(
                                    egui::Label::new(egui::RichText::new(&o.reason).weak()).wrap(),
                                );
                                ui.end_row();
                            }
                        });
                });
            if !preview.none.is_empty() {
                ui.add_space(4.0);
                ui.colored_label(
                    palette.warning,
                    format!(
                        "No part in the catalog fits {}: {}. Add one in the Parts window.",
                        preview.none.len(),
                        tag_list(&preview.none)
                    ),
                );
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let n = preview.chosen_count();
                ui.add_enabled_ui(n > 0, |ui| {
                    let text = format!("Assign {n} part{}", if n == 1 { "" } else { "s" });
                    go = novo_theme::widgets::primary_button(ui, &text).clicked();
                });
                cancel = novo_theme::widgets::ghost_button(
                    ui,
                    if preview.rows.is_empty() {
                        "Close"
                    } else {
                        "Cancel"
                    },
                )
                .clicked();
            });
        });
        if go {
            self.confirm_auto_assign();
        } else if cancel || modal.should_close() {
            self.wb.auto_assign = None;
        }
    }

    /// The command `parts.clear`: takes the part off the selected devices.
    pub(super) fn clear_selected_parts(&mut self) {
        let selection = self.selection();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let devices: Vec<DeviceId> = selected_devices(project, &selection);
        if devices.is_empty() {
            self.message(Level::Info, "Select a device on the drawing first");
            return;
        }
        let Some(command) = clear_parts(project, &devices) else {
            self.message(Level::Info, "The selected devices have no part");
            return;
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(
                Level::Info,
                format!("Removed the part from {} selected", devices.len()),
            ),
            Some(Err(e)) => self.message(Level::Error, format!("Could not remove parts: {e}")),
            None => {}
        }
    }

    /// The command `parts.remove_unused`: deletes project parts nothing uses.
    pub(super) fn remove_unused_project_parts(&mut self) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let n = unused_parts(project).len();
        let Some(command) = remove_unused_parts(project) else {
            self.message(Level::Info, "No unused parts in this project");
            return;
        };
        match self.execute(command) {
            Some(Ok(_)) => self.message(Level::Info, format!("Removed {n} unused part(s)")),
            Some(Err(e)) => self.message(Level::Error, format!("Could not remove parts: {e}")),
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::gate1_tests::Drive;
    use novo_core::DeviceTag;
    use novo_parts::CatalogPart;

    fn drive_with_devices() -> (tempfile::TempDir, Drive) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, crate::app::Action::NewProject);
        d.frame();
        let doc = d.app.backend.document_mut().unwrap();
        doc.transact("Add", |tx| {
            tx.add_device(DeviceTag::parse("-K1").unwrap(), "Pump")?;
            tx.add_device(DeviceTag::parse("-K2").unwrap(), "Fan")
        })
        .unwrap();
        (dir, d)
    }

    fn contactor_catalog() -> Catalog {
        let mut c = Catalog::new();
        let mut p = CatalogPart::new("Acme", "C-1");
        p.letter_codes = vec!["K".into()];
        c.upsert(p);
        c
    }

    #[test]
    fn auto_assign_previews_then_applies_as_one_undo_step() {
        let (_dir, mut d) = drive_with_devices();
        d.app.start_auto_assign_with(&contactor_catalog());
        let preview = d.app.wb.auto_assign.as_ref().expect("a preview");
        assert_eq!(preview.rows.len(), 2);
        // Nothing is applied before Accept all.
        let project = d.app.backend.document().unwrap().project();
        assert!(project.devices().all(|dev| dev.parts.is_empty()));
        assert!(d.app.confirm_auto_assign());
        let project = d.app.backend.document().unwrap().project();
        assert!(project.devices().all(|dev| dev.parts.len() == 1));
        d.app.backend.document_mut().unwrap().undo().unwrap();
        let project = d.app.backend.document().unwrap().project();
        assert!(project.devices().all(|dev| dev.parts.is_empty()));
    }

    #[test]
    fn a_skipped_row_is_left_alone_and_a_chosen_part_is_taken() {
        let (_dir, mut d) = drive_with_devices();
        let mut catalog = contactor_catalog();
        let mut other = CatalogPart::new("Acme", "C-2");
        other.letter_codes = vec!["K".into()];
        catalog.upsert(other);
        d.app.start_auto_assign_with(&catalog);
        let preview = d.app.wb.auto_assign.as_mut().expect("a preview");
        // Two letter-only parts fit each contactor: nothing is ticked.
        assert_eq!(preview.chosen_count(), 0);
        assert_eq!(preview.rows[0].options.len(), 2);
        // The person picks the second part for -K1 and skips -K2.
        preview.rows[0].chosen = Some(1);
        assert!(d.app.confirm_auto_assign());
        let project = d.app.backend.document().unwrap().project();
        let part_of = |tag: &str| {
            let dev = project
                .devices()
                .find(|x| x.tag.to_string() == tag)
                .unwrap();
            dev.parts
                .first()
                .map(|p| project.part(*p).unwrap().order_number.clone())
        };
        assert_eq!(part_of("-K1").as_deref(), Some("C-2"));
        assert_eq!(part_of("-K2"), None);
    }

    #[test]
    fn a_catalog_with_nothing_of_the_right_kind_says_so_and_changes_nothing() {
        use novo_core::{Pin, Placement, Point, SymbolRole};
        let (_dir, mut d) = drive_with_devices();
        let page = d
            .app
            .backend
            .document()
            .unwrap()
            .project()
            .pages()
            .next()
            .unwrap()
            .id;
        d.app
            .backend
            .document_mut()
            .unwrap()
            .transact("Drive", |tx| {
                let t1 = tx.add_device(DeviceTag::parse("-T1").unwrap(), "")?;
                tx.place_symbol(
                    page,
                    "iec/drives/frequency_converter",
                    Placement::at(Point::new(100.0, 100.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: t1,
                        main: true,
                    },
                )
            })
            .unwrap();
        // Only a power supply, which also carries the letter T.
        let mut catalog = Catalog::new();
        let mut psu = CatalogPart::new("Acme", "PSU-24");
        psu.letter_codes = vec!["T".into()];
        psu.description = "Power supply 24 V DC, 5 A".into();
        catalog.upsert(psu);
        d.app.start_auto_assign_with(&catalog);
        // The dialog opens with no rows and names the devices that fit nothing.
        let preview = d.app.wb.auto_assign.as_ref().expect("the dialog explains");
        assert!(preview.rows.is_empty());
        assert!(
            preview.none.iter().any(|t| t == "-T1"),
            "{:?}",
            preview.none
        );
        let project = d.app.backend.document().unwrap().project();
        assert!(project.devices().all(|dev| dev.parts.is_empty()));
    }

    #[test]
    fn escape_closes_the_parts_window() {
        let (_dir, mut d) = drive_with_devices();
        d.app.open_parts_browser();
        d.frame();
        d.frame();
        assert!(d.app.wb.parts.is_some());
        d.key(egui::Key::Escape, egui::Modifiers::NONE);
        d.frame();
        assert!(d.app.wb.parts.is_none(), "Esc closes the Parts window");
    }

    #[test]
    fn library_parts_opens_and_closes_the_window() {
        let (_dir, mut d) = drive_with_devices();
        d.app.wb.queued.push("parts.browser");
        d.frame();
        assert!(d.app.wb.parts.is_some());
        d.app.wb.queued.push("parts.browser");
        d.frame();
        assert!(d.app.wb.parts.is_none());
    }

    #[test]
    fn use_for_selected_device_is_not_primary_without_a_selection() {
        let (_dir, mut d) = drive_with_devices();
        d.app.open_parts_browser();
        d.frame();
        d.frame();
        assert!(!d.app.wb.parts.as_ref().unwrap().device_selected);
    }

    #[test]
    fn proposals_always_have_candidates() {
        let (_dir, d) = drive_with_devices();
        let project = d.app.backend.document().unwrap().project();
        for p in propose_parts(&contactor_catalog(), project, None) {
            assert!(!p.candidates.is_empty());
        }
        assert!(propose_parts(&Catalog::new(), project, None).is_empty());
    }

    #[test]
    fn a_copied_catalog_file_never_overwrites_one_in_the_inbox() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.csv"), "x").unwrap();
        let next = free_name(dir.path(), std::ffi::OsStr::new("a.csv"));
        assert_eq!(next, dir.path().join("a (2).csv"));
        std::fs::write(&next, "y").unwrap();
        assert_eq!(
            free_name(dir.path(), std::ffi::OsStr::new("a.csv")),
            dir.path().join("a (3).csv")
        );
    }

    #[test]
    fn a_second_new_part_with_the_same_key_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut win = crate::app::parts::PartsWindow::new(
            Catalog::new(),
            None,
            novo_parts::SourceTable::builtin(),
        );
        win.set_catalog_path_for_test(dir.path().join("parts.json"));
        let mut first = CatalogPart::new("Acme", "X-1");
        first.description = "first".into();
        win.add_user_part(first).unwrap();
        let mut second = CatalogPart::new("ACME", "x-1");
        second.description = "second".into();
        assert!(win.add_user_part(second).is_err());
        let saved = Catalog::load(&dir.path().join("parts.json")).unwrap();
        assert_eq!(saved.parts().len(), 1);
        assert_eq!(saved.parts()[0].description, "first");
    }

    #[test]
    fn the_commands_are_run_by_the_app() {
        for id in [
            "parts.import_folder",
            "parts.auto_assign",
            "parts.clear",
            "parts.remove_unused",
        ] {
            assert!(crate::app::workbench::app_runs(id), "{id}");
        }
    }

    #[test]
    fn remove_unused_and_clear_run_from_the_queue() {
        let (_dir, mut d) = drive_with_devices();
        d.app.start_auto_assign_with(&contactor_catalog());
        d.app.confirm_auto_assign();
        // Clearing with nothing selected only says so.
        d.app.wb.queued.push("parts.clear");
        d.frame();
        let project = d.app.backend.document().unwrap().project();
        assert!(project.devices().all(|dev| dev.parts.len() == 1));
        // Unassign through the engine command path, then remove the leftover.
        let ids: Vec<DeviceId> = project.devices().map(|dev| dev.id).collect();
        let cmd = clear_parts(project, &ids).unwrap();
        d.app.execute(cmd).unwrap().unwrap();
        assert_eq!(
            unused_parts(d.app.backend.document().unwrap().project()).len(),
            1
        );
        d.app.wb.queued.push("parts.remove_unused");
        d.frame();
        assert!(unused_parts(d.app.backend.document().unwrap().project()).is_empty());
        assert_eq!(
            d.app.backend.document().unwrap().project().parts().count(),
            0
        );
    }

    #[test]
    fn bulk_import_reads_a_folder_and_reports() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("inbox");
        std::fs::create_dir_all(inbox.join("wago")).unwrap();
        std::fs::write(
            inbox.join("wago").join("parts.csv"),
            "order_number,description\n221-412,Splicing connector\n",
        )
        .unwrap();
        std::fs::write(inbox.join("wago").join("empty.csv"), "").unwrap();
        let catalog = dir.path().join("parts.json");
        let mut d = Drive::new(&dir.path().join("untitled"));
        assert!(d.app.bulk_import_from(&inbox, Some(&catalog)));
        let report = d.app.wb.parts_bulk.as_ref().expect("a result is shown");
        assert!(report.text.contains("Total:"), "{}", report.text);
        assert!(catalog.exists());
    }
}
