//! The tool dock (DESIGN.md §3), floating at the bottom centre of the
//! canvas: Select, Place, Connect and Text, the hint for the current step,
//! and the proposals count. It moves aside while a gesture is in progress
//! near it rather than fading.

use eframe::egui;
use novo_editor::Tool;
use novo_editor::tool::WiringMode;
use novo_theme::{icons, widgets};

use super::NovoApp;
use novo_workflow::palette::Mode;

/// What a click on the dock asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DockTool {
    Select,
    Place,
    Connect,
    Cable,
    Text,
}

/// What the tool in hand has to say (see `NovoApp::tool_hint`).
#[derive(Default)]
pub(super) struct ToolHint {
    pub(super) text: String,
    /// The text is the empty-page tip, which gives way to a message.
    pub(super) tip: bool,
    pub(super) refused: bool,
    pub(super) proposals: usize,
}

/// How close to the dock a gesture has to come to move it aside.
const NEAR: f32 = 32.0;
/// The dock's distance from the canvas edge.
const MARGIN: f32 = 12.0;
/// The canvas width from which Auto puts a word beside each dock icon: the
/// labelled dock is about 630 wide, plus room to either side.
pub(super) const LABELS_MIN_CANVAS: f32 = 720.0;

/// One dock button: an icon, plus its word when `labels` is on.
fn dock_button(
    ui: &mut egui::Ui,
    labels: bool,
    icon: &str,
    label: &str,
    active: bool,
    tip: &str,
) -> egui::Response {
    if labels {
        widgets::tool_button_labeled(ui, icon, label, active, tip)
    } else {
        widgets::tool_button(ui, icon, active, tip)
    }
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

    pub(super) fn tool_dock(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let ToolHint {
            text: hint,
            refused,
            proposals,
            ..
        } = self.tool_hint();
        // The status bar carries the hint; the dock keeps to its controls so
        // they never move.
        let in_bar = self.status_bar_shown();
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        if !self.is_open() {
            return;
        }
        let active = match view.editor.tool() {
            Tool::Place(_) => DockTool::Place,
            Tool::Wire(_) => DockTool::Connect,
            Tool::Cable(_) => DockTool::Cable,
            Tool::Text => DockTool::Text,
            Tool::Select | Tool::Move(_) | Tool::Paste(_) => DockTool::Select,
        };
        // Aside: the bottom right corner, while the pointer works near the
        // dock's usual place (a drag, or a tool following the pointer).
        // Measured against that place, so the dock does not flicker back.
        let size = self.wb.dock_size;
        let home = egui::Rect::from_min_size(
            canvas.center_bottom() - egui::vec2(size.x / 2.0, size.y + MARGIN),
            size,
        );
        // A gesture: a drag that did not start on the dock, a wire under
        // way or a move. A tool merely being active is not one, so the dock
        // stays put to be clicked.
        let dragging = ctx.input(|i| {
            i.pointer.any_down() && i.pointer.press_origin().is_some_and(|o| !home.contains(o))
        });
        let busy = dragging
            || match view.editor.tool() {
                Tool::Wire(w) => w.start.is_some(),
                Tool::Move(_) | Tool::Paste(_) | Tool::Text => true,
                Tool::Cable(c) => !c.points.is_empty(),
                Tool::Select | Tool::Place(_) => false,
            };
        let aside = busy
            && ctx
                .pointer_hover_pos()
                .is_some_and(|p| canvas.contains(p) && home.expand(NEAR).contains(p));
        let (pivot, at) = if aside {
            (
                egui::Align2::RIGHT_BOTTOM,
                canvas.right_bottom() - egui::vec2(MARGIN, MARGIN),
            )
        } else {
            (
                egui::Align2::CENTER_BOTTOM,
                canvas.center_bottom() - egui::vec2(0.0, MARGIN),
            )
        };

        let labels = match self.settings.dock_labels() {
            crate::settings::DockLabels::Always => true,
            crate::settings::DockLabels::Never => false,
            crate::settings::DockLabels::Auto => canvas.width() >= LABELS_MIN_CANVAS,
        };
        let place_tip = match self.wb.keymap.shortcut_text("tool.place") {
            Some(key) => format!("Place symbol · {key} (hold for the wheel)"),
            None => "Place symbol".to_owned(),
        };
        let select_tip = super::tips::tip_text(&self.wb.keymap, "Select", "edit.escape");
        let connect_tip = match self.wb.keymap.shortcut_text("wire.mode") {
            Some(key) => format!(
                "{} · {key} switches",
                super::tips::tip_text(&self.wb.keymap, "Connect", "tool.connect")
            ),
            None => super::tips::tip_text(&self.wb.keymap, "Connect", "tool.connect"),
        };
        let cable_tip = super::tips::tip_text(&self.wb.keymap, "Cable", "tool.cable");
        let undo_tip = super::tips::tip_text(&self.wb.keymap, "Undo", "edit.undo");
        let redo_tip = super::tips::tip_text(&self.wb.keymap, "Redo", "edit.redo");
        let can_undo = self.is_enabled(crate::actions::Action::Undo);
        let can_redo = self.is_enabled(crate::actions::Action::Redo);
        let mut history = None;
        let text_tip = super::tips::tip_text(&self.wb.keymap, "Add text", "draw.text");
        let chosen = self.settings.wiring_mode();
        let mut chip = None;
        let mut picked = None;
        // Where the controls are, for the lessons: id, rect, clicked.
        let mut spots: Vec<(&'static str, egui::Rect, bool)> = Vec::new();
        let area = egui::Area::new(egui::Id::new("tool_dock"))
            .order(egui::Order::Middle)
            .pivot(pivot)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (tool, icon, word, tip, id) in [
                            (
                                DockTool::Select,
                                icons::tool::SELECT,
                                "Select",
                                select_tip.as_str(),
                                "dock.select",
                            ),
                            (
                                DockTool::Place,
                                icons::tool::PLACE,
                                "Place",
                                place_tip.as_str(),
                                "dock.place",
                            ),
                            (
                                DockTool::Connect,
                                icons::tool::CONNECT,
                                "Connect",
                                connect_tip.as_str(),
                                "dock.connect",
                            ),
                        ] {
                            let response = dock_button(ui, labels, icon, word, active == tool, tip);
                            spots.push((id, response.rect, response.clicked()));
                            if response.clicked() {
                                picked = Some(tool);
                            }
                            // The dock never keeps the keyboard: keys stay
                            // with the drawing.
                            response.surrender_focus();
                        }
                        // The wiring mode: two icons of one choice, always
                        // there, faint while another tool is active; a click
                        // connects in that mode.
                        ui.separator();
                        for (mode, icon, word, tip, id) in [
                            (
                                WiringMode::Corners,
                                icons::tool::WIRE_CORNERS,
                                "Corners",
                                "Wire with corners (right angles) · Shift+W switches",
                                "dock.corners",
                            ),
                            (
                                WiringMode::Lines,
                                icons::tool::WIRE_LINES,
                                "Lines",
                                "Wire with lines (any angle) · Shift+W switches",
                                "dock.lines",
                            ),
                        ] {
                            // Copper only for the active tool: with another
                            // tool in hand the chosen mode is the brighter
                            // of two faint icons.
                            let connecting = active == DockTool::Connect;
                            let response = ui
                                .scope(|ui| {
                                    if !connecting {
                                        ui.multiply_opacity(if mode == chosen {
                                            0.8
                                        } else {
                                            0.45
                                        });
                                    }
                                    dock_button(
                                        ui,
                                        labels,
                                        icon,
                                        word,
                                        connecting && mode == chosen,
                                        tip,
                                    )
                                })
                                .inner;
                            spots.push((id, response.rect, response.clicked()));
                            if response.clicked() {
                                chip = Some(mode);
                            }
                            response.surrender_focus();
                        }
                        ui.separator();
                        // The cable tool and the history sit after the wiring
                        // mode: a tool of its own, then two plain actions.
                        let response = dock_button(
                            ui,
                            labels,
                            icons::tool::CABLE,
                            "Cable",
                            active == DockTool::Cable,
                            cable_tip.as_str(),
                        );
                        spots.push(("dock.cable", response.rect, response.clicked()));
                        if response.clicked() {
                            picked = Some(DockTool::Cable);
                        }
                        response.surrender_focus();
                        ui.separator();
                        for (icon, word, tip, id, enabled, command) in [
                            (
                                icons::tool::UNDO,
                                "Undo",
                                undo_tip.as_str(),
                                "dock.undo",
                                can_undo,
                                "edit.undo",
                            ),
                            (
                                icons::tool::REDO,
                                "Redo",
                                redo_tip.as_str(),
                                "dock.redo",
                                can_redo,
                                "edit.redo",
                            ),
                        ] {
                            let response = ui
                                .add_enabled_ui(enabled, |ui| {
                                    dock_button(ui, labels, icon, word, false, tip)
                                })
                                .inner;
                            spots.push((id, response.rect, response.clicked()));
                            if response.clicked() {
                                history = Some(command);
                            }
                            response.surrender_focus();
                        }
                        let response = dock_button(
                            ui,
                            labels,
                            icons::tool::TEXT,
                            "Text",
                            active == DockTool::Text,
                            &text_tip,
                        );
                        if response.clicked() {
                            picked = Some(DockTool::Text);
                        }
                        response.surrender_focus();
                        if in_bar {
                            // See `tool_hint`: the bar shows it.
                        } else if !hint.is_empty() {
                            ui.separator();
                            if refused {
                                ui.colored_label(
                                    crate::theme::ui_palette().error,
                                    format!("{} {hint}", icons::status::WARNING),
                                );
                            } else {
                                let response = ui.label(egui::RichText::new(&hint).weak());
                                spots.push(("dock.hint", response.rect, false));
                            }
                        }
                        if proposals > 0 && !in_bar {
                            ui.separator();
                            let response = ui
                                .label(egui::RichText::new(format!("{proposals} proposed")).weak())
                                .on_hover_text("↵ accept · Tab next · Delete decline");
                            spots.push(("dock.proposals", response.rect, false));
                        }
                    });
                });
            });
        self.wb.dock_size = area.response.rect.size();
        for (id, rect, clicked) in spots {
            let label = match id {
                "dock.select" => "Select",
                "dock.place" => "Place",
                "dock.connect" => "Connect",
                "dock.cable" => "Cable",
                "dock.undo" => "Undo",
                "dock.redo" => "Redo",
                "dock.corners" => "Corners",
                "dock.lines" => "Lines",
                "dock.hint" => "hint",
                _ => "proposals",
            };
            self.anchor(id, rect, label);
            if clicked {
                self.learn_event(novo_learn::Event::Clicked(id.to_owned()));
            }
        }

        if let Some(command) = history {
            self.wb.queued.push(command);
        }
        if let Some(mode) = chip {
            self.set_wiring_mode(mode, true);
        }
        match picked {
            Some(DockTool::Select) => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.leave_tool();
                }
            }
            Some(DockTool::Place) => self.wb.field.open(Mode::Place, None),
            Some(DockTool::Connect) => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_wire();
                }
            }
            Some(DockTool::Cable) => self.wb.queued.push("tool.cable"),
            Some(DockTool::Text) => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_text();
                }
            }
            None => {}
        }
    }
}
