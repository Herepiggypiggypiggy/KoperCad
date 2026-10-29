//! The Schematic rail (docs/design/visual-redesign.md section 3.2): the
//! entries as data, what each one reads from the app, and what a click does.
//! It replaces the floating tool dock. Undo and redo sit at the foot until
//! the top bar (lane V2a) takes them.
//!
//! Where the user finds it: the left edge of the Schematic workspace.

use eframe::egui;
use novo_editor::Tool;
use novo_editor::tool::{ConnectMode, WiringMode};
use novo_theme::icons;
use novo_theme::presentation::{self, Glyph};
use novo_workflow::commands;
use novo_workflow::palette::Mode;

use super::NovoApp;
use super::rail::{Badge, Entry, FlyEntry, Rail};
use super::workbench::Drawer;

/// The rail's rows, in order, by zone. Command ids, or ids starting with
/// `rail.` for the few rows that are not a command. A new feature adds one
/// row here; the tests keep every row real and every zone to six.
pub(super) const TOOLS: &[&str] = &[
    "rail.select",
    "tool.place",
    "tool.connect",
    "tool.cable",
    "draw.text",
];
pub(super) const ASSISTS: &[&str] = &[
    "wires.autopilot",
    "proposal.accept_all",
    "draw.tidy",
    "xref.link",
    "macro.place",
];
pub(super) const PANELS: &[&str] = &[
    "rail.pages",
    "check.problems",
    "view.terminals_drawer",
    "view.cables_drawer",
    "view.devices_drawer",
];
/// The foot cluster until V2a moves it to the top bar.
pub(super) const HISTORY: &[&str] = &["edit.undo", "edit.redo"];

/// The flyout rows of Connect.
pub(super) const CONNECT_FLYOUT: &[&str] = &[
    "rail.wire_corners",
    "rail.wire_lines",
    "rail.auto_draw",
    "rail.auto_ask",
    "rail.auto_never",
];

/// The lesson anchor each row keeps from the dock it replaces.
pub(super) fn anchor_of(id: &str) -> Option<&'static str> {
    Some(match id {
        "rail.select" => "dock.select",
        "tool.place" => "dock.place",
        "tool.connect" => "dock.connect",
        "tool.cable" => "dock.cable",
        "edit.undo" => "dock.undo",
        "edit.redo" => "dock.redo",
        "rail.wire_corners" => "dock.corners",
        "rail.wire_lines" => "dock.lines",
        _ => return None,
    })
}

/// A row that is not a command: its picture, short name and sentence.
fn local(id: &str) -> Option<(Glyph, &'static str, &'static str)> {
    Some(match id {
        "rail.select" => (
            Glyph::Phosphor(icons::tool::SELECT),
            "Select",
            "Back to the pointer: select, move and edit",
        ),
        "rail.pages" => (
            Glyph::Phosphor(novo_theme::icons::FILES),
            "Pages",
            "The page list of the project",
        ),
        "rail.wire_corners" => (
            Glyph::Phosphor(icons::tool::WIRE_CORNERS),
            "Corners",
            "Wire with corners (right angles) · Shift+W switches",
        ),
        "rail.wire_lines" => (
            Glyph::Phosphor(icons::tool::WIRE_LINES),
            "Lines",
            "Wire with lines (any angle) · Shift+W switches",
        ),
        "rail.auto_draw" => (
            Glyph::Phosphor(novo_theme::icons::MAGIC_WAND),
            "Draw them",
            "Pins that touch are connected with the drop, as one undo step",
        ),
        "rail.auto_ask" => (
            Glyph::Phosphor(novo_theme::icons::QUESTION),
            "Ask first",
            "Each connection is proposed; Enter accepts, Delete declines",
        ),
        "rail.auto_never" => (
            Glyph::Phosphor(novo_theme::icons::PROHIBIT),
            "Off",
            "Draw and propose nothing",
        ),
        _ => return None,
    })
}

/// The picture, short name and sentence of any rail row.
pub(super) fn look(id: &str) -> Option<(Glyph, String, String)> {
    if let Some((glyph, label, sentence)) = local(id)
        .or_else(|| super::rail_3d::local(id))
        .or_else(|| super::rail_cabinet::local(id))
        .or_else(|| super::rail_reports::local(id))
    {
        return Some((glyph, label.to_owned(), sentence.to_owned()));
    }
    let label = presentation::short_label(id)?;
    let sentence = commands::get(id).map_or(String::new(), |c| c.label.to_owned());
    Some((presentation::glyph(id), label.to_owned(), sentence))
}

