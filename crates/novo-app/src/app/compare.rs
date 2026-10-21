//! The Compare window (DESIGN §7, "any two revisions can be picked"): what
//! changed between two states of the project, each a revision or now, as a
//! list grouped by kind of object with old and new values. Read only; the
//! comparison is `novo_revisions::compare`.

use eframe::egui;
use novo_canvas::marks::{Mark, MarkKind};
use novo_canvas::model::{PageScene, item_id};
use novo_canvas::widget::CanvasView;
use novo_core::{PageId, Project, RevisionId};
use novo_editor::art::LibraryArt;
use novo_revisions::{
    ChangeKind, ChangeRow, Comparison, FieldChange, Group, RestoreScope, Side, group_of,
};
use novo_symbols::Library;

use super::changes::shown_on;

use super::{Level, NovoApp};

/// The Compare window while it is open.
pub(crate) struct CompareWindow {
    from: Side,
    to: Side,
    /// The comparison for `(from, to)` at project revision `.0`, or why
    /// there is none.
    pub(super) shown: Option<(u64, Side, Side, Result<Comparison, String>)>,
    filter: String,
    /// The read-only drawing of one side, while "Show drawing" is on.
    drawing: Option<DrawView>,
}

impl CompareWindow {
    /// Turns the drawing pane on, showing the To side.
    #[cfg(test)]
    pub(super) fn show_drawing(&mut self) {
        self.drawing = Some(DrawView::new(true));
    }

    /// The marks on the drawing pane now (None while it is off).
    #[cfg(test)]
    pub(super) fn drawing_marks(&self) -> Option<&[Mark]> {
        self.drawing
            .as_ref()
            .map(|d| d.canvas.marks.list.as_slice())
    }
}

/// A page of one side's state, drawn read only with the change marks: what
/// was removed is outlined on the From side, what was added on the To side,
/// what changed on both.
struct DrawView {
    /// Whether the To side is shown (otherwise the From side).
    show_to: bool,
    /// The shown state, kept while neither side nor the project changes.
    state: Option<(u64, Side, Project)>,
    page: Option<PageId>,
    scene: Option<(PageId, PageScene)>,
    marks_for: Option<(u64, Side, Side, bool, PageId)>,
    canvas: CanvasView,
}

impl DrawView {
    fn new(show_to: bool) -> Self {
        let mut canvas = CanvasView::new();
        canvas.style = crate::theme::canvas_style();
        canvas.bold_family = Some(novo_theme::fonts::strong_family());
        DrawView {
            show_to,
            state: None,
            page: None,
            scene: None,
            marks_for: None,
            canvas,
        }
    }
}

/// The marks for `page` of the shown state: an outline on each added
/// object (To side), removed one (From side) and changed one (both).
fn marks_of(c: &Comparison, state: &Project, page: PageId, show_to: bool) -> Vec<Mark> {
    let mut marks = Vec::new();
    for row in &c.rows {
        let kind = match row.kind {
            ChangeKind::Added if show_to => MarkKind::Added,
            ChangeKind::Removed if !show_to => MarkKind::Removed,
            ChangeKind::Changed { .. } => MarkKind::Modified,
            _ => continue,
        };
        for id in shown_on(state, row.id, page) {
            let mut mark = Mark::on(kind, item_id(id));
            mark.text = format!("{}: {}", kind_word(&row.kind), row.label);
            marks.push(mark);
        }
    }
    marks
}

fn group_title(group: Group) -> &'static str {
    match group {
        Group::Page => "Pages",
        Group::Device => "Devices",
        Group::Terminal => "Terminals",
        Group::Cable => "Cables",
        Group::Wire => "Wires",
        Group::Text => "Texts",
        Group::Drawing => "Symbols and junctions",
        Group::Part => "Parts",
        Group::Project => "Project",
    }
}

fn kind_word(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "added",
        ChangeKind::Removed => "removed",
        ChangeKind::Changed { .. } => "changed",
    }
}

