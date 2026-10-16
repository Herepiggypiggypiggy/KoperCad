//! The Report designer window (Reports > Report designer, Ctrl+K "report
//! designer"): pick a template, change it, check it, see it, save it.
//!
//! The engine is `novo_reports::designer`; this file is the thin UI on it:
//!
//! - the template list holds the project's templates, the user's own
//!   (`%APPDATA%\KoperCAD\templates`) and the built-in ones; "Duplicate"
//!   makes an editable copy, "New table of" starts from a report;
//! - the editor changes the name, page size and orientation, frame, title
//!   block rows, text elements and the tables (report, heading, which
//!   columns in which order with which header and width);
//! - the problem list is `designer::validate` plus what rendering found;
//! - the preview is `designer::render` on sample rows or on the project's
//!   own data;
//! - "Save to my templates" writes the file, "Save in project" stores the
//!   template in the project (one undo step), "Export PDF" writes the
//!   rendered pages next to the project.
//!
//! The Reports workspace also uses the saved templates: its template
//! picker and PDF button print the selected report through a template.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_core::{Document, PaperSize, Value};
use novo_reports::designer::{
    ColumnSpec, DesignProblem, Element, Fields, Rendered, TableError, TableSpec, Template,
    TemplateKind, TitleRow, builtin, preview_data, render,
    store::{self, Origin},
};
use novo_reports::{Align, ReportOutput, standard_reports};
use novo_theme::tokens;

use super::pdf::write_replacing;
use super::sheets::{hash, ours_or_free};
use super::{Level, NovoApp};

/// Rows of sample data in the preview, enough for a second page.
const SAMPLE_ROWS: usize = 40;

/// The window's state: the draft being edited and what it was loaded from.
pub(super) struct DesignerState {
    /// Project, user and built-in templates, in that order of priority.
    library: Vec<(Origin, Template)>,
    /// Files in the user folder that could not be read.
    unreadable: Vec<String>,
    /// The user's template folder.
    user_dir: Option<PathBuf>,
    /// The draft.
    pub(super) draft: Template,
    /// The draft as loaded or last saved; differs while there are changes.
    base: Template,
    /// Where the draft came from, if it has a name in the library.
    origin: Option<Origin>,
    /// Preview page, 0-based.
    page: usize,
    /// Preview with the project's data instead of sample rows.
    project_data: bool,
    /// The last render, kept while the draft and data are unchanged.
    cache: Option<(Template, bool, u64, Rendered)>,
    /// A pick or a new template waiting for the person to agree to drop
    /// the unsaved edits.
    pending: Option<Act>,
}

impl DesignerState {
    fn open(project: &novo_core::Project, user_dir: Option<PathBuf>) -> Self {
        let mut s = DesignerState {
            library: Vec::new(),
            unreadable: Vec::new(),
            user_dir,
            draft: builtin::report_a4("Parts list", "bom"),
            base: builtin::report_a4("Parts list", "bom"),
            origin: Some(Origin::Builtin),
            page: 0,
            project_data: false,
            cache: None,
            pending: None,
        };
        s.reload(project);
        s
    }

    /// Reads the project's templates and the user folder again.
    fn reload(&mut self, project: &novo_core::Project) {
        let project_templates = store::project_templates(project).unwrap_or_default();
        let (user, bad) = match &self.user_dir {
            Some(dir) => store::load_dir(dir),
            None => (Vec::new(), Vec::new()),
        };
        self.unreadable = bad
            .into_iter()
            .map(|(path, e)| format!("{}: {e}", path.display()))
            .collect();
        self.library = store::all(&project_templates, &user);
    }

    fn load(&mut self, origin: Origin, template: Template) {
        self.base = template.clone();
        self.draft = template;
        self.origin = Some(origin);
        self.page = 0;
    }

    fn is_dirty(&self) -> bool {
        self.draft != self.base
    }

    /// An editable copy: a new name, not in the library yet.
    fn duplicate(&mut self) {
        let mut copy = self.draft.clone();
        copy.name = format!("{} copy", copy.name);
        self.base = Template::new("", TemplateKind::Report, copy.format);
        self.draft = copy;
        self.origin = None;
    }

    /// A new report template on A4 landscape for report `id`.
    fn new_from_report(&mut self, id: &str, title: &str) {
        let t = builtin::report_a4(&format!("{title} (new)"), id);
        self.base = Template::new("", TemplateKind::Report, t.format);
        self.draft = t;
        self.origin = None;
        self.page = 0;
    }
}

/// The report ids and titles the designer can build a table from.
fn report_choices() -> Vec<(String, String)> {
    standard_reports()
        .iter()
        .map(|r| (r.id().to_owned(), r.title()))
        .collect()
}

