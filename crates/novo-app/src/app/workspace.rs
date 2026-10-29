//! Workspaces (docs/design/workspaces.md): what the canvas area shows and
//! which panels are home there. One is current; each remembers its own
//! panels, so leaving the Cabinet and coming back finds the drawer and the
//! outline as they were.

use eframe::egui;

use super::workbench::Drawer;
use super::{NovoApp, OutlineMode};

/// The places the window can be in. Schematic and Cabinet are built; the
/// others have their switcher chip now (disabled, saying why) and gain
/// their content in later steps of the design's plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Workspace {
    #[default]
    Schematic,
    Cabinet,
    View3d,
    Reports,
    Commissioning,
}

impl Workspace {
    /// In switcher order (Ctrl+1 to Ctrl+5 once the keys move).
    pub(crate) const ALL: [Workspace; 5] = [
        Workspace::Schematic,
        Workspace::Cabinet,
        Workspace::View3d,
        Workspace::Reports,
        Workspace::Commissioning,
    ];

    /// Stable id, kept in the settings file.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Workspace::Schematic => "schematic",
            Workspace::Cabinet => "cabinet",
            Workspace::View3d => "3d",
            Workspace::Reports => "reports",
            Workspace::Commissioning => "commissioning",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Workspace> {
        Workspace::ALL.into_iter().find(|w| w.id() == id)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Workspace::Schematic => "Schematic",
            Workspace::Cabinet => "Cabinet",
            Workspace::View3d => "3D",
            Workspace::Reports => "Reports",
            Workspace::Commissioning => "Commissioning",
        }
    }

    /// The command that opens it (`view.cabinet`), as the menu and the
    /// switcher run it.
    pub(crate) fn command(self) -> &'static str {
        match self {
            Workspace::Schematic => "view.schematic",
            Workspace::Cabinet => "view.cabinet",
            Workspace::View3d => "view.3d",
            Workspace::Reports => "view.reports",
            Workspace::Commissioning => "view.commissioning",
        }
    }

    /// Whether the workspace has any content yet.
    fn is_built(self) -> bool {
        matches!(
            self,
            Workspace::Schematic
                | Workspace::Cabinet
                | Workspace::View3d
                | Workspace::Reports
                | Workspace::Commissioning
        )
    }
}

pub(super) fn outline_mode_id(mode: OutlineMode) -> &'static str {
    match mode {
        OutlineMode::Pages => "pages",
        OutlineMode::Issues => "issues",
        OutlineMode::Changes => "changes",
        OutlineMode::Commissioning => "commissioning",
    }
}

pub(super) fn outline_mode_from_id(id: &str) -> OutlineMode {
    match id {
        "issues" => OutlineMode::Issues,
        "changes" => OutlineMode::Changes,
        "commissioning" => OutlineMode::Commissioning,
        _ => OutlineMode::Pages,
    }
}

pub(super) fn drawer_id(drawer: Drawer) -> &'static str {
    match drawer {
        Drawer::Terminals => "terminals",
        Drawer::Cables => "cables",
        Drawer::Devices => "devices",
    }
}

pub(super) fn drawer_from_id(id: &str) -> Option<Drawer> {
    match id {
        "terminals" => Some(Drawer::Terminals),
        "cables" => Some(Drawer::Cables),
        "devices" => Some(Drawer::Devices),
        _ => None,
    }
}

impl NovoApp {
    /// The workspace on screen.
    pub(crate) fn workspace(&self) -> Workspace {
        self.workspace
    }

