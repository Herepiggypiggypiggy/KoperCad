//! Changes mode (docs/DESIGN.md §7): the drawing marked with what changed
//! since a revision. Added and changed objects get an outline in place;
//! what was removed is outlined dashed where it used to be, taken from the
//! older state. The comparison is novo-revisions' `diff` on the model.

use eframe::egui;
use novo_canvas::marks::{Mark, MarkKind, MarkTarget};
use novo_canvas::model::item_id;
use novo_core::{AnyId, PageId, Project, RevisionId};
use novo_revisions::{Change, ChangeKind, ChangeRow, ChangeSet, diff, list_of};
use novo_theme::widgets::{self, section_label};

use super::{Level, NovoApp};

/// Changes mode while it is on.
pub(crate) struct ChangesMode {
    base: RevisionId,
    /// The index of `base`, for the texts.
    index: String,
    snapshot: Project,
    /// The comparison with the project as of `.0` (its revision counter).
    changes: Option<(u64, ChangeSet)>,
    /// The named rows of that comparison, for the outline's list.
    rows: Option<(u64, Vec<ChangeRow>)>,
    /// The dashed marks for what was removed on a page, kept for the
    /// project revision and page they were worked out for: building them
    /// draws the page of the older state.
    ghosts: Option<((u64, PageId), Vec<Mark>)>,
}

impl ChangesMode {
    /// The index of the revision Changes mode compares with.
    pub(super) fn base_index(&self) -> &str {
        &self.index
    }

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

    /// The rows of the Changes list, worked out once per project revision.
    pub(super) fn rows(&mut self, project: &Project) -> &[ChangeRow] {
        if self
            .rows
            .as_ref()
            .is_none_or(|(r, _)| *r != project.revision())
        {
            self.changes(project);
            let set = &self.changes.as_ref().expect("computed above").1;
            let rows = list_of(set, &self.snapshot, project);
            self.rows = Some((project.revision(), rows));
        }
        &self.rows.as_ref().expect("just computed").1
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

/// "Since [revision ▾]": the revisions to compare with, newest first.
/// Returns the one picked, if it is not the current base.
fn base_picker(
    ui: &mut egui::Ui,
    revisions: &[(RevisionId, String)],
    base: RevisionId,
) -> Option<RevisionId> {
    let mut pick = None;
    ui.horizontal(|ui| {
        ui.weak("Since");
        let shown = revisions
            .iter()
            .find(|(id, _)| *id == base)
            .map_or(String::new(), |(_, t)| t.clone());
        egui::ComboBox::from_id_salt("changes_base")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                for (id, text) in revisions.iter().rev() {
                    if ui.selectable_label(*id == base, text).clicked() {
                        pick = Some(*id);
                    }
                }
            });
    });
    pick
}

