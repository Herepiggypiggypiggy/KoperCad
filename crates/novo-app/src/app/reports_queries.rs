//! "My reports": the person's own reports, written as a query or put
//! together by choosing a table, columns, filters and a sort, and kept inside
//! the project. The Reports workspace lists them like any report (rows are
//! sortable, filterable, exportable to Excel, CSV and PDF, and jump to the
//! drawing); this is the editor above their tables.
//!
//! The query layer itself is `novo_reports::query`; nothing here can change
//! the project except saving or deleting a saved report.

use eframe::egui;
use novo_reports::query::{
    self, Builder, EXAMPLES, Filter, FilterOp, Preset, Presets, QValue, QueryResult, TABLES, Table,
};
use novo_theme::widgets::help_mark;

use super::{Level, NovoApp};

/// Rows the preview under the editor shows.
const PREVIEW_ROWS: usize = 8;

const HELP: &str = "A report is a question about the project, written like this:\n\n\
SELECT tag, description FROM devices WHERE order_number IS NULL ORDER BY tag\n\n\
Or choose a table, columns and filters under Build by choosing and press Write query. \
Totals work too: count(*), sum(x), min(x), max(x), group_concat(x) with GROUP BY. \
Text compares without regard to case; K2 sorts before K10; an empty value is NULL. \
A report can only read: it never changes the project. Saved reports are kept in the \
project file and print with every export.";

/// What the editor remembers while the app runs.
#[derive(Default)]
pub(super) struct QueriesEditor {
    /// The saved report being edited; `None` is a new one.
    selected: Option<String>,
    name: String,
    sql: String,
    builder: Option<Builder>,
    /// The tables and columns for the pickers, kept per project revision.
    schema: Option<(u64, Vec<Table>)>,
    /// The answer to the text as it stands, kept while neither changes.
    preview: Option<(u64, String, Result<QueryResult, String>)>,
    /// Which saved report the fields were last loaded from, so another
    /// project does not show the old one's text.
    project_name: String,
}

enum Act {
    Save,
    Delete(String),
}

