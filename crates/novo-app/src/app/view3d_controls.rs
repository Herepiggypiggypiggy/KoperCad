//! The 3D view's toolbar: layer toggles (enclosure, door, wires) and the door
//! control. The state is view state; the project is untouched. The scene
//! build and the door swing are applied in `view3d.rs`.

use eframe::egui;
use std::collections::BTreeSet;

use novo_view3d::{ItemId, Layers, Section, SectionAxis};

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
    /// Devices, rails and ducts the user hid; the project is untouched.
    pub(super) hidden: BTreeSet<ItemId>,
    /// Screen-space ambient occlusion (the frame governor may pause it).
    pub(super) ssao: bool,
    /// This GPU draws occlusion at all. Set by the host each frame.
    pub(super) ssao_available: bool,
    /// A swipe in wheel lines pans instead of zooming (Windows trackpads).
    pub(super) trackpad_pans: bool,
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
    /// The door, walls, roof, floor and extra plates as faint panes (View
    /// > Cabinet > Cabinet surfaces picks one to highlight).
    pub(super) surfaces: bool,
    /// A marker where each terminal of a part is (Parts window > Connection
    /// points finds or edits them).
    pub(super) pins: bool,
    /// Ducts tinted by how full the routed wires leave them.
    pub(super) duct_fill: bool,
    /// How many devices are drawn with a maker model, set by the host each frame.
    pub(super) models_text: Option<String>,
}

impl Default for Controls {
    fn default() -> Self {
        Controls {
            layers: Layers::default(),
            door_deg: 0.0,
            move_items: true,
            shadows: true,
            hidden: BTreeSet::new(),
            ssao: true,
            ssao_available: true,
            trackpad_pans: false,
            dimensions: false,
            reduced: false,
            restore: false,
            missing_data: false,
            heat_panel: false,
            surfaces: false,
            pins: false,
            duct_fill: false,
            models_text: None,
        }
    }
}

/// What a frame of the toolbar changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Changed {
    pub(super) layers: bool,
    pub(super) door: bool,
    /// What the Views menu asked for.
    pub(super) views: Option<super::view3d_views::ViewsAction>,
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
#[allow(clippy::too_many_arguments)] // one call site; a struct would only rename them
pub(super) fn toolbar(
    ctx: &egui::Context,
    view: egui::Rect,
    controls: &mut Controls,
    measure: &mut bool,
    limit: Option<f64>,
    views: &[crate::settings::CameraBookmark],
    selected: bool,
    section: &mut Option<Section>,
    bounds: &dyn Fn() -> Option<novo_view3d::Aabb>,
) -> (Changed, Option<&'static str>) {
    let mut changed = Changed::default();
    let mut command = None;
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
                controls.layers.shapes,
                controls.shadows,
                controls.ssao && controls.ssao_available,
                controls.missing_data,
                controls.surfaces,
                controls.pins,
                controls.duct_fill,
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
                    changed.layers |= toggle(
                        ui,
                        &mut layers.shapes,
                        "Device shapes",
                        "Draw each device with the typical form of its family (breaker, contactor, \
                         terminal block, PLC, drive, power supply) at the part's real width, height \
                         and depth. The details are typical, not the maker's. Off draws plain boxes.",
                    );
                    if let Some(text) = &controls.models_text {
                        ui.label(egui::RichText::new(text).weak().small());
                    }
                    toggle(
                        ui,
                        &mut controls.surfaces,
                        "Surfaces",
                        "Show the door, walls, roof, floor and extra plates as faint panes. \
                         The surface picked in the Cabinet header is highlighted.",
                    );
                    toggle(
                        ui,
                        &mut controls.duct_fill,
                        "Duct fill",
                        "Tint each duct by how full the routed wires leave it: green up to 80 % \
                         of the allowed fill, amber above, red over the limit. A duct nothing runs in \
                         keeps its normal colour; grey means the duct has no cross-section yet. The figure is the routing \
                         engine's (wire section and the project's fill limit).",
                    );
                    toggle(
                        ui,
                        &mut controls.pins,
                        "Connection points",
                        "Mark where each terminal of a part is: green when measured or \
                         typed, amber when estimated. Set them in the Parts window > \
                         Connection points.",
                    );
                    toggle(
                        ui,
                        measure,
                        "Measure",
                        "Click two points to read the distance, and the gap between two \
                         devices (View > 3D > Measure)",
                    );
                    toggle(
                        ui,
                        &mut controls.shadows,
                        "Shadows",
                        "Soft shadows from the key light (View > 3D > Shadows)",
                    );
                    if controls.ssao_available {
                        toggle(
                            ui,
                            &mut controls.ssao,
                            "Occlusion",
                            "Ambient occlusion: corners and contacts darken (View > 3D > \
                             Ambient occlusion). The 3D view turns it off by itself when it \
                             runs slowly; \"Restore quality\" or switching it off and on asks \
                             for it back.",
                        );
                    } else {
                        ui.add_enabled_ui(false, |ui| {
                            novo_theme::widgets::filter_toggle(ui, "Occlusion", false)
                                .on_disabled_hover_text(
                                    "Needs a discrete GPU; this one cannot draw ambient occlusion",
                                );
                        });
                    }
                    toggle(
                        ui,
                        &mut controls.trackpad_pans,
                        "Trackpad pans",
                        "Two-finger swipes pan instead of zoom. For Windows touchpads, which \
                         send swipes as mouse-wheel steps; a real mouse wheel then pans too. \
                         Zoom with Ctrl + swipe or a pinch.",
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
            changed.views = super::view3d_views::menu(ui, views);
            command = selection_menu(ui, selected, controls.hidden.len());
            section_menu(ui, section, bounds);
            toggle(
                ui,
                &mut controls.dimensions,
                "Dimensions",
                "Measuring lines in mm: the enclosure, and for one selected part its \
                 size and its distances to neighbours and the plate edge, and a dashed \
                 rectangle around it: the free space it asks for (View > 3D > \
                 Dimensions)",
            );
            if controls.reduced {
                ui.label(
                    egui::RichText::new("Reduced quality")
                        .color(ui.visuals().warn_fg_color)
                        .small(),
                )
                .on_hover_text(
                    "The view stayed slow, so it draws at lower resolution while it moves.",
                );
                if ui
                    .button("Restore quality")
                    .on_hover_text("Draw at full quality again (View > 3D > Restore 3D quality).")
                    .clicked()
                {
                    controls.restore = true;
                }
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
                    .on_hover_text(super::tips::hover(
                        ui.ctx(),
                        "Open or close the door",
                        "view.3d_door",
                    ))
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
    (changed, command)
}

/// The Select and Hide menu: pick every device, rail or duct, hide or isolate
/// the selection, bring everything back. Returns the command asked for.
fn selection_menu(ui: &mut egui::Ui, selected: bool, hidden: usize) -> Option<&'static str> {
    let mut command = None;
    let title = if hidden > 0 {
        format!("Select and hide ({hidden} hidden)")
    } else {
        "Select and hide".to_owned()
    };
    ui.menu_button(title, |ui| {
        let mut row = |ui: &mut egui::Ui, label: &str, id: &'static str, enabled: bool| {
            if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                command = Some(id);
                ui.close();
            }
        };
        row(ui, "Select all devices", "view.3d_select_devices", true);
        row(ui, "Select all rails", "view.3d_select_rails", true);
        row(ui, "Select all ducts", "view.3d_select_ducts", true);
        ui.separator();
        row(ui, "Hide selection   Ctrl+H", "view.3d_hide", selected);
        row(
            ui,
            "Isolate selection   Ctrl+I",
            "view.3d_isolate",
            selected,
        );
        row(
            ui,
            "Show all   Ctrl+Shift+H",
            "view.3d_show_all",
            hidden > 0,
        );
    });
    command
}

