//! Workspaces (docs/design/workspaces.md): what the canvas area shows and
//! which panels are home there. One is current; each remembers its own
//! panels, so leaving the Cabinet and coming back finds the drawer and the
//! outline as they were.

use eframe::egui;

use super::workbench::Drawer;
use super::{NovoApp, OutlineMode};
use crate::settings::WorkspaceLayout;

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

    /// The chip text on a narrow window.
    pub(crate) fn short_label(self) -> &'static str {
        match self {
            Workspace::Schematic => "Sch",
            Workspace::Cabinet => "Cab",
            Workspace::View3d => "3D",
            Workspace::Reports => "Rpt",
            Workspace::Commissioning => "Cmn",
        }
    }

    /// Whether the workspace has any content yet.
    fn is_built(self) -> bool {
        matches!(
            self,
            Workspace::Schematic
                | Workspace::Cabinet
                | Workspace::Reports
                | Workspace::Commissioning
        )
    }

    /// The panels a workspace starts with the first time it opens
    /// (design §3). Nothing differs from today's behaviour yet: the Cabinet
    /// starts like the schematic, with the drawer closed.
    pub(crate) fn default_layout(self) -> WorkspaceLayout {
        WorkspaceLayout {
            outline_pinned: None,
            outline_mode: outline_mode_id(OutlineMode::Pages).to_owned(),
            drawer: None,
        }
    }
}

fn outline_mode_id(mode: OutlineMode) -> &'static str {
    match mode {
        OutlineMode::Pages => "pages",
        OutlineMode::Issues => "issues",
        OutlineMode::Changes => "changes",
        OutlineMode::Commissioning => "commissioning",
    }
}

fn outline_mode_from_id(id: &str) -> OutlineMode {
    match id {
        "issues" => OutlineMode::Issues,
        "changes" => OutlineMode::Changes,
        "commissioning" => OutlineMode::Commissioning,
        _ => OutlineMode::Pages,
    }
}

fn drawer_id(drawer: Drawer) -> &'static str {
    match drawer {
        Drawer::Terminals => "terminals",
        Drawer::Cables => "cables",
        Drawer::Devices => "devices",
    }
}

fn drawer_from_id(id: &str) -> Option<Drawer> {
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
            Workspace::View3d => Some("The 3D view is not built yet"),
            Workspace::Commissioning if !self.is_open() => Some("Open a project first"),
            Workspace::Commissioning => None,
        }
    }

    /// Leaves the current workspace, keeping its panels as they are, and
    /// shows `workspace` with the panels it was left with. Does nothing for
    /// the current one or one that is unavailable.
    pub(crate) fn switch_workspace(&mut self, workspace: Workspace) {
        if workspace == self.workspace
            || !workspace.is_built()
            || self.workspace_unavailable(workspace).is_some()
        {
            return;
        }
        let left_commissioning = self.workspace == Workspace::Commissioning;
        let left = self.layout_snapshot();
        self.settings
            .workspaces
            .insert(self.workspace.id().to_owned(), left);
        self.workspace = workspace;
        self.settings.last_workspace = workspace.id().to_owned();
        let layout = self.layout_of(workspace);
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

    /// The layout `workspace` has stored, or its default.
    fn layout_of(&self, workspace: Workspace) -> WorkspaceLayout {
        self.settings
            .workspaces
            .get(workspace.id())
            .cloned()
            .unwrap_or_else(|| workspace.default_layout())
    }

    /// The panels as they are now.
    fn layout_snapshot(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            outline_pinned: self.settings.outline_pinned,
            outline_mode: outline_mode_id(self.outline_mode).to_owned(),
            drawer: self.wb.drawer.map(|d| drawer_id(d).to_owned()),
        }
    }

    fn apply_layout(&mut self, layout: &WorkspaceLayout) {
        self.settings.outline_pinned = layout.outline_pinned;
        self.outline_mode = outline_mode_from_id(&layout.outline_mode);
        self.wb.drawer = layout.drawer.as_deref().and_then(drawer_from_id);
        // A flyout left open would show over the next workspace's canvas.
        self.outline_open = false;
    }

    /// At start-up: the panels of the schematic as last left. The
    /// workspace that was last open follows once its project is open.
    pub(super) fn restore_layout_at_start(&mut self) {
        let layout = self.layout_of(Workspace::Schematic);
        self.apply_layout(&layout);
    }

    /// After the last project opened: back to the workspace it was left in,
    /// when that is available.
    pub(super) fn restore_workspace_at_start(&mut self) {
        if let Some(last) = Workspace::from_id(&self.settings.last_workspace) {
            self.switch_workspace(last);
        }
    }

    /// The switcher in the top bar: one chip per workspace. A chip that
    /// cannot open yet stays visible and says why on hover.
    pub(super) fn workspace_switcher(
        &mut self,
        ui: &mut egui::Ui,
        short: bool,
    ) -> Option<Workspace> {
        let mut picked = None;
        for workspace in Workspace::ALL {
            let reason = self.workspace_unavailable(workspace);
            let response = ui
                .add_enabled_ui(reason.is_none(), |ui| {
                    let label = if short {
                        workspace.short_label()
                    } else {
                        workspace.label()
                    };
                    novo_theme::widgets::segment(ui, label, workspace == self.workspace)
                })
                .inner;
            if response.clicked() {
                picked = Some(workspace);
            }
            if let Some(reason) = reason {
                response.on_disabled_hover_text(reason);
            } else if short {
                response.on_hover_text(workspace.label());
            }
        }
        picked
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
