//! The 3D rail (docs/design/visual-redesign.md section 3.2, lane V3b): the
//! entries as data, what each one reads from the app, and what a click does.
//! A row runs the command it names, so the rail owns no state: a switch is
//! read from the 3D option registry (`view3d_options`) and written back to it
//! by the same command the menu and Ctrl K use.
//!
//! Where the user finds it: the left edge of the 3D workspace. The status
//! chip of the simulation (#1637) and the toolbar sit on the canvas to its
//! right and are not covered.

use eframe::egui;
use novo_theme::icons;
use novo_theme::presentation::Glyph;

use super::NovoApp;
use super::rail::{self, FlyEntry, Rail};
use super::rail_schematic::{HISTORY, look};
use super::view3d_options::{ALL, ViewOption};

/// The rows by zone. A new 3D feature adds one row; the tests keep every row
/// real and every zone to six. The side panel of `visual-redesign-3d.md`
/// section 5 is not built yet, so its three entries wait: Heat and EMC switch
/// their overlay here until the panel takes them over.
pub(super) const TOOLS: &[&str] = &["rail.select", "view.3d_move", "view.3d_place", "rail.align"];
pub(super) const ASSISTS: &[&str] = &[
    "view.3d_measure",
    "view.3d_section",
    "rail.hide",
    "view.3d_save_view",
    "rail.layers",
];
pub(super) const PANELS: &[&str] = &[
    "heat.toggle",
    "view.emc",
    "view.3d_position",
    "view.3d_stats",
];

/// The flyout rows of Align and spread, and of Hide and isolate.
pub(super) const ALIGN_FLYOUT: &[&str] = &[
    "view.3d_align_left",
    "view.3d_align_hcentre",
    "view.3d_align_right",
    "view.3d_align_top",
    "view.3d_align_vcentre",
    "view.3d_align_bottom",
    "view.3d_spread_h",
    "view.3d_spread_v",
];
pub(super) const HIDE_FLYOUT: &[&str] = &[
    "view.3d_select_devices",
    "view.3d_select_rails",
    "view.3d_select_ducts",
    "view.3d_hide",
    "view.3d_isolate",
    "view.3d_show_all",
];
/// The flyout of Layers: what is drawn, the measuring aid, the two walks
/// through the cabinet and how the view is handled. It replaces the Layers
/// menu of the old 3D toolbar; each row is a switch of the option registry.
pub(super) const LAYERS_FLYOUT: &[&str] = &[
    "view.3d_shell",
    "view.3d_door_layer",
    "view.3d_wires",
    "view.3d_device_shapes",
    "view.3d_slotted_ducts",
    "view.3d_surfaces",
    "view.3d_duct_fill",
    "view.3d_pins",
    "view.3d_shadows",
    "view.3d_finish",
    "view.3d_ssao",
    "view.3d_missing_data",
    "view.3d_dimensions",
    "view.3d_assembly",
    "view.3d_wire_it",
    "view.3d_trackpad_pans",
];

/// A row that is not a command: its picture, short name and sentence.
pub(super) fn local(id: &str) -> Option<(Glyph, &'static str, &'static str)> {
    Some(match id {
        "rail.align" => (
            Glyph::Phosphor(icons::ALIGN_LEFT),
            "Align",
            "Align or spread the selected devices (needs two or more)",
        ),
        "rail.hide" => (
            Glyph::Phosphor(icons::EYE_SLASH),
            "Hide",
            "Hide or isolate the selection, or show everything again",
        ),
        "rail.layers" => (
            Glyph::Phosphor(icons::STACK),
            "Layers",
            "What the 3D view draws, the dimensions, and the walks through the cabinet",
        ),
        _ => return None,
    })
}

impl NovoApp {
    /// Whether the row's switch is on, read from its owner.
    fn rail_3d_active(&self, id: &str) -> bool {
        if let Some(spec) = ALL.iter().find(|s| s.command == id) {
            return self.view3d_option(spec.option);
        }
        match id {
            "rail.select" => {
                !self.view3d_option(ViewOption::Move)
                    && !self.view3d_option(ViewOption::Measure)
                    && !self.view3d.place.open
            }
            "view.3d_place" => self.view3d.place.open,
            "view.3d_position" => !self.view3d.position.hidden,
            "view.3d_stats" => self.view3d.stats.shown,
            _ => false,
        }
    }

