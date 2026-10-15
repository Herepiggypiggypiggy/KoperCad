//! The Reports hub: every report as a live table beside the
//! drawing, with its exports in one place. It is the third view next to
//! Schematic and Cabinet, so a wire list, a cut list or a cable schedule is
//! one key away and never a page to hunt for in the outline.
//!
//! The hub reads the same [`novo_reports::LiveReports`] the generated pages
//! do, so what it shows is what the page and the PDF print. A click on a
//! row goes to the drawing object the row describes. Excel and CSV exports
//! ask where to put the file, so a spreadsheet the user keeps editing is
//! never replaced behind their back.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_core::{AnyId, PageId, PageKind, Project};
use novo_reports::{Align, Cell, CsvOptions, ReportOutput, Section};
use novo_theme::tokens;
use novo_xref::Place;

use super::{Level, NovoApp};

/// Narrowest a table column is drawn, in points.
const MIN_COLUMN: f32 = 84.0;

/// A file format a report can be written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    Xlsx,
    Csv,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Csv => "csv",
        }
    }

    fn filter(self) -> &'static str {
        match self {
            Format::Xlsx => "Excel workbook",
            Format::Csv => "CSV (comma or semicolon separated)",
        }
    }
}

/// The hub's own state: which report, which section of it, the row filter.
#[derive(Default)]
pub(crate) struct Hub {
    pub(super) open: bool,
    /// The id of the report shown; the first one when empty or unknown.
    pub(super) selected: String,
    /// The section shown, for a report with several (a strip, a cable).
    section: usize,
    /// Rows must contain this text, in any cell.
    pub(super) filter: String,
}

/// What a click in the hub asks for, done once the report borrow ends.
enum HubAsk {
    Select(String),
    Section(usize),
    Go(AnyId),
    ShowPage(PageId),
    AddPage(String, String),
    Export(Option<String>, Format),
    Semicolon(bool),
}

/// One report in the list on the left.
struct ListRow {
    id: String,
    title: String,
    rows: usize,
    page: Option<PageId>,
}

