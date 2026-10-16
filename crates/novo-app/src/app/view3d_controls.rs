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

/// The toolbar, top left of the view. `limit` is how far the enclosure's door
/// opens, `None` when it has none.
pub(super) fn toolbar(
    ctx: &egui::Context,
    view: egui::Rect,
    controls: &mut Controls,
    limit: Option<f64>,
) -> Changed {
    let mut changed = Changed::default();
    egui::Area::new(egui::Id::new("view3d_toolbar"))
        .order(egui::Order::Middle)
        .fixed_pos(view.left_top() + egui::vec2(MARGIN, MARGIN))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
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
                            changed.layers |= ui
                                .checkbox(&mut layers.shell, "Enclosure")
                                .on_hover_text("Show the enclosure box")
                                .changed();
                            changed.layers |= ui
                                .checkbox(&mut layers.door, "Door")
                                .on_hover_text("Show the door")
                                .changed();
                            changed.layers |= ui
                                .checkbox(&mut layers.wires, "Wires")
                                .on_hover_text("Show the routed wires")
                                .changed();
                            ui.checkbox(&mut controls.shadows, "Shadows").on_hover_text(
                                "Soft shadows from the key light (View > 3D > Shadows)",
                            );
                            ui.checkbox(&mut controls.missing_data, "Missing data")
                                .on_hover_text(
                                    "Mark every device that has no part data with a \"?\". Off: \
                                     the \"?\" shows on the device under the pointer.",
                                );
                        });
                    ui.separator();
                    // Modes and panels: what the pointer does, what sits at the side.
                    toggle(
                        ui,
                        &mut controls.move_items,
                        "Move",
                        "Move mode. Drag a device, rail or duct to move it on the plate (grid \
                         and checks as in the Cabinet). R turns the selection, Delete takes it \
                         off. Off: a drag draws a selection box.",
                    );
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
        });
    changed
}

/// A toggle button whose state is also in its look (a tick), so it never
/// relies on colour alone.
fn toggle(ui: &mut egui::Ui, on: &mut bool, name: &str, hint: &str) -> bool {
    let text = if *on {
        format!("✔ {name}")
    } else {
        name.to_owned()
    };
    if ui.selectable_label(*on, text).on_hover_text(hint).clicked() {
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