    /// Why `workspace` cannot be opened now, or `None` when it can. The
    /// switcher shows the reason as the tooltip of its disabled chip.
    pub(crate) fn workspace_unavailable(&self, workspace: Workspace) -> Option<&'static str> {
        match workspace {
            Workspace::Schematic => None,
            Workspace::Cabinet if !self.is_open() => Some("Open a project first"),
            Workspace::Cabinet => None,
            Workspace::Reports if !self.is_open() => Some("Open a project first"),
            Workspace::Reports => None,
            Workspace::View3d if !self.is_open() => Some("Open a project first"),
            Workspace::View3d if !self.view3d_has_cabinet() => {
                Some("3D needs a mounting plate: open Cabinet and press Set plate (Plate setup)")
            }
            Workspace::View3d => None,
            Workspace::Commissioning if !self.is_open() => Some("Open a project first"),
            Workspace::Commissioning => None,
        }
    }

    /// Leaves the current workspace, keeping its panels as they are, and
    /// shows `workspace` with the panels it was left with. Does nothing for
    /// the current one or one that is unavailable.
    pub(crate) fn switch_workspace(&mut self, workspace: Workspace) {
        if workspace == self.workspace || !workspace.is_built() {
            return;
        }
        if let Some(reason) = self.workspace_unavailable(workspace) {
            // A key or the command field gave no sign why nothing happened.
            self.message(super::Level::Info, reason);
            return;
        }
        let left_commissioning = self.workspace == Workspace::Commissioning;
        // A warning or an error is about the work done where it was raised; it
        // does not follow the person to the next workspace (it stays in the
        // log). Info toasts and the file toast of an export fade on their own.
        for m in &mut self.messages {
            if m.level != super::Level::Info {
                m.dismissed = true;
            }
        }
        // The live view draws only over the schematic; it does not wait
        // switched on for the next visit.
        if workspace != Workspace::Schematic {
            self.wb.live.leave();
        }
        self.store_layout();
        self.workspace = workspace;
        self.settings.last_workspace = workspace.id().to_owned();
        self.slot = self.slot_for(workspace);
        self.settings.last_slot = self.slot.id().to_owned();
        let layout = self.layout_of(self.slot);
        self.apply_layout(&layout);
        // Commissioning is the schematic page, read-only, with the
        // commissioning panel (design section 3.5): the mode goes on and
        // off with the workspace.
        if workspace == Workspace::Commissioning {
            self.enter_commissioning();
        } else if left_commissioning {
            self.leave_commissioning();
        }
        // The selection travels (§6): a row picked in Reports opens the
        // drawing on its page with the device selected.
        if workspace == Workspace::Schematic
            && let Some(place) = self.wb.pending
            && Some(place.page) != self.current_page
        {
            self.show_page(place.page);
            self.wb.pending = Some(place);
        }
    }

    /// After the last project opened: back to the workspace it was left in,
    /// when that is available.
    pub(super) fn restore_workspace_at_start(&mut self) {
        if let Some(slot) = super::layout_presets::Slot::from_id(&self.settings.last_slot)
            && slot.workspace() == Workspace::Schematic
        {
            self.schematic_slot = slot;
        }
        if let Some(last) = Workspace::from_id(&self.settings.last_workspace)
            && self.workspace_unavailable(last).is_none()
        {
            self.switch_workspace(last);
        }
    }

    /// The segment in the top bar (docs/design/visual-redesign.md section 3.1): one icon
    /// button per workspace in a single track. The current one is on the accent
    /// wash and shows its name; `names` says whether the others do too. A button
    /// that cannot open yet stays visible and says why on hover. Every button
    /// carries the full name for the accessibility tree and the QA harness, shown
    /// or not.
    pub(super) fn workspace_switcher(
        &mut self,
        ui: &mut egui::Ui,
        names: SegmentNames,
    ) -> Option<Workspace> {
        let pal = crate::theme::ui_palette();
        let mut picked = None;
        let height = 28.0;
        // The track behind the five buttons.
        let track = ui.painter().add(egui::Shape::Noop);
        let start = ui.cursor().min;
        let mut right = start.x;
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.add_space(2.0);
        for workspace in Workspace::ALL {
            let reason = self.workspace_unavailable(workspace);
            let current = workspace == self.workspace;
            let show_name = match names {
                SegmentNames::All => true,
                SegmentNames::Current => current,
                SegmentNames::None => false,
            };
            let label = workspace.label();
            let font = egui::TextStyle::Button.resolve(ui.style());
            let color = if reason.is_some() {
                pal.text_tertiary
            } else if current {
                pal.text_strong
            } else {
                pal.text_secondary
            };
            let galley =
                show_name.then(|| ui.painter().layout_no_wrap(label.to_owned(), font, color));
            let width = 8.0 + 16.0 + galley.as_ref().map_or(0.0, |g| 6.0 + g.size().x) + 8.0;
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(width, height),
                if reason.is_none() {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                },
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::Button,
                    reason.is_none(),
                    current,
                    label,
                )
            });
            let rounding = egui::CornerRadius::same(6);
            if current {
                ui.painter().rect_filled(rect, rounding, pal.accent_wash);
            } else if reason.is_none() && response.hovered() {
                ui.painter().rect_filled(rect, rounding, pal.hover);
            }
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect,
                    rounding,
                    egui::Stroke::new(if pal.high_contrast { 2.5 } else { 2.0 }, pal.accent),
                    egui::StrokeKind::Inside,
                );
            }
            workspace.glyph().paint(
                ui.painter(),
                egui::Rect::from_center_size(
                    egui::pos2(rect.left() + 16.0, rect.center().y),
                    egui::Vec2::splat(16.0),
                ),
                if current { pal.accent } else { color },
            );
            if let Some(galley) = galley {
                ui.painter().galley(
                    egui::pos2(rect.left() + 30.0, rect.center().y - galley.size().y / 2.0),
                    galley,
                    color,
                );
            }
            right = rect.right();
            if response.clicked() {
                picked = Some(workspace);
            }
            let (anchor, name) = match workspace {
                Workspace::Schematic => ("switcher.schematic", "Schematic chip"),
                Workspace::Cabinet => ("switcher.cabinet", "Cabinet chip"),
                Workspace::View3d => ("switcher.view3d", "3D chip"),
                Workspace::Reports => ("switcher.reports", "Reports chip"),
                Workspace::Commissioning => ("switcher.commissioning", "Commissioning chip"),
            };
            self.anchor(anchor, response.rect, name);
            if let Some(reason) = reason {
                response.on_disabled_hover_text(reason);
            } else if !show_name {
                response.on_hover_text(label);
            }
        }
        ui.add_space(2.0);
        let outer = egui::Rect::from_min_max(
            egui::pos2(start.x, ui.cursor().top()),
            egui::pos2(right + 2.0, ui.cursor().top() + height),
        );
        ui.painter().set(
            track,
            egui::Shape::rect_filled(outer, egui::CornerRadius::same(8), pal.control),
        );
        picked
    }
}

