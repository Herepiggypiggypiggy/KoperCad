//! File > Export > Export EPLAN project (PXF): the options the writer has,
//! chosen before the file is written: the EPLAN release (dialect), the
//! language of the texts, and the person's own table that maps KoperCAD
//! symbols to EPLAN symbol keys (without one a device is written with no
//! symbol reference).

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_interop::eplan::pxf::{Dialect, EPLAN_2024, PxfWriteOptions};
use novo_interop::eplan::symbols::UserTable;

use super::{Level, NovoApp};

/// The EPLAN releases the writer has a dialect for. Adding a release means
/// adding a dialect in `novo-interop`, never guessing one here.
pub(crate) const DIALECTS: &[Dialect] = &[EPLAN_2024];

/// Common language codes offered next to the free text box.
const LANGUAGES: &[&str] = &["en_US", "de_DE", "nl_NL", "fr_FR", "es_ES", "it_IT"];

/// A user symbol table in the shape the file has.
const TABLE_HINT: &str = "[[symbol]]\nset = 1\nnumber = 5\nvariant = 0\nkopercad = \"contacts/no\"";

/// The export options being chosen.
#[derive(Debug, Clone)]
pub(crate) struct EplanExport {
    pub dialect: usize,
    pub lang: String,
    /// The symbol table file and what it holds.
    pub symbols: Option<(PathBuf, UserTable)>,
    /// What went wrong reading the table file, if it did.
    pub problem: Option<String>,
}

impl Default for EplanExport {
    fn default() -> Self {
        EplanExport {
            dialect: 0,
            lang: DIALECTS[0].default_lang.to_owned(),
            symbols: None,
            problem: None,
        }
    }
}

impl EplanExport {
    /// The writer's options for these choices.
    pub(crate) fn options(&self) -> PxfWriteOptions {
        let dialect = DIALECTS.get(self.dialect).copied().unwrap_or(EPLAN_2024);
        let lang = self.lang.trim();
        PxfWriteOptions {
            dialect,
            lang: if lang.is_empty() {
                dialect.default_lang.to_owned()
            } else {
                lang.to_owned()
            },
            symbols: self
                .symbols
                .as_ref()
                .map(|(_, t)| t.clone())
                .unwrap_or_default(),
        }
    }

    /// Reads a symbol table file; a bad file leaves the old table and says
    /// why.
    pub(crate) fn load_table(&mut self, path: &Path) {
        match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|t| UserTable::from_toml(&t).map_err(|e| e.to_string()))
        {
            Ok(table) => {
                self.symbols = Some((path.to_owned(), table));
                self.problem = None;
            }
            Err(e) => self.problem = Some(format!("{}: {e}", path.display())),
        }
    }
}

impl NovoApp {
    /// The command `export.eplan`: opens the options, then writes.
    pub(super) fn open_eplan_export(&mut self) {
        if self.backend.document().is_none() {
            self.message(Level::Info, "Open a project first");
            return;
        }
        self.wb.eplan_export = Some(self.wb.eplan_export_last.clone().unwrap_or_default());
    }