impl NovoApp {
    /// Opens the page list, or closes the flyout that shows it.
    pub(super) fn show_pages(&mut self, toggle: bool) {
        let showing = self.outline_mode == super::OutlineMode::Pages;
        self.outline_mode = super::OutlineMode::Pages;
        if !self.outline_pinned() {
            self.outline_open = !(toggle && showing && self.outline_open);
        }
    }

    /// Whether the panel behind a panels-zone row is open.
    pub(super) fn rail_panel_open(&self, id: &str) -> bool {
        let outline =
            |mode| self.outline_mode == mode && (self.outline_pinned() || self.outline_open);
        match id {
            "rail.pages" => outline(super::OutlineMode::Pages),
            "check.problems" => outline(super::OutlineMode::Issues),
            "view.terminals_drawer" => self.wb.drawer == Some(Drawer::Terminals),
            "view.cables_drawer" => self.wb.drawer == Some(Drawer::Cables),
            "view.devices_drawer" => self.wb.drawer == Some(Drawer::Devices),
            _ => false,
        }
    }

    /// One row: its picture, name, key and tooltip, from the presentation
    /// table and the command it names.
    pub(super) fn rail_entry(&self, id: &'static str) -> Option<Entry> {
        let (glyph, label, sentence) = look(id)?;
        let key_id = if id == "rail.select" {
            "edit.escape"
        } else {
            id
        };
        let key = self.wb.keymap.shortcut_text(key_id);
        let reason = commands::get(id).and_then(|info| self.command_unavailable(info));
        let mut tip = match &key {
            Some(key) => format!("{label} · {key}"),
            None => label.clone(),
        };
        if !sentence.is_empty() && sentence != label {
            tip.push('\n');
            tip.push_str(&sentence);
        }
        if let Some(reason) = reason {
            tip.push_str(&format!("\n{reason}"));
        }
        Some(Entry {
            id,
            anchor: anchor_of(id).or_else(|| super::learn_labels::moved_anchor(id)),
            glyph,
            label,
            tip,
            key,
            active: false,
            enabled: reason.is_none(),
            badge: None,
            flyout: Vec::new(),
        })
    }

    fn connect_flyout(&self) -> Vec<FlyEntry> {
        let wiring = self.settings.wiring_mode();
        let connect = self.settings.connect_mode();
        CONNECT_FLYOUT
            .iter()
            .filter_map(|&id| {
                let (glyph, label, sentence) = look(id)?;
                let current = match id {
                    "rail.wire_corners" => wiring == WiringMode::Corners,
                    "rail.wire_lines" => wiring == WiringMode::Lines,
                    "rail.auto_draw" => connect == ConnectMode::Draw,
                    "rail.auto_ask" => connect == ConnectMode::Ask,
                    _ => connect == ConnectMode::Never,
                };
                let caption = match id {
                    "rail.wire_corners" => Some("Wire shape"),
                    "rail.auto_draw" => Some("Auto-connect pins that touch"),
                    _ => None,
                };
                Some(FlyEntry {
                    id,
                    anchor: anchor_of(id),
                    glyph,
                    label,
                    tip: sentence,
                    key: None,
                    current,
                    enabled: true,
                    caption,
                })
            })
            .collect()
    }

