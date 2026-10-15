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
}

/// How close to the dock a gesture has to come to move it aside.
const NEAR: f32 = 32.0;
/// The dock's distance from the canvas edge.
const MARGIN: f32 = 12.0;

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
        let carries = self.settings.move_carries;
        if let Some(view) = self.canvas.schematic() {
            if view.editor.connect_mode() != mode {
                view.editor.set_connect_mode(mode);
            }
            view.move_carries = carries;
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

    pub(super) fn tool_dock(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
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
            return;
        };
        if !self.is_open() {
            return;
        }
        let active = match view.editor.tool() {
            Tool::Place(_) => DockTool::Place,
            Tool::Wire(_) => DockTool::Connect,
            Tool::Select | Tool::Move(_) | Tool::Cable(_) => DockTool::Select,
        };
        let band = view.band_count();
        let hint = view
            .status_text()
            .filter(|h| !h.is_empty())
            .unwrap_or(elsewhere);
        // Dragging a selection box says what letting go would take.
        let hint = match band {
            Some(n) => format!("{n} selected"),
            None => hint,
        };
        // A refusal reads as one: the Error tint and a warning icon, never
        // colour alone (DESIGN.md §4).
        let refused = view.editor.notice().is_some();
        let proposals = view.editor.proposals().len() + self.page_proposals().len();

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
                Tool::Move(_) => true,
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

        let place_tip = match self.wb.keymap.shortcut_text("tool.place") {
            Some(key) => format!("Place symbol · {key} (hold for the wheel)"),
            None => "Place symbol".to_owned(),
        };
        let chosen = self.settings.wiring_mode();
        let mut chip = None;
        let mut picked = None;
        let area = egui::Area::new(egui::Id::new("tool_dock"))
            .order(egui::Order::Middle)
            .pivot(pivot)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (tool, icon, tip) in [
                            (DockTool::Select, icons::tool::SELECT, "Select · Esc"),
                            (DockTool::Place, icons::tool::PLACE, place_tip.as_str()),
                            (
                                DockTool::Connect,
                                icons::tool::CONNECT,
                                "Connect · W · Shift+W switches",
                            ),
                        ] {
                            let response = widgets::tool_button(ui, icon, active == tool, tip);
                            if response.clicked() {
                                picked = Some(tool);
                            }
                            // The dock never keeps the keyboard: keys stay
                            // with the drawing.
                            response.surrender_focus();
                        }
                        // The wiring mode: always there, faint while another
                        // tool is active; a click connects in that mode.
                        for (mode, label) in [
                            (WiringMode::Corners, "Corners"),
                            (WiringMode::Lines, "Lines"),
                        ] {
                            let mut text = egui::RichText::new(label).small();
                            if active != DockTool::Connect {
                                text = text.weak();
                            }
                            let selected = active == DockTool::Connect && mode == chosen;
                            let response = ui
                                .selectable_label(selected, text)
                                .on_hover_text("Wiring mode · Shift+W switches");
                            if response.clicked() {
                                chip = Some(mode);
                            }
                            response.surrender_focus();
                        }
                        ui.add_enabled_ui(false, |ui| {
                            widgets::tool_button(ui, icons::tool::TEXT, false, "Text")
                                .on_disabled_hover_text("Text · Comes later");
                        });
                        if !hint.is_empty() {
                            ui.separator();
                            if refused {
                                ui.colored_label(
                                    novo_theme::tokens::ERROR,
                                    format!("{} {hint}", icons::status::WARNING),
                                );
                            } else {
                                ui.label(egui::RichText::new(&hint).weak());
                            }
                        }
                        if proposals > 0 {
                            ui.separator();
                            ui.label(egui::RichText::new(format!("{proposals} proposed")).weak())
                                .on_hover_text("↵ accept · Tab next · Delete decline");
                        }
                    });
                });
            });
        self.wb.dock_size = area.response.rect.size();

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
            None => {}
        }
    }
}