impl NovoApp {
    /// The hub, filling the window beside the top bar.
    pub(super) fn hub_show(&mut self, ui: &mut egui::Ui) {
        let semicolon = self.settings.csv_semicolon;
        // Esc goes back to the drawing, unless a text field has the keys.
        if ui.input(|i| i.key_pressed(egui::Key::Escape))
            && ui.ctx().memory(|m| m.focused().is_none())
        {
            self.hub.open = false;
            return;
        }
        let mut asks: Vec<HubAsk> = Vec::new();
        {
            let Some(doc) = self.backend.document() else {
                return;
            };
            let project = doc.project();
            let live = self.wb.reports.live(project);
            let ids: Vec<String> = live.ids().iter().map(|s| (*s).to_owned()).collect();
            let outputs = live.all(project);
            let list: Vec<ListRow> = outputs
                .iter()
                .map(|o| ListRow {
                    id: o.report.clone(),
                    title: o.title.clone(),
                    rows: o.row_count(),
                    page: generated_page(project, &o.report),
                })
                .collect();
            let current = ids
                .iter()
                .position(|id| *id == self.hub.selected)
                .unwrap_or(0);
            let shown = outputs.get(current).copied();
            egui::Frame::new()
                .fill(tokens::BG_0)
                .inner_margin(egui::Margin::same(16))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            egui::vec2(260.0, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| report_list(ui, &list, current, &mut asks),
                        );
                        ui.add_space(8.0);
                        ui.vertical(|ui| match shown {
                            Some(output) => preview(
                                ui,
                                project,
                                output,
                                list.get(current).and_then(|r| r.page),
                                &mut self.hub,
                                semicolon,
                                &mut asks,
                            ),
                            None => {
                                ui.weak("No reports");
                            }
                        });
                    });
                });
        }
        for ask in asks {
            self.hub_ask(ask);
        }
    }

    fn hub_ask(&mut self, ask: HubAsk) {
        match ask {
            HubAsk::Select(id) => {
                self.hub.selected = id;
                self.hub.section = 0;
                self.hub.filter.clear();
            }
            HubAsk::Section(i) => self.hub.section = i,
            HubAsk::Semicolon(on) => self.settings.csv_semicolon = on,
            HubAsk::ShowPage(page) => self.show_page(page),
            HubAsk::AddPage(report, title) => self.add_report_page(&report, &title),
            HubAsk::Go(id) => {
                let place = match id {
                    AnyId::Device(device) => self.device_main(device),
                    other => self
                        .backend
                        .document()
                        .and_then(|d| place_of(d.project(), other)),
                };
                match place {
                    Some(place) => {
                        self.hub.open = false;
                        self.jump(place, true);
                    }
                    None => self.message(Level::Info, "That row is not drawn on any page"),
                }
            }
            HubAsk::Export(only, format) => {
                self.export_reports_dialog(only.as_deref(), format);
            }
        }
    }

    /// Adds a generated page for `report` after the highest section in use,
    /// one undo step, and shows it.
    pub(super) fn add_report_page(&mut self, report: &str, title: &str) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let project = doc.project();
        if project.pages().any(|p| p.frozen.is_some()) {
            self.message(
                Level::Info,
                "The project has frozen pages; no page was added",
            );
            return;
        }
        // Letters are ignored on purpose: `20a` counts as section 20.
        let highest = project
            .pages()
            .filter_map(|p| {
                let digits: String = p.name.chars().take_while(char::is_ascii_digit).collect();
                digits.parse::<u64>().ok()
            })
            .max()
            .unwrap_or(0);
        let spec = novo_reports::GeneratedPageSpec {
            generator: report.to_owned(),
            name: ((highest / 10 + 1) * 10).to_string(),
            title: title.to_owned(),
        };
        match doc.transact("Add report page", |tx| {
            novo_reports::add_generated_pages(tx, std::slice::from_ref(&spec))
        }) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Added page {} ({title})", spec.name));
            }
            Err(e) => self.message(Level::Error, format!("Could not add the page: {e}")),
        }
    }

    /// Asks where to write one report (`only`) or every report, then
    /// writes it. CSV holds one report, so it needs `only`.
    pub(super) fn export_reports_dialog(&mut self, only: Option<&str>, format: Format) {
        let Some(name) = self.project_name() else {
            return;
        };
        let title = match only {
            Some(id) => self.report_title(id).unwrap_or_else(|| id.to_owned()),
            None => "Reports".to_owned(),
        };
        let file = format!("{} {}.{}", name, title, format.extension());
        let mut dialog = rfd::FileDialog::new()
            .set_title("Export")
            .set_file_name(sanitize(&file))
            .add_filter(format.filter(), &[format.extension()]);
        let dir = self
            .backend
            .file()
            .filter(|_| !self.backend.is_untitled())
            .and_then(|f| f.parent().map(Path::to_owned))
            .or_else(super::projects_dir)
            .filter(|dir| dir.is_dir());
        if let Some(dir) = dir {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.save_file() {
            self.export_reports_to(only, format, &path);
        }
    }

    fn report_title(&mut self, id: &str) -> Option<String> {
        let project = self.backend.document()?.project();
        self.wb.reports.live(project).report(id).map(|r| r.title())
    }

    /// Writes one report (`only`) or every report to `path`. Excel gets
    /// one sheet per report; CSV is a single report.
    pub(super) fn export_reports_to(
        &mut self,
        only: Option<&str>,
        format: Format,
        path: &Path,
    ) -> Option<PathBuf> {
        let options = CsvOptions {
            delimiter: if self.settings.csv_semicolon {
                ';'
            } else {
                ','
            },
            ..Default::default()
        };
        let doc = self.backend.document()?;
        let project = doc.project();
        let live = self.wb.reports.live(project);
        let outputs: Vec<&ReportOutput> = match only {
            Some(id) => live.get(project, id).into_iter().collect(),
            None => live.all(project),
        };
        let bytes = match (format, outputs.as_slice()) {
            (_, []) => {
                self.message(Level::Error, "There is no such report to export");
                return None;
            }
            (Format::Xlsx, all) => novo_reports::xlsx_bytes(all).map_err(|e| e.to_string()),
            (Format::Csv, [one]) => Ok(novo_reports::to_csv(one, &options).into_bytes()),
            (Format::Csv, _) => {
                Err("CSV holds one report; pick a report, or export Excel".to_owned())
            }
        };
        let open_file = self.backend.file();
        let written = bytes.and_then(|bytes| {
            if is_project_target(path, open_file.as_deref()) {
                return Err("an export is never written over a project file".to_owned());
            }
            super::pdf::write_replacing(path, &bytes).map_err(|e| e.to_string())
        });
        match written {
            Ok(()) => {
                self.message(Level::Info, format!("Exported {}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export: {e}"));
                None
            }
        }
    }
}

