//! File > Import parts > Import parts from a table: a CSV or Excel file with any
//! column layout becomes catalog parts in three steps the person can see:
//! the columns (what each one becomes, in which unit), the rows (what each
//! row turns into and what was left out and why), and Add.
//!
//! The engine is `novo_parts::{TableFile, Mapping, build_preview}`; this file
//! is the dialog. Nothing is written to the catalog before Add, and what the
//! file does not say stays unknown on the part (the Rows step counts it).

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_parts::{
    Catalog, DataSource, DuplicateRule, ImportPreview, Mapping, RowStatus, SavedMappings, Severity,
    TableFile, Target, build_preview, guess_header_row, saved_mappings_path, user_catalog_path,
};
use novo_theme::widgets;

use super::revisions::today;
use super::{Level, NovoApp, projects_dir};

/// Rows listed in the Rows step; the rest are in the counts and the report.
const ROWS_SHOWN: usize = 300;

/// The open import, waiting for Add or Cancel.
pub(crate) struct PartsTableImport {
    pub(super) file: String,
    table: TableFile,
    sheet: usize,
    pub(super) mapping: Mapping,
    pub(super) preview: ImportPreview,
    existing: Catalog,
    /// The catalog file Add saves to; the user's catalog when `None`.
    catalog_path: Option<PathBuf>,
    saved: SavedMappings,
    saved_path: Option<PathBuf>,
    /// The mapping came from the person's saved mapping for this layout.
    from_saved: bool,
    remember: bool,
    /// 0: columns, 1: rows.
    tab: usize,
    only_checks: bool,
    today: String,
}

fn existing_catalog(path: Option<&Path>) -> Catalog {
    path.map(Path::to_path_buf)
        .or_else(user_catalog_path)
        .filter(|p| p.exists())
        .and_then(|p| Catalog::load(&p).ok())
        .unwrap_or_default()
}

impl PartsTableImport {
    pub(crate) fn new(
        file: String,
        table: TableFile,
        existing: Catalog,
        catalog_path: Option<PathBuf>,
        saved_path: Option<PathBuf>,
        today: String,
    ) -> Self {
        let saved = saved_path
            .as_deref()
            .map(SavedMappings::load)
            .unwrap_or_default();
        let sheet = table.best_sheet();
        let mut me = PartsTableImport {
            file,
            mapping: Mapping::suggest(&table.sheets[sheet], 0),
            table,
            sheet,
            preview: ImportPreview::default(),
            existing,
            catalog_path,
            saved,
            saved_path,
            from_saved: false,
            remember: true,
            tab: 0,
            only_checks: false,
            today,
        };
        me.start_sheet();
        me
    }

    fn sheet(&self) -> &novo_parts::Sheet {
        &self.table.sheets[self.sheet]
    }

    /// Picks the header row and the first mapping for the current sheet:
    /// the person's saved mapping for these columns, else the suggestion.
    fn start_sheet(&mut self) {
        let header_row = guess_header_row(self.sheet());
        let headers = self
            .sheet()
            .rows
            .get(header_row)
            .cloned()
            .unwrap_or_default();
        let maker = self.mapping.default_manufacturer.clone();
        let saved = self
            .saved
            .find(&headers)
            .filter(|m| m.header_row == header_row);
        self.from_saved = saved.is_some();
        self.mapping = saved.unwrap_or_else(|| Mapping::suggest(self.sheet(), header_row));
        if self.mapping.default_manufacturer.is_empty() {
            self.mapping.default_manufacturer = maker;
        }
        self.refresh();
    }

    fn set_header_row(&mut self, row: usize) {
        let keep = (
            self.mapping.default_manufacturer.clone(),
            self.mapping.currency.clone(),
            self.mapping.origin,
            self.mapping.duplicates,
        );
        self.mapping = Mapping::suggest(self.sheet(), row);
        (
            self.mapping.default_manufacturer,
            self.mapping.currency,
            self.mapping.origin,
            self.mapping.duplicates,
        ) = keep;
        self.from_saved = false;
        self.refresh();
    }

