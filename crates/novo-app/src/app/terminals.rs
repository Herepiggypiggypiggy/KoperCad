//! Terminals in the app (novo-terminals, WS-11): the Terminals drawer
//! (Ctrl+3) shows the selection's strip, and terminals where a wire crosses
//! a location boundary are proposed on the drawing (Enter accepts, Tab
//! moves to the next, Delete declines one, `accept all` takes every one).

use eframe::egui;
use novo_canvas::{Aabb, Mark, MarkKind, MarkTarget, Vec2};
use novo_core::{AnyId, DeviceId, SymbolRole};
use novo_terminals::Suggestion;

use super::workbench::{DRAWER_SHARE, Drawer};
use super::{Level, NovoApp};

impl NovoApp {
    /// Terminal proposals on the shown page, in suggestion order.
    pub(super) fn page_proposals(&self) -> Vec<Suggestion> {
        let (Some(doc), Some(page)) = (self.backend.document(), self.current_page) else {
            return Vec::new();
        };
        let project = doc.project();
        novo_terminals::suggest_terminals(project)
            .into_iter()
            .filter(|s| project.wire(s.wire).is_some_and(|w| w.page == page))
            .collect()
    }

    /// Enter: accepts the highlighted proposal, but only while it is on
    /// screen; otherwise it is brought into view first, so nothing is
    /// accepted that the user cannot see.
    pub(super) fn accept_proposal(&mut self) {
        let proposals = self.page_proposals();
        let Some(s) = proposals.get(self.wb.proposal.min(proposals.len().saturating_sub(1))) else {
            return;
        };
        if !self.in_view(s) {
            self.bring_into_view(s);
            return;
        }
        self.accept_terminals("Accept terminal", std::slice::from_ref(s));
        self.wb.proposal = 0;
    }

    /// Delete: declines the highlighted proposal for good (undo brings it
    /// back). Like Enter, only while it is on screen.
    pub(super) fn dismiss_proposal(&mut self) {
        let proposals = self.page_proposals();
        let Some(s) = proposals.get(self.wb.proposal.min(proposals.len().saturating_sub(1))) else {
            return;
        };
        if !self.in_view(s) {
            self.bring_into_view(s);
            return;
        }
        let wire = s.wire;
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Decline terminal", |tx| {
            novo_terminals::dismiss_suggestion(tx, wire)
        }) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                self.wb.proposal = 0;
            }
            Err(e) => self.message(Level::Error, format!("Could not decline the terminal: {e}")),
        }
    }

    /// Whether the pointer is on the highlighted terminal proposal.
    pub(super) fn pointer_on_proposal(&self) -> bool {
        let Some(at) = self.wb.pointer_world else {
            return false;
        };
        let proposals = self.page_proposals();
        proposals
            .get(self.wb.proposal.min(proposals.len().saturating_sub(1)))
            .is_some_and(|s| {
                let area = proposal_area(s);
                let (x, y) = (at.x as f32, at.y as f32);
                (area.min.x..=area.max.x).contains(&x) && (area.min.y..=area.max.y).contains(&y)
            })
    }

    /// Tab: the next proposal, brought into view when it is off screen.
    pub(super) fn next_proposal(&mut self) {
        let proposals = self.page_proposals();
        if proposals.is_empty() {
            return;
        }
        self.wb.proposal = (self.wb.proposal + 1) % proposals.len();
        let s = &proposals[self.wb.proposal];
        if !self.in_view(s) {
            self.bring_into_view(s);
        }
    }

    fn in_view(&self, s: &Suggestion) -> bool {
        let Some(view) = self.canvas.schematic_ref() else {
            return true;
        };
        let seen = view.canvas.viewport.visible_world();
        let (x, y) = (s.at.x as f32, s.at.y as f32);
        (seen.min.x..=seen.max.x).contains(&x) && (seen.min.y..=seen.max.y).contains(&y)
    }

    fn bring_into_view(&mut self, s: &Suggestion) {
        if let Some(view) = self.canvas.schematic() {
            view.canvas.viewport.center = Vec2::new(s.at.x as f32, s.at.y as f32);
        }
    }

    pub(super) fn accept_all_proposals(&mut self) {
        let all = match self.backend.document() {
            Some(doc) => novo_terminals::suggest_terminals(doc.project()),
            None => return,
        };
        if all.is_empty() {
            self.message(Level::Info, "No terminal proposals");
            return;
        }
        let summary = novo_terminals::summarize_suggestions(&all);
        self.accept_terminals("Accept all terminals", &all);
        self.message(Level::Info, format!("Accepted {summary}"));
    }

    pub(super) fn accept_terminals(&mut self, label: &str, suggestions: &[Suggestion]) {
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact(label, |tx| {
            novo_terminals::accept_suggestions(tx, suggestions)
        }) {
            Ok((placed, changes)) => {
                let strip = placed
                    .first()
                    .and_then(|p| doc.project().terminal(p.terminal))
                    .map(|t| t.strip);
                if let (Some(view), Some(strip)) = (self.canvas.schematic(), strip) {
                    view.editor.strip = Some(strip);
                }
                self.changed_outside_canvas(&changes);
            }
            Err(e) => self.message(Level::Error, format!("Could not place the terminal: {e}")),
        }
    }

    pub(super) fn toggle_drawer(&mut self, tab: Drawer) {
        self.wb.drawer = if self.wb.drawer == Some(tab) {
            None
        } else {
            Some(tab)
        };
    }

    /// The strip the Terminals drawer shows: the selection's strip, else
    /// the one the editor is using, else the first.
    pub(super) fn drawer_strip(&self) -> Option<DeviceId> {
        let project = self.backend.document()?.project();
        let from_selection = self.selection().into_iter().find_map(|id| match id {
            AnyId::Symbol(s) => match &project.symbol(s)?.role {
                SymbolRole::Terminal { terminals } => {
                    project.terminal(*terminals.first()?).map(|t| t.strip)
                }
                SymbolRole::Device { device, .. } => project
                    .device(*device)
                    .filter(|d| d.kind == novo_core::DeviceKind::TerminalStrip)
                    .map(|d| d.id),
                _ => None,
            },
            AnyId::Device(d) => Some(d),
            _ => None,
        });
        from_selection
            .or_else(|| self.canvas.schematic_ref().and_then(|v| v.editor.strip))
            .filter(|s| project.device(*s).is_some())
            .or_else(|| novo_terminals::strips(project).first().copied())
    }

    /// The drawer, docked under the canvas while a tab is open.
    pub(super) fn drawer(&mut self, ui: &mut egui::Ui) {
        if self.wb.drawer.is_none() || !self.is_open() {
            return;
        }
        let height = (ui.ctx().content_rect().height() * DRAWER_SHARE).max(120.0);
        let strip = self.drawer_strip();
        let mut close = false;
        egui::Panel::bottom("drawer")
            .exact_size(height)
            .resizable(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let _ = ui.selectable_label(true, "Terminals");
                    for tab in ["Cables", "Devices"] {
                        ui.add_enabled(false, egui::Button::selectable(false, tab))
                            .on_disabled_hover_text("Comes later");
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("Close").clicked() {
                            close = true;
                        }
                        let proposals = self
                            .backend
                            .document()
                            .map_or(0, |d| novo_terminals::suggest_terminals(d.project()).len());
                        if proposals > 0 {
                            ui.weak(format!("{proposals} proposed"));
                        }
                    });
                });
                ui.separator();
                let Some(doc) = self.backend.document() else {
                    return;
                };
                let Some(strip) = strip else {
                    ui.weak("No terminal strip yet. Place a terminal (I, terminal) or accept a proposal.");
                    return;
                };
                let view = novo_terminals::strip_view(doc.project(), strip);
                strip_table(ui, &view);
            });
        if close {
            self.wb.drawer = None;
        }
    }
}