    /// The Schematic rail as it stands now.
    pub(super) fn schematic_rail(&mut self, proposals: usize) -> Rail {
        let tool = self
            .canvas
            .schematic_ref()
            .map(|view| view.editor.tool().clone());
        let issues = {
            let s = novo_checks::Summary::of(self.shown_problems());
            s.errors + s.warnings
        };
        let mut rail = Rail::default();
        let zones: [(&[&'static str], &mut Vec<Entry>); 4] = [
            (TOOLS, &mut rail.tools),
            (ASSISTS, &mut rail.assists),
            (PANELS, &mut rail.panels),
            (HISTORY, &mut rail.history),
        ];
        for (ids, out) in zones {
            for &id in ids {
                let Some(mut entry) = self.rail_entry(id) else {
                    continue;
                };
                entry.active = match (id, tool.as_ref()) {
                    ("rail.select", Some(t)) => {
                        matches!(t, Tool::Select | Tool::Move(_) | Tool::Paste(_))
                    }
                    ("tool.place", Some(t)) => matches!(t, Tool::Place(_)),
                    ("tool.connect", Some(t)) => matches!(t, Tool::Wire(_)),
                    ("tool.cable", Some(t)) => matches!(t, Tool::Cable(_)),
                    ("draw.text", Some(t)) => matches!(t, Tool::Text),
                    _ => self.rail_panel_open(id),
                };
                match id {
                    "tool.connect" => entry.flyout = self.connect_flyout(),
                    "proposal.accept_all" => {
                        entry.enabled = entry.enabled && proposals > 0;
                        entry.badge = Some(Badge {
                            count: proposals,
                            warn: false,
                        });
                        entry.tip.push_str(&match proposals {
                            0 => "\nNo proposals wait".to_owned(),
                            1 => "\n1 proposal waits".to_owned(),
                            n => format!("\n{n} proposals wait"),
                        });
                    }
                    "check.problems" => {
                        entry.badge = Some(Badge {
                            count: issues,
                            warn: true,
                        });
                        entry.tip.push_str(&match issues {
                            0 => "\nNo errors or warnings".to_owned(),
                            1 => "\n1 error or warning".to_owned(),
                            n => format!("\n{n} errors and warnings"),
                        });
                    }
                    _ => {}
                }
                out.push(entry);
            }
        }
        rail
    }

    /// Draws the rail as the leftmost panel and acts on what was clicked.
    pub(super) fn schematic_rail_show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let super::dock::ToolHint { proposals, .. } = self.tool_hint();
        let rail = self.schematic_rail(proposals);
        let shown = self.rail_draw(ui, &rail);
        let mut spots = shown.spots;
        // A lesson that points at Corners or Lines while the flyout is shut
        // points at Connect, which opens it.
        for wanted in ["dock.corners", "dock.lines"] {
            if !spots.iter().any(|(id, _, _)| *id == wanted)
                && let Some(&(_, rect, _)) = spots.iter().find(|(id, _, _)| *id == "dock.connect")
            {
                spots.push((wanted, rect, false));
            }
        }
        for (id, rect, clicked) in spots {
            if id.starts_with("cmd.") {
                continue;
            }
            let label = match id {
                "dock.select" => "Select",
                "dock.place" => "Place",
                "dock.connect" => "Connect",
                "dock.cable" => "Cable",
                "dock.undo" => "Undo",
                "dock.redo" => "Redo",
                "dock.corners" => "Corners",
                _ => "Lines",
            };
            self.anchor(id, rect, label);
            if clicked {
                self.learn_event(novo_learn::Event::Clicked(id.to_owned()));
            }
        }
        if let Some(id) = shown.clicked {
            self.rail_click(&ctx, id);
        }
    }

    /// Draws `rail` as the leftmost panel, keeps the expanded choice in the
    /// settings and records where it stands. Every workspace's rail goes
    /// through here.
    pub(super) fn rail_draw(&mut self, ui: &mut egui::Ui, rail: &Rail) -> super::rail::Shown {
        let mut expanded = self.settings.rail_expanded
            || self.settings.dock_labels() == crate::settings::DockLabels::Always;
        let was = expanded;
        let shown = super::rail::show(ui, rail, &mut expanded);
        if expanded != was {
            self.settings.rail_expanded = expanded;
        }
        self.wb.rail_rect = Some(shown.rect);
        // A row whose menu row moved here keeps the anchor `cmd.<id>`.
        for &(id, rect, clicked) in &shown.spots {
            if id.starts_with("cmd.") {
                let label = super::learn_labels::any_anchor_label(id).unwrap_or_default();
                self.anchor(id, rect, &label);
                if clicked {
                    self.learn_event(novo_learn::Event::Clicked(id.to_owned()));
                }
            }
        }
        shown
    }

    /// What a click on a row does. The editor tools start as the dock's
    /// buttons did; the rest run their command.
    pub(super) fn rail_click(&mut self, ctx: &egui::Context, id: &'static str) {
        match id {
            "rail.select" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.leave_tool();
                }
            }
            "tool.place" => self.wb.field.open(Mode::Place, None),
            "tool.connect" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_wire();
                }
            }
            "draw.text" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_text();
                }
            }
            "rail.wire_corners" => self.set_wiring_mode(WiringMode::Corners, true),
            "rail.wire_lines" => self.set_wiring_mode(WiringMode::Lines, true),
            "rail.auto_draw" => self.settings.set_connect_mode(ConnectMode::Draw),
            "rail.auto_ask" => self.settings.set_connect_mode(ConnectMode::Ask),
            "rail.auto_never" => self.settings.set_connect_mode(ConnectMode::Never),
            "rail.pages" => self.show_pages(true),
            "check.problems" => self.show_issues(true),
            other => self.wb.queued.push(other),
        }
        ctx.request_repaint();
    }
}