    /// Reads every row again after any change to the mapping.
    pub(crate) fn refresh(&mut self) {
        self.preview = build_preview(
            self.sheet(),
            &self.mapping,
            &self.file,
            &self.today,
            &self.existing,
        );
    }

    pub(crate) fn set_target(&mut self, column: usize, target: Target) {
        if let Some(c) = self.mapping.columns.get_mut(column) {
            c.target = target;
            c.unit.clear();
            c.why = "set by you".into();
        }
        self.refresh();
    }

    fn headers(&self) -> Vec<String> {
        self.sheet()
            .rows
            .get(self.mapping.header_row)
            .cloned()
            .unwrap_or_default()
    }

    /// First few values of a column, for the person to recognise it.
    fn samples(&self, column: usize) -> String {
        let values: Vec<&str> = self
            .sheet()
            .rows
            .iter()
            .skip(self.mapping.header_row + 1)
            .filter_map(|r| r.get(column))
            .map(String::as_str)
            .filter(|s| !s.is_empty())
            .take(3)
            .collect();
        let text = values.join("  \u{b7}  ");
        if text.chars().count() > 46 {
            format!("{}\u{2026}", text.chars().take(45).collect::<String>())
        } else {
            text
        }
    }
}

impl NovoApp {
    /// The command `parts.import_table`: asks for a CSV or Excel parts
    /// table and opens the mapping dialog.
    pub(super) fn import_parts_table(&mut self) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Parts table (CSV or Excel)")
            .add_filter("Parts table", &["csv", "tsv", "txt", "xlsx"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_parts_table_from(&path, None, saved_mappings_path());
        }
    }

    /// Reads `path` and opens the dialog. `catalog_path` is the catalog file
    /// Add saves to (default: the user's catalog); `saved_path` holds the
    /// remembered mappings.
    pub(crate) fn import_parts_table_from(
        &mut self,
        path: &Path,
        catalog_path: Option<PathBuf>,
        saved_path: Option<PathBuf>,
    ) -> bool {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Refuse before reading: a huge file would take the app down.
        if std::fs::metadata(path).is_ok_and(|m| m.len() > novo_parts::MAX_FILE_BYTES) {
            self.message(Level::Error, novo_parts::too_large_file(&name).to_string());
            return false;
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not read {name}: {}", novo_parts::plain_io_error(&e)),
                );
                return false;
            }
        };
        let table = match TableFile::read(&name, &bytes) {
            Ok(t) => t,
            Err(e) => {
                self.message(Level::Error, format!("{name}: {e}"));
                return false;
            }
        };
        let existing = existing_catalog(catalog_path.as_deref());
        self.dismiss_toasts();
        self.wb.parts_table = Some(PartsTableImport::new(
            name,
            table,
            existing,
            catalog_path,
            saved_path,
            today(),
        ));
        true
    }

    /// Adds the previewed parts to the catalog and saves it.
    pub(crate) fn confirm_parts_table(&mut self) -> bool {
        let Some(import) = self.wb.parts_table.take() else {
            return false;
        };
        let Some(path) = import.catalog_path.clone().or_else(user_catalog_path) else {
            self.message(Level::Error, "No profile folder to save the catalog in");
            return false;
        };
        let mut catalog = existing_catalog(Some(&path));
        let added = import.preview.apply(&mut catalog);
        if let Err(e) = catalog.save(&path) {
            self.message(Level::Error, format!("Could not save the catalog: {e}"));
            return false;
        }
        // Best effort: the report next to the catalog, and the mapping for
        // the next file with these columns.
        let report = path.with_file_name("parts-table-import-report.txt");
        let _ = std::fs::write(&report, import.preview.render(&import.file));
        if import.remember
            && let Some(saved_path) = &import.saved_path
        {
            let mut saved = import.saved.clone();
            saved.remember(&import.headers(), &import.mapping);
            let _ = saved.save(saved_path);
        }
        let counts = import.preview.counts();
        self.message(
            Level::Info,
            format!(
                "Parts from {}: {added} added, {} updated, {} not imported{}",
                import.file,
                counts.changed,
                counts.skipped,
                if counts.repeated > 0 {
                    format!(
                        ", {} repeated {} not separate parts",
                        counts.repeated,
                        plural(counts.repeated, "row", "rows")
                    )
                } else {
                    String::new()
                }
            ),
        );
        if import.catalog_path.is_none()
            && let Some(win) = self.wb.parts.as_mut()
            && let Ok(c) = Catalog::load_default()
        {
            win.set_catalog(c);
        }
        true
    }

    pub(super) fn parts_table_dialog(&mut self, ctx: &egui::Context) {
        let Some(import) = &mut self.wb.parts_table else {
            return;
        };
        let screen = ctx.content_rect().size();
        let width = (screen.x - 80.0).clamp(640.0, 1180.0);
        let body_height = (screen.y - 330.0).clamp(150.0, 560.0);
        let (mut go, mut cancel) = (false, false);
        let mut changed = false;
        let modal = egui::Modal::new(egui::Id::new("parts_table_import")).show(ctx, |ui| {
            ui.set_width(width);
            super::dialog_ui::title(
                ui,
                &format!("Import parts from {}", import.file),
                "Say what each column is. Nothing is added until you choose Add; what the file does not say stays unknown.",
            );
            changed |= settings_ui(ui, import);
            ui.add_space(6.0);
            let mut tab = import.tab;
            widgets::tabs(ui, &mut tab, &["Columns", "Rows"]);
            import.tab = tab;
            ui.add_space(6.0);
            if import.tab == 0 {
                changed |= columns_ui(ui, import, body_height);
            } else {
                rows_ui(ui, import, body_height);
            }
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);
            go = footer_ui(ui, import, &mut cancel);
        });
        if changed {
            import.refresh();
        }
        if go {
            self.confirm_parts_table();
        } else if cancel || modal.should_close() {
            self.wb.parts_table = None;
        }
    }
}