/// The page that shows this report in the drawing set, if the project has one.
fn generated_page(project: &Project, report: &str) -> Option<PageId> {
    project
        .pages()
        .find(|p| matches!(&p.kind, PageKind::Generated { generator } if generator == report))
        .map(|p| p.id)
}

/// Where a report row's object is drawn: a symbol's page and the symbol, a
/// wire's page, or a page itself.
pub(super) fn place_of(project: &Project, id: AnyId) -> Option<Place> {
    match id {
        AnyId::Symbol(s) => Some(Place {
            page: project.symbol(s)?.page,
            symbol: Some(s),
        }),
        AnyId::Wire(w) => Some(Place {
            page: project.wire(w)?.page,
            symbol: None,
        }),
        AnyId::Page(page) => Some(Place { page, symbol: None }),
        _ => None,
    }
}

/// The first row source that can be shown: the place a click goes to.
fn target_of(project: &Project, sources: &[AnyId]) -> Option<AnyId> {
    sources
        .iter()
        .copied()
        .find(|id| matches!(id, AnyId::Device(_)) || place_of(project, *id).is_some())
}

/// Whether `path` is the open project's file, a project or recovery file by
/// name, or an existing SQLite database (every project file is one). An
/// export must never replace any of them.
fn is_project_target(path: &Path, open: Option<&Path>) -> bool {
    use std::io::Read;
    let same = open.is_some_and(
        |open| matches!((open.canonicalize(), path.canonicalize()), (Ok(a), Ok(b)) if a == b),
    );
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let by_name = name.ends_with(".novo") || name.ends_with(".novo.autosave");
    let mut head = [0u8; 16];
    let by_content = std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|()| &head == b"SQLite format 3\0");
    same || by_name || by_content
}

/// File names Windows accepts.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