/// The columns (key, header, relative width) of report `id`'s first section.
fn report_columns(id: &str) -> Vec<(String, String, f64)> {
    let empty = Document::default();
    standard_reports()
        .iter()
        .find(|r| r.id() == id)
        .map(|r| r.generate(empty.project()))
        .and_then(|o| o.sections.into_iter().next())
        .map(|s| {
            s.columns
                .into_iter()
                .map(|c| (c.key, c.header, c.width))
                .collect()
        })
        .unwrap_or_default()
}

/// Puts the table of a one-table report template back under the frame, over
/// the title block, after the page size changed.
fn refit(template: &mut Template) {
    let tables = template
        .elements
        .iter()
        .filter(|e| matches!(e, Element::Table(_)))
        .count();
    if template.kind != TemplateKind::Report || tables != 1 {
        return;
    }
    let (ix, iy, iw, _) = template.inner_rect();
    let block_top = template.title_block_rect().map_or(iy, |b| b.1);
    for e in &mut template.elements {
        if let Element::Table(t) = e {
            t.x = ix + 5.0;
            t.y = iy + 5.0;
            t.width = (iw - 10.0).max(10.0);
            t.height = (block_top - 4.0 - (iy + 5.0)).max(10.0);
        }
    }
}

/// `template` with every table showing report `id` (its own columns and
/// filters are dropped and the heading becomes `title` when the report
/// differs). `None` when the template
/// has no table to retarget.
pub(super) fn retarget(template: &Template, id: &str, title: &str) -> Option<Template> {
    let mut out = template.clone();
    let mut any = false;
    for e in &mut out.elements {
        if let Element::Table(t) = e {
            any = true;
            if t.spec.report != id {
                t.spec = TableSpec::new(id);
                t.spec.heading = title.to_owned();
            }
        }
    }
    any.then_some(out)
}

/// One line for the problem list.
pub(super) fn describe(p: &DesignProblem) -> String {
    match p {
        DesignProblem::EmptyName => "The template has no name".to_owned(),
        DesignProblem::UnknownField { place, field } => {
            format!("Unknown field {{{field}}} in {place}")
        }
        DesignProblem::OutsideSheet { index } => {
            format!("Element {} reaches outside the sheet or frame", index + 1)
        }
        DesignProblem::OverlapsTitleBlock { index } => {
            format!("Element {} overlaps the title block", index + 1)
        }
        DesignProblem::TablesOverlap { a, b } => {
            format!("Tables {} and {} overlap", a + 1, b + 1)
        }
        DesignProblem::TableTooSmall { index } => {
            format!("Table {} is too small for a header and one row", index + 1)
        }
        DesignProblem::Table { index, error } => {
            let what = match error {
                TableError::MissingReport(r) => format!("report {r} does not exist"),
                TableError::MissingSection { report, section } => {
                    format!("report {report} has no section {section}")
                }
                TableError::MissingColumn { report, column } => {
                    format!("report {report} has no column {column}")
                }
            };
            format!("Table {}: {what}", index + 1)
        }
    }
}

enum Act {
    Pick(usize),
    Duplicate,
    NewFrom(String, String),
    SaveUser,
    SaveProject,
    ExportPdf,
    /// Drop the edits and do the waiting pick.
    Discard,
    /// Keep editing; forget the waiting pick.
    Keep,
}

impl NovoApp {
    /// Opens the Report designer window on the first built-in template.
    pub(super) fn open_designer(&mut self) {
        self.open_designer_in(store::user_dir());
    }

