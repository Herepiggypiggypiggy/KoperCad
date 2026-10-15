//! Changes mode (docs/DESIGN.md §7): the drawing marked with what changed
//! since a revision. Added and changed objects get an outline in place;
//! what was removed is outlined dashed where it used to be, taken from the
//! older state. The comparison is novo-revisions' `diff` on the model.

use eframe::egui;
use novo_canvas::marks::{Mark, MarkKind, MarkTarget};
use novo_canvas::model::item_id;
use novo_core::{AnyId, PageId, Project, RevisionId};
use novo_revisions::{Change, ChangeKind, ChangeSet, diff};
use novo_theme::widgets;

use super::{Level, NovoApp};

/// Changes mode while it is on.
pub(crate) struct ChangesMode {
    base: RevisionId,
    /// The index of `base`, for the texts.
    index: String,
    snapshot: Project,
    /// The comparison with the project as of `.0` (its revision counter).
    changes: Option<(u64, ChangeSet)>,
}

impl ChangesMode {
    fn changes(&mut self, project: &Project) -> &ChangeSet {
        if self
            .changes
            .as_ref()
            .is_none_or(|(r, _)| *r != project.revision())
        {
            self.changes = Some((project.revision(), diff(&self.snapshot, project)));
        }
        &self.changes.as_ref().expect("just computed").1
    }
}

