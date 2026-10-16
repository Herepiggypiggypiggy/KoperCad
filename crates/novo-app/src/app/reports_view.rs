//! The Reports workspace (docs/design/workspaces.md §3.4): every report of
//! the project as a live table, next to the drawing instead of buried in
//! the page list. It hosts `novo-reports` and has no report logic of its
//! own: the tables are the ones the generated pages print and the
//! spreadsheet exports write.
//!
//! Left: the list of reports with their row counts. Centre: the selected
//! report's tables, sortable and filterable. A click on a row selects the
//! device it describes (the selection travels: Schematic then opens on it,
//! and rows of what the drawing has selected are lit); a double-click or
//! Enter jumps to it. Up and Down walk the rows, Ctrl+F focuses the filter
//! and Escape clears it. Exports write the whole report, not the filtered
//! view, next to the project file and never overwrite an earlier one.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use eframe::egui;
use novo_core::{AnyId, natural_cmp};
use novo_reports::{Align, Cell, CsvOptions, ReportOutput, Section};
use novo_theme::table::{
    ROW_HEIGHT, RowState, TableAlign, TableCell, TableColumn, column_widths, table_header,
    table_row,
};
use novo_theme::widgets::section_label;
use novo_xref::Place;

use super::workspace::Workspace;
use super::{Level, NovoApp};

/// What the Reports workspace remembers while the app runs.
#[derive(Default)]
pub(crate) struct ReportsView {
    /// The report shown, by id (`bom`).
    selected: Option<String>,
    /// Text a row must contain to be listed.
    filter: String,
    /// Section key, column and direction (`true` is ascending).
    sort: Option<(String, usize, bool)>,
    /// The shown report's output, kept while the project is unchanged.
    cache: Option<(String, u64, ReportOutput)>,
    /// The row the keys act on: its section key and index in the section.
    cursor: Option<(String, usize)>,
    /// Put the cursor row in view on the next frame.
    reveal: bool,
    /// The object the clicked row describes, for the status bar.
    picked: Option<AnyId>,
    /// Name of the report template the PDF button prints through; `None`
    /// is the plain report layout.
    pub(super) template: Option<String>,
}

/// A file format a report can be written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReportFormat {
    Excel,
    Csv,
    /// A PDF: through the template picked in the header, else the plain
    /// report layout.
    Pdf,
}

impl ReportFormat {
    fn extension(self) -> &'static str {
        match self {
            ReportFormat::Excel => "xlsx",
            ReportFormat::Csv => "csv",
            ReportFormat::Pdf => "pdf",
        }
    }
}

/// The report ids that have a command of their own (`report.bom`), in the
/// order the Reports menu lists them.
pub(crate) const REPORT_COMMANDS: &[(&str, &str)] = &[
    ("report.toc", "toc"),
    ("report.bom", "bom"),
    ("report.devices", "devices"),
    ("report.wires", "wires"),
    ("report.terminals", "terminals"),
    ("report.cables", "cables"),
    ("report.cutlist", "cutlist"),
    ("report.labels", "labels"),
];

impl NovoApp {
    /// Opens the Reports workspace on report `id`.
    pub(crate) fn show_report(&mut self, id: &str) {
        self.reports_view.selected = Some(id.to_owned());
        self.reports_view.sort = None;
        self.switch_workspace(Workspace::Reports);
    }

    /// The report the workspace shows: the chosen one, else the first.
    fn report_selected(&mut self) -> Option<String> {
        let doc = self.backend.document()?;
        let live = self.wb.reports.live(doc.project());
        let ids = live.ids();
        match &self.reports_view.selected {
            Some(id) if ids.contains(&id.as_str()) => Some(id.clone()),
            _ => ids.first().map(|s| (*s).to_owned()),
        }
    }

    /// The selected report's output at the project's current state.
    fn report_output(&mut self, id: &str) -> Option<ReportOutput> {
        let doc = self.backend.document()?;
        let revision = doc.project().revision();
        if let Some((cached, at, output)) = &self.reports_view.cache
            && cached == id
            && *at == revision
        {
            return Some(output.clone());
        }
        let output = self
            .wb
            .reports
            .live(doc.project())
            .get(doc.project(), id)?
            .clone();
        self.reports_view.cache = Some((id.to_owned(), revision, output.clone()));
        Some(output)
    }