/// "2 added · 1 changed · 1 removed", leaving out what is zero.
fn counts_text(changes: &[Change]) -> String {
    let count = |f: fn(&ChangeKind) -> bool| changes.iter().filter(|c| f(&c.kind)).count();
    [
        (count(|k| matches!(k, ChangeKind::Added)), "added"),
        (
            count(|k| matches!(k, ChangeKind::Changed { .. })),
            "changed",
        ),
        (count(|k| matches!(k, ChangeKind::Removed)), "removed"),
    ]
    .into_iter()
    .filter(|(n, _)| *n > 0)
    .map(|(n, what)| format!("{n} {what}"))
    .collect::<Vec<_>>()
    .join(" · ")
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
            rows: None,
            ghosts: None,
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
        let mut marks = Vec::new();
        let mut removed = Vec::new();
        for change in mode.changes(project).on_page(page) {
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
        let key = (project.revision(), page);
        if mode.ghosts.as_ref().is_none_or(|(k, _)| *k != key) {
            let Some(view) = self.canvas.schematic_ref() else {
                return marks;
            };
            let ids: Vec<AnyId> = removed.iter().map(|(id, _)| *id).collect();
            let ghosts = view
                .bounds_in(&mode.snapshot, page, &ids)
                .into_iter()
                .map(|(id, area)| Mark {
                    kind: MarkKind::Removed,
                    target: MarkTarget::Area(area),
                    text: removed
                        .iter()
                        .find(|(r, _)| *r == id)
                        .map(|(_, t)| t.clone())
                        .unwrap_or_default(),
                })
                .collect();
            mode.ghosts = Some((key, ghosts));
        }
        if let Some((_, ghosts)) = &mode.ghosts {
            marks.extend(ghosts.iter().cloned());
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
        let set = mode.changes(project);
        let here = set.on_page(page);
        let count = |f: fn(&ChangeKind) -> bool| here.iter().filter(|c| f(&c.kind)).count();
        let (added, changed, removed) = (
            count(|k| matches!(k, ChangeKind::Added)),
            count(|k| matches!(k, ChangeKind::Changed { .. })),
            count(|k| matches!(k, ChangeKind::Removed)),
        );
        let nothing_here = here.is_empty();
        let pages = set.pages.len();
        let elsewhere = set.project.len() + usize::from(!set.info.is_empty());
        let mut pick = None;
        let mut leave = false;
        let mut compare = false;
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
                    pick = base_picker(ui, &revisions, mode.base);
                    if ui
                        .small_button("Compare…")
                        .on_hover_text("List what changed, field by field")
                        .clicked()
                    {
                        compare = true;
                    }
                    if nothing_here {
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
        if compare {
            self.open_compare_from_changes();
        }
    }

    /// Whether Changes mode is on and `page` has changes since its
    /// revision: the outline marks such pages.
    pub(super) fn page_has_changes(&mut self, page: PageId) -> bool {
        let (Some(mode), Some(doc)) = (self.wb.changes.as_mut(), self.backend.document()) else {
            return false;
        };
        !mode.changes(doc.project()).on_page(page).is_empty()
    }

    /// The outline's Changes list (DESIGN §3): the pages that changed since
    /// a revision, each with its counts, and the changes that belong to no
    /// page. Opening it turns Changes mode on against the latest revision.
    /// Returns whether a page was picked.
    pub(super) fn changes_outline(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return false;
        };
        let Some(latest) = project.latest_revision().map(|r| r.id) else {
            ui.weak("No revision yet. Issue one (Ctrl+K, issue), then Changes compares with it.");
            return false;
        };
        if self.wb.changes.is_none() {
            if self.backend.revision_snapshot(latest).is_none() {
                ui.weak("The latest revision has no kept copy to compare with.");
                return false;
            }
            self.toggle_changes(None);
        }
        let (Some(mode), Some(doc)) = (self.wb.changes.as_mut(), self.backend.document()) else {
            return false;
        };
        let project = doc.project();
        let revisions: Vec<(RevisionId, String)> = project
            .revisions()
            .map(|r| (r.id, format!("{} · {}", r.index, r.date)))
            .collect();
        let base = mode.base;
        let all_rows = mode.rows(project).to_vec();
        let mode = &*mode;
        let set = &mode.changes.as_ref().expect("computed above").1;
        let name = |id: PageId| match project.page(id) {
            Some(p) => Some(page_title(&p.name, &p.title)),
            None => mode
                .snapshot
                .page(id)
                .map(|p| format!("{} (removed)", page_title(&p.name, &p.title))),
        };
        // Pages in page order, then the removed ones.
        let mut ordered: Vec<PageId> = project
            .pages_in_order()
            .into_iter()
            .map(|p| p.id)
            .filter(|id| set.pages.contains_key(id))
            .collect();
        let gone: Vec<PageId> = set
            .pages
            .keys()
            .filter(|id| !ordered.contains(id))
            .copied()
            .collect();
        ordered.extend(gone);
        let rows: Vec<(PageId, String, String)> = ordered
            .into_iter()
            .filter_map(|id| Some((id, name(id)?, counts_text(set.on_page(id)))))
            .collect();
        let elsewhere = set.project.len();
        let info = set.info.join(", ");
        let pick = base_picker(ui, &revisions, base);
        let mut chosen = None;
        let mut reveal = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                if rows.is_empty() && elsewhere == 0 && info.is_empty() {
                    ui.weak("Nothing changed since then");
                }
                for (id, label, counts) in &rows {
                    let selected = self.current_page == Some(*id);
                    let response = ui
                        .with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                            ui.add(
                                egui::Button::selectable(selected, label)
                                    .min_size(egui::vec2(0.0, 22.0)),
                            )
                        })
                        .inner;
                    ui.weak(format!("    {counts}"));
                    if response.clicked() && project.page(*id).is_some() {
                        chosen = Some(*id);
                    }
                    for row in all_rows.iter().filter(|r| r.page == Some(*id)) {
                        let text = format!("{} {}", kind_mark(&row.kind), row.label);
                        if ui.selectable_label(false, text).clicked() {
                            reveal = Some(row.clone());
                        }
                    }
                }
                if elsewhere > 0 || !info.is_empty() {
                    section_label(ui, "Project");
                    if elsewhere > 0 {
                        ui.label(format!(
                            "{elsewhere} change{} to parts, potentials, rules or structure",
                            if elsewhere == 1 { "" } else { "s" }
                        ));
                    }
                    if !info.is_empty() {
                        ui.label(format!("Project information: {info}"));
                    }
                }
            });
        if let Some(id) = pick
            && id != base
            && let Some(index) = project.issued_revision(id).map(|r| r.index.clone())
        {
            self.wb.changes = None;
            self.toggle_changes(Some(&index));
        }
        if let Some(row) = reveal {
            self.reveal_change(&row);
            return true;
        }
        if let Some(page) = chosen {
            self.show_page(page);
            return true;
        }
        false
    }

    /// Opens the page a row of the Changes list is on and selects the
    /// object there. A removed object is not in the project any more, so
    /// only its page opens (its dashed ghost shows where it was).
    pub(super) fn reveal_change(&mut self, row: &ChangeRow) {
        let (Some(page), Some(doc)) = (row.page, self.backend.document()) else {
            return;
        };
        let target = shown_on(doc.project(), row.id, page).first().copied();
        self.show_page(page);
        self.wb.pending_item = target.map(|id| (page, id));
    }
}

/// The mark in front of a row of the Changes list.
fn kind_mark(kind: &ChangeKind) -> &'static str {
    match kind {
        ChangeKind::Added => "+",
        ChangeKind::Removed => "−",
        ChangeKind::Changed { .. } => "~",
    }
}

/// A page's name and title as the outline lists it.
fn page_title(name: &str, title: &str) -> String {
    if title.is_empty() {
        name.to_owned()
    } else {
        format!("{name}  {title}")
    }
}