/// One field as `name: old → new`, with the plain-words note after it.
pub(crate) fn field_line(f: &FieldChange) -> String {
    let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "(none)".into());
    let values = format!("{}: {} → {}", f.name, show(&f.old), show(&f.new));
    match &f.note {
        Some(note) => format!("{values} ({note})"),
        None => values,
    }
}

/// The text of a revision in the pickers: `B · 2026-10-16 · Second`.
fn revision_text(project: &Project, id: RevisionId) -> String {
    project.issued_revision(id).map_or_else(String::new, |r| {
        if r.description.is_empty() {
            format!("{} · {}", r.index, r.date)
        } else {
            format!("{} · {} · {}", r.index, r.date, r.description)
        }
    })
}

fn side_text(project: &Project, side: Side) -> String {
    match side {
        Side::Now => "Now".to_owned(),
        Side::Revision(id) => revision_text(project, id),
    }
}

/// The rows to list: those whose label contains `filter`, grouped (stable,
/// so page order holds within a group).
fn grouped<'c>(c: &'c Comparison, filter: &str) -> Vec<(Group, Vec<&'c ChangeRow>)> {
    let needle = filter.trim().to_lowercase();
    let mut rows: Vec<&ChangeRow> = c
        .rows
        .iter()
        .filter(|r| needle.is_empty() || r.label.to_lowercase().contains(&needle))
        .collect();
    rows.sort_by_key(|r| group_of(r.id));
    let mut out: Vec<(Group, Vec<&ChangeRow>)> = Vec::new();
    for row in rows {
        let group = group_of(row.id);
        match out.last_mut() {
            Some((g, list)) if *g == group => list.push(row),
            _ => out.push((group, vec![row])),
        }
    }
    out
}