/// Which workspace buttons show their name (the current one always does, except
/// when there is no room at all).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SegmentNames {
    /// Every name: from 1500 points.
    All,
    /// Only the current one: 1100 to 1500.
    Current,
    /// Icons only: under 760 points.
    None,
}

impl Workspace {
    /// The icon of the workspace's button.
    pub(crate) fn glyph(self) -> novo_theme::presentation::Glyph {
        use novo_theme::presentation::Glyph;
        match self {
            Workspace::Schematic => Glyph::Drawn(&novo_theme::drawn::SCHEMATIC),
            Workspace::Cabinet => Glyph::Drawn(&novo_theme::drawn::CABINET),
            Workspace::View3d => Glyph::Phosphor(novo_theme::icons::CUBE),
            Workspace::Reports => Glyph::Drawn(&novo_theme::drawn::REPORT),
            Workspace::Commissioning => Glyph::Phosphor(novo_theme::icons::CLIPBOARD_TEXT),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;
    use crate::app::gate1_tests::Drive;

    #[test]
    fn ids_round_trip() {
        for w in Workspace::ALL {
            assert_eq!(Workspace::from_id(w.id()), Some(w));
        }
        assert_eq!(Workspace::from_id("nope"), None);
    }

    #[test]
    fn unbuilt_workspaces_say_why_and_do_not_open() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        let w = Workspace::View3d;
        assert!(d.app.workspace_unavailable(w).is_some());
        d.app.switch_workspace(w);
        assert_eq!(d.app.workspace(), Workspace::Schematic);
        assert!(d.app.workspace_unavailable(Workspace::Cabinet).is_none());
    }

    #[test]
    fn commissioning_is_a_workspace_that_turns_the_mode_on_and_off() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        assert!(
            d.app
                .workspace_unavailable(Workspace::Commissioning)
                .is_none()
        );
        d.app.switch_workspace(Workspace::Commissioning);
        d.frame();
        assert_eq!(d.app.workspace(), Workspace::Commissioning);
        assert!(d.app.commissioning_locks_edits());
        d.app.switch_workspace(Workspace::Schematic);
        d.frame();
        assert!(!d.app.commissioning_locks_edits());
        assert!(d.app.wb.commissioning.is_none());
    }