fn report_list(ui: &mut egui::Ui, list: &[ListRow], current: usize, asks: &mut Vec<HubAsk>) {
    novo_theme::widgets::section_label(ui, "Reports");
    egui::ScrollArea::vertical()
        .id_salt("hub_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, row) in list.iter().enumerate() {
                let selected = i == current;
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), tokens::CONTROL_HEIGHT + 4.0),
                    egui::Sense::click(),
                );
                if selected {
                    ui.painter()
                        .rect_filled(rect, 3.0, tokens::ACCENT_SELECTION);
                } else if response.hovered() {
                    ui.painter().rect_filled(rect, 3.0, tokens::BG_2);
                }
                let count = if row.rows == 0 {
                    "empty".to_owned()
                } else {
                    row.rows.to_string()
                };
                let count_galley = ui.painter().layout_no_wrap(
                    count,
                    egui::FontId::proportional(tokens::FONT_SMALL),
                    tokens::TEXT_FAINT,
                );
                let count_width = count_galley.size().x;
                ui.painter().galley(
                    egui::pos2(
                        rect.right() - 8.0 - count_width,
                        rect.center().y - count_galley.size().y / 2.0,
                    ),
                    count_galley,
                    tokens::TEXT_FAINT,
                );
                // The title is cut before the count, never over it.
                let title_area = egui::Rect::from_min_max(
                    rect.left_top(),
                    egui::pos2(rect.right() - count_width - 16.0, rect.bottom()),
                );
                ui.painter().with_clip_rect(title_area).text(
                    rect.left_center() + egui::vec2(8.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    &row.title,
                    egui::FontId::proportional(tokens::FONT_BODY),
                    if selected {
                        tokens::TEXT
                    } else {
                        tokens::TEXT_MUTED
                    },
                );
                if response.clicked() {
                    asks.push(HubAsk::Select(row.id.clone()));
                }
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn preview(
    ui: &mut egui::Ui,
    project: &Project,
    output: &ReportOutput,
    page: Option<PageId>,
    hub: &mut Hub,
    semicolon: bool,
    asks: &mut Vec<HubAsk>,
) {
    let section_count = output.sections.len();
    let section = hub.section.min(section_count.saturating_sub(1));
    ui.horizontal(|ui| {
        ui.heading(&output.title);
        ui.weak(format!(
            "{} rows{}",
            output.row_count(),
            if section_count > 1 {
                format!(" in {section_count} tables")
            } else {
                String::new()
            }
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.menu_button("Export", |ui| {
                if ui.button("This report as Excel…").clicked() {
                    asks.push(HubAsk::Export(Some(output.report.clone()), Format::Xlsx));
                    ui.close();
                }
                if ui.button("This report as CSV…").clicked() {
                    asks.push(HubAsk::Export(Some(output.report.clone()), Format::Csv));
                    ui.close();
                }
                if ui.button("All reports as one Excel workbook…").clicked() {
                    asks.push(HubAsk::Export(None, Format::Xlsx));
                    ui.close();
                }
                ui.separator();
                let mut on = semicolon;
                if ui
                    .checkbox(&mut on, "CSV with semicolons (Excel in Dutch or German)")
                    .changed()
                {
                    asks.push(HubAsk::Semicolon(on));
                }
            });
            match page {
                Some(page) => {
                    if ui
                        .button("Show page")
                        .on_hover_text("The page of this report in the drawing set")
                        .clicked()
                    {
                        asks.push(HubAsk::ShowPage(page));
                    }
                }
                None => {
                    if ui
                        .button("Add as page")
                        .on_hover_text("Put this report in the drawing set as a live page")
                        .clicked()
                    {
                        asks.push(HubAsk::AddPage(output.report.clone(), output.title.clone()));
                    }
                }
            }
        });
    });
    ui.add_space(6.0);

    if section_count > 1 {
        egui::ScrollArea::horizontal()
            .id_salt("hub_sections")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, s) in output.sections.iter().enumerate() {
                        let label = if s.title.is_empty() { &s.key } else { &s.title };
                        if ui.selectable_label(i == section, label).clicked() {
                            asks.push(HubAsk::Section(i));
                        }
                    }
                });
            });
        ui.add_space(4.0);
    }
    ui.add(
        egui::TextEdit::singleline(&mut hub.filter)
            .hint_text("Filter rows")
            .desired_width(280.0),
    );
    ui.add_space(6.0);

    let Some(table) = output.sections.get(section) else {
        ui.weak(
            "Nothing to list yet. It fills in as the drawing grows. Esc returns to the drawing.",
        );
        return;
    };
    if table.rows.is_empty() {
        ui.weak("Nothing to list yet. It fills in as the drawing grows.");
        return;
    }
    let filter = hub.filter.trim().to_lowercase();
    let rows: Vec<usize> = (0..table.rows.len())
        .filter(|&i| {
            filter.is_empty()
                || table.rows[i]
                    .cells
                    .iter()
                    .any(|c| c.to_string().to_lowercase().contains(&filter))
        })
        .collect();
    if rows.is_empty() {
        ui.weak("No row has that text.");
        return;
    }
    table_view(ui, project, table, &rows, asks);
}

fn table_view(
    ui: &mut egui::Ui,
    project: &Project,
    table: &Section,
    rows: &[usize],
    asks: &mut Vec<HubAsk>,
) {
    // Columns keep a readable width; a wide table scrolls sideways.
    let available = ui.available_width();
    let width = available.max(table.columns.len() as f32 * MIN_COLUMN);
    if width > available {
        egui::ScrollArea::horizontal()
            .id_salt("hub_wide")
            .show(ui, |ui| table_body(ui, project, table, rows, width, asks));
    } else {
        table_body(ui, project, table, rows, width, asks);
    }
}

