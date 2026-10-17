//! Layout presets: named places to work (Draw, Review, Cabinet, 3D and heat,
//! Reports, Commissioning), each remembering its own panels for each window
//! size (docs/design/workspaces.md §4.1).
//!
//! A *slot* is one named layout. Each slot belongs to a workspace; Draw and
//! Review are both the schematic with different panels (Review keeps the
//! issue list beside the drawing). A *size class* is the window width in
//! points, which already includes the display scaling: a 1920 px screen at
//! 150% is 1280 points and gets the compact layouts, at 125% it is 1536
//! points and gets the regular ones. A window under 800 points high is
//! also short, and keeps its layouts apart (`"<slot>/<class>/short"`; until
//! one is stored it starts from the layout of the same width).
//!
//! The panels a slot keeps are the outline (pinned or flyout, and what it
//! lists), the drawer tab, whether the inspector stays docked, and where the
//! person left the floating panels (the inspectors, the lesson card, the 3D
//! toolbar; their size is egui's own and is shared by all layouts). Leaving a
//! slot, changing size class or closing the app stores them under
//! `"<slot>/<class>"` in the settings; coming back puts them back. A slot
//! never opened at a size starts from the default below.
//!
//! Where the user finds it: View › Layouts, the command field (`layout`),
//! and the workspace chips, which open the slot last used in them.

use std::collections::BTreeMap;

use super::workspace::{
    Workspace, drawer_from_id, drawer_id, outline_mode_from_id, outline_mode_id,
};
use super::{Level, NovoApp, OutlineMode};
use crate::settings::WorkspaceLayout;

/// A window narrower than this many points is compact (a 1366 px laptop).
pub(super) const COMPACT_BELOW: f32 = 1400.0;
/// From this width the outline pins itself and the drawer may dock.
pub(super) const WIDE_FROM: f32 = 1700.0;
/// A window shorter than this many points is short (a 1366 x 768 laptop, or
/// 1920 x 1080 at 150%): a layout is kept for it apart from the taller ones.
pub(super) const SHORT_BELOW: f32 = 800.0;
/// A window must pass a boundary by this much before the class changes, so
/// dragging the window edge across it does not flip the panels back and
/// forth.
const HYSTERESIS: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SizeClass {
    Compact,
    Regular,
    Wide,
}

impl SizeClass {
    pub(crate) fn id(self) -> &'static str {
        match self {
            SizeClass::Compact => "compact",
            SizeClass::Regular => "regular",
            SizeClass::Wide => "wide",
        }
    }

    fn raw(width: f32) -> SizeClass {
        if width < COMPACT_BELOW {
            SizeClass::Compact
        } else if width < WIDE_FROM {
            SizeClass::Regular
        } else {
            SizeClass::Wide
        }
    }

    /// The class for `width`, staying in `current` while the width is
    /// within the hysteresis band around its edges.
    pub(crate) fn next(current: Option<SizeClass>, width: f32) -> SizeClass {
        let raw = SizeClass::raw(width);
        let Some(current) = current else { return raw };
        let (lo, hi) = match current {
            SizeClass::Compact => (0.0, COMPACT_BELOW),
            SizeClass::Regular => (COMPACT_BELOW, WIDE_FROM),
            SizeClass::Wide => (WIDE_FROM, f32::MAX),
        };
        if width >= lo - HYSTERESIS && width < hi + HYSTERESIS {
            current
        } else {
            raw
        }
    }
}

/// Whether the window is short, staying as it was within the hysteresis
/// band around [`SHORT_BELOW`]. A height of 0 (not measured yet) is tall.
pub(crate) fn next_short(current: Option<bool>, height: f32) -> bool {
    let raw = height > 0.0 && height < SHORT_BELOW;
    match current {
        Some(current) if (height - SHORT_BELOW).abs() < HYSTERESIS => current,
        _ => raw,
    }
}

/// One named layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Slot {
    #[default]
    Draw,
    Review,
    Cabinet,
    ThreeD,
    Reports,
    Commissioning,
}

impl Slot {
    /// The presets in menu order. Commissioning is the workspace's own
    /// layout and has no preset command.
    pub(crate) const PRESETS: [Slot; 5] = [
        Slot::Draw,
        Slot::Cabinet,
        Slot::ThreeD,
        Slot::Review,
        Slot::Reports,
    ];
    const ALL: [Slot; 6] = [
        Slot::Draw,
        Slot::Review,
        Slot::Cabinet,
        Slot::ThreeD,
        Slot::Reports,
        Slot::Commissioning,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Slot::Draw => "draw",
            Slot::Review => "review",
            Slot::Cabinet => "cabinet",
            Slot::ThreeD => "3d",
            Slot::Reports => "reports",
            Slot::Commissioning => "commissioning",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Slot> {
        Slot::ALL.into_iter().find(|s| s.id() == id)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Slot::Draw => "Draw",
            Slot::Review => "Review",
            Slot::Cabinet => "Cabinet",
            Slot::ThreeD => "3D and heat",
            Slot::Reports => "Reports",
            Slot::Commissioning => "Commissioning",
        }
    }