/// Sheet, header row, maker, origin and currency. Returns whether the
/// preview must be read again.
fn settings_ui(ui: &mut egui::Ui, import: &mut PartsTableImport) -> bool {
    let mut changed = false;
    let mut new_sheet = None;
    let mut new_header = None;
    ui.horizontal_wrapped(|ui| {
        if import.table.sheets.len() > 1 {
            ui.label("Sheet");
            let name = |s: &novo_parts::Sheet, i: usize| {
                if s.name.is_empty() {
                    format!("Sheet {}", i + 1)
                } else {
                    s.name.clone()
                }
            };
            egui::ComboBox::from_id_salt("parts_table_sheet")
                .selected_text(name(import.sheet(), import.sheet))
                .show_ui(ui, |ui| {
                    for (i, s) in import.table.sheets.iter().enumerate() {
                        if ui.selectable_label(i == import.sheet, name(s, i)).clicked() {
                            new_sheet = Some(i);
                        }
                    }
                });
            if import.sheet != 0 && import.sheet == import.table.best_sheet() {
                ui.weak("Started on the sheet with the order numbers; change it here.");
            }
            ui.add_space(12.0);
        }
        ui.label("Headers are in row");
        let mut line = import.sheet().line_of(import.mapping.header_row);
        let lines: Vec<usize> = (0..import.sheet().rows.len())
            .map(|i| import.sheet().line_of(i))
            .collect();
        if ui
            .add(egui::DragValue::new(&mut line).range(1..=lines.last().copied().unwrap_or(1)))
            .changed()
        {
            // The nearest kept row at or after the typed row number.
            let row = lines
                .iter()
                .position(|l| *l >= line)
                .unwrap_or(lines.len().saturating_sub(1));
            new_header = Some(row);
        }
        ui.add_space(12.0);
        ui.label("Maker for rows that name none");
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut import.mapping.default_manufacturer)
                    .desired_width(150.0)
                    .hint_text("maker of this file"),
            )
            .changed();
    });
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("This file is").on_hover_text(
            "Recorded on every imported value, so calculations can say where a number came from.",
        );
        let options = [
            (DataSource::MakerDatasheet, "Maker data"),
            (DataSource::Typed, "Typed by a person"),
            (DataSource::Estimated, "Estimates"),
            (DataSource::Unknown, "Not stated"),
        ];
        let mut index = options
            .iter()
            .position(|(s, _)| *s == import.mapping.origin)
            .unwrap_or(3);
        let labels: Vec<&str> = options.iter().map(|(_, l)| *l).collect();
        if widgets::segmented(ui, &mut index, &labels) {
            import.mapping.origin = options[index].0;
            changed = true;
        }
        if import.mapping.column_of(&Target::Price).is_some() {
            ui.add_space(12.0);
            ui.label("Price currency when a cell has none");
            changed |= ui
                .add(egui::TextEdit::singleline(&mut import.mapping.currency).desired_width(48.0))
                .changed();
        }
    });
    if let Some(i) = new_sheet {
        import.sheet = i;
        import.start_sheet();
    }
    if let Some(row) = new_header {
        import.set_header_row(row);
    }
    changed
}