fn table_body(
    ui: &mut egui::Ui,
    project: &Project,
    table: &Section,
    rows: &[usize],
    width: f32,
    asks: &mut Vec<HubAsk>,
) {
    let total: f64 = table.columns.iter().map(|c| c.width).sum::<f64>().max(1.0);
    let widths: Vec<f32> = table
        .columns
        .iter()
        .map(|c| ((c.width / total) as f32 * width).max(MIN_COLUMN))
        .collect();
    let height = tokens::CONTROL_HEIGHT + 2.0;
    let font = egui::FontId::proportional(tokens::FONT_BODY);

    // The header stays while the rows scroll.
    let (header, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    ui.painter().rect_filled(header, 0.0, tokens::BG_1);
    draw_cells(
        ui,
        header,
        &widths,
        table.columns.iter().map(|c| (c.header.as_str(), c.align)),
        &egui::FontId::proportional(tokens::FONT_SMALL),
        tokens::TEXT_MUTED,
    );
    ui.painter().hline(
        header.x_range(),
        header.bottom(),
        egui::Stroke::new(1.0, tokens::BORDER_STRONG),
    );

    egui::ScrollArea::vertical()
        .id_salt("hub_rows")
        .auto_shrink([false, false])
        .show_rows(ui, height, rows.len(), |ui, range| {
            for shown in range {
                let row = &table.rows[rows[shown]];
                let target = target_of(project, &row.sources);
                let sense = if target.is_some() {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                };
                let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), sense);
                if response.hovered() && target.is_some() {
                    ui.painter().rect_filled(rect, 2.0, tokens::BG_3);
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                } else if shown % 2 == 1 {
                    ui.painter().rect_filled(rect, 0.0, tokens::BG_1);
                }
                let texts: Vec<String> = row.cells.iter().map(Cell::to_string).collect();
                draw_cells(
                    ui,
                    rect,
                    &widths,
                    texts
                        .iter()
                        .zip(&table.columns)
                        .map(|(t, c)| (t.as_str(), c.align)),
                    &font,
                    tokens::TEXT,
                );
                if let Some(id) = target
                    && response.clicked()
                {
                    asks.push(HubAsk::Go(id));
                }
            }
        });
}

/// Draws one line of cells, each clipped to its own column.
fn draw_cells<'a>(
    ui: &egui::Ui,
    rect: egui::Rect,
    widths: &[f32],
    cells: impl Iterator<Item = (&'a str, Align)>,
    font: &egui::FontId,
    color: egui::Color32,
) {
    let mut x = rect.left();
    for ((text, align), width) in cells.zip(widths) {
        let cell =
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(*width, rect.height()));
        let painter = ui
            .painter()
            .with_clip_rect(cell.shrink2(egui::vec2(2.0, 0.0)));
        let (pos, anchor) = match align {
            Align::Right => (
                cell.right_center() - egui::vec2(8.0, 0.0),
                egui::Align2::RIGHT_CENTER,
            ),
            Align::Center => (cell.center(), egui::Align2::CENTER_CENTER),
            Align::Left => (
                cell.left_center() + egui::vec2(6.0, 0.0),
                egui::Align2::LEFT_CENTER,
            ),
        };
        painter.text(pos, anchor, text, font.clone(), color);
        x += width;
    }
}

#[cfg(test)]
mod target_tests {
    use super::is_project_target;

    #[test]
    fn project_files_are_never_export_targets() {
        let dir = tempfile::tempdir().unwrap();
        let open = dir.path().join("plant.dat");
        std::fs::write(&open, b"x").unwrap();
        // The open file, whatever its name.
        assert!(is_project_target(&open, Some(&open)));
        // A project extension, even if the file does not exist yet.
        assert!(is_project_target(&dir.path().join("A.NOVO"), None));
        assert!(is_project_target(&dir.path().join("a.novo.autosave"), None));
        // An existing SQLite database under another name.
        let db = dir.path().join("renamed.xlsx");
        std::fs::write(&db, b"SQLite format 3\0rest").unwrap();
        assert!(is_project_target(&db, None));
        // A fresh report file and an ordinary existing one are fine.
        assert!(!is_project_target(
            &dir.path().join("out.xlsx"),
            Some(&open)
        ));
        let plain = dir.path().join("plain.csv");
        std::fs::write(&plain, b"a;b").unwrap();
        assert!(!is_project_target(&plain, Some(&open)));
    }
}