    /// [`Self::open_designer`] with the user template folder given.
    pub(super) fn open_designer_in(&mut self, user_dir: Option<PathBuf>) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        self.wb.designer = Some(DesignerState::open(project, user_dir));
    }

    /// The generated reports of the project, as templates draw them.
    fn all_report_outputs(&self) -> Vec<ReportOutput> {
        match self.backend.document() {
            Some(doc) => standard_reports()
                .iter()
                .map(|r| r.generate(doc.project()))
                .collect(),
            None => Vec::new(),
        }
    }

    /// `template` rendered with the project's data and fields.
    pub(super) fn render_with_project(&self, template: &Template) -> Rendered {
        let fields = match self.backend.document() {
            Some(doc) => Fields::from_project(doc.project()),
            None => Fields::sample(),
        };
        render(template, &self.all_report_outputs(), &fields)
    }

    /// Every report template the Reports workspace can print through: the
    /// project's, the user's and the built-in ones.
    pub(super) fn report_templates(&self) -> Vec<Template> {
        let project = self
            .backend
            .document()
            .and_then(|d| store::project_templates(d.project()).ok())
            .unwrap_or_default();
        let user = store::user_dir()
            .map(|d| store::load_dir(&d).0)
            .unwrap_or_default();
        store::all(&project, &user)
            .into_iter()
            .map(|(_, t)| t)
            .filter(|t| t.kind == TemplateKind::Report)
            .collect()
    }

    /// The window. Edits go to the draft; the buttons act after drawing.
    pub(super) fn designer_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.wb.designer.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let revision = project.revision();
        // Render again only when the draft, the data choice or the project
        // changed.
        let stale = state.cache.as_ref().is_none_or(|(t, live, rev, _)| {
            *t != state.draft || *live != state.project_data || *rev != revision
        });
        if stale {
            let rendered = if state.project_data {
                self.render_with_project(&state.draft)
            } else {
                let data = preview_data(&state.draft, SAMPLE_ROWS);
                render(&state.draft, &data, &Fields::sample())
            };
            state.cache = Some((state.draft.clone(), state.project_data, revision, rendered));
        }
        let rendered = state
            .cache
            .as_ref()
            .map(|c| c.3.clone())
            .unwrap_or_default();
        state.page = state.page.min(rendered.pages.len().saturating_sub(1));

        let mut open = true;
        let mut act: Option<Act> = None;
        super::panel::dialog(ctx, "Report designer", [1040.0, 640.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(430.0);
                        egui::ScrollArea::vertical()
                            .id_salt("designer_editor")
                            .auto_shrink([false, false])
                            .show(ui, |ui| editor(ui, &mut state, &rendered, &mut act));
                    });
                    ui.separator();
                    ui.vertical(|ui| preview(ui, &mut state, &rendered));
                });
            });
        let mut state = open.then_some(state);
        if let (Some(st), Some(act)) = (state.as_mut(), act) {
            self.designer_act(st, act);
        }
        self.wb.designer = state;
    }

    fn designer_act(&mut self, st: &mut DesignerState, act: Act) {
        match act {
            Act::Pick(_) | Act::NewFrom(..) if st.is_dirty() => st.pending = Some(act),
            Act::Discard => {
                st.base = st.draft.clone();
                if let Some(next) = st.pending.take() {
                    self.designer_act(st, next);
                }
            }
            Act::Keep => st.pending = None,
            Act::Pick(i) => {
                if let Some((origin, t)) = st.library.get(i).cloned() {
                    st.load(origin, t);
                }
            }
            Act::Duplicate => st.duplicate(),
            Act::NewFrom(id, title) => st.new_from_report(&id, &title),
            Act::SaveUser => {
                let Some(dir) = st.user_dir.clone() else {
                    self.message(Level::Error, "Could not find your template folder");
                    return;
                };
                match store::save_to_dir(&dir, &st.draft) {
                    Ok(path) => {
                        self.message(Level::Info, format!("Saved template {}", path.display()));
                        st.base = st.draft.clone();
                        st.origin = Some(Origin::User);
                        if let Some(doc) = self.backend.document() {
                            st.reload(doc.project());
                        }
                    }
                    Err(e) => self.message(Level::Error, format!("Could not save template: {e}")),
                }
            }
            Act::SaveProject => {
                if self.save_template_in_project(&st.draft) {
                    st.base = st.draft.clone();
                    st.origin = Some(Origin::Project);
                    if let Some(doc) = self.backend.document() {
                        st.reload(doc.project());
                    }
                }
            }
            Act::ExportPdf => {
                self.export_designer_pdf(&st.draft.clone());
            }
        }
    }

    /// Stores `template` in the project's template list, replacing one of
    /// the same name. One undo step.
    pub(super) fn save_template_in_project(&mut self, template: &Template) -> bool {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return false;
        };
        let mut list = match store::project_templates(project) {
            Ok(l) => l,
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("The project's templates cannot be read, not overwriting them: {e}"),
                );
                return false;
            }
        };
        match list.iter_mut().find(|t| t.name == template.name) {
            Some(slot) => *slot = template.clone(),
            None => list.push(template.clone()),
        }
        let text = store::to_property(&list);
        let name = template.name.clone();
        let ok = self.cabinet_edit("Save report template", move |tx| {
            tx.set_info(|info| {
                info.properties
                    .insert(store::PROJECT_KEY.to_owned(), Value::Text(text));
            })
        });
        if ok {
            self.message(Level::Info, format!("Saved template {name} in the project"));
        }
        ok
    }

    /// Renders `template` with the project's data and writes the PDF next
    /// to the project.
    pub(super) fn export_designer_pdf(&mut self, template: &Template) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = ours_or_free(
            &dir,
            &format!("{stem} {}", template.name),
            "pdf",
            &self.wb.sheet_exports,
        );
        self.export_designer_pdf_to(template, &path)
    }

    /// [`Self::export_designer_pdf`] to `path`.
    pub fn export_designer_pdf_to(&mut self, template: &Template, path: &Path) -> Option<PathBuf> {
        let rendered = self.render_with_project(template);
        if rendered.pages.is_empty() {
            self.message(Level::Info, "The template has no pages");
            return None;
        }
        let name = self.project_name().unwrap_or_default();
        let bytes = novo_pdf::export_layouts_pdf(
            &rendered.pages,
            &format!("{name}  {}", template.name),
            &[],
        );
        let replaced = path.exists();
        match write_replacing(path, &bytes) {
            Ok(()) => {
                self.wb.sheet_exports.insert(path.to_owned(), hash(&bytes));
                let verb = if replaced { "Replaced" } else { "Exported" };
                let n = rendered.problems.len();
                let tail = if n == 0 {
                    String::new()
                } else {
                    format!(" ({n} design problems)")
                };
                self.message(Level::Info, format!("{verb} {}{tail}", path.display()));
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
}

fn origin_tag(o: Origin) -> &'static str {
    match o {
        Origin::Project => "project",
        Origin::User => "mine",
        Origin::Builtin => "built-in",
    }
}

fn editor(ui: &mut egui::Ui, st: &mut DesignerState, rendered: &Rendered, act: &mut Option<Act>) {
    // Template picker and file actions.
    ui.horizontal(|ui| {
        ui.label("Template");
        let mut label = st.draft.name.clone();
        if st.is_dirty() {
            label.push_str(" *");
        }
        egui::ComboBox::from_id_salt("designer_pick")
            .selected_text(label)
            .width(220.0)
            .show_ui(ui, |ui| {
                for (i, (origin, t)) in st.library.iter().enumerate() {
                    let selected = t.name == st.draft.name;
                    let text = format!("{}  [{}]", t.name, origin_tag(*origin));
                    if ui.selectable_label(selected, text).clicked() {
                        *act = Some(Act::Pick(i));
                    }
                }
            });
        if ui
            .button("Duplicate")
            .on_hover_text("Make an editable copy of this template under a new name")
            .clicked()
        {
            *act = Some(Act::Duplicate);
        }
    });
    ui.horizontal(|ui| {
        ui.label("New table of");
        egui::ComboBox::from_id_salt("designer_new")
            .selected_text("choose a report...")
            .show_ui(ui, |ui| {
                for (id, title) in report_choices() {
                    if ui.selectable_label(false, &title).clicked() {
                        *act = Some(Act::NewFrom(id, title));
                    }
                }
            });
    });
    if st.pending.is_some() {
        ui.horizontal(|ui| {
            ui.colored_label(
                tokens::WARNING,
                format!("Discard changes to {}?", st.draft.name),
            );
            if ui.button("Discard").clicked() {
                *act = Some(Act::Discard);
            }
            if ui.button("Keep editing").clicked() {
                *act = Some(Act::Keep);
            }
        });
    }
    if let Some(o) = st.origin {
        ui.weak(match o {
            Origin::Builtin => "Built-in: Duplicate it, or save it under another name",
            Origin::User => "In your template folder",
            Origin::Project => "Stored in this project",
        });
    }
    for line in &st.unreadable {
        ui.colored_label(tokens::WARNING, format!("Unreadable: {line}"));
    }
    ui.horizontal(|ui| {
        if ui
            .button("Save to my templates")
            .on_hover_text("Write the template to your template folder for every project")
            .clicked()
        {
            *act = Some(Act::SaveUser);
        }
        if ui
            .button("Save in project")
            .on_hover_text("Store the template in this project file (one undo step)")
            .clicked()
        {
            *act = Some(Act::SaveProject);
        }
        if ui
            .button("Export PDF")
            .on_hover_text("Print the template with this project's data next to the project file")
            .clicked()
        {
            *act = Some(Act::ExportPdf);
        }
    });
    ui.separator();

    ui.strong("Template");
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.text_edit_singleline(&mut st.draft.name);
    });
    ui.horizontal(|ui| {
        ui.label("Description");
        ui.text_edit_singleline(&mut st.draft.description);
    });
    page_fields(ui, &mut st.draft);
    frame_fields(ui, &mut st.draft);
    title_block_fields(ui, &mut st.draft);
    ui.separator();
    ui.strong("Elements");
    element_fields(ui, &mut st.draft);
    ui.separator();
    ui.strong(format!("Problems ({})", rendered.problems.len()));
    if rendered.problems.is_empty() {
        ui.weak("None");
    }
    for p in &rendered.problems {
        ui.colored_label(tokens::WARNING, describe(p));
    }
    ui.collapsing("Fields you can use in text", |ui| {
        let mut names: Vec<String> = Fields::sample().names().map(str::to_owned).collect();
        names.push("page".to_owned());
        names.push("pages".to_owned());
        names.sort();
        ui.weak(
            names
                .iter()
                .map(|n| format!("{{{n}}}"))
                .collect::<Vec<_>>()
                .join("  "),
        );
    });
}