    /// The left list: each report with its row count.
    pub(super) fn reports_list(&mut self, ui: &mut egui::Ui) {
        let Some(selected) = self.report_selected() else {
            return;
        };
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let live = self.wb.reports.live(project);
        let rows: Vec<(String, String, usize)> = live
            .ids()
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .into_iter()
            .filter_map(|id| {
                let title = live.report(&id)?.title();
                let count = live.get(project, &id).map_or(0, ReportOutput::row_count);
                Some((id, title, count))
            })
            .collect();
        section_label(ui, "Reports");
        let mut chosen = None;
        for (id, title, count) in &rows {
            ui.horizontal(|ui| {
                let row = ui.selectable_label(*id == selected, title);
                if row.clicked() {
                    chosen = Some(id.clone());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak(count.to_string());
                });
            });
        }
        if let Some(id) = chosen {
            self.reports_view.selected = Some(id);
            self.reports_view.sort = None;
        }
    }

    /// The centre: header, filter and the report's tables.
    pub(super) fn reports_central(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.report_selected() else {
            ui.weak("No reports");
            return;
        };
        let Some(output) = self.report_output(&id) else {
            ui.weak("This report is not available");
            return;
        };
        let orders: Vec<Vec<usize>> = output.sections.iter().map(|s| self.row_order(s)).collect();
        let filter_id = egui::Id::new("reports_filter");
        let mut jump = self.report_keys(ui, &output, &orders, filter_id);
        let mut export = None;
        ui.horizontal(|ui| {
            ui.heading(&output.title);
            ui.weak(format!("{} rows", output.row_count()));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);
                ui.menu_button("Export", |ui| {
                    if ui
                        .button("Excel")
                        .on_hover_text(
                            "Write the whole report as an Excel file next to the project \
                             (ignores the filter and sort)",
                        )
                        .clicked()
                    {
                        export = Some(ReportFormat::Excel);
                        ui.close();
                    }
                    if ui
                        .button("CSV")
                        .on_hover_text(
                            "Write the whole report as CSV (ignores the filter and sort)",
                        )
                        .clicked()
                    {
                        export = Some(ReportFormat::Csv);
                        ui.close();
                    }
                    if ui
                        .button("PDF")
                        .on_hover_text(
                            "Write the whole report as a PDF, through the template picked \
                             next to Export (design one in Reports > Report designer)",
                        )
                        .clicked()
                    {
                        export = Some(ReportFormat::Pdf);
                        ui.close();
                    }
                });
                self.template_picker(ui);
                ui.add(
                    egui::TextEdit::singleline(&mut self.reports_view.filter)
                        .id(filter_id)
                        .hint_text("Filter rows (Ctrl+F)")
                        .desired_width(200.0),
                );
            });
        });
        ui.separator();
        let lit = self.lit_sources();
        let clicked = egui::ScrollArea::both()
            .auto_shrink(false)
            .show(ui, |ui| {
                let mut jump = None;
                for (section, order) in output.sections.iter().zip(&orders) {
                    if let Some(source) = self.section_table(ui, section, order, &lit) {
                        jump = Some(source);
                    }
                    ui.add_space(12.0);
                }
                if output.sections.iter().all(|s| s.rows.is_empty()) {
                    ui.weak("Nothing to list yet");
                }
                jump
            })
            .inner;
        self.reports_view.reveal = false;
        jump = jump.or(clicked);
        if let Some(format) = export {
            self.export_report(&id, format);
        }
        if let Some(source) = jump {
            self.jump_to_source(source);
        }
    }

    /// The devices the drawing has selected, as a row's sources name them.
    fn lit_sources(&self) -> Vec<AnyId> {
        let Some(doc) = self.backend.document() else {
            return Vec::new();
        };
        let project = doc.project();
        let mut lit: Vec<AnyId> = self.selection();
        let devices: Vec<AnyId> = lit
            .iter()
            .filter_map(|id| novo_editor::edit::device_of(project, *id))
            .map(AnyId::Device)
            .collect();
        lit.extend(devices);
        lit.extend(self.reports_view.picked);
        lit
    }

    /// Up and Down walk the rows, Enter jumps to the cursor row, Ctrl+F
    /// focuses the filter, Escape clears it (then the cursor). Returns the
    /// object to jump to. Keys typed into the filter stay there, except
    /// Down and Enter, which hand over to the table.
    fn report_keys(
        &mut self,
        ui: &mut egui::Ui,
        output: &ReportOutput,
        orders: &[Vec<usize>],
        filter_id: egui::Id,
    ) -> Option<AnyId> {
        use egui::{Key, Modifiers};
        let typing = ui.memory(|m| m.has_focus(filter_id));
        if !typing && (ui.ctx().text_edit_focused() || self.modal_open()) {
            return None;
        }
        let (find, up, down, enter, escape) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::COMMAND, Key::F),
                !typing && i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if find {
            ui.memory_mut(|m| m.request_focus(filter_id));
            return None;
        }
        if escape {
            if typing || !self.reports_view.filter.is_empty() {
                self.reports_view.filter.clear();
                ui.memory_mut(|m| m.surrender_focus(filter_id));
            } else {
                self.reports_view.cursor = None;
                self.reports_view.picked = None;
            }
            return None;
        }
        if typing && (down || enter) {
            ui.memory_mut(|m| m.surrender_focus(filter_id));
        } else if typing {
            return None;
        }
        // Every visible row, in the order they are drawn.
        let flat: Vec<(usize, usize)> = orders
            .iter()
            .enumerate()
            .flat_map(|(s, order)| order.iter().map(move |&row| (s, row)))
            .collect();
        let at = self.reports_view.cursor.as_ref().and_then(|(key, row)| {
            flat.iter()
                .position(|&(s, r)| output.sections[s].key == *key && r == *row)
        });
        if enter && !typing {
            let (s, r) = flat.get(at?)?;
            return output.sections[*s].rows[*r].sources.first().copied();
        }
        let next = match (at, up, down) {
            (Some(i), true, _) => i.saturating_sub(1),
            (Some(i), _, true) => (i + 1).min(flat.len().saturating_sub(1)),
            (None, _, true) => 0,
            (None, true, _) => flat.len().saturating_sub(1),
            _ => return None,
        };
        let &(s, r) = flat.get(next)?;
        let section = &output.sections[s];
        self.reports_view.cursor = Some((section.key.clone(), r));
        self.reports_view.reveal = true;
        self.pick_source(section.rows[r].sources.first().copied());
        None
    }

    /// The tag of the device the clicked row describes, for the status bar.
    pub(super) fn reports_picked(&self) -> Option<String> {
        let project = self.backend.document()?.project();
        let device = novo_editor::edit::device_of(project, self.reports_view.picked?)?;
        Some(project.device(device)?.tag.to_string())
    }

    /// Selects what a row describes: the next time the drawing shows, its
    /// page is open and the symbol selected.
    fn pick_source(&mut self, source: Option<AnyId>) {
        self.reports_view.picked = source;
        let place = match source {
            Some(AnyId::Device(device)) => self.device_main(device),
            Some(AnyId::Symbol(symbol)) => self
                .backend
                .document()
                .and_then(|d| d.project().symbol(symbol))
                .map(|s| Place {
                    page: s.page,
                    symbol: Some(symbol),
                }),
            _ => None,
        };
        if place.is_some() {
            self.wb.pending = place;
        }
    }

    /// One section as a table. Returns the first model object of a row the
    /// person double-clicked.
    fn section_table(
        &mut self,
        ui: &mut egui::Ui,
        section: &Section,
        order: &[usize],
        lit: &[AnyId],
    ) -> Option<AnyId> {
        if !section.title.is_empty() {
            section_label(ui, &section.title);
        }
        let widths = column_widths(
            ui.available_width(),
            &section.columns.iter().map(|c| c.width).collect::<Vec<_>>(),
        );
        let columns: Vec<TableColumn> = section
            .columns
            .iter()
            .map(|c| TableColumn {
                header: c.header.clone(),
                align: match c.align {
                    Align::Left => TableAlign::Left,
                    Align::Center => TableAlign::Center,
                    Align::Right => TableAlign::Right,
                },
            })
            .collect();
        let sorted = match &self.reports_view.sort {
            Some((key, col, asc)) if *key == section.key => Some((*col, *asc)),
            _ => None,
        };
        if let Some(i) = table_header(ui, &columns, &widths, sorted) {
            self.reports_view.sort = match self.reports_view.sort.take() {
                Some((key, col, asc)) if key == section.key && col == i => {
                    asc.then_some((key, col, false))
                }
                _ => Some((section.key.clone(), i, true)),
            };
        }

        let mut jump = None;
        let mut area = egui::ScrollArea::vertical()
            .id_salt(("report_rows", &section.key))
            .auto_shrink([false, true])
            .max_height(ui.available_height().max(240.0));
        if self.reports_view.reveal
            && let Some((key, row)) = &self.reports_view.cursor
            && *key == section.key
            && let Some(pos) = order.iter().position(|r| r == row)
        {
            let step = ROW_HEIGHT + ui.spacing().item_spacing.y;
            area = area.vertical_scroll_offset((pos as f32 - 4.0).max(0.0) * step);
        }
        area.show_rows(ui, ROW_HEIGHT, order.len(), |ui, range| {
            for &index in &order[range] {
                let row = &section.rows[index];
                let texts: Vec<String> = row.cells.iter().map(|c| c.to_string()).collect();
                let cells: Vec<TableCell> = row
                    .cells
                    .iter()
                    .zip(&texts)
                    .map(|(c, text)| TableCell {
                        text,
                        numeric: matches!(c, Cell::Int(_) | Cell::Number(_)),
                    })
                    .collect();
                let at_cursor = self
                    .reports_view
                    .cursor
                    .as_ref()
                    .is_some_and(|(key, r)| *key == section.key && *r == index);
                let state = if at_cursor || row.sources.iter().any(|s| lit.contains(s)) {
                    RowState::Selected
                } else {
                    RowState::Normal
                };
                let response = table_row(
                    ui,
                    &columns,
                    &widths,
                    &cells,
                    state,
                    ("report_row", &section.key, index),
                );
                if response.clicked() {
                    self.reports_view.cursor = Some((section.key.clone(), index));
                    self.pick_source(row.sources.first().copied());
                }
                if response.double_clicked()
                    && let Some(source) = row.sources.first()
                {
                    jump = Some(*source);
                }
            }
        });
        jump
    }

    /// The rows of a section that pass the filter, in the chosen order.
    pub(super) fn row_order(&self, section: &Section) -> Vec<usize> {
        let filter = self.reports_view.filter.trim().to_lowercase();
        let mut order: Vec<usize> = (0..section.rows.len())
            .filter(|&i| {
                filter.is_empty()
                    || section.rows[i]
                        .cells
                        .iter()
                        .any(|c| c.to_string().to_lowercase().contains(&filter))
            })
            .collect();
        if let Some((key, col, asc)) = &self.reports_view.sort
            && *key == section.key
            && *col < section.columns.len()
        {
            order.sort_by(|&a, &b| {
                let ord = compare_cells(&section.rows[a].cells[*col], &section.rows[b].cells[*col]);
                if *asc { ord } else { ord.reverse() }
            });
        }
        order
    }

    /// Shows the device or symbol a report row describes on the drawing.
    fn jump_to_source(&mut self, source: AnyId) {
        let place = match source {
            AnyId::Device(device) => self.device_main(device),
            AnyId::Symbol(symbol) => self
                .backend
                .document()
                .and_then(|d| d.project().symbol(symbol))
                .map(|s| Place {
                    page: s.page,
                    symbol: Some(symbol),
                }),
            _ => None,
        };
        match place {
            Some(place) => self.jump(place, true),
            None => self.message(Level::Info, "That row is not drawn on a page"),
        }
    }

    /// Writes report `id` next to the project file (or in Documents for an
    /// untitled project), under a name that is free. Never overwrites.
    pub(crate) fn export_report(&mut self, id: &str, format: ReportFormat) -> Option<PathBuf> {
        let output = self.report_output(id)?;
        let path = self.export_path(&output.title, format)?;
        self.write_report(&[&output], &path, format)
    }

    /// Writes every report to one Excel file, a sheet each.
    pub(crate) fn export_all_reports(&mut self) -> Option<PathBuf> {
        let doc = self.backend.document()?;
        let project = doc.project();
        let outputs: Vec<ReportOutput> = self
            .wb
            .reports
            .live(project)
            .all(project)
            .into_iter()
            .cloned()
            .collect();
        let refs: Vec<&ReportOutput> = outputs.iter().collect();
        let path = self.export_path("Reports", ReportFormat::Excel)?;
        self.write_report(&refs, &path, ReportFormat::Excel)
    }

    fn write_report(
        &mut self,
        outputs: &[&ReportOutput],
        path: &Path,
        format: ReportFormat,
    ) -> Option<PathBuf> {
        let bytes = match format {
            ReportFormat::Excel => match novo_reports::xlsx_bytes(outputs) {
                Ok(bytes) => bytes,
                Err(e) => {
                    self.message(Level::Error, format!("Could not export to Excel: {e}"));
                    return None;
                }
            },
            ReportFormat::Csv => outputs
                .iter()
                .map(|o| novo_reports::to_csv(o, &CsvOptions::default()))
                .collect::<Vec<_>>()
                .join("\r\n")
                .into_bytes(),
            ReportFormat::Pdf => self.report_pdf_bytes(outputs)?,
        };
        match super::pdf::write_replacing(path, &bytes) {
            Ok(()) => {
                self.message(Level::Info, format!("Exported {}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not write {}: {e}", path.display()),
                );
                None
            }
        }
    }

    /// The template picker next to the PDF button: the report templates of
    /// the project, the user folder and the built-in ones.
    fn template_picker(&mut self, ui: &mut egui::Ui) {
        let shown = self
            .reports_view
            .template
            .clone()
            .unwrap_or_else(|| "Plain layout".to_owned());
        egui::ComboBox::from_id_salt("reports_template")
            .selected_text(shown)
            .width(150.0)
            .show_ui(ui, |ui| {
                let none = self.reports_view.template.is_none();
                if ui.selectable_label(none, "Plain layout").clicked() {
                    self.reports_view.template = None;
                }
                for t in self.report_templates() {
                    let on = self.reports_view.template.as_deref() == Some(t.name.as_str());
                    if ui.selectable_label(on, &t.name).clicked() {
                        self.reports_view.template = Some(t.name);
                    }
                }
            })
            .response
            .on_hover_text("Template the PDF is printed through");
    }

    /// The PDF of the (first) report in `outputs`.
    fn report_pdf_bytes(&mut self, outputs: &[&ReportOutput]) -> Option<Vec<u8>> {
        let output = outputs.first()?;
        let name = self.project_name().unwrap_or_default();
        let Some(chosen) = self.reports_view.template.clone() else {
            let options = novo_pdf::ReportPdfOptions {
                title: format!("{name}  {}", output.title),
                ..novo_pdf::ReportPdfOptions::default()
            };
            return Some(novo_pdf::export_report_pdf(outputs, &options));
        };
        let Some(template) = self
            .report_templates()
            .into_iter()
            .find(|t| t.name == chosen)
        else {
            self.reports_view.template = None;
            self.message(
                Level::Warning,
                format!("Template {chosen} no longer exists, using the plain layout"),
            );
            return None;
        };
        let Some(template) = super::designer::retarget(&template, &output.report, &output.title)
        else {
            self.message(
                Level::Warning,
                format!("Template {chosen} has no table to put the report in"),
            );
            return None;
        };
        let rendered = self.render_with_project(&template);
        Some(novo_pdf::export_layouts_pdf(
            &rendered.pages,
            &format!("{name}  {}", output.title),
            &[],
        ))
    }

    /// `<project> - <what>.xlsx`, or `... 2.xlsx` when that exists.
    fn export_path(&mut self, what: &str, format: ReportFormat) -> Option<PathBuf> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let dir = match file.as_deref().and_then(Path::parent) {
            Some(dir) => dir.to_owned(),
            None => match super::projects_dir() {
                Some(dir) => dir,
                None => {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                }
            },
        };
        let stem = Path::new(&super::project_file_name(&name))
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        let what = what.replace(['/', '\\', ':'], "-");
        let extension = format.extension();
        (1..)
            .map(|n| {
                if n == 1 {
                    dir.join(format!("{stem} - {what}.{extension}"))
                } else {
                    dir.join(format!("{stem} - {what} {n}.{extension}"))
                }
            })
            .find(|p| !p.exists())
    }
}

