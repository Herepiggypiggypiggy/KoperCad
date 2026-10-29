//! The Cabinet rail (docs/design/visual-redesign.md section 3.2, lane V3c):
//! the entries as data, what each one reads from the app, and what a click
//! does. Every row runs the command it names, the same one the Cabinet bar,
//! the menu and Ctrl K run, so the rail owns no state. The bar keeps its
//! captioned groups for now; the rail is the quick path to the same actions.
//!
//! Where the user finds it: the left edge of the Cabinet workspace (Ctrl+2).

use eframe::egui;
use novo_theme::presentation::Glyph;

use super::NovoApp;
use super::rail::Rail;
use super::rail_schematic::HISTORY;

/// The rows by zone. Select is left out: the Cabinet has no tool in hand
/// that Esc does not already put down.
pub(super) const TOOLS: &[&str] = &[
    "cabinet.place-all",
    "cabinet.rails",
    "cabinet.plate",
    "cabinet.surface",
];
pub(super) const ASSISTS: &[&str] = &[
    "cabinet.size-enclosure",
    "cabinet.routes",
    "cabinet.rules",
    "cabinet.emc",
    "cabinet.cutlist",
];
pub(super) const PANELS: &[&str] = &[
    "rail.pages",
    "heat.toggle",
    "heat.vents",
    "view.cabinet_table",
];

/// A row that is not a command. None yet; Pages is shared with the
/// Schematic rail.
pub(super) fn local(_id: &str) -> Option<(Glyph, &'static str, &'static str)> {
    None
}

impl NovoApp {
    /// Whether the panel or switch behind a row is on, read from its owner.
    fn rail_cabinet_active(&self, id: &str) -> bool {
        match id {
            "cabinet.rails" => self.cabinet.rails.open,
            "cabinet.routes" => self.cabinet.routes,
            "cabinet.rules" => self.cabinet.rules.editor_open,
            "cabinet.cutlist" => self.cabinet.rules.cut_open,
            "cabinet.emc" => self.cabinet.emc.on,
            "heat.toggle" => self.cabinet.heat.on,
            "view.cabinet_table" => self.cabinet.table,
            other => self.rail_panel_open(other),
        }
    }

    /// The Cabinet rail as it stands now.
    pub(super) fn cabinet_rail(&mut self) -> Rail {
        let mut rail = Rail::default();
        let zones: [(&[&'static str], &mut Vec<_>); 4] = [
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
                entry.active = self.rail_cabinet_active(id);
                out.push(entry);
            }
        }
        rail
    }

    /// Draws the rail as the leftmost panel and acts on what was clicked.
    pub(super) fn cabinet_rail_show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rail = self.cabinet_rail();
        let shown = self.rail_draw(ui, &rail);
        if let Some(id) = shown.clicked {
            self.rail_cabinet_click(&ctx, id);
        }
    }

    /// What a click on a row does: Pages opens the page list, every other
    /// row runs its command.
    pub(super) fn rail_cabinet_click(&mut self, ctx: &egui::Context, id: &'static str) {
        match id {
            "rail.pages" => self.show_pages(true),
            other => self.wb.queued.push(other),
        }
        ctx.request_repaint();
    }
}