/// The strip editor's rows as a table: terminal, internal targets, external
/// targets, state.
pub(super) fn strip_table(ui: &mut egui::Ui, view: &novo_terminals::StripView) {
    ui.horizontal(|ui| {
        ui.strong(&view.tag);
        ui.weak(format!("{} terminals", view.rows.len()));
    });
    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        egui::Grid::new("strip")
            .striped(true)
            .num_columns(4)
            .spacing([24.0, 4.0])
            .show(ui, |ui| {
                for head in ["Terminal", "Internal", "External", "State"] {
                    ui.small(head);
                }
                ui.end_row();
                for row in &view.rows {
                    let join = |targets: Vec<&novo_terminals::Target>| {
                        targets
                            .iter()
                            .map(|t| t.designation.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    ui.label(&row.name);
                    ui.label(join(row.internal_targets()));
                    ui.label(join(row.external_targets()));
                    ui.weak(row.state().label());
                    ui.end_row();
                }
            });
    });
}

/// The world area a proposed terminal would take, around its spot.
fn proposal_area(s: &Suggestion) -> Aabb {
    let (x, y) = (s.at.x as f32, s.at.y as f32);
    Aabb::new(Vec2::new(x - 2.0, y - 3.0), Vec2::new(x + 2.0, y + 3.0))
}

/// Terminal proposals as canvas marks (DESIGN.md §5): every one dashed at
/// the proposal colour. The one Enter takes is marked twice, so it reads
/// heavier by opacity only, and carries the "↵ accept · Tab next" note,
/// which the canvas shows on the proposal nearest the pointer.
pub(super) fn proposal_marks(proposals: &[Suggestion], current: usize) -> Vec<Mark> {
    let current = current.min(proposals.len().saturating_sub(1));
    let mut marks = Vec::with_capacity(proposals.len() + 1);
    for (i, s) in proposals.iter().enumerate() {
        let mark = Mark {
            kind: MarkKind::Proposal,
            target: MarkTarget::Area(proposal_area(s)),
            text: if i == current {
                format!("{} terminal · ↵ accept · Tab next", s.strip)
            } else {
                String::new()
            },
        };
        // The texted mark goes first: of two marks on the same area the
        // canvas notes the first, and it skips marks without text.
        let heavier = (i == current).then(|| Mark {
            text: String::new(),
            ..mark.clone()
        });
        marks.push(mark);
        marks.extend(heavier);
    }
    marks
}