impl NovoApp {
    /// The `compare revisions` command: opens the window on `A B` (two
    /// revision indexes, or `now`), on `A` against now, or on the latest
    /// revision against now.
    pub(super) fn open_compare(&mut self, argument: Option<&str>) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let words: Vec<&str> = argument.unwrap_or("").split_whitespace().collect();
        let side = |word: &str| -> Result<Side, String> {
            if word.eq_ignore_ascii_case("now") {
                return Ok(Side::Now);
            }
            project
                .revision_by_index(word)
                .map(|r| Side::Revision(r.id))
                .ok_or_else(|| format!("There is no revision {word}"))
        };
        let picked = match words.as_slice() {
            [] => project
                .latest_revision()
                .map(|r| (Side::Revision(r.id), Side::Now))
                .ok_or_else(|| {
                    "No revision yet: Issue revision first, then Compare lists what changed"
                        .to_owned()
                }),
            [from] => side(from).map(|f| (f, Side::Now)),
            [from, to, ..] => side(from).and_then(|f| side(to).map(|t| (f, t))),
        };
        match picked {
            Ok((from, to)) => {
                self.wb.compare = Some(CompareWindow {
                    from,
                    to,
                    shown: None,
                    filter: String::new(),
                    drawing: None,
                });
            }
            Err(text) => self.message(Level::Info, text),
        }
    }

    /// Opens the window on the revision Changes mode compares with, against
    /// now (the Changes card's "Compare…" button).
    pub(super) fn open_compare_from_changes(&mut self) {
        let base = self.wb.changes.as_ref().map(|m| m.base_index().to_owned());
        self.open_compare(base.as_deref());
    }

    pub(super) fn compare_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.compare.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let stale = win.shown.as_ref().is_none_or(|(rev, f, t, _)| {
            *rev != project.revision() || *f != win.from || *t != win.to
        });
        if stale {
            let result = novo_revisions::compare(
                project,
                |id| Ok::<_, String>(self.backend.revision_snapshot(id)),
                win.from,
                win.to,
            )
            .map_err(|e| e.to_string());
            win.shown = Some((project.revision(), win.from, win.to, result));
        }
        // Newest first, then Now.
        if let Some(d) = win.drawing.as_mut() {
            let side = if d.show_to { win.to } else { win.from };
            let key = (project.revision(), side);
            if d.state.as_ref().is_none_or(|(r, s, _)| (*r, *s) != key) {
                let state = match side {
                    Side::Now => Some(project.clone()),
                    Side::Revision(id) => self.backend.revision_snapshot(id),
                };
                d.state = state.map(|p| (key.0, key.1, p));
                d.scene = None;
                d.marks_for = None;
            }
        }
        let library = self.library();
        let mut options: Vec<Side> = project.revisions().map(|r| Side::Revision(r.id)).collect();
        options.reverse();
        options.push(Side::Now);
        let mut open = true;
        let mut jump: Option<PageId> = None;
        let mut restore: Option<(RevisionId, RestoreScope)> = None;
        super::panel::dialog(ctx, "Compare revisions", [520.0, 480.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("From");
                    side_picker(ui, "compare_from", project, &options, &mut win.from);
                    if novo_theme::widgets::icon_button(
                        ui,
                        novo_theme::icons::action::CROSS_REFERENCE,
                        "Swap the two sides",
                    )
                    .clicked()
                    {
                        std::mem::swap(&mut win.from, &mut win.to);
                    }
                    ui.label("To");
                    side_picker(ui, "compare_to", project, &options, &mut win.to);
                });
                let mut drawing_on = win.drawing.is_some();
                if ui
                    .checkbox(&mut drawing_on, "Show drawing")
                    .on_hover_text("Draw one side read only, with the changes outlined")
                    .changed()
                {
                    win.drawing = drawing_on.then(|| DrawView::new(true));
                }
                for (label, side) in [("From", win.from), ("To", win.to)] {
                    if let Side::Revision(id) = side
                        && let Some(r) = project.issued_revision(id)
                        && !r.note().is_empty()
                    {
                        ui.weak(format!("{label} {}: {}", r.index, r.note()));
                    }
                }
                if let Side::Revision(id) = win.from {
                    let index = project
                        .issued_revision(id)
                        .map(|r| r.index.clone())
                        .unwrap_or_default();
                    ui.horizontal(|ui| {
                        let page = self.current_page;
                        if ui
                            .add_enabled(
                                page.is_some(),
                                egui::Button::new(format!("Restore this page to {index}")),
                            )
                            .on_hover_text("Puts the page on screen back as issued. Undo takes it back")
                            .clicked()
                            && let Some(page) = page
                        {
                            restore = Some((id, RestoreScope::Pages(vec![page])));
                        }
                        if ui
                            .button(format!("Restore the project to {index}"))
                            .on_hover_text(
                                "Puts every page and device back as issued; the revision list stays. Undo takes it back",
                            )
                            .clicked()
                        {
                            restore = Some((id, RestoreScope::Project));
                        }
                    });
                }
                ui.add_space(4.0);
                let Some((_, _, _, result)) = &win.shown else {
                    return;
                };
                if let Some(d) = win.drawing.as_mut() {
                    drawing_ui(
                        ui,
                        d,
                        library,
                        result.as_ref().ok(),
                        (project.revision(), win.from, win.to),
                    );
                    ui.separator();
                }
                match result {
                    Err(why) => {
                        novo_theme::widgets::banner(
                            ui,
                            novo_theme::widgets::Level::Warning,
                            why,
                            |_| {},
                        );
                    }
                    Ok(c) if c.is_empty() => {
                        ui.weak("No differences between these two states");
                    }
                    Ok(c) => {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut win.filter)
                                    .hint_text("Filter")
                                    .desired_width(160.0),
                            );
                            ui.weak(counts(c));
                        });
                        ui.separator();
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            rows_ui(ui, project, c, &win.filter, &mut jump);
                        });
                    }
                }
            });
        if let Some(page) = jump
            && self
                .backend
                .document()
                .is_some_and(|d| d.project().page(page).is_some())
        {
            self.show_page(page);
        }
        if let Some((revision, scope)) = restore {
            self.restore_to(revision, scope);
        }
        if open {
            self.wb.compare = Some(win);
        }
    }
}