    pub(super) fn eplan_export_dialog(&mut self, ctx: &egui::Context) {
        let Some(form) = &mut self.wb.eplan_export else {
            return;
        };
        let (mut go, mut cancel, mut pick, mut clear) = (false, false, false, false);
        let modal = egui::Modal::new(egui::Id::new("eplan_export")).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.heading("Export EPLAN project (PXF)");
            ui.add_space(6.0);
            egui::Grid::new("eplan_export_grid")
                .num_columns(2)
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    ui.label("EPLAN release");
                    egui::ComboBox::from_id_salt("eplan_export_dialect")
                        .selected_text(DIALECTS[form.dialect.min(DIALECTS.len() - 1)].release)
                        .show_ui(ui, |ui| {
                            for (i, d) in DIALECTS.iter().enumerate() {
                                ui.selectable_value(&mut form.dialect, i, d.release);
                            }
                        });
                    ui.end_row();
                    ui.label("Language of texts");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut form.lang).desired_width(80.0));
                        egui::ComboBox::from_id_salt("eplan_export_lang")
                            .selected_text("common")
                            .show_ui(ui, |ui| {
                                for l in LANGUAGES {
                                    ui.selectable_value(&mut form.lang, (*l).to_owned(), *l);
                                }
                            });
                    });
                    ui.end_row();
                    ui.label("Symbol table");
                    ui.horizontal(|ui| {
                        match &form.symbols {
                            Some((path, table)) => {
                                ui.label(format!(
                                    "{} ({} symbol(s))",
                                    path.file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_default(),
                                    table.len()
                                ));
                                clear = ui.small_button("Clear").clicked();
                            }
                            None => {
                                ui.weak("none: devices are written without a symbol");
                            }
                        }
                        pick = ui.button("Choose file...").clicked();
                    });
                    ui.end_row();
                });
            if let Some(problem) = &form.problem {
                ui.colored_label(crate::theme::ui_palette().warning, problem);
            }
            ui.add_space(4.0);
            egui::CollapsingHeader::new("About the symbol table file")
                .id_salt("eplan_export_table_help")
                .show(ui, |ui| {
                    ui.weak(
                        "A text (TOML) file you write from your own EPLAN symbol keys: one row \
                         per KoperCAD symbol. Without it the devices still export, but EPLAN \
                         has no symbol to draw them with.",
                    );
                    ui.add(egui::Label::new(
                        egui::RichText::new(TABLE_HINT).monospace().small(),
                    ));
                });
            ui.add_space(6.0);
            ui.weak(
                "No real EPLAN has opened a KoperCAD file yet; the report after the export \
                 lists what was left out.",
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                go = ui.button("Export").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if clear {
            form.symbols = None;
        }
        if pick {
            let mut dialog = crate::dialogs::FileDialog::new()
                .set_title("EPLAN symbol table")
                .add_filter("Symbol table (TOML)", &["toml", "txt"]);
            if let Some(dir) = form
                .symbols
                .as_ref()
                .and_then(|(p, _)| p.parent())
                .filter(|d| d.is_dir())
            {
                dialog = dialog.set_directory(dir);
            }
            if let Some(path) = dialog.pick_file() {
                form.load_table(&path);
            }
        }
        if go {
            self.confirm_eplan_export();
        } else if cancel || modal.should_close() {
            self.wb.eplan_export = None;
        }
    }

    /// Writes the export with the options chosen, and remembers them.
    pub(crate) fn confirm_eplan_export(&mut self) -> Option<PathBuf> {
        let form = self.wb.eplan_export.take()?;
        let written = self.export_eplan(form.options());
        self.wb.eplan_export_last = Some(form);
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_writers_defaults() {
        let o = EplanExport::default().options();
        let d = PxfWriteOptions::default();
        assert_eq!(o.lang, d.lang);
        assert_eq!(o.dialect, d.dialect);
        assert!(o.symbols.is_empty());
    }

    #[test]
    fn an_empty_language_falls_back_to_the_releases_default() {
        let form = EplanExport {
            lang: "  ".into(),
            ..EplanExport::default()
        };
        assert_eq!(form.options().lang, EPLAN_2024.default_lang);
    }

    #[test]
    fn a_symbol_table_file_is_read_and_a_bad_one_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("symbols.toml");
        std::fs::write(&good, TABLE_HINT).unwrap();
        let mut form = EplanExport::default();
        form.load_table(&good);
        assert_eq!(form.options().symbols.len(), 1);
        assert!(form.problem.is_none());
        let bad = dir.path().join("bad.toml");
        std::fs::write(&bad, "[[symbol]\nnope").unwrap();
        form.load_table(&bad);
        assert!(form.problem.is_some());
        // The earlier table stays.
        assert_eq!(form.options().symbols.len(), 1);
    }
}
