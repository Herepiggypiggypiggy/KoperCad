//! The Reports and Commissioning rails (docs/design/visual-redesign.md
//! section 3.2, lane V3d): the same widget as the other rails, with the
//! rows of these two workspaces. Rows are data, as there; a row names a
//! command or a `rail.*` id that only this file knows.
//!
//! Where the user finds it: the left edge of the Reports workspace and of
//! the Commissioning workspace (switcher chips, or Reports and Commissioning
//! in the app menu).
//!
//! Commissioning has no Photo or Note tool of its own: the panel's Redlines
//! and Test tabs are what the design called them, so the rail names the four
//! tabs that exist. Mark done is the big tick on Space and stays there; a
//! rail button that marks work passed would be one slip from a false record,
//! so the rail offers Next open, which only moves.

use eframe::egui;
use novo_theme::icons;
use novo_theme::presentation::Glyph;

use super::NovoApp;
use super::commissioning::Tab;
use super::rail::{self, Entry, FlyEntry, Rail};

pub(super) const REPORTS_TOOLS: &[&str] = &["report.add_page", "report.designer", "export.labels"];
pub(super) const REPORTS_ASSISTS: &[&str] = &["view.bench", "view.build"];
pub(super) const REPORTS_PANELS: &[&str] = &["rail.export", "export.print_preview"];
/// The flyout rows of Export.
pub(super) const EXPORT_FLYOUT: &[&str] = &[
    "export.reports",
    "export.report_pdf",
    "export.bom",
    "export.pdf",
];

pub(super) const COMMISSIONING_TOOLS: &[&str] = &[
    "rail.cm_checks",
    "rail.cm_punch",
    "rail.cm_redlines",
    "rail.cm_test",
];
pub(super) const COMMISSIONING_ASSISTS: &[&str] = &["rail.cm_next"];
pub(super) const COMMISSIONING_PANELS: &[&str] = &["export.commissioning", "export.punch_list"];

/// A row that is not a command: its picture, short name and sentence.
pub(super) fn local(id: &str) -> Option<(Glyph, &'static str, &'static str)> {
    Some(match id {
        "rail.export" => (
            Glyph::Phosphor(icons::EXPORT),
            "Export",
            "Send the open report or the whole set out as a file",
        ),
        "rail.cm_checks" => (
            Glyph::Phosphor(icons::LIST_CHECKS),
            "Checks",
            "Tick off wires, terminals and devices at the machine",
        ),
        "rail.cm_punch" => (
            Glyph::Phosphor(icons::FLAG),
            "Punch",
            "The punch list: what is still wrong, with a photo and a note",
        ),
        "rail.cm_redlines" => (
            Glyph::Phosphor(icons::PENCIL_LINE),
            "Redlines",
            "What the machine differs in from the drawing",
        ),
        "rail.cm_test" => (
            Glyph::Phosphor(icons::LIGHTNING),
            "Test",
            "The continuity and insulation test sheet",
        ),
        "rail.cm_next" => (
            Glyph::Phosphor(icons::SKIP_FORWARD),
            "Next open",
            "Go to the next wire, terminal or device with an open check · decides nothing",
        ),
        _ => return None,
    })
}

fn tab_of(id: &str) -> Option<Tab> {
    Some(match id {
        "rail.cm_checks" => Tab::Checks,
        "rail.cm_punch" => Tab::Punch,
        "rail.cm_redlines" => Tab::Redlines,
        "rail.cm_test" => Tab::Test,
        _ => return None,
    })
}

impl NovoApp {
    /// The Reports rail, drawn as the leftmost panel.
    pub(super) fn reports_rail_show(&mut self, ui: &mut egui::Ui) {
        let rail = self.workspace_rail(REPORTS_TOOLS, REPORTS_ASSISTS, REPORTS_PANELS);
        self.workspace_rail_draw(ui, &rail);
    }

    /// The Commissioning rail, drawn as the leftmost panel.
    pub(super) fn commissioning_rail_show(&mut self, ui: &mut egui::Ui) {
        let rail = self.workspace_rail(
            COMMISSIONING_TOOLS,
            COMMISSIONING_ASSISTS,
            COMMISSIONING_PANELS,
        );
        self.workspace_rail_draw(ui, &rail);
    }

    fn workspace_rail_draw(&mut self, ui: &mut egui::Ui, rail: &Rail) {
        let ctx = ui.ctx().clone();
        let shown = self.rail_draw(ui, rail);
        if let Some(id) = shown.clicked {
            self.workspace_rail_click(&ctx, id);
        }
    }

    /// One row of the Reports or Commissioning rail.
    fn workspace_entry(&self, id: &'static str) -> Option<Entry> {
        let (glyph, label, sentence) = super::rail_schematic::look(id)?;
        let key = self.wb.keymap.shortcut_text(id);
        let reason = novo_workflow::commands::get(id).and_then(|c| self.command_unavailable(c));
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
        let shown_tab = self.wb.commissioning.as_ref().map(|m| m.tab);
        let active = match (tab_of(id), shown_tab) {
            (Some(tab), Some(shown)) => tab == shown,
            _ => false,
        };
        let flyout = if id == "rail.export" {
            self.export_flyout()
        } else {
            Vec::new()
        };
        Some(Entry {
            id,
            anchor: super::learn_labels::moved_anchor(id),
            glyph,
            label,
            tip,
            key,
            active,
            enabled: reason.is_none(),
            badge: None,
            flyout,
        })
    }

    fn export_flyout(&self) -> Vec<FlyEntry> {
        EXPORT_FLYOUT
            .iter()
            .filter_map(|&id| {
                let (glyph, label, sentence) = super::rail_schematic::look(id)?;
                let reason =
                    novo_workflow::commands::get(id).and_then(|c| self.command_unavailable(c));
                let mut tip = sentence;
                if let Some(reason) = reason {
                    tip.push_str(&format!("\n{reason}"));
                }
                Some(FlyEntry {
                    id,
                    anchor: None,
                    glyph,
                    label,
                    tip,
                    key: self.wb.keymap.shortcut_text(id),
                    current: false,
                    enabled: reason.is_none(),
                    caption: None,
                })
            })
            .collect()
    }

    pub(super) fn workspace_rail(
        &self,
        tools: &[&'static str],
        assists: &[&'static str],
        panels: &[&'static str],
    ) -> Rail {
        let zone = |ids: &[&'static str]| -> Vec<Entry> {
            ids.iter()
                .filter_map(|&id| self.workspace_entry(id))
                .collect()
        };
        Rail {
            tools: zone(tools),
            assists: zone(assists),
            panels: zone(panels),
            history: Vec::new(),
        }
    }

    /// What a click on one of this file's `rail.*` rows does.
    pub(super) fn workspace_rail_click(&mut self, ctx: &egui::Context, id: &'static str) {
        if let Some(tab) = tab_of(id) {
            self.load_commissioning();
            if let Some(mode) = self.wb.commissioning.as_mut() {
                mode.tab = tab;
            }
        } else if id == "rail.cm_next" {
            self.commissioning_next_open();
        } else if id == "rail.export" {
            rail::toggle_flyout(ctx, id);
        } else {
            self.wb.queued.push(id);
        }
        ctx.request_repaint();
    }
}
