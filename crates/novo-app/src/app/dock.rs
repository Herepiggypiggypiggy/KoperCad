//! What the tool in hand says (the hint, the proposals count) and the editor
//! settings kept in step with the person's choices. The tool dock that used
//! to live here is the tool rail now (`rail.rs`, `rail_schematic.rs`).

use novo_editor::tool::WiringMode;

use super::NovoApp;

/// What the tool in hand has to say (see `NovoApp::tool_hint`).
#[derive(Default)]
pub(super) struct ToolHint {
    pub(super) text: String,
    /// The text is the empty-page tip, which gives way to a message.
    pub(super) tip: bool,
    pub(super) refused: bool,
    pub(super) proposals: usize,
}

impl NovoApp {
    /// Chooses the wiring mode: remembered for this person, applied to the
    /// editor, and named in the status line. `begin` also starts Connect.
    pub(super) fn set_wiring_mode(&mut self, mode: WiringMode, begin: bool) {
        self.settings.set_wiring_mode(mode);
        if let Some(view) = self.canvas.schematic() {
            view.editor.set_wire_mode(mode);
            if begin {
                view.editor.begin_wire();
            }
        }
        if !begin {
            let name = match mode {
                WiringMode::Corners => "Corners",
                WiringMode::Lines => "Lines",
            };
            self.message(super::Level::Info, format!("Wiring mode: {name}"));
        }
    }

    /// Keeps the editor on the person's wiring mode (the setting is read
    /// on start and changes only here).
    pub(super) fn sync_wiring_mode(&mut self) {
        let mode = self.settings.wiring_mode();
        if let Some(view) = self.canvas.schematic()
            && view.editor.wire_mode() != mode
        {
            view.editor.set_wire_mode(mode);
        }
    }

    /// Keeps the editor on the person's auto-connection mode.
    pub(super) fn sync_connect_mode(&mut self) {
        let mode = self.settings.connect_mode();
        let carries = !self.settings.move_leaves_wires;
        if let Some(view) = self.canvas.schematic() {
            if view.editor.connect_mode() != mode {
                view.editor.set_connect_mode(mode);
            }
            view.move_carries = carries;
            view.reduce_motion = self.settings.reduce_motion;
            view.canvas.glide_secs = super::motion::glide_secs(self.settings.reduce_motion);
        }
    }

    /// Gives the context menu the keys the keymap binds, so its rows show
    /// the current key and never a stale one.
    pub(super) fn sync_menu_keys(&mut self) {
        let keys: std::collections::HashMap<&'static str, String> =
            novo_editor::view::SchematicView::MENU_COMMANDS
                .iter()
                .filter_map(|&id| self.wb.keymap.shortcut_text(id).map(|k| (id, k)))
                .collect();
        if let Some(view) = self.canvas.schematic() {
            view.set_shortcuts(keys);
        }
    }

    /// Keeps the editor's symbol catalog on the project's drawing standard.
    /// The recently used symbols follow to the new pack.
    pub(super) fn sync_catalog(&mut self) {
        let pack = self.standard();
        if self.sync_symbol_folder(pack) {
            return;
        }
        let Some(view) = self.canvas.schematic() else {
            return;
        };
        // A company library has no single pack and is left alone.
        if view
            .editor
            .catalog
            .library()
            .pack()
            .is_none_or(|p| p == pack)
        {
            return;
        }
        let mut catalog = novo_editor::Catalog::builtin_pack(pack);
        for id in view.editor.catalog.recent().iter().rev() {
            let id = catalog.in_pack(id);
            if catalog.get(&id).is_some() {
                catalog.used(&id);
            }
        }
        // The favourites row keeps its order, each in the new pack.
        let favourites: Vec<String> = view
            .editor
            .catalog
            .favourites()
            .iter()
            .map(|id| catalog.in_pack(id))
            .collect();
        catalog.set_favourites(favourites);
        view.editor.catalog = catalog;
    }

    /// What the drawing has to say about the tool in hand: a refusal, a
    /// step hint, the rest of the selected device, the first move on an
    /// empty page, and how many proposals wait. The status bar shows it; the
    /// dock does only when the bar is off.
    pub(super) fn tool_hint(&mut self) -> ToolHint {
        // With nothing in hand, a selected coil or contact names where the
        // rest of its device is drawn.
        let elsewhere = match self.selected_symbol() {
            Some(symbol) => self
                .xref()
                .map(|index| index.highlight(symbol).dock_hint())
                .unwrap_or_default(),
            None => String::new(),
        };
        let Some(view) = self.canvas.schematic_ref() else {
            return ToolHint::default();
        };
        let hint = view
            .status_text()
            .filter(|h| !h.is_empty())
            .unwrap_or(elsewhere);
        // Dragging a selection box says what letting go would take.
        let hint = match view.band_count() {
            Some(n) => format!("{n} selected"),
            None => hint,
        };
        // Nothing else to say on an empty page: name the first move.
        let (hint, tip) = if hint.is_empty() {
            match self.empty_page_hint() {
                Some(t) => (t, true),
                None => (hint, false),
            }
        } else {
            (hint, false)
        };
        ToolHint {
            text: hint,
            tip,
            // A refusal reads as one: the Error tint and a warning icon,
            // never colour alone (DESIGN.md §4).
            refused: view.editor.notice().is_some(),
            proposals: view.editor.proposals().len() + self.page_proposals().len(),
        }
    }
}
