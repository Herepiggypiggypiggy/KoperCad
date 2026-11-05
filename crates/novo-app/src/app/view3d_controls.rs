//! The 3D view's toolbar: layer toggles (enclosure, door, wires) and the door
//! control. The state is view state; the project is untouched. The scene
//! build and the door swing are applied in `view3d.rs`.

use eframe::egui;
use std::collections::BTreeSet;

use novo_view3d::{ItemId, Layers, Section, SectionAxis};

use super::view3d_options::{Values, ViewOption};

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
    /// Build order: how many steps are done, `None` for the whole cabinet.
    pub(super) assembly: Option<usize>,
    /// Wire-it: the wire being run, `None` for off (`view3d_wire_it::RESUME`
    /// opens at the first wire not run).
    pub(super) wire_it: Option<usize>,
    /// Brushed grain on the plate, streaks on rails.
    pub(super) finish: bool,
    /// Devices, rails and ducts the user hid; the project is untouched.
    pub(super) hidden: BTreeSet<ItemId>,
    /// Screen-space ambient occlusion (the frame governor may pause it).
    pub(super) ssao: bool,
    /// This GPU draws occlusion at all. Set by the host each frame.
    pub(super) ssao_available: bool,
    /// Measuring lines: the enclosure's size and the selected part's.
    pub(super) dimensions: bool,
    /// The frame governor has stepped the quality down: the toolbar offers
    /// to restore it. Set by the host each frame.
    pub(super) reduced: bool,
    /// The user pressed "Restore quality"; the host acts on it and clears it.
    pub(super) restore: bool,
    /// The "?" badge (no part data) on every device, not only on hover.
    pub(super) missing_data: bool,
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
            move_items: false,
            shadows: true,
            assembly: None,
            wire_it: None,
            finish: true,
            hidden: BTreeSet::new(),
            ssao: true,
            ssao_available: true,
            dimensions: false,
            reduced: false,
            restore: false,
            missing_data: false,
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

/// The pane's width while it floats (docked, it takes its strip's).
const PANE_WIDTH: f32 = 300.0;

/// Whether the 3D view pane is docked in a strip: it then covers nothing of
/// the picture, and the panels start at the top.
fn pane_docked(ctx: &egui::Context) -> bool {
    super::panel_dock::slot_of(ctx, "view3d_toolbar").is_some()
}

/// Where the panels under the toolbar start: the compact toolbar of a short
/// window is one row, so they start higher.
pub(super) fn below(ctx: &egui::Context) -> f32 {
    if pane_docked(ctx) {
        MARGIN
    } else {
        // The pane floats over the top left (a narrow window, or the person
        // floated it): the panels start under it.
        PANE_ROOM
    }
}

/// A window under 1000 pt wide: the toolbar is one row taller than it was
/// before the rail.
pub(super) fn narrow_window(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < 1000.0
}

/// The room a floating 3D view pane takes at the top left, with its gap.
const PANE_ROOM: f32 = 110.0;

/// What the toolbar is assumed to cover before it has been drawn once, which
/// is when the first fit of the camera happens.
fn first_inset(ctx: &egui::Context) -> f32 {
    if pane_docked(ctx) { 0.0 } else { PANE_ROOM }
}

/// How much of the view's height the toolbar covers at the top, as a
/// fraction: a fit frames the model below it. Before the toolbar has been
/// drawn once it is [`PANE_ROOM`], or nothing when the pane is docked.
pub(super) fn toolbar_inset(ctx: &egui::Context, view: egui::Rect) -> f32 {
    if pane_docked(ctx) {
        return 0.0;
    }
    let covered = ctx
        .memory(|m| m.area_rect(super::panel::id("view3d_toolbar")))
        .map_or(first_inset(ctx), |r| r.bottom() - view.top());
    (covered / view.height().max(1.0)).clamp(0.0, 0.4)
}

/// The 3D view pane: saved views, the section plane while it is on, the
/// quality warning and the door. It is a pane of the right strip (it folds,
/// floats and moves to another edge), not a chip over the picture.
/// `limit` is how far the enclosure's door opens, `None` when it has none.
/// `values` is a snapshot of the registry (`view3d_options`): a switch edits
/// it and the host writes the difference back to the owner of each option.
#[allow(clippy::too_many_arguments)] // one call site; a struct would only rename them
pub(super) fn toolbar(
    ctx: &egui::Context,
    view: egui::Rect,
    controls: &mut Controls,
    values: &mut Values,
    limit: Option<f64>,
    views: &[crate::settings::CameraBookmark],
    section: &mut Option<Section>,
    bounds: &dyn Fn() -> Option<novo_view3d::Aabb>,
) -> Changed {
    let mut changed = Changed::default();
    let panel = super::panel::Panel::new(
        "view3d_toolbar",
        "3D view",
        view,
        super::panel::Side::Left,
        PANE_WIDTH,
    )
    .top(MARGIN)
    .fit()
    .dockable()
    .dock_by_default();
    panel.show(ctx, |ui| {
        // Layers, Move, Heat, EMC, Measure, Dimensions and Select and hide
        // live on the 3D rail; this pane keeps what the rail cannot hold:
        // saved views, the section plane while it is on, the door and the
        // quality warning.
        changed.views = super::view3d_views::list(ui, views);
        if values.get(ViewOption::Section) {
            ui.separator();
            section_controls(ui, values, section, bounds);
        }
        if controls.reduced {
            ui.separator();
            ui.label(
                egui::RichText::new("Reduced quality")
                    .color(ui.visuals().warn_fg_color)
                    .small(),
            )
            .on_hover_text("The view stayed slow, so it draws at lower resolution while it moves.");
            if ui
                .button("Restore quality")
                .on_hover_text("Draw at full quality again (View > 3D > Restore 3D quality).")
                .clicked()
            {
                controls.restore = true;
            }
        }
        if let Some(limit) = limit
            && values.get(ViewOption::Door)
        {
            ui.horizontal(|ui| {
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
            });
        }
    });
    changed
}

/// The Section controls: a plane through the enclosure that cuts the front of
/// the picture away so the inside shows. Only the picture is cut.
fn section_controls(
    ui: &mut egui::Ui,
    values: &mut Values,
    section: &mut Option<Section>,
    bounds: &dyn Fn() -> Option<novo_view3d::Aabb>,
) {
    ui.strong("Section");
    {
        let mut on = values.get(ViewOption::Section);
        if ui
            .checkbox(&mut on, "Cut the picture with a plane")
            .on_hover_text("Hide everything on one side of a plane to see inside")
            .changed()
        {
            values.set(ViewOption::Section, on);
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_quiet_defaults_hide_badges() {
        let c = Controls::default();
        assert!(!c.missing_data);
        assert!(!c.move_items, "a plain drag must not move parts (KC-15)");
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