impl NovoApp {
    /// The editor above the tables of the "My reports" report.
    pub(super) fn queries_editor(&mut self, ui: &mut egui::Ui) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let revision = project.revision();
        let presets = Presets::of(project);
        let project_name = project.info().name.clone();
        let state = &mut self.reports_view.queries;
        if state.project_name != project_name {
            *state = QueriesEditor {
                project_name,
                ..QueriesEditor::default()
            };
        }
        // Keep the answer to the typed text current.
        let stale = state
            .preview
            .as_ref()
            .is_none_or(|(rev, sql, _)| *rev != revision || *sql != state.sql);
        if stale {
            let answer = if state.sql.trim().is_empty() {
                Err(String::new())
            } else {
                query::run(project, &state.sql).map_err(|e| e.describe(&state.sql))
            };
            state.preview = Some((revision, state.sql.clone(), answer));
        }
        let p = crate::theme::ui_palette();
        let mut act = None;
        let mut load: Option<Option<Preset>> = None;
        let heading = egui::RichText::new("Edit my reports").strong();
        egui::CollapsingHeader::new(heading)
            .id_salt("my_reports_editor")
            .default_open(presets.list.is_empty())
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for preset in &presets.list {
                        let on = state.selected.as_deref() == Some(preset.name.as_str());
                        if ui.selectable_label(on, &preset.name).clicked() {
                            load = Some(Some(preset.clone()));
                        }
                    }
                    if ui
                        .selectable_label(state.selected.is_none(), "+ New report")
                        .clicked()
                    {
                        load = Some(None);
                    }
                    ui.weak("read-only");
                    help_mark(ui, HELP);
                });
                ui.horizontal(|ui| {
                    ui.label("Name");
                    ui.add(
                        egui::TextEdit::singleline(&mut state.name)
                            .hint_text("Wires without a size")
                            .desired_width(240.0),
                    );
                    ui.menu_button("Start from example", |ui| {
                        for (title, sql) in EXAMPLES {
                            if ui.button(*title).clicked() {
                                if state.name.trim().is_empty() {
                                    state.name = (*title).to_owned();
                                }
                                state.sql = (*sql).to_owned();
                                ui.close();
                            }
                        }
                    });
                });
                builder_ui(ui, state, project, revision);
                ui.add(
                    egui::TextEdit::multiline(&mut state.sql)
                        .code_editor()
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .hint_text("SELECT ... FROM devices WHERE ..."),
                );
                match state.preview.as_ref().map(|(_, _, r)| r) {
                    Some(Err(e)) if !e.is_empty() => {
                        ui.colored_label(p.warning, e);
                    }
                    Some(Ok(result)) => preview_table(ui, result),
                    _ => {}
                }
                let runs = matches!(state.preview, Some((_, _, Ok(_))));
                ui.horizontal(|ui| {
                    let save = ui.add_enabled(
                        runs && !state.name.trim().is_empty(),
                        egui::Button::new("Save report"),
                    );
                    let hint = if state.name.trim().is_empty() {
                        "Give the report a name first"
                    } else if !runs {
                        "Fix the query first"
                    } else {
                        "Keep this report in the project; it prints with every export"
                    };
                    if save.on_hover_text(hint).clicked() {
                        act = Some(Act::Save);
                    }
                    if let Some(name) = state.selected.clone()
                        && ui
                            .button("Delete")
                            .on_hover_text("Remove this report from the project")
                            .clicked()
                    {
                        act = Some(Act::Delete(name));
                    }
                });
            });
        if let Some(chosen) = load {
            match chosen {
                Some(preset) => {
                    state.selected = Some(preset.name.clone());
                    state.name = preset.name;
                    state.sql = preset.sql;
                }
                None => {
                    state.selected = None;
                    state.name.clear();
                    state.sql.clear();
                }
            }
        }
        let Some(act) = act else {
            return;
        };
        let (name, sql, selected) = (
            state.name.trim().to_owned(),
            state.sql.clone(),
            state.selected.clone(),
        );
        let mut presets = presets;
        match &act {
            Act::Save => {
                if let Some(old) = selected.as_deref()
                    && !old.trim().eq_ignore_ascii_case(&name)
                {
                    presets.remove(old);
                }
                if let Err(e) = presets.upsert(&name, &sql) {
                    self.message(Level::Warning, e);
                    return;
                }
            }
            Act::Delete(name) => {
                presets.remove(name);
            }
        }
        let label = match act {
            Act::Save => format!("Save report {name}"),
            Act::Delete(ref n) => format!("Delete report {n}"),
        };
        if self.edit_project(&label, |tx| presets.save(tx)) {
            match act {
                Act::Save => {
                    self.reports_view.queries.selected = Some(name.clone());
                    self.message(Level::Info, format!("Saved report {name}"));
                }
                Act::Delete(n) => {
                    let q = &mut self.reports_view.queries;
                    q.selected = None;
                    q.name.clear();
                    q.sql.clear();
                    self.message(Level::Info, format!("Deleted report {n}"));
                }
            }
        }
    }
}

/// Choose a table, columns, filters and a sort; the button writes the query.
/// The body is built only while the header is open, so it costs nothing closed.
fn builder_ui(
    ui: &mut egui::Ui,
    state: &mut QueriesEditor,
    project: &novo_core::Project,
    revision: u64,
) {
    egui::CollapsingHeader::new("Build by choosing")
        .id_salt("my_reports_builder")
        .show(ui, |ui| builder_body(ui, state, project, revision));
}

