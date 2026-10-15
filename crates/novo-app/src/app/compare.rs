//! The Compare window (DESIGN §7, "any two revisions can be picked"): what
//! changed between two states of the project, each a revision or now, as a
//! list grouped by kind of object with old and new values. Read only; the
//! comparison is `novo_revisions::compare`.

use eframe::egui;
use novo_core::{PageId, Project, RevisionId};
use novo_revisions::{ChangeKind, ChangeRow, Comparison, FieldChange, Group, Side, group_of};

use super::{Level, NovoApp};

/// The Compare window while it is open.
pub(crate) struct CompareWindow {
    from: Side,
    to: Side,
    /// The comparison for `(from, to)` at project revision `.0`, or why
    /// there is none.
    pub(super) shown: Option<(u64, Side, Side, Result<Comparison, String>)>,
    filter: String,
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
        let mut options: Vec<Side> = project.revisions().map(|r| Side::Revision(r.id)).collect();
        options.reverse();
        options.push(Side::Now);
        let mut open = true;
        let mut jump: Option<PageId> = None;
        egui::Window::new("Compare revisions")
            .open(&mut open)
            .default_size([520.0, 480.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("From");
                    side_picker(ui, "compare_from", project, &options, &mut win.from);
                    if ui.button("⇄").on_hover_text("Swap the two sides").clicked() {
                        std::mem::swap(&mut win.from, &mut win.to);
                    }
                    ui.label("To");
                    side_picker(ui, "compare_to", project, &options, &mut win.to);
                });
                ui.add_space(4.0);
                let Some((_, _, _, result)) = &win.shown else {
                    return;
                };
                match result {
                    Err(why) => {
                        ui.colored_label(ui.visuals().warn_fg_color, why);
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
        if open {
            self.wb.compare = Some(win);
        }
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
        ui.strong("Project information");
        for f in &c.info {
            ui.weak(format!("      {}", field_line(f)));
        }
    }
    if !c.sheets.is_empty() {
        ui.add_space(4.0);
        ui.strong("Generated sheets");
        for s in &c.sheets {
            ui.weak(format!("      {} ({})", s.name, s.report));
        }
    }
}
