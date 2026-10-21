//! Draw > PLC > Import I/O list: a machine's signal list (CSV or Excel) becomes
//! PLC cards, a page per card, a terminal strip per card and the wires, in
//! one undo step (novo-plc `generate`).
//!
//! The review comes first, like Autopilot's: which cards the list needs,
//! how many signals each carries, and what was left out and why. Nothing
//! changes in the project until Enter; Esc leaves it as it was. Importing a
//! changed list again adds only the signals that are new.

use eframe::egui;
use novo_plc::generate::{IoList, Options, Plan, apply, plan, read_list};
use novo_symbols::{Library, Pack};

use super::plc_link::notes;
use super::{Level, NovoApp, projects_dir};

/// The review waiting for Enter or Esc.
#[derive(Debug, Clone)]
pub(crate) struct ListImport {
    pub name: String,
    pub list: IoList,
    pub options: Options,
    pub plan: Plan,
}

impl NovoApp {
    /// Asks for a list file and opens the review.
    pub(super) fn plc_import_list(&mut self) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Import I/O list")
            .add_filter("I/O list", &["xlsx", "csv", "tsv", "txt"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.plc_import_list_from(&path);
        }
    }

    /// Reads the file at `path` and opens the review. Nothing changes in
    /// the project until the review is applied.
    pub fn plc_import_list_from(&mut self, path: &std::path::Path) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        const MAX_BYTES: u64 = 32 * 1024 * 1024;
        if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
            self.message(
                Level::Error,
                format!("{name} is larger than 32 MB, which is too big for an I/O list"),
            );
            return false;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message(Level::Error, format!("Could not read {name}: {e}"));
                return false;
            }
        };
        let list = match read_list(&bytes) {
            Ok(l) => l,
            Err(e) => {
                self.message(Level::Error, format!("{name}: {e}"));
                return false;
            }
        };
        let Some(doc) = self.backend.document() else {
            return false;
        };
        let options = Options::default();
        let plan = plan(doc.project(), &list, &options);
        self.wb.plc_list = Some(ListImport {
            name,
            list,
            options,
            plan,
        });
        true
    }

    /// The review: the cards to add and what was left out. Enter builds,
    /// Esc leaves the project untouched.
    pub(super) fn plc_list_dialog(&mut self, ctx: &egui::Context) {
        let Some(review) = &mut self.wb.plc_list else {
            return;
        };
        let before = (
            review.options.keep_addresses,
            review.options.digital_per_card,
        );
        let modal = egui::Modal::new(egui::Id::new("plc_list")).show(ctx, |ui| {
            ui.set_width(620.0);
            ui.heading("Import I/O list");
            ui.label(egui::RichText::new(&review.name).weak());
            ui.add_space(4.0);
            ui.label(review.plan.summary());
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut review.options.keep_addresses,
                    "Keep the addresses of the list",
                );
                ui.label("Digital channels per card");
                for n in [8u16, 16, 32] {
                    ui.selectable_value(&mut review.options.digital_per_card, n, n.to_string());
                }
            });
            ui.add_space(6.0);
            egui::ScrollArea::vertical()
                .max_height(300.0)
                .show(ui, |ui| {
                    egui::Grid::new("plc_list_cards")
                        .num_columns(4)
                        .spacing([14.0, 4.0])
                        .show(ui, |ui| {
                            for h in ["Card", "Channels", "Signals", "Starts"] {
                                ui.label(egui::RichText::new(h).weak());
                            }
                            ui.end_row();
                            for card in &review.plan.cards {
                                let station = if card.station.is_empty() {
                                    String::new()
                                } else {
                                    format!("  ({})", card.station)
                                };
                                ui.strong(format!("{}{station}", card.tag));
                                ui.label(card.layout.to_string());
                                ui.label(if card.spares() > 0 {
                                    format!("{} + {} spare", card.signals(), card.spares())
                                } else {
                                    card.signals().to_string()
                                });
                                ui.label(match card.start {
                                    Some(byte) => format!("byte {byte}, as in the list"),
                                    None => "next free range".to_owned(),
                                });
                                ui.end_row();
                            }
                        });
                    let dropped =
                        |l: &Option<usize>| l.map_or(String::new(), |l| format!("row {l}: "));
                    notes(
                        ui,
                        "Check",
                        review
                            .plan
                            .notes
                            .iter()
                            .map(|n| format!("{}{}", dropped(&n.line), n.text)),
                    );
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let apply = ui
                    .add_enabled_ui(!review.plan.is_empty(), |ui| {
                        novo_theme::widgets::primary_button(ui, "Add pages (Enter)")
                    })
                    .inner
                    .clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                (apply, cancel)
            })
            .inner
        });
        let (apply_clicked, cancel) = modal.inner;
        let after = (
            review.options.keep_addresses,
            review.options.digital_per_card,
        );
        if before != after
            && let Some(doc) = self.backend.document()
        {
            review.plan = plan(doc.project(), &review.list, &review.options);
        }
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        let can_apply = self
            .wb
            .plc_list
            .as_ref()
            .is_some_and(|r| !r.plan.is_empty());
        if (apply_clicked || enter) && can_apply {
            if let Some(review) = self.wb.plc_list.take() {
                self.apply_plc_list(&review);
            }
        } else if cancel || modal.should_close() {
            self.wb.plc_list = None;
        }
    }

    /// Builds a review's plan as one undo step and shows its first page.
    pub(crate) fn apply_plc_list(&mut self, review: &ListImport) {
        let library = self
            .library()
            .cloned()
            .unwrap_or_else(|| Library::shared(Pack::Iec).clone());
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Import I/O list", |tx| {
            apply(tx, &library, &review.list, &review.plan)
        });
        match result {
            Ok((built, changes)) => {
                self.changed_outside_canvas(&changes);
                if let Some(&first) = built.pages.first() {
                    self.show_page(first);
                }
                let n = built.cards.len();
                self.message(
                    Level::Info,
                    format!(
                        "Imported {} signal{}: {n} card{} with a page each, {} terminals",
                        built.channels,
                        if built.channels == 1 { "" } else { "s" },
                        if n == 1 { "" } else { "s" },
                        built.terminals,
                    ),
                );
                if let Some(note) = built.notes.first() {
                    self.message(Level::Warning, note.text.clone());
                }
            }
            Err(e) => self.message(Level::Error, format!("Could not import the I/O list: {e}")),
        }
    }
}