fn builder_body(
    ui: &mut egui::Ui,
    state: &mut QueriesEditor,
    project: &novo_core::Project,
    revision: u64,
) {
    if state
        .schema
        .as_ref()
        .is_none_or(|(rev, _)| *rev != revision)
    {
        state.schema = Some((revision, query::schema(project)));
    }
    let builder = state.builder.get_or_insert_with(|| Builder::new("devices"));
    let Some((_, schema)) = &state.schema else {
        return;
    };
    let Some(table) = schema.iter().find(|t| t.name == builder.table) else {
        builder.table = TABLES[0].0.to_owned();
        return;
    };
    let columns: Vec<(String, String)> = table
        .columns
        .iter()
        .map(|c| (c.name.clone(), c.header.clone()))
        .collect();
    let mut write = false;
    ui.indent("my_reports_builder_body", |ui| {
        ui.horizontal(|ui| {
            ui.label("Table");
            egui::ComboBox::from_id_salt("my_reports_table")
                .selected_text(builder.table.clone())
                .show_ui(ui, |ui| {
                    for (name, about) in TABLES {
                        if ui
                            .selectable_value(&mut builder.table, (*name).to_owned(), *name)
                            .on_hover_text(*about)
                            .changed()
                        {
                            builder.columns.clear();
                            builder.filters.clear();
                            builder.sort.clear();
                        }
                    }
                });
            ui.menu_button(
                if builder.columns.is_empty() {
                    "Columns: all".to_owned()
                } else {
                    format!("Columns: {}", builder.columns.len())
                },
                |ui| {
                    for (name, header) in &columns {
                        let mut on = builder.columns.contains(name);
                        if ui.checkbox(&mut on, header).changed() {
                            if on {
                                builder.columns.push(name.clone());
                            } else {
                                builder.columns.retain(|c| c != name);
                            }
                        }
                    }
                },
            );
        });
        let mut remove = None;
        for (i, filter) in builder.filters.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                column_combo(ui, ("my_reports_fcol", i), &columns, &mut filter.column);
                egui::ComboBox::from_id_salt(("my_reports_fop", i))
                    .selected_text(filter.op.label())
                    .show_ui(ui, |ui| {
                        for op in FilterOp::ALL {
                            ui.selectable_value(&mut filter.op, op, op.label());
                        }
                    });
                if filter.op.takes_value() {
                    ui.add(egui::TextEdit::singleline(&mut filter.value).desired_width(140.0));
                }
                if ui
                    .small_button("x")
                    .on_hover_text("Remove this filter")
                    .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            builder.filters.remove(i);
        }
        ui.horizontal(|ui| {
            if ui.button("+ Filter").clicked() {
                builder.filters.push(Filter {
                    column: columns[0].0.clone(),
                    op: FilterOp::Equals,
                    value: String::new(),
                });
            }
            if builder.filters.len() > 1 {
                ui.checkbox(&mut builder.match_all, "Every filter must match");
            }
            let mut sorted = builder.sort.first().cloned();
            let before = sorted.clone();
            let label = sorted
                .as_ref()
                .map_or("Sort: none".to_owned(), |(c, _)| format!("Sort: {c}"));
            ui.menu_button(label, |ui| {
                if ui.button("None").clicked() {
                    sorted = None;
                    ui.close();
                }
                for (name, header) in &columns {
                    if ui.button(header).clicked() {
                        sorted = Some((name.clone(), sorted.as_ref().is_some_and(|s| s.1)));
                        ui.close();
                    }
                }
            });
            if let Some((_, descending)) = &mut sorted {
                ui.checkbox(descending, "Descending");
            }
            if sorted != before {
                builder.sort = sorted.into_iter().collect();
            }
            if ui
                .button("Write query")
                .on_hover_text("Replace the text below with a query for these choices")
                .clicked()
            {
                write = true;
            }
        });
    });
    if write {
        state.sql = builder.to_sql();
    }
}

fn column_combo(
    ui: &mut egui::Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    columns: &[(String, String)],
    value: &mut String,
) {
    if !columns.iter().any(|(n, _)| n == value)
        && let Some((first, _)) = columns.first()
    {
        value.clone_from(first);
    }
    let shown = columns
        .iter()
        .find(|(n, _)| n == value)
        .map_or(value.clone(), |(_, h)| h.clone());
    egui::ComboBox::from_id_salt(id)
        .selected_text(shown)
        .show_ui(ui, |ui| {
            for (name, header) in columns {
                ui.selectable_value(value, name.clone(), header);
            }
        });
}

/// The first rows of the answer to the typed query.
fn preview_table(ui: &mut egui::Ui, result: &QueryResult) {
    ui.weak(format!(
        "{} - first {} shown",
        novo_reports::count_text(result.rows.len(), "rows"),
        result.rows.len().min(PREVIEW_ROWS)
    ));
    egui::ScrollArea::horizontal()
        .id_salt("my_reports_preview")
        .show(ui, |ui| {
            egui::Grid::new("my_reports_preview_grid")
                .striped(true)
                .show(ui, |ui| {
                    for c in &result.columns {
                        ui.strong(&c.header);
                    }
                    ui.end_row();
                    for row in result.rows.iter().take(PREVIEW_ROWS) {
                        for v in row {
                            ui.label(match v {
                                QValue::Null => String::new(),
                                other => other.display(),
                            });
                        }
                        ui.end_row();
                    }
                });
        });
}
