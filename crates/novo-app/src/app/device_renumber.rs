//! Renumber devices: the `edit.renumber` command. A window with the scope
//! (project, this page, the selection) and a preview of every old and new
//! tag; Renumber applies it as one undo step.
//!
//! Where the user finds it: Edit > Renumber > Renumber devices, or Ctrl+K
//! "renumber" (with `page`, `selection` or `project` to pick the scope).

use eframe::egui;
use novo_core::{DeviceId, PageId};
use novo_workflow::renumber::{self, Renaming, Scope};

use super::{Level, NovoApp};

/// Which devices the window works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeviceScope {
    Project,
    ThisPage,
    Selection,
}

impl DeviceScope {
    /// The scope a typed argument names (`renumber page`).
    pub(crate) fn from_argument(argument: Option<&str>) -> Option<Self> {
        match argument?.trim().to_lowercase().as_str() {
            "project" => Some(Self::Project),
            "page" => Some(Self::ThisPage),
            "selection" => Some(Self::Selection),
            _ => None,
        }
    }
}

/// The Renumber devices window while it is open.
pub(crate) struct DeviceRenumber {
    pub(crate) scope: DeviceScope,
    /// The devices of the canvas selection when it opened.
    selected: Vec<DeviceId>,
    page: Option<PageId>,
    /// The plan for `(revision, scope)`.
    shown: Option<(u64, DeviceScope, Vec<Renaming>)>,
}

impl DeviceRenumber {
    fn scope(&self) -> Option<Scope> {
        match self.scope {
            DeviceScope::Project => Some(Scope::Project),
            DeviceScope::ThisPage => self.page.map(Scope::Page),
            DeviceScope::Selection => Some(Scope::Devices(self.selected.clone())),
        }
    }
}

impl NovoApp {
    /// Opens the window. A selection starts on "Selection", else the
    /// argument or the whole project decides.
    pub(super) fn open_device_renumber(&mut self, argument: Option<&str>) {
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let mut selected: Vec<DeviceId> = self
            .selection()
            .into_iter()
            .filter_map(|id| novo_editor::edit::device_of(project, id))
            .collect();
        selected.sort();
        selected.dedup();
        let scope = DeviceScope::from_argument(argument).unwrap_or(if selected.is_empty() {
            DeviceScope::Project
        } else {
            DeviceScope::Selection
        });
        self.wb.device_renumber = Some(DeviceRenumber {
            scope,
            selected,
            page: self.current_page,
            shown: None,
        });
    }

    /// Applies a renumbering plan as one undo step.
    pub(super) fn apply_device_renumber(&mut self, plan: &[Renaming]) {
        if plan.is_empty() {
            return;
        }
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        match doc.transact("Renumber devices", |tx| renumber::apply(tx, plan)) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(Level::Info, format!("Renumbered {} devices", plan.len()));
            }
            Err(e) => self.message(Level::Error, format!("Renumber devices: {e}")),
        }
    }

    pub(super) fn device_renumber_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.device_renumber.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        // A scope with nothing behind it falls back to the project.
        if (win.scope == DeviceScope::ThisPage && win.page.is_none())
            || (win.scope == DeviceScope::Selection && win.selected.is_empty())
        {
            win.scope = DeviceScope::Project;
        }
        let revision = project.revision();
        if win
            .shown
            .as_ref()
            .is_none_or(|(r, s, _)| *r != revision || *s != win.scope)
        {
            let plan = win
                .scope()
                .map(|scope| renumber::plan(project, &scope))
                .unwrap_or_default();
            win.shown = Some((revision, win.scope, plan));
        }
        let mut open = true;
        let mut apply = false;
        egui::Window::new("Renumber devices")
            .open(&mut open)
            .default_size([420.0, 420.0])
            .show(ctx, |ui| {
                ui.weak(
                    "Numbers each letter code from 1 in reading order: page, column, top to \
bottom. Numbers used outside the scope are skipped. Terminal strips keep their tags.",
                );
                ui.horizontal(|ui| {
                    ui.label("Devices");
                    ui.selectable_value(&mut win.scope, DeviceScope::Project, "Project");
                    ui.add_enabled_ui(win.page.is_some(), |ui| {
                        ui.selectable_value(&mut win.scope, DeviceScope::ThisPage, "This page")
                    });
                    ui.add_enabled_ui(!win.selected.is_empty(), |ui| {
                        ui.selectable_value(&mut win.scope, DeviceScope::Selection, "Selection")
                    });
                });
                ui.separator();
                let plan: &[Renaming] = win.shown.as_ref().map_or(&[], |(_, _, p)| p);
                if plan.is_empty() {
                    ui.weak("Nothing to change: the numbers already run in reading order");
                } else {
                    ui.weak(format!("{} devices change", plan.len()));
                    egui::ScrollArea::vertical()
                        .max_height(260.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            egui::Grid::new("device_renumber_preview")
                                .num_columns(2)
                                .striped(true)
                                .show(ui, |ui| {
                                    for r in plan {
                                        ui.label(r.old.to_string());
                                        ui.label(format!("\u{2192} {}", r.new));
                                        ui.end_row();
                                    }
                                });
                        });
                }
                ui.separator();
                if ui
                    .add_enabled(!plan.is_empty(), egui::Button::new("Renumber"))
                    .on_hover_text("One undo step")
                    .clicked()
                {
                    apply = true;
                }
            });
        if apply {
            let plan = win.shown.take().map(|(_, _, p)| p).unwrap_or_default();
            self.apply_device_renumber(&plan);
            return;
        }
        if open && !ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.wb.device_renumber = Some(win);
        }
    }
}