fn columns_ui(ui: &mut egui::Ui, import: &mut PartsTableImport, height: f32) -> bool {
    let p = crate::theme::ui_palette();
    let mut changed = false;
    if import.from_saved {
        ui.colored_label(
            p.info,
            "The columns are set from your saved mapping for files with these headers.",
        );
    }
    for problem in &import.preview.problems {
        ui.colored_label(p.error, problem);
    }
    let samples: Vec<String> = (0..import.mapping.columns.len())
        .map(|i| import.samples(i))
        .collect();
    let mut pick: Option<(usize, Target)> = None;
    egui::ScrollArea::vertical()
        .id_salt("parts_table_columns")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("parts_table_columns_grid")
                .num_columns(5)
                .spacing([12.0, 6.0])
                .striped(true)
                .show(ui, |ui| {
                    for h in ["Column in the file", "First values", "Becomes", "Unit", "Why"] {
                        ui.label(egui::RichText::new(h).weak());
                    }
                    ui.end_row();
                    for (i, column) in import.mapping.columns.iter_mut().enumerate() {
                        let header = if column.header.trim().is_empty() {
                            format!("(column {})", i + 1)
                        } else {
                            column.header.clone()
                        };
                        ui.label(egui::RichText::new(header).strong());
                        ui.label(egui::RichText::new(&samples[i]).weak());
                        egui::ComboBox::from_id_salt(("parts_table_target", i))
                            .width(230.0)
                            .selected_text(column.target.label())
                            .show_ui(ui, |ui| {
                                for t in Target::choices() {
                                    if ui
                                        .selectable_label(column.target == t, t.label())
                                        .clicked()
                                    {
                                        pick = Some((i, t));
                                    }
                                }
                                let key = novo_parts::property_key(&column.header)
                                    .unwrap_or_else(|| format!("column_{}", i + 1));
                                let custom = Target::Custom(key);
                                if ui
                                    .selectable_label(
                                        matches!(column.target, Target::Custom(_)),
                                        "Other data, kept as written",
                                    )
                                    .clicked()
                                {
                                    pick = Some((i, custom));
                                }
                            });
                        let wants_unit = column
                            .target
                            .dim()
                            .is_some_and(|d| !novo_parts::stored_unit(d).is_empty());
                        ui.add_enabled_ui(wants_unit, |ui| {
                            let hint = column
                                .target
                                .dim()
                                .map(novo_parts::stored_unit)
                                .unwrap_or_default();
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut column.unit)
                                        .desired_width(52.0)
                                        .hint_text(hint),
                                )
                                .on_hover_text(
                                    "The unit of cells that carry none. Empty: the unit shown in grey is assumed.",
                                )
                                .changed();
                        });
                        ui.add(
                            egui::Label::new(egui::RichText::new(&column.why).weak().small())
                                .truncate(),
                        );
                        ui.end_row();
                    }
                });
        });
    if let Some((i, target)) = pick {
        import.set_target(i, target);
    }
    changed
}