    fn rail_3d_flyout(
        &self,
        ids: &[&'static str],
        caption: &[(&str, &'static str)],
    ) -> Vec<FlyEntry> {
        self.rail_3d_flyout_with(ids, caption, false)
    }

    /// `switches` marks each row that is an option of the registry with a
    /// tick while it is on.
    fn rail_3d_flyout_with(
        &self,
        ids: &[&'static str],
        caption: &[(&str, &'static str)],
        switches: bool,
    ) -> Vec<FlyEntry> {
        ids.iter()
            .filter_map(|&id| {
                let (glyph, label, sentence) = look(id)?;
                let reason = novo_workflow::commands::get(id)
                    .and_then(|info| self.command_unavailable(info));
                let mut tip = match ALL.iter().find(|s| switches && s.command == id) {
                    Some(spec) => spec.hint.to_owned(),
                    None => sentence,
                };
                if let Some(reason) = reason {
                    tip.push_str(&format!("\n{reason}"));
                }
                Some(FlyEntry {
                    id,
                    anchor: super::learn_labels::moved_anchor(id),
                    glyph,
                    label,
                    tip,
                    key: self.wb.keymap.shortcut_text(id),
                    current: switches && self.rail_3d_active(id),
                    enabled: reason.is_none()
                        && !(id == "view.3d_ssao" && !self.view3d_ssao_available()),
                    caption: caption.iter().find(|(c, _)| *c == id).map(|(_, c)| *c),
                })
            })
            .collect()
    }

    /// The 3D rail as it stands now.
    pub(super) fn view3d_rail(&mut self) -> Rail {
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
                entry.active = self.rail_3d_active(id);
                if id == "heat.toggle" {
                    // The lessons point at the Heat switch by this name.
                    entry.anchor = Some("view3d.heat");
                }
                if id == "rail.layers" {
                    entry.active = false;
                }
                match id {
                    "rail.align" => {
                        entry.flyout = self.rail_3d_flyout(
                            ALIGN_FLYOUT,
                            &[
                                ("view.3d_align_left", "Align"),
                                ("view.3d_spread_h", "Spread"),
                            ],
                        );
                        // The entry only opens its card, so it is not greyed
                        // by the command registry.
                        entry.enabled = true;
                    }
                    "rail.layers" => {
                        entry.flyout = self.rail_3d_flyout_with(
                            LAYERS_FLYOUT,
                            &[
                                ("view.3d_shell", "Drawn"),
                                ("view.3d_dimensions", "Measuring"),
                                ("view.3d_assembly", "Walk through"),
                                ("view.3d_trackpad_pans", "Handling"),
                            ],
                            true,
                        );
                        entry.enabled = true;
                    }
                    "rail.hide" => {
                        entry.flyout = self.rail_3d_flyout(HIDE_FLYOUT, &[]);
                        entry.enabled = true;
                    }
                    _ => {}
                }
                out.push(entry);
            }
        }
        rail
    }

    /// Draws the rail as the leftmost panel and acts on what was clicked.
    pub(super) fn view3d_rail_show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let rail = self.view3d_rail();
        let shown = self.rail_draw(ui, &rail);
        if let Some(id) = shown.clicked {
            self.rail_3d_click(&ctx, id);
        }
    }

    /// What a click on a row does: Select puts the pointer back, Align and
    /// Hide open their card, every other row runs its command.
    pub(super) fn rail_3d_click(&mut self, ctx: &egui::Context, id: &'static str) {
        match id {
            "rail.select" => {
                self.set_view3d_option(ViewOption::Move, false);
                self.set_view3d_option(ViewOption::Measure, false);
                self.view3d.place.open = false;
                self.view3d.place.armed = None;
            }
            "rail.align" | "rail.hide" | "rail.layers" => rail::open(ctx, id),
            other => self.wb.queued.push(other),
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view3d_options::Home;

    /// The Layers flyout holds every layer, walk and handling switch, so a
    /// new one cannot be left without a place now the toolbar menu is gone.
    #[test]
    fn the_layers_flyout_holds_every_layer_walk_and_preference() {
        for spec in ALL {
            let wanted = matches!(spec.home, Home::Layer | Home::Walk | Home::Preference)
                || spec.option == ViewOption::Dimensions;
            assert_eq!(
                LAYERS_FLYOUT.contains(&spec.command),
                wanted,
                "{}",
                spec.command
            );
        }
        for id in LAYERS_FLYOUT {
            assert!(look(id).is_some(), "{id} has no look");
        }
    }
}