/// The Section menu: a plane through the enclosure that cuts the front of
/// the picture away so the inside shows. Only the picture is cut.
fn section_menu(
    ui: &mut egui::Ui,
    section: &mut Option<Section>,
    bounds: &dyn Fn() -> Option<novo_view3d::Aabb>,
) {
    let title = if section.is_some() {
        "✔ Section"
    } else {
        "Section"
    };
    ui.menu_button(title, |ui| {
        let mut on = section.is_some();
        if ui
            .checkbox(&mut on, "Cut the picture with a plane")
            .on_hover_text("Hide everything on one side of a plane to see inside")
            .changed()
        {
            *section = on.then(Section::default);
        }
        let Some(s) = section.as_mut() else {
            return;
        };
        ui.horizontal(|ui| {
            for (axis, name, hint) in [
                (SectionAxis::X, "X", "Across the plate"),
                (SectionAxis::Y, "Y", "Up the plate"),
                (
                    SectionAxis::Z,
                    "Z",
                    "Out of the plate: slices the depth of the enclosure",
                ),
            ] {
                ui.selectable_value(&mut s.axis, axis, name)
                    .on_hover_text(hint);
            }
            ui.checkbox(&mut s.flip, "Flip")
                .on_hover_text("Hide the other side of the plane");
        });
        // In mm along the axis when the scene is at hand.
        match bounds().filter(|b| !b.is_empty()) {
            Some(b) => {
                let (lo, hi) = s.range(&b);
                let mut mm = s.position_mm(&b);
                if ui
                    .add(
                        egui::Slider::new(&mut mm, lo..=hi)
                            .suffix(" mm")
                            .max_decimals(1),
                    )
                    .on_hover_text("Where the plane stands, in mm along the axis; type a value")
                    .changed()
                {
                    s.set_position_mm(&b, mm);
                }
            }
            None => {
                ui.add(egui::Slider::new(&mut s.at, 0.0..=1.0).show_value(false))
                    .on_hover_text("Where the plane stands");
            }
        }
    });
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
