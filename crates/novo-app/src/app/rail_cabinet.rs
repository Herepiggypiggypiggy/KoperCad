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
use super::rail_schematic::{HISTORY, look};

/// The rows by zone. Select is left out: the Cabinet has no tool in hand
/// that Esc does not already put down.
pub(super) const TOOLS: &[&str] = &[
    "cabinet.place-all",
    "rail.arrange",
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

/// A row that is not a command: Arrange opens the card of the arrange tools.
/// Pages is shared with the Schematic rail.
pub(super) fn local(id: &str) -> Option<(Glyph, &'static str, &'static str)> {
    match id {
        "rail.arrange" => Some((
            Glyph::Phosphor(novo_theme::icons::MAGNET),
            "Arrange",
            "Auto-reconfigure, autoplace on rail, auto rails and ducts, autogroup and autodistance for the \
             selected devices (all of them when none is selected), previewed before anything \
             is stored. Simple rules: a planning aid, not a clearance check or a heat calculation",
        )),
        _ => None,
    }
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

    /// The rows of the Arrange card.
    fn arrange_flyout(&self) -> Vec<super::rail::FlyEntry> {
        super::cabinet_arrange::TOOLS
            .iter()
            .filter_map(|&id| {
                let (glyph, label, sentence) = look(id)?;
                let reason = novo_workflow::commands::get(id)
                    .and_then(|info| self.command_unavailable(info));
                let mut tip = sentence;
                if id.ends_with("tight") || id.ends_with("even") {
                    tip.push_str(
                        "\nKeeps the gaps the parts ask for; not a clearance check or a heat \
                         calculation.",
                    );
                }
                if let Some(reason) = reason {
                    tip.push_str(&format!("\n{reason}"));
                }
                Some(super::rail::FlyEntry {
                    id,
                    anchor: None,
                    glyph,
                    label,
                    tip,
                    key: self.wb.keymap.shortcut_text(id),
                    current: false,
                    enabled: reason.is_none(),
                    caption: match id {
                        "cabinet.arrange_all" => Some("All in one step"),
                        "cabinet.arrange_onto_rail" => Some("Rails and ducts"),
                        "cabinet.arrange_group" => Some("Order and distance"),
                        _ => None,
                    },
                })
            })
            .collect()
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
                if id == "rail.arrange" {
                    entry.flyout = self.arrange_flyout();
                    // The entry only opens its card.
                    entry.enabled = true;
                    entry.active = false;
                }
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
            "rail.arrange" => super::rail::open(ctx, id),
            other => self.wb.queued.push(other),
        }
        ctx.request_repaint();
    }
}