    /// The command that opens it.
    pub(crate) fn command(self) -> &'static str {
        match self {
            Slot::Draw => "layout.draw",
            Slot::Review => "layout.review",
            Slot::Cabinet => "layout.cabinet",
            Slot::ThreeD => "layout.3d",
            Slot::Reports => "layout.reports",
            Slot::Commissioning => "view.commissioning",
        }
    }

    pub(crate) fn from_command(command: &str) -> Option<Slot> {
        Slot::PRESETS.into_iter().find(|s| s.command() == command)
    }

    pub(crate) fn workspace(self) -> Workspace {
        match self {
            Slot::Draw | Slot::Review => Workspace::Schematic,
            Slot::Cabinet => Workspace::Cabinet,
            Slot::ThreeD => Workspace::View3d,
            Slot::Reports => Workspace::Reports,
            Slot::Commissioning => Workspace::Commissioning,
        }
    }

    /// The key a layout written before sizes existed was stored under.
    fn legacy_key(self) -> Option<&'static str> {
        match self {
            Slot::Review => None,
            Slot::Draw => Some("schematic"),
            other => Some(other.id()),
        }
    }

    /// The key a layout is stored under: `"<slot>/<width class>"`, and
    /// `"<slot>/<width class>/short"` for a short window.
    pub(crate) fn key(self, class: SizeClass, short: bool) -> String {
        let plain = format!("{}/{}", self.id(), class.id());
        if short {
            format!("{plain}/short")
        } else {
            plain
        }
    }

    /// The panels the slot starts with at a window size, before the person
    /// has arranged them.
    pub(crate) fn default_layout(self, class: SizeClass) -> WorkspaceLayout {
        let pages = outline_mode_id(OutlineMode::Pages).to_owned();
        let base = WorkspaceLayout {
            outline_pinned: None,
            outline_mode: pages,
            drawer: None,
            inspector_pinned: Some(false),
            panels: BTreeMap::new(),
        };
        match self {
            // The drawing and nothing else; the outline follows the width.
            Slot::Draw => base,
            // The drawing with the issue list beside it, docked at any size.
            Slot::Review => WorkspaceLayout {
                outline_pinned: Some(true),
                outline_mode: outline_mode_id(OutlineMode::Issues).to_owned(),
                inspector_pinned: Some(class == SizeClass::Wide),
                ..base
            },
            // The plate, the outline docked where there is room, and the
            // inspector docked on a wide window so placements stay in view.
            Slot::Cabinet => WorkspaceLayout {
                inspector_pinned: Some(class == SizeClass::Wide),
                ..base
            },
            // The whole window for the view; the outline is a flyout.
            Slot::ThreeD => WorkspaceLayout {
                outline_pinned: Some(false),
                ..base
            },
            Slot::Reports => base,
            Slot::Commissioning => WorkspaceLayout {
                inspector_pinned: None,
                ..base
            },
        }
    }
}

impl NovoApp {
    /// The size class now; before the first frame measured the window,
    /// regular.
    fn size_class(&self) -> SizeClass {
        self.size_class.unwrap_or(SizeClass::Regular)
    }

    /// The slot a workspace opens in: Draw or Review for the schematic,
    /// whichever was used last.
    pub(super) fn slot_for(&self, workspace: Workspace) -> Slot {
        match workspace {
            Workspace::Schematic => self.schematic_slot,
            Workspace::Cabinet => Slot::Cabinet,
            Workspace::View3d => Slot::ThreeD,
            Workspace::Reports => Slot::Reports,
            Workspace::Commissioning => Slot::Commissioning,
        }
    }

    /// The layout stored for `slot` at the current size: its own, else (in a
    /// short window) the one for the same width, else the one written before
    /// sizes existed, else the default.
    pub(super) fn layout_of(&self, slot: Slot) -> WorkspaceLayout {
        let class = self.size_class();
        let stored = &self.settings.workspaces;
        stored
            .get(&slot.key(class, self.window_short))
            .or_else(|| stored.get(&slot.key(class, false)))
            .or_else(|| slot.legacy_key().and_then(|k| stored.get(k)))
            .cloned()
            .unwrap_or_else(|| slot.default_layout(class))
    }

    /// The panels as they are now.
    pub(super) fn layout_snapshot(&self) -> WorkspaceLayout {
        WorkspaceLayout {
            outline_pinned: self.settings.outline_pinned,
            outline_mode: outline_mode_id(self.outline_mode).to_owned(),
            drawer: self.wb.drawer.map(|d| drawer_id(d).to_owned()),
            inspector_pinned: Some(self.wb.inspector.pinned),
            panels: self.panel_places.clone(),
        }
    }