fn rows_ui(ui: &mut egui::Ui, import: &mut PartsTableImport, height: f32) {
    let p = crate::theme::ui_palette();
    for problem in &import.preview.problems {
        ui.colored_label(p.error, problem);
    }
    if !import.preview.problems.is_empty() {
        ui.weak("Fix the columns first; the rows are read once the mapping is complete.");
        return;
    }
    // What the notes above the list take comes off the list's own height, so
    // the buttons below it stay on a small window.
    let notes_top = ui.cursor().top();
    for a in &import.preview.assumptions {
        ui.colored_label(p.warning, a);
    }
    repeated_ui(ui, import);
    let height = (height - (ui.cursor().top() - notes_top)).max(90.0);
    let has_date = import.mapping.column_of(&Target::PriceDate).is_some();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("What the file tells").weak());
        for c in &import.preview.coverage {
            let price_note = c.label == "Price";
            let full = c.total > 0 && c.have == c.total;
            let color = if c.have == 0 {
                p.text_tertiary
            } else if full {
                p.success
            } else {
                p.warning
            };
            ui.colored_label(color, format!("{} {}/{}", c.label, c.have, c.total))
                .on_hover_text(if price_note && has_date {
                    "Prices are list prices as in the file, dated with the price list date column (a cell without a readable date gets the day of the import)."
                } else if price_note {
                    "Prices are list prices as in the file. Their date is the day of the import, not the date of the price list; map a date column to use the list's own date."
                } else if c.have == c.total {
                    "Every imported part has this."
                } else {
                    "Parts without it stay unknown: nothing is filled in."
                });
        }
    });
    ui.checkbox(&mut import.only_checks, "Only rows with something to check");
    ui.add_space(2.0);
    const WIDTHS: [f32; 4] = [46.0, 112.0, 140.0, 160.0];
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (w, h) in WIDTHS
            .iter()
            .zip(["Row", "Result", "Maker", "Order number"])
        {
            cell(ui, *w, egui::RichText::new(h).weak());
        }
        ui.label(egui::RichText::new("Description").weak());
    });
    egui::ScrollArea::vertical()
        .id_salt("parts_table_rows")
        .max_height(height)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let mut shown = 0;
            for row in &import.preview.rows {
                if import.only_checks && row.issues.is_empty() {
                    continue;
                }
                if shown >= ROWS_SHOWN {
                    break;
                }
                shown += 1;
                let (text, color) = match row.status {
                    RowStatus::New => ("New", p.success),
                    RowStatus::Changed => ("Replaces entry", p.warning),
                    RowStatus::Unchanged => ("Already there", p.text_secondary),
                    // A row a repeated-number rule left out is a choice, not an error.
                    RowStatus::Skipped if row.worst() < Some(Severity::Error) => {
                        ("Repeated", p.warning)
                    }
                    RowStatus::Skipped => ("Not imported", p.error),
                };
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    cell(ui, WIDTHS[0], egui::RichText::new(row.line.to_string()));
                    cell(ui, WIDTHS[1], egui::RichText::new(text).color(color));
                    cell(ui, WIDTHS[2], egui::RichText::new(&row.manufacturer));
                    cell(ui, WIDTHS[3], egui::RichText::new(&row.order_number));
                    ui.add(egui::Label::new(&row.description).truncate());
                });
                for issue in &row.issues {
                    let (mark, color) = match issue.severity {
                        Severity::Error => ("\u{25cf}", p.error),
                        Severity::Warning => ("\u{25cf}", p.warning),
                        Severity::Note => ("\u{25cb}", p.text_secondary),
                    };
                    let text = if issue.column.is_empty() {
                        issue.message.clone()
                    } else {
                        format!("{}: {}", issue.column, issue.message)
                    };
                    ui.horizontal_top(|ui| {
                        ui.add_space(WIDTHS[0]);
                        ui.colored_label(color, mark);
                        ui.add(egui::Label::new(egui::RichText::new(text).small()).wrap());
                    });
                }
                ui.add_space(2.0);
            }
            let total = import.preview.rows.len();
            if total > ROWS_SHOWN && !import.only_checks {
                ui.weak(format!(
                    "{} more {} not listed; the report saved with the catalog has every finding.",
                    total - ROWS_SHOWN,
                    plural(total - ROWS_SHOWN, "row", "rows")
                ));
            }
        });
}