fn page_fields(ui: &mut egui::Ui, t: &mut Template) {
    let before = t.format;
    ui.horizontal(|ui| {
        ui.label("Paper");
        for size in [PaperSize::A4, PaperSize::A3] {
            ui.selectable_value(&mut t.format.size, size, format!("{size:?}"));
        }
        ui.checkbox(&mut t.format.landscape, "Landscape");
    });
    if t.format != before {
        refit(t);
    }
}

fn frame_fields(ui: &mut egui::Ui, t: &mut Template) {
    let mut on = t.frame.is_some();
    let mut changed = false;
    ui.horizontal(|ui| {
        if ui.checkbox(&mut on, "Frame").changed() {
            t.frame = on.then(novo_reports::designer::Frame::default);
            changed = true;
        }
        if let Some(f) = &mut t.frame {
            ui.label("margin");
            let r = ui.add(
                egui::DragValue::new(&mut f.margin_mm)
                    .range(0.0..=40.0)
                    .suffix(" mm"),
            );
            changed |= r.changed();
            ui.checkbox(&mut f.column_numbers, "Column numbers");
        }
    });
    if changed {
        refit(t);
    }
}

fn title_block_fields(ui: &mut egui::Ui, t: &mut Template) {
    let mut on = t.title_block.is_some();
    let mut changed = false;
    if ui.checkbox(&mut on, "Title block").changed() {
        t.title_block = on.then(novo_reports::designer::TitleBlock::default);
        changed = true;
    }
    if let Some(block) = &mut t.title_block {
        let mut remove = None;
        egui::Grid::new("designer_title_block")
            .num_columns(3)
            .show(ui, |ui| {
                for (i, row) in block.rows.iter_mut().enumerate() {
                    ui.add(egui::TextEdit::singleline(&mut row.caption).desired_width(90.0));
                    ui.add(egui::TextEdit::singleline(&mut row.value).desired_width(230.0));
                    if ui
                        .small_button("x")
                        .on_hover_text("Remove this row")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
        if let Some(i) = remove {
            block.rows.remove(i);
            changed = true;
        }
        ui.horizontal(|ui| {
            if ui.button("Add row").clicked() {
                block.rows.push(TitleRow::new("Caption", "{project.name}"));
                changed = true;
            }
            ui.label("width");
            let r = ui.add(
                egui::DragValue::new(&mut block.width_mm)
                    .range(60.0..=250.0)
                    .suffix(" mm"),
            );
            changed |= r.changed();
        });
    }
    if changed {
        refit(t);
    }
}

fn element_fields(ui: &mut egui::Ui, t: &mut Template) {
    let mut remove = None;
    for (i, e) in t.elements.iter_mut().enumerate() {
        let title = match e {
            Element::Text { text, .. } => format!("{}. Text: {}", i + 1, text),
            Element::Line { .. } => format!("{}. Line", i + 1),
            Element::Rect { .. } => format!("{}. Rectangle", i + 1),
            Element::Table(t) => format!("{}. Table of {}", i + 1, t.spec.report),
        };
        egui::CollapsingHeader::new(title)
            .id_salt(("designer_element", i))
            .default_open(matches!(e, Element::Table(_)))
            .show(ui, |ui| {
                match e {
                    Element::Text {
                        x,
                        y,
                        width,
                        height,
                        text,
                        text_mm,
                        bold,
                        align,
                        every_page,
                        ..
                    } => {
                        ui.text_edit_multiline(text);
                        ui.horizontal(|ui| geometry(ui, x, y, width, height));
                        ui.horizontal(|ui| {
                            ui.label("size");
                            ui.add(
                                egui::DragValue::new(text_mm)
                                    .range(1.0..=30.0)
                                    .suffix(" mm"),
                            );
                            ui.checkbox(bold, "Bold");
                            ui.checkbox(every_page, "Every page");
                            for (a, name) in [
                                (Align::Left, "L"),
                                (Align::Center, "C"),
                                (Align::Right, "R"),
                            ] {
                                ui.selectable_value(align, a, name);
                            }
                        });
                    }
                    Element::Line {
                        x1,
                        y1,
                        x2,
                        y2,
                        heavy,
                        ..
                    } => {
                        ui.horizontal(|ui| {
                            for v in [x1, y1, x2, y2] {
                                ui.add(egui::DragValue::new(v).suffix(" mm"));
                            }
                            ui.checkbox(heavy, "Heavy");
                        });
                    }
                    Element::Rect {
                        x,
                        y,
                        width,
                        height,
                        heavy,
                        ..
                    } => {
                        ui.horizontal(|ui| {
                            geometry(ui, x, y, width, height);
                            ui.checkbox(heavy, "Heavy");
                        });
                    }
                    Element::Table(table) => {
                        ui.horizontal(|ui| {
                            geometry(
                                ui,
                                &mut table.x,
                                &mut table.y,
                                &mut table.width,
                                &mut table.height,
                            );
                        });
                        table_fields(ui, &mut table.spec);
                    }
                }
                if ui.small_button("Remove element").clicked() {
                    remove = Some(i);
                }
            });
    }
    if let Some(i) = remove {
        t.elements.remove(i);
    }
    if ui.button("Add text").clicked() {
        let (x, y, w, _) = t.inner_rect();
        t.elements.push(Element::text(
            x + 5.0,
            y + 5.0,
            (w - 10.0).max(20.0),
            8.0,
            "Text",
            4.0,
        ));
    }
}

fn geometry(ui: &mut egui::Ui, x: &mut f64, y: &mut f64, w: &mut f64, h: &mut f64) {
    for (name, v) in [("x", x), ("y", y), ("w", w), ("h", h)] {
        ui.label(name);
        ui.add(egui::DragValue::new(v).range(0.0..=1200.0).speed(0.5));
    }
}

fn table_fields(ui: &mut egui::Ui, spec: &mut TableSpec) {
    let reports = report_choices();
    ui.horizontal(|ui| {
        ui.label("Report");
        let shown = reports
            .iter()
            .find(|(id, _)| *id == spec.report)
            .map_or(spec.report.clone(), |(_, t)| t.clone());
        egui::ComboBox::from_id_salt("designer_report")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (id, title) in &reports {
                    if ui.selectable_label(spec.report == *id, title).clicked()
                        && spec.report != *id
                    {
                        let heading = std::mem::take(&mut spec.heading);
                        *spec = TableSpec::new(id);
                        spec.heading = heading;
                    }
                }
            });
    });
    ui.horizontal(|ui| {
        ui.label("Heading");
        ui.text_edit_singleline(&mut spec.heading);
    });
    ui.horizontal(|ui| {
        ui.label("text");
        ui.add(
            egui::DragValue::new(&mut spec.text_mm)
                .range(1.0..=10.0)
                .suffix(" mm"),
        );
        ui.label("row");
        ui.add(
            egui::DragValue::new(&mut spec.row_mm)
                .range(2.0..=30.0)
                .suffix(" mm"),
        );
        ui.label("header");
        ui.add(
            egui::DragValue::new(&mut spec.header_mm)
                .range(2.0..=30.0)
                .suffix(" mm"),
        );
    });
    let available = report_columns(&spec.report);
    ui.label("Columns");
    if spec.columns.is_empty() {
        ui.weak("Every column of the report");
        if ui.button("Choose columns").clicked() {
            spec.columns = available
                .iter()
                .map(|(k, _, _)| ColumnSpec::new(k))
                .collect();
        }
        return;
    }
    let (mut swap, mut remove) = (None, None);
    let count = spec.columns.len();
    egui::Grid::new(("designer_columns", &spec.report))
        .num_columns(5)
        .show(ui, |ui| {
            for (i, c) in spec.columns.iter_mut().enumerate() {
                let default = available.iter().find(|(k, _, _)| *k == c.key);
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(i > 0, egui::Button::new("^").small())
                        .on_hover_text("Move left")
                        .clicked()
                    {
                        swap = Some(i);
                    }
                    if ui
                        .add_enabled(i + 1 < count, egui::Button::new("v").small())
                        .on_hover_text("Move right")
                        .clicked()
                    {
                        swap = Some(i + 1);
                    }
                });
                ui.label(&c.key);
                let mut header = c.header.clone().unwrap_or_default();
                let hint = default.map_or("", |(_, h, _)| h.as_str());
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut header)
                            .hint_text(hint)
                            .desired_width(110.0),
                    )
                    .changed()
                {
                    c.header = (!header.is_empty()).then_some(header);
                }
                let mut width = c.width.unwrap_or(default.map_or(10.0, |d| d.2));
                if ui
                    .add(
                        egui::DragValue::new(&mut width)
                            .range(1.0..=200.0)
                            .speed(0.5),
                    )
                    .on_hover_text("Relative width")
                    .changed()
                {
                    c.width = Some(width);
                }
                if ui
                    .small_button("x")
                    .on_hover_text("Leave this column out")
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = swap
        && i > 0
        && i < spec.columns.len()
    {
        spec.columns.swap(i - 1, i);
    }
    if let Some(i) = remove {
        spec.columns.remove(i);
    }
    ui.horizontal(|ui| {
        let missing: Vec<_> = available
            .iter()
            .filter(|(k, _, _)| !spec.columns.iter().any(|c| c.key == *k))
            .collect();
        if !missing.is_empty() {
            egui::ComboBox::from_id_salt("designer_add_column")
                .selected_text("Add column...")
                .show_ui(ui, |ui| {
                    for (k, h, _) in missing {
                        if ui.selectable_label(false, format!("{h} ({k})")).clicked() {
                            spec.columns.push(ColumnSpec::new(k));
                        }
                    }
                });
        }
        if ui.small_button("All columns").clicked() {
            spec.columns.clear();
        }
    });
}