    /// Once a frame, before the window size is followed: hands a layout's
    /// panel positions to the panels, or reads where the panels are now so
    /// that the next store keeps them.
    pub(super) fn sync_panels(&mut self, ctx: &eframe::egui::Context) {
        if let Some(layout) = self.panel_pending.take() {
            super::panel::place_all(ctx, &layout);
            self.panel_places = layout;
        } else {
            self.panel_places = super::panel::places(ctx);
        }
    }

    /// Keeps the panels of the current slot at the current size.
    pub(super) fn store_layout(&mut self) {
        let snapshot = self.layout_snapshot();
        self.settings.workspaces.insert(
            self.slot.key(self.size_class(), self.window_short),
            snapshot,
        );
    }

    pub(super) fn apply_layout(&mut self, layout: &WorkspaceLayout) {
        self.settings.outline_pinned = layout.outline_pinned;
        self.outline_mode = outline_mode_from_id(&layout.outline_mode);
        self.wb.drawer = layout.drawer.as_deref().and_then(drawer_from_id);
        if let Some(pinned) = layout.inspector_pinned {
            self.wb.inspector.pinned = pinned;
        }
        self.panel_pending = Some(layout.panels.clone());
        // A flyout left open would show over the next layout's canvas.
        self.outline_open = false;
    }

    /// At start-up: the panels of the drawing as last left. The workspace
    /// that was last open follows once its project is open.
    pub(super) fn restore_layout_at_start(&mut self) {
        if let Some(slot) = Slot::from_id(&self.settings.last_slot)
            && slot.workspace() == Workspace::Schematic
        {
            self.schematic_slot = slot;
        }
        self.slot = self.schematic_slot;
        let layout = self.layout_of(self.slot);
        self.apply_layout(&layout);
    }

    /// The window height as last measured, with the width given.
    #[cfg(test)]
    pub(super) fn follow_window_size(&mut self, width: f32) {
        self.follow_window(width, self.window_height);
    }

    /// Called every frame with the window size in points. The first call
    /// puts the panels of the current slot for the size; a later change of
    /// class (a resized window, another screen, a new display scale) stores
    /// the panels for the old size and puts back those of the new one.
    /// A window under [`SHORT_BELOW`] points high keeps its own layouts too.
    pub(super) fn follow_window(&mut self, width: f32, height: f32) {
        if width <= 0.0 {
            return;
        }
        let class = SizeClass::next(self.size_class, width);
        let short = next_short(self.size_class.map(|_| self.window_short), height);
        if self.size_class == Some(class) && self.window_short == short {
            return;
        }
        if self.size_class.is_some() {
            self.store_layout();
        }
        self.size_class = Some(class);
        self.window_short = short;
        let layout = self.layout_of(self.slot);
        self.apply_layout(&layout);
        // The Commissioning workspace always shows its own panel; a stored
        // layout must not turn it back to another outline tab (that would
        // leave the workspace).
        if self.workspace == Workspace::Commissioning {
            self.outline_mode = OutlineMode::Commissioning;
        }
    }

    /// Opens a preset: its workspace, with the panels it was left with at
    /// this window size.
    pub(super) fn open_slot(&mut self, slot: Slot) {
        let workspace = slot.workspace();
        if let Some(reason) = self.workspace_unavailable(workspace) {
            self.message(Level::Info, format!("{}: {reason}.", slot.label()));
            return;
        }
        if workspace == Workspace::Schematic {
            self.schematic_slot = slot;
        }
        if workspace != self.workspace {
            self.switch_workspace(workspace);
        } else if slot != self.slot {
            // Draw and Review share the schematic: only the panels change.
            self.store_layout();
            self.slot = slot;
            self.settings.last_slot = slot.id().to_owned();
            let layout = self.layout_of(slot);
            self.apply_layout(&layout);
        }
    }

    /// Puts the current slot back to its default at this size.
    pub(super) fn reset_slot_layout(&mut self) {
        let class = self.size_class();
        self.settings
            .workspaces
            .remove(&self.slot.key(class, self.window_short));
        if let Some(legacy) = self.slot.legacy_key() {
            self.settings.workspaces.remove(legacy);
        }
        let layout = self.slot.default_layout(class);
        self.apply_layout(&layout);
        self.message(
            Level::Info,
            format!("{} layout reset for this window size.", self.slot.label()),
        );
    }

    /// Forgets every stored layout, at every size.
    pub(super) fn reset_all_layouts(&mut self) {
        self.settings.workspaces.clear();
        let layout = self.slot.default_layout(self.size_class());
        self.apply_layout(&layout);
        self.message(Level::Info, "All workspace layouts reset.");
    }

    /// Runs a layout command; false when `id` is not one.
    pub(super) fn run_layout_command(&mut self, id: &str) -> bool {
        if let Some(slot) = Slot::from_command(id) {
            self.open_slot(slot);
        } else if id == "layout.reset" {
            self.reset_slot_layout();
        } else if id == "layout.reset_all" {
            self.reset_all_layouts();
        } else {
            return false;
        }
        true
    }
}

#[cfg(test)]
#[path = "layout_presets_tests.rs"]
mod tests;