/// The objects a change shows as on `page` in `project`: the object itself,
/// or the symbols and wires that draw a device, terminal or cable.
fn shown_on(project: &Project, id: AnyId, page: PageId) -> Vec<AnyId> {
    match id {
        AnyId::Symbol(_) | AnyId::Wire(_) | AnyId::Text(_) | AnyId::Junction(_) => project
            .record(id)
            .and_then(|r| r.page())
            .filter(|p| *p == page)
            .map(|_| vec![id])
            .unwrap_or_default(),
        AnyId::Device(d) => project
            .symbols_of_device(d)
            .iter()
            .filter(|s| s.page == page)
            .map(|s| AnyId::Symbol(s.id))
            .collect(),
        AnyId::Terminal(t) => project
            .symbols_of_terminal(t)
            .iter()
            .filter(|s| s.page == page)
            .map(|s| AnyId::Symbol(s.id))
            .collect(),
        AnyId::Cable(c) => project
            .cable(c)
            .map(|cable| {
                cable
                    .cores
                    .iter()
                    .filter_map(|core| core.wire)
                    .filter_map(|w| project.wire(w))
                    .filter(|w| w.page == page)
                    .map(|w| AnyId::Wire(w.id))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn describe(change: &Change, since: &str) -> String {
    match &change.kind {
        ChangeKind::Added => format!("Added since {since}"),
        ChangeKind::Removed => format!("Removed since {since}"),
        ChangeKind::Changed { fields } => {
            format!("Changed since {since}: {}", fields.join(", "))
        }
    }
}

impl NovoApp {
    /// The `changes` command: Changes mode on (against the latest revision,
    /// or the one named in `argument`) or off.
    pub(super) fn toggle_changes(&mut self, argument: Option<&str>) {
        let named = argument.map(str::trim).filter(|a| !a.is_empty());
        if self.wb.changes.is_some() && named.is_none() {
            self.wb.changes = None;
            return;
        }
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let revision = match named {
            Some(index) => project.revision_by_index(index),
            None => project.latest_revision(),
        };
        let Some(revision) = revision else {
            let text = match named {
                Some(index) => format!("There is no revision {index}"),
                None => "No revision yet: Issue revision first, then Changes compares with it"
                    .to_owned(),
            };
            self.message(Level::Info, text);
            return;
        };
        let (id, index) = (revision.id, revision.index.clone());
        let Some(snapshot) = self.backend.revision_snapshot(id) else {
            self.message(
                Level::Warning,
                format!("Revision {index} has no kept copy to compare with"),
            );
            return;
        };
        self.wb.changes = Some(ChangesMode {
            base: id,
            index,
            snapshot,
            changes: None,
        });
    }

    /// The marks of Changes mode for `page`: an outline on each added or
    /// changed object, a dashed one where an object was removed.
    pub(super) fn changes_marks(&mut self, page: PageId) -> Vec<Mark> {
        let (Some(mode), Some(doc)) = (self.wb.changes.as_mut(), self.backend.document()) else {
            return Vec::new();
        };
        let project = doc.project();
        let since = mode.index.clone();
        let changes = mode.changes(project).on_page(page).to_vec();
        let mut marks = Vec::new();
        let mut removed = Vec::new();
        for change in &changes {
            let kind = match change.kind {
                ChangeKind::Added => MarkKind::Added,
                ChangeKind::Changed { .. } => MarkKind::Modified,
                ChangeKind::Removed => {
                    removed.push((change.id, describe(change, &since)));
                    continue;
                }
            };
            for id in shown_on(project, change.id, page) {
                marks.push(Mark::on(kind, item_id(id)).with_text(describe(change, &since)));
            }
        }
        if removed.is_empty() {
            return marks;
        }
        let ids: Vec<AnyId> = removed.iter().map(|(id, _)| *id).collect();
        let Some(mode) = self.wb.changes.as_ref() else {
            return marks;
        };
        let Some(view) = self.canvas.schematic_ref() else {
            return marks;
        };
        for (id, area) in view.bounds_in(&mode.snapshot, page, &ids) {
            let text = removed
                .iter()
                .find(|(r, _)| *r == id)
                .map(|(_, t)| t.clone())
                .unwrap_or_default();
            marks.push(Mark {
                kind: MarkKind::Removed,
                target: MarkTarget::Area(area),
                text,
            });
        }
        marks
    }

    /// The card that says what Changes mode compares and lets the person
    /// pick another revision.
    pub(super) fn changes_card(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let Some(page) = self.current_page else {
            return;
        };
        let (Some(mode), Some(doc)) = (self.wb.changes.as_mut(), self.backend.document()) else {
            return;
        };
        let project = doc.project();
        let revisions: Vec<(RevisionId, String)> = project
            .revisions()
            .map(|r| (r.id, format!("{} · {}", r.index, r.date)))
            .collect();
        let set = mode.changes(project).clone();
        let here = set.on_page(page);
        let count = |f: fn(&ChangeKind) -> bool| here.iter().filter(|c| f(&c.kind)).count();
        let (added, changed, removed) = (
            count(|k| matches!(k, ChangeKind::Added)),
            count(|k| matches!(k, ChangeKind::Changed { .. })),
            count(|k| matches!(k, ChangeKind::Removed)),
        );
        let pages = set.pages.len();
        let elsewhere = set.project.len() + usize::from(!set.info.is_empty());
        let mut pick = None;
        let mut leave = false;
        // Below the live potentials card when both are on.
        let live = self.wb.live.on;
        egui::Area::new(egui::Id::new("changes_card"))
            .order(egui::Order::Middle)
            .fixed_pos(canvas.left_top() + egui::vec2(12.0, if live { 150.0 } else { 10.0 }))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(260.0);
                    ui.horizontal(|ui| {
                        widgets::section_label(ui, "Changes");
                        if ui.small_button("✕").on_hover_text("Close").clicked() {
                            leave = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.weak("Since");
                        let shown = revisions
                            .iter()
                            .find(|(id, _)| *id == mode.base)
                            .map_or(String::new(), |(_, t)| t.clone());
                        egui::ComboBox::from_id_salt("changes_base")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (id, text) in revisions.iter().rev() {
                                    if ui.selectable_label(*id == mode.base, text).clicked() {
                                        pick = Some(*id);
                                    }
                                }
                            });
                    });
                    if here.is_empty() {
                        ui.weak("Nothing changed on this page");
                    } else {
                        ui.label(format!(
                            "This page: {added} added · {changed} changed · {removed} removed"
                        ));
                    }
                    ui.weak(format!(
                        "{pages} page{} changed{}",
                        if pages == 1 { "" } else { "s" },
                        if elsewhere > 0 {
                            format!(" · {elsewhere} in the project")
                        } else {
                            String::new()
                        }
                    ));
                });
            });
        if let Some(id) = pick
            && id != mode.base
            && let Some(index) = project.issued_revision(id).map(|r| r.index.clone())
        {
            self.wb.changes = None;
            self.toggle_changes(Some(&index));
        }
        if leave {
            self.wb.changes = None;
        }
    }
}