/// Order numbers that appear in more than one row: how many, which rows, and
/// the choice of what the catalog gets. Nothing is dropped in silence.
fn repeated_ui(ui: &mut egui::Ui, import: &mut PartsTableImport) {
    if import.preview.repeated.is_empty() {
        return;
    }
    let p = crate::theme::ui_palette();
    let rows: usize = import.preview.repeated.iter().map(|r| r.lines.len()).sum();
    ui.colored_label(
        p.warning,
        format!(
            "{} {} more than one row ({rows} rows). The catalog keeps one entry per maker and order number: choose what it gets.",
            import.preview.repeated.len(),
            if import.preview.repeated.len() == 1 {
                "order number appears in"
            } else {
                "order numbers appear in"
            }
        ),
    );
    let labels = [
        "Last row",
        "First row",
        "First row, gaps filled from the others",
    ];
    let mut index = DuplicateRule::ALL
        .iter()
        .position(|r| *r == import.mapping.duplicates)
        .unwrap_or(0);
    ui.horizontal_wrapped(|ui| {
        if widgets::segmented(ui, &mut index, &labels) {
            import.mapping.duplicates = DuplicateRule::ALL[index];
            import.refresh();
        }
    });
    let shown = 4;
    for r in import.preview.repeated.iter().take(shown) {
        ui.label(
            egui::RichText::new(format!(
                "{} {} is in {}; row {} is used",
                r.manufacturer,
                r.order_number,
                novo_parts::rows_text(&r.lines),
                r.used
            ))
            .small(),
        );
    }
    if import.preview.repeated.len() > shown {
        ui.weak(format!(
            "and {} more; every row of them says so below.",
            import.preview.repeated.len() - shown
        ));
    }
    ui.add_space(4.0);
}

fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

/// One fixed-width, left-aligned, truncated cell of a row.
fn cell(ui: &mut egui::Ui, width: f32, text: egui::RichText) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, ui.spacing().interact_size.y),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_width(width);
            ui.add(egui::Label::new(text).truncate());
        },
    );
}

/// Summary, remember and the two buttons. Returns whether Add was pressed.
fn footer_ui(ui: &mut egui::Ui, import: &mut PartsTableImport, cancel: &mut bool) -> bool {
    let p = crate::theme::ui_palette();
    let counts = import.preview.counts();
    let mut go = false;
    if import.preview.problems.is_empty() {
        let text = format!(
            "{} {}: {}{}",
            import.preview.rows.len(),
            plural(import.preview.rows.len(), "row", "rows"),
            import.preview.summary(),
            if counts.warned > 0 {
                format!(", {} with warnings", counts.warned)
            } else {
                String::new()
            }
        );
        ui.colored_label(
            if counts.skipped + counts.warned + counts.repeated > 0 {
                p.warning
            } else {
                p.text_primary
            },
            text,
        )
        .on_hover_text(
            "Not imported: rows that cannot be a part (no order number, no maker, a mangled order number). \
             Repeated: rows of an order number that appears more than once, left out by the rule chosen in the Rows step. \
             The Rows step says why for every row.",
        );
    } else {
        ui.colored_label(
            p.error,
            "The columns need a change before anything can be added.",
        );
    }
    ui.horizontal(|ui| {
        let n = counts.new + counts.changed;
        ui.add_enabled_ui(import.preview.can_import(), |ui| {
            go = widgets::primary_button(
                ui,
                &match n {
                    1 => "Add 1 part to the catalog".to_owned(),
                    n => format!("Add {n} parts to the catalog"),
                },
            )
            .clicked();
        });
        *cancel = ui.button("Cancel").clicked();
        ui.checkbox(
            &mut import.remember,
            "Remember these columns for files with the same headers",
        );
    });
    go
}