/// Numbers by value, everything else by natural text order, empty last.
fn compare_cells(a: &Cell, b: &Cell) -> Ordering {
    let number = |c: &Cell| match c {
        Cell::Int(i) => Some(*i as f64),
        Cell::Number(n) => Some(*n),
        _ => None,
    };
    match (a, b) {
        (Cell::Empty, Cell::Empty) => Ordering::Equal,
        (Cell::Empty, _) => Ordering::Greater,
        (_, Cell::Empty) => Ordering::Less,
        _ => match (number(a), number(b)) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => natural_cmp(&a.to_string(), &b.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use crate::app::gate1_tests::Drive;

    #[test]
    fn numbers_sort_by_value_and_empty_last() {
        assert_eq!(compare_cells(&Cell::Int(2), &Cell::Int(10)), Ordering::Less);
        assert_eq!(
            compare_cells(&Cell::text("K2"), &Cell::text("K10")),
            Ordering::Less
        );
        assert_eq!(
            compare_cells(&Cell::Empty, &Cell::text("a")),
            Ordering::Greater
        );
    }

    #[test]
    fn showing_a_report_opens_the_workspace_on_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.app.show_report("bom");
        d.frame();
        d.frame();
        assert_eq!(d.app.workspace(), Workspace::Reports);
        assert_eq!(d.app.report_selected().as_deref(), Some("bom"));
        // A page pick leaves it for the drawing.
        let page = d.app.current_page.expect("a page");
        d.app.show_page(page);
        assert_eq!(d.app.workspace(), Workspace::Schematic);
    }

    #[test]
    fn the_example_project_fills_every_report_table() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::OpenExample);
        d.frame();
        for (_, id) in REPORT_COMMANDS {
            d.app.show_report(id);
            // Draws the list and the tables, filtered and sorted.
            d.app.reports_view.filter = "k".into();
            d.frame();
            d.app.reports_view.filter.clear();
            d.app.reports_view.sort = Some(("parts".into(), 0, false));
            d.frame();
        }
        let output = d.app.report_output("devices").unwrap();
        assert!(output.row_count() > 0, "the example lists devices");
    }

    /// One frame with `key` pressed in the Reports workspace.
    fn press(d: &mut Drive, key: egui::Key, modifiers: egui::Modifiers) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            events: vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
            ..Default::default()
        };
        let app = &mut d.app;
        let mut out = d.ctx.run_ui(input, |ui| app.show(ui));
        out.textures_delta.clear();
    }

    #[test]
    fn keys_walk_the_rows_and_a_row_selects_its_device() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::OpenExample);
        d.frame();
        d.app.show_report("devices");
        d.frame();
        d.frame();
        let none = egui::Modifiers::NONE;
        press(&mut d, egui::Key::ArrowDown, none);
        let first = d.app.reports_view.cursor.clone().expect("a cursor");
        press(&mut d, egui::Key::ArrowDown, none);
        let second = d.app.reports_view.cursor.clone().unwrap();
        assert_ne!(first, second, "Down moves");
        press(&mut d, egui::Key::ArrowUp, none);
        assert_eq!(d.app.reports_view.cursor, Some(first));
        assert!(d.app.reports_view.picked.is_some(), "the row is picked");
        assert!(d.app.reports_picked().is_some(), "the status bar names it");
        // Opening the drawing lands on the picked device's page.
        let place = d.app.wb.pending.expect("a pending selection");
        d.app.switch_workspace(Workspace::Schematic);
        assert_eq!(d.app.current_page, Some(place.page));
        d.frame();
        d.frame();
        assert_eq!(d.app.selected_symbol(), place.symbol);
    }

    #[test]
    fn escape_clears_the_filter_then_the_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::OpenExample);
        d.frame();
        d.app.show_report("devices");
        d.frame();
        d.app.reports_view.filter = "k".into();
        let none = egui::Modifiers::NONE;
        press(&mut d, egui::Key::ArrowDown, none);
        assert!(d.app.reports_view.cursor.is_some());
        press(&mut d, egui::Key::Escape, none);
        assert!(d.app.reports_view.filter.is_empty());
        assert!(d.app.reports_view.cursor.is_some(), "first Escape: filter");
        press(&mut d, egui::Key::Escape, none);
        assert!(d.app.reports_view.cursor.is_none());
    }

    #[test]
    fn a_report_exports_to_a_free_name_beside_the_project() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        let project = dir.path().join("Plant.novo");
        assert!(d.app.name_and_save(&project));
        d.frame();
        let first = d.app.export_report("bom", ReportFormat::Excel).unwrap();
        assert_eq!(first, dir.path().join("Plant - Bill of materials.xlsx"));
        assert!(first.is_file());
        let second = d.app.export_report("bom", ReportFormat::Excel).unwrap();
        assert_eq!(second, dir.path().join("Plant - Bill of materials 2.xlsx"));
        let csv = d.app.export_report("devices", ReportFormat::Csv).unwrap();
        assert!(csv.extension().is_some_and(|e| e == "csv"));
        let all = d.app.export_all_reports().unwrap();
        assert!(all.is_file());
    }
}