    #[test]
    fn commissioning_survives_jumps_a_new_project_and_stays_locked() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.app.switch_workspace(Workspace::Commissioning);
        d.frame();
        // A jump from the panel shows a page without leaving.
        let page = d.app.current_page.expect("a page");
        d.app.show_page(page);
        d.frame();
        assert_eq!(d.app.workspace(), Workspace::Commissioning);
        assert!(d.app.wb.commissioning.is_some());
        // A new project drops the store; the workspace loads it again and
        // the lock never lets go.
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.frame();
        assert_eq!(d.app.workspace(), Workspace::Commissioning);
        assert!(d.app.wb.commissioning.is_some());
        assert!(d.app.commissioning_locks_edits());
        // Another outline tab leaves the workspace and unlocks.
        d.app.outline_mode = OutlineMode::Pages;
        d.frame();
        assert_eq!(d.app.workspace(), Workspace::Schematic);
        assert!(!d.app.commissioning_locks_edits());
        // Closing the project leaves it too.
        d.app.switch_workspace(Workspace::Commissioning);
        d.frame();
        d.app.run(&ctx, Action::CloseProject);
        d.frame();
        assert_eq!(d.app.workspace(), Workspace::Schematic);
    }

    #[test]
    fn an_error_toast_does_not_follow_the_person_to_another_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.app.message(
            crate::app::Level::Error,
            "Cannot rename -T2: both symbols are main functions",
        );
        d.app
            .message(crate::app::Level::Warning, "A warning about the drawing");
        d.app
            .message(crate::app::Level::Info, "Placed Motor starter DOL");
        assert!(d.app.messages.iter().all(|m| !m.dismissed));
        d.app.run(&ctx, Action::ShowCabinet);
        d.frame();
        let up = |level| {
            d.app
                .messages
                .iter()
                .any(|m| m.level == level && !m.dismissed)
        };
        assert!(
            !up(crate::app::Level::Error),
            "the error left with the schematic"
        );
        assert!(!up(crate::app::Level::Warning));
        assert!(
            up(crate::app::Level::Info),
            "an info toast fades on its own"
        );
        // The log keeps them.
        assert_eq!(d.app.messages.len(), 3);
    }

    #[test]
    fn each_workspace_keeps_its_panels() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        // The schematic gets the Cables drawer and the Issues outline.
        d.app.wb.drawer = Some(Drawer::Cables);
        d.app.outline_mode = OutlineMode::Issues;
        d.app.run(&ctx, Action::ShowCabinet);
        d.frame();
        // The Cabinet starts from its default, not from the schematic's.
        assert_eq!(d.app.workspace(), Workspace::Cabinet);
        assert_eq!(d.app.wb.drawer, None);
        assert_eq!(d.app.outline_mode, OutlineMode::Pages);
        d.app.wb.drawer = Some(Drawer::Devices);
        d.app.run(&ctx, Action::ShowSchematic);
        d.frame();
        assert_eq!(d.app.wb.drawer, Some(Drawer::Cables));
        assert_eq!(d.app.outline_mode, OutlineMode::Issues);
        d.app.run(&ctx, Action::ShowCabinet);
        d.frame();
        assert_eq!(d.app.wb.drawer, Some(Drawer::Devices));
        assert_eq!(d.app.settings.last_workspace, "cabinet");
    }

    #[test]
    fn layouts_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.app.wb.drawer = Some(Drawer::Terminals);
        d.app.run(&ctx, Action::ShowCabinet);
        d.frame();
        let text = ron::to_string(&d.app.settings).unwrap();
        let settings: crate::settings::Settings = ron::from_str(&text).unwrap();
        let mut again = NovoApp::with_settings(
            Box::new(crate::backend::MemoryBackend::default()),
            Box::new(crate::editor_canvas::EditorCanvas::default()),
            settings,
        );
        assert_eq!(again.wb.drawer, Some(Drawer::Terminals));
        assert_eq!(again.workspace(), Workspace::Schematic);
        // No project yet, so the Cabinet is not offered back.
        again.restore_workspace_at_start();
        assert_eq!(again.workspace(), Workspace::Schematic);
    }
}