fn preview(ui: &mut egui::Ui, st: &mut DesignerState, rendered: &Rendered) {
    ui.horizontal(|ui| {
        ui.strong("Preview");
        let pages = rendered.pages.len().max(1);
        if ui
            .add_enabled(st.page > 0, egui::Button::new("<").small())
            .clicked()
        {
            st.page -= 1;
        }
        ui.label(format!("page {} of {pages}", st.page + 1));
        if ui
            .add_enabled(st.page + 1 < pages, egui::Button::new(">").small())
            .clicked()
        {
            st.page += 1;
        }
        ui.checkbox(&mut st.project_data, "This project's data")
            .on_hover_text("Off: sample rows, so a template can be designed on an empty project");
    });
    let Some(layout) = rendered.pages.get(st.page) else {
        return;
    };
    let avail = ui.available_size();
    let scale = (f64::from(avail.x) / layout.width)
        .min(f64::from(avail.y.max(200.0)) / layout.height)
        .max(0.5) as f32;
    let size = egui::vec2(layout.width as f32 * scale, layout.height as f32 * scale);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    // Paper is white with black ink whatever the theme.
    painter.rect_filled(rect, 0.0, egui::Color32::WHITE);
    let at = |x: f64, y: f64| rect.min + egui::vec2(x as f32 * scale, y as f32 * scale);
    for line in &layout.lines {
        let w = if line.heavy { 1.6 } else { 0.8 };
        painter.line_segment(
            [at(line.from.0, line.from.1), at(line.to.0, line.to.1)],
            egui::Stroke::new(w, egui::Color32::BLACK),
        );
    }
    for tb in &layout.texts {
        let font = egui::FontId::proportional((tb.text_mm as f32 * scale * 1.35).clamp(3.0, 60.0));
        let (anchor, x) = match tb.align {
            Align::Left => (egui::Align2::LEFT_CENTER, tb.x),
            Align::Center => (egui::Align2::CENTER_CENTER, tb.x + tb.width / 2.0),
            Align::Right => (egui::Align2::RIGHT_CENTER, tb.x + tb.width),
        };
        let clip = egui::Rect::from_min_max(at(tb.x, tb.y), at(tb.x + tb.width, tb.y + tb.height));
        painter.with_clip_rect(clip.intersect(rect)).text(
            at(x, tb.y + tb.height / 2.0),
            anchor,
            &tb.text,
            font,
            egui::Color32::BLACK,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use crate::app::gate1_tests::Drive;
    use crate::app::reports_view::ReportFormat;

    fn app(dir: &Path) -> Drive {
        let mut d = Drive::new(&dir.join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        // Saved, so exports land beside it in the temp folder.
        assert!(d.app.name_and_save(&dir.join("Plant.novo")));
        d.frame();
        d
    }

    fn open(d: &mut Drive, user: &Path) {
        d.app.wb.queued.push("report.designer");
        d.frame();
        // The command opened it on the real user folder; use the test's.
        d.app.open_designer_in(Some(user.to_owned()));
        d.frame();
    }

    fn state(d: &mut Drive) -> &mut DesignerState {
        d.app.wb.designer.as_mut().expect("the designer is open")
    }

    fn is_pdf(path: &Path) -> bool {
        std::fs::read(path).is_ok_and(|b| b.starts_with(b"%PDF"))
    }

    fn act(d: &mut Drive, a: Act) {
        let mut st = d.app.wb.designer.take().unwrap();
        d.app.designer_act(&mut st, a);
        d.app.wb.designer = Some(st);
    }

    #[test]
    fn the_command_opens_the_window_and_it_draws() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = app(dir.path());
        assert!(d.app.wb.designer.is_none());
        d.app.wb.queued.push("report.designer");
        d.frame();
        assert!(d.app.wb.designer.is_some());
        let count = state(&mut d).library.len();
        assert!(count >= 5, "the built-in templates are listed");
        // Every listed template draws and previews.
        for i in 0..count {
            let st = state(&mut d);
            let (o, t) = st.library[i].clone();
            st.load(o, t);
            d.frame();
            let (_, _, _, rendered) = state(&mut d).cache.clone().expect("rendered");
            assert!(!rendered.pages.is_empty());
        }
    }

    #[test]
    fn editing_columns_changes_the_preview_and_saving_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let user = dir.path().join("templates");
        let mut d = app(dir.path());
        open(&mut d, &user);
        state(&mut d).duplicate();
        {
            let st = state(&mut d);
            st.draft.name = "Short parts".into();
            let Element::Table(t) = &mut st.draft.elements[0] else {
                panic!("table")
            };
            let keys: Vec<String> = report_columns("bom").into_iter().map(|c| c.0).collect();
            assert!(keys.len() > 2, "{keys:?}");
            t.spec.columns = keys.iter().take(2).map(|k| ColumnSpec::new(k)).collect();
            t.spec.columns.reverse();
            t.spec.columns[0].header = Some("Renamed".into());
            st.draft.format.size = PaperSize::A3;
            refit(&mut st.draft);
            assert!(st.is_dirty());
        }
        d.frame();
        let (_, _, _, rendered) = state(&mut d).cache.clone().unwrap();
        assert!(rendered.problems.is_empty(), "{:?}", rendered.problems);
        assert!(
            rendered.pages[0].texts.iter().any(|t| t.text == "Renamed"),
            "the renamed header is previewed"
        );
        assert!(rendered.pages[0].width > 400.0, "A3 landscape");

        act(&mut d, Act::SaveUser);
        assert!(!state(&mut d).is_dirty(), "saved");
        let (found, bad) = store::load_dir(&user);
        assert!(bad.is_empty());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Short parts");
        // The new template is in the list, tagged as the user's.
        assert!(
            state(&mut d)
                .library
                .iter()
                .any(|(o, t)| *o == Origin::User && t.name == "Short parts")
        );
    }

    #[test]
    fn save_in_project_is_one_undo_step_and_wins_over_built_in() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = app(dir.path());
        open(&mut d, &dir.path().join("templates"));
        state(&mut d).draft.description = "Ours".into();
        act(&mut d, Act::SaveProject);
        let saved = store::project_templates(d.doc().project()).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].description, "Ours");
        // Same name as a built-in: the project's copy is listed instead.
        let origin = state(&mut d)
            .library
            .iter()
            .find(|(_, t)| t.name == "Parts list")
            .map(|(o, _)| *o);
        assert_eq!(origin, Some(Origin::Project));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::Undo);
        assert!(
            store::project_templates(d.doc().project())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_broken_template_lists_its_problems() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = app(dir.path());
        open(&mut d, &dir.path().join("templates"));
        state(&mut d).draft.elements.push(Element::text(
            400.0,
            5.0,
            50.0,
            8.0,
            "{no.such.field}",
            4.0,
        ));
        d.frame();
        let (_, _, _, rendered) = state(&mut d).cache.clone().unwrap();
        let lines: Vec<String> = rendered.problems.iter().map(describe).collect();
        assert!(
            lines.iter().any(|l| l.contains("Unknown field")),
            "{lines:?}"
        );
        assert!(lines.iter().any(|l| l.contains("outside")), "{lines:?}");
    }

    #[test]
    fn export_pdf_writes_the_template_with_project_data() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = app(dir.path());
        let path = dir.path().join("out.pdf");
        let t = builtin::cover_form();
        assert_eq!(d.app.export_designer_pdf_to(&t, &path), Some(path.clone()));
        assert!(is_pdf(&path));
        // Through the window button: next to the project, never silent.
        open(&mut d, &dir.path().join("templates"));
        d.app.wb.last_export = None;
        act(&mut d, Act::ExportPdf);
        let last = d.app.wb.last_export.clone().expect("a file");
        assert!(is_pdf(&last), "{}", last.display());
    }

    #[test]
    fn the_reports_workspace_prints_through_a_template() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = app(dir.path());
        d.app.show_report("devices");
        let plain = d.app.export_report("devices", ReportFormat::Pdf).unwrap();
        assert!(is_pdf(&plain));
        d.app.reports_view.template = Some("Parts list".into());
        d.frame();
        let through = d.app.export_report("devices", ReportFormat::Pdf).unwrap();
        assert!(is_pdf(&through));
        assert_ne!(plain, through, "a second file, nothing overwritten");
        // A name that does not exist is a message, not a file.
        d.app.reports_view.template = Some("Gone".into());
        assert_eq!(d.app.export_report("devices", ReportFormat::Pdf), None);
        assert_eq!(
            d.app.reports_view.template, None,
            "reset to the plain layout"
        );
    }

    #[test]
    fn picking_while_dirty_asks_first() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = app(dir.path());
        open(&mut d, &dir.path().join("templates"));
        state(&mut d).draft.description = "edited".into();
        let other = state(&mut d)
            .library
            .iter()
            .position(|(_, t)| t.name == "Wire list")
            .unwrap();
        act(&mut d, Act::Pick(other));
        assert_eq!(state(&mut d).draft.description, "edited");
        assert!(state(&mut d).pending.is_some());
        act(&mut d, Act::Keep);
        assert!(state(&mut d).pending.is_none());
        act(&mut d, Act::NewFrom("bom".into(), "Parts".into()));
        assert_eq!(state(&mut d).draft.description, "edited");
        act(&mut d, Act::Discard);
        assert_eq!(state(&mut d).draft.name, "Parts (new)");
        // A new, unsaved template is itself unsaved work; a saved one is not.
        act(&mut d, Act::Pick(other));
        assert!(state(&mut d).pending.is_some());
        act(&mut d, Act::Discard);
        assert_eq!(state(&mut d).draft.name, "Wire list");
        act(&mut d, Act::Pick(0));
        assert!(
            state(&mut d).pending.is_none(),
            "clean drafts switch freely"
        );
        assert_ne!(state(&mut d).draft.name, "Wire list");
    }

    #[test]
    fn retarget_swaps_the_report_and_drops_its_columns() {
        let mut t = builtin::report_a4("Parts list", "bom");
        if let Element::Table(table) = &mut t.elements[0] {
            table.spec.columns = vec![ColumnSpec::new("quantity")];
        }
        let r = retarget(&t, "wires", "Wire list").unwrap();
        let Element::Table(table) = &r.elements[0] else {
            panic!()
        };
        assert_eq!(table.spec.report, "wires");
        assert!(table.spec.columns.is_empty());
        assert_eq!(table.spec.heading, "Wire list");
        assert!(retarget(&builtin::cover_form(), "wires", "x").is_none());
    }
}
