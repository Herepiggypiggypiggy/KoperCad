//! The 3D view's toolbar: layer toggles (enclosure, door, wires) and the door
//! control. The state is view state; the project is untouched. The scene
//! build and the door swing are applied in `view3d.rs`.

use eframe::egui;
use novo_view3d::Layers;

/// Margin from the view's corner, in points.
const MARGIN: f32 = 12.0;
/// Width of the door angle slider.
const SLIDER_WIDTH: f32 = 120.0;

/// What the toolbar sets.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Controls {
    pub(super) layers: Layers,
    /// How far the door is asked to be open, in degrees; 0 closed.
    pub(super) door_deg: f64,
    /// The Move tool: a drag on a device, rail or duct moves it.
    pub(super) move_items: bool,
    /// Soft shadows from the key light.
    pub(super) shadows: bool,
    /// Measuring lines: the enclosure's size and the selected part's.
    pub(super) dimensions: bool,
    /// The frame governor has stepped the quality down: the toolbar offers
    /// to restore it. Set by the host each frame.
    pub(super) reduced: bool,
    /// The user pressed "Restore quality"; the host acts on it and clears it.
    pub(super) restore: bool,
    /// The "?" badge (no part data) on every device, not only on hover.
    pub(super) missing_data: bool,
    /// The Heat panel in the right-hand slot, under the view cube.
    pub(super) heat_panel: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            layers: Layers::default(),
            door_deg: 0.0,
            move_items: true,
            shadows: true,
            dimensions: false,
            reduced: false,
            restore: false,
            missing_data: false,
            heat_panel: false,
        }
    }
}

/// What a frame of the toolbar changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Changed {
    pub(super) layers: bool,
    pub(super) door: bool,
}

impl Controls {
    /// Opens a closed door to its limit and closes an open one. Without a
    /// door (`limit` is `None`) nothing happens.
    pub(super) fn toggle_door(&mut self, limit: Option<f64>) {
        let Some(limit) = limit else {
            return;
        };
        self.door_deg = if self.door_deg > 0.0 { 0.0 } else { limit };
    }

    /// The door angle, held within what the enclosure's door can do.
    pub(super) fn clamp_door(&mut self, limit: Option<f64>) {
        self.door_deg = self.door_deg.clamp(0.0, limit.unwrap_or(0.0));
    }

    /// The door angle the scene should show: closed when the door layer is
    /// off or the enclosure has no door.
    pub(super) fn door_target(&self, limit: Option<f64>) -> f32 {
        match limit {
            Some(limit) if self.layers.door => self.door_deg.clamp(0.0, limit) as f32,
            _ => 0.0,
        }
    }
}

/// Wide enough for every toggle and the door slider on one row.
const TOOLBAR_WIDTH: f32 = 900.0;

/// Where a panel under the toolbar starts, from the top of the view.
pub(super) const BELOW: f32 = 140.0;