/// The drawing pane: which side and page, then the read-only canvas.
fn drawing_ui(
    ui: &mut egui::Ui,
    d: &mut DrawView,
    library: Option<&Library>,
    comparison: Option<&Comparison>,
    (rev, from, to): (u64, Side, Side),
) {
    ui.horizontal(|ui| {
        ui.label("Draw");
        let mut side = usize::from(d.show_to);
        if novo_theme::widgets::segmented(ui, &mut side, &["From", "To"]) {
            d.show_to = side == 1;
        }
    });
    let Some((_, _, state)) = &d.state else {
        ui.weak("This revision has no stored drawing");
        return;
    };
    let pages = state.pages_in_order();
    if d.page.is_none_or(|p| state.page(p).is_none()) {
        d.page = pages.first().map(|p| p.id);
        d.scene = None;
    }
    let Some(page) = d.page else {
        ui.weak("No pages");
        return;
    };
    let name = |id: PageId| state.page(id).map_or_else(String::new, |p| p.name.clone());
    let mut picked = page;
    egui::ComboBox::from_id_salt("compare_page")
        .selected_text(format!("Page {}", name(page)))
        .show_ui(ui, |ui| {
            for p in &pages {
                ui.selectable_value(&mut picked, p.id, format!("Page {}", p.name));
            }
        });
    if picked != page {
        d.page = Some(picked);
        d.scene = None;
        d.canvas.zoom_to_fit();
    }
    let page = picked;
    if d.scene.as_ref().is_none_or(|(p, _)| *p != page) {
        let scene = match library {
            Some(lib) => PageScene::new(
                state,
                page,
                &LibraryArt::new(lib, crate::theme::model_style().symbol),
            ),
            None => PageScene::new(state, page, &novo_canvas::model::PlaceholderArt),
        };
        d.scene = Some((page, scene));
        d.marks_for = None;
        d.canvas.zoom_to_fit();
    }
    let key = (rev, from, to, d.show_to, page);
    if d.marks_for != Some(key) {
        d.canvas.marks.list =
            comparison.map_or_else(Vec::new, |c| marks_of(c, state, page, d.show_to));
        d.marks_for = Some(key);
    }
    if let Some((_, scene)) = &d.scene {
        ui.allocate_ui(egui::vec2(ui.available_width(), 300.0), |ui| {
            d.canvas.show(ui, scene.scene());
        });
    }
}

fn counts(c: &Comparison) -> String {
    let n = |f: fn(&ChangeKind) -> bool| c.rows.iter().filter(|r| f(&r.kind)).count();
    format!(
        "{} added · {} changed · {} removed",
        n(|k| matches!(k, ChangeKind::Added)),
        n(|k| matches!(k, ChangeKind::Changed { .. })),
        n(|k| matches!(k, ChangeKind::Removed)),
    )
}

fn side_picker(ui: &mut egui::Ui, id: &str, project: &Project, options: &[Side], value: &mut Side) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(side_text(project, *value))
        .width(170.0)
        .show_ui(ui, |ui| {
            for side in options {
                ui.selectable_value(value, *side, side_text(project, *side));
            }
        });
}

fn rows_ui(
    ui: &mut egui::Ui,
    project: &Project,
    c: &Comparison,
    filter: &str,
    jump: &mut Option<PageId>,
) {
    for (group, rows) in grouped(c, filter) {
        ui.add_space(4.0);
        ui.strong(group_title(group));
        for row in rows {
            let page = row
                .page
                .and_then(|p| project.page(p))
                .map_or_else(String::new, |p| format!("  (page {})", p.name));
            let text = format!("{}  {}{page}", kind_word(&row.kind), row.label);
            if ui.selectable_label(false, text).clicked() {
                *jump = row.page;
            }
            for f in &row.fields {
                ui.weak(format!("      {}", field_line(f)));
            }
        }
    }
    if !c.info.is_empty() {
        ui.add_space(4.0);
        novo_theme::widgets::section_label(ui, "Project information");
        for f in &c.info {
            ui.weak(format!("      {}", field_line(f)));
        }
    }
    if !c.sheets.is_empty() {
        ui.add_space(4.0);
        novo_theme::widgets::section_label(ui, "Generated sheets");
        for s in &c.sheets {
            ui.weak(format!("      {} ({})", s.name, s.report));
        }
    }
}