/// The toolbar, top left of the view. `limit` is how far the enclosure's door
/// opens, `None` when it has none.
pub(super) fn toolbar(
    ctx: &egui::Context,
    view: egui::Rect,
    controls: &mut Controls,
    limit: Option<f64>,
) -> Changed {
    let mut changed = Changed::default();
    // Leave the view cube (top right) uncovered.
    let width = TOOLBAR_WIDTH
        .min(view.width() - super::view3d_heat::CUBE_SIZE - 3.0 * MARGIN)
        .max(300.0);
    super::panel::Panel::new(
        "view3d_toolbar",
        "3D view",
        view,
        super::panel::Side::Left,
        width,
    )
    .top(MARGIN)
    .fit()
    .show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            // Layers: things that are drawn or not, in one popover.
            let n_on = [
                controls.layers.shell,
                controls.layers.door,
                controls.layers.wires,
                controls.shadows,
                controls.missing_data,
            ]
            .iter()
            .filter(|b| **b)
            .count();
            let config = egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            egui::containers::menu::MenuButton::new(format!("Layers ({n_on})"))
                .config(config)
                .ui(ui, |ui| {
                    let layers = &mut controls.layers;
                    changed.layers |=
                        toggle(ui, &mut layers.shell, "Enclosure", "Show the enclosure box");
                    changed.layers |= toggle(ui, &mut layers.door, "Door", "Show the door");
                    changed.layers |=
                        toggle(ui, &mut layers.wires, "Wires", "Show the routed wires");
                    toggle(
                        ui,
                        &mut controls.shadows,
                        "Shadows",
                        "Soft shadows from the key light (View > 3D > Shadows)",
                    );
                    toggle(
                        ui,
                        &mut controls.missing_data,
                        "Missing data",
                        "Mark every device that has no part data with a \"?\". Off: \
                         the \"?\" shows on the device under the pointer.",
                    );
                });
            ui.separator();
            // Modes and panels: what the pointer does, what sits at the side.
            // A mode, not a layer: keeps the copper wash of a tool.
            let mode = if controls.move_items {
                "✔ Move"
            } else {
                "Move"
            };
            if ui
                .selectable_label(controls.move_items, mode)
                .on_hover_text(
                    "Move mode. Drag a device, rail or duct to move it on the plate (grid \
                     and checks as in the Cabinet). R turns the selection, Delete takes it \
                     off. Off: a drag draws a selection box.",
                )
                .clicked()
            {
                controls.move_items ^= true;
            }
            toggle(
                ui,
                &mut controls.heat_panel,
                "Heat",
                "The Heat panel: simulate how heat and air move in this cabinet",
            );
            toggle(
                ui,
                &mut controls.dimensions,
                "Dimensions",
                "Measuring lines in mm: the enclosure, and for one selected part its \
                 size and its distances to neighbours and the plate edge (View > 3D > \
                 Dimensions)",
            );
            if controls.reduced
                && ui
                    .button("Restore quality")
                    .on_hover_text(
                        "The view slowed down, so it draws at lower quality. Draw at \
                         full quality again (View > 3D > Restore 3D quality).",
                    )
                    .clicked()
            {
                controls.restore = true;
            }
            if let Some(limit) = limit
                && controls.layers.door
            {
                ui.separator();
                let label = if controls.door_deg > 0.0 {
                    "Close door"
                } else {
                    "Open door"
                };
                if ui
                    .button(label)
                    .on_hover_text("Open or close the door (Shift+D)")
                    .clicked()
                {
                    controls.toggle_door(Some(limit));
                    changed.door = true;
                }
                ui.spacing_mut().slider_width = SLIDER_WIDTH;
                let slider = egui::Slider::new(&mut controls.door_deg, 0.0..=limit)
                    .suffix("°")
                    .max_decimals(0);
                if ui
                    .add(slider)
                    .on_hover_text("How far the door is open")
                    .changed()
                {
                    changed.door = true;
                }
            }
        });
    });
    changed
}

/// A layer toggle: neutral, with a tick when on, so it never relies on
/// colour alone and never reads as copper.
fn toggle(ui: &mut egui::Ui, on: &mut bool, name: &str, hint: &str) -> bool {
    if novo_theme::widgets::filter_toggle(ui, name, *on)
        .on_hover_text(hint)
        .clicked()
    {
        *on = !*on;
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quiet_defaults_hide_badges_and_the_heat_panel() {
        let c = Controls::default();
        assert!(!c.missing_data);
        assert!(!c.heat_panel);
        assert!(c.move_items);
    }

    #[test]
    fn the_door_toggles_between_closed_and_its_limit() {
        let mut c = Controls::default();
        c.toggle_door(Some(110.0));
        assert_eq!(c.door_deg, 110.0);
        c.toggle_door(Some(110.0));
        assert_eq!(c.door_deg, 0.0);
        // A half-open door closes.
        c.door_deg = 40.0;
        c.toggle_door(Some(110.0));
        assert_eq!(c.door_deg, 0.0);
    }

    #[test]
    fn no_door_means_nothing_to_open() {
        let mut c = Controls::default();
        c.toggle_door(None);
        assert_eq!(c.door_deg, 0.0);
        c.door_deg = 30.0;
        c.clamp_door(None);
        assert_eq!(c.door_deg, 0.0);
        assert_eq!(c.door_target(None), 0.0);
    }

    #[test]
    fn the_target_is_held_within_the_limit_and_closed_without_the_door_layer() {
        let mut c = Controls {
            door_deg: 150.0,
            ..Controls::default()
        };
        assert_eq!(c.door_target(Some(120.0)), 120.0);
        c.clamp_door(Some(120.0));
        assert_eq!(c.door_deg, 120.0);
        c.layers.door = false;
        assert_eq!(c.door_target(Some(120.0)), 0.0);
    }
}
