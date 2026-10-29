//! The 3D view's toolbar: layer toggles (enclosure, door, wires) and the door
//! control. The state is view state; the project is untouched. The scene
//! build and the door swing are applied in `view3d.rs`.

use eframe::egui;
use std::collections::BTreeSet;

use novo_view3d::{ItemId, Layers, Section, SectionAxis};

use super::view3d_options::{ALL, Home, Values, ViewOption};

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

/// Wide enough for every toggle and the door slider on one row.
const TOOLBAR_WIDTH: f32 = 980.0;

/// Where the panels under the toolbar start: the compact toolbar of a short
/// window is one row, so they start higher.
pub(super) fn below(ctx: &egui::Context) -> f32 {
    if short_window(ctx) {
        first_inset(ctx) + 14.0 + narrow_extra(ctx)
    } else {
        BELOW
    }
}

/// The rail takes 48 pt of a narrow canvas and the toolbar wraps onto
/// another row there; the panels under it start one row lower.
fn narrow_extra(ctx: &egui::Context) -> f32 {
    if narrow_window(ctx) { 30.0 } else { 0.0 }
}

/// A window under 1000 pt wide: the toolbar is one row taller than it was
/// before the rail.
pub(super) fn narrow_window(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < 1000.0
}

/// A window shorter than this (a 1280 x 688 one) gets the compact toolbar.
/// The window, not the view: the view shrinks when the Heat strip opens, and
/// the camera is fitted once, before that.
const SHORT_WINDOW: f32 = 720.0;

fn short_window(ctx: &egui::Context) -> bool {
    ctx.content_rect().height() < SHORT_WINDOW
}

/// Where a panel under the toolbar starts, from the top of the view.
pub(super) const BELOW: f32 = 140.0;

/// The simulation status chip is the first row of the toolbar. In a window
/// with a title bar it takes that bar's place; the compact toolbar of a short
/// window has no bar, so the row adds this much.
pub(super) const CHIP_ROW: f32 = 30.0;

/// What the toolbar is assumed to cover before it has been drawn once, which
/// is when the first fit of the camera happens.
fn first_inset(ctx: &egui::Context) -> f32 {
    if short_window(ctx) {
        // The status row and one row of buttons, no title bar.
        56.0 + CHIP_ROW
    } else {
        BELOW
    }
}

/// How much of the view's height the toolbar covers at the top, as a
/// fraction: a fit frames the model below it. Before the toolbar has been
/// drawn once it is [`BELOW`].
pub(super) fn toolbar_inset(ctx: &egui::Context, view: egui::Rect) -> f32 {
    let covered = ctx
        .memory(|m| m.area_rect(super::panel::id("view3d_toolbar")))
        .map_or(first_inset(ctx), |r| r.bottom() - view.top());
    (covered / view.height().max(1.0)).clamp(0.0, 0.4)
}

/// The toolbar, top left of the view. `limit` is how far the enclosure's door
/// opens, `None` when it has none. `values` is a snapshot of the registry
/// (`view3d_options`): a switch edits it and the host writes the difference
/// back to the owner of each option.
#[allow(clippy::too_many_arguments)] // one call site; a struct would only rename them
pub(super) fn toolbar(
    ctx: &egui::Context,
    view: egui::Rect,
    controls: &mut Controls,
    values: &mut Values,
    limit: Option<f64>,
    views: &[crate::settings::CameraBookmark],
    selected: bool,
    section: &mut Option<Section>,
    bounds: &dyn Fn() -> Option<novo_view3d::Aabb>,
    status: &mut dyn FnMut(&mut egui::Ui),
) -> (Changed, Option<&'static str>) {
    let mut changed = Changed::default();
    let mut command = None;
    // Leave the view cube (top right) uncovered.
    let width = TOOLBAR_WIDTH
        .min(view.width() - super::view3d_heat::CUBE_SIZE - 3.0 * MARGIN)
        .max(300.0);
    // The simulation status is the first row, where the title bar was: the
    // toolbar has no title (it is still dragged by its body), so the status
    // is in sight with every panel folded and takes no room from the panels
    // under the toolbar.
    let panel = super::panel::Panel::new(
        "view3d_toolbar",
        "3D view",
        view,
        super::panel::Side::Left,
        width,
    )
    .top(MARGIN)
    .bare()
    .fit();
    panel.show(ctx, |ui| {
        status(ui);
        ui.horizontal_wrapped(|ui| {
            // Layers: things that are drawn or not, in one popover.
            let n_on = ALL
                .iter()
                .filter(|s| matches!(s.home, Home::Layer))
                .filter(|s| values.get(s.option))
                .filter(|s| s.option != ViewOption::Occlusion || controls.ssao_available)
                .count();
            let config = egui::containers::menu::MenuConfig::new()
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            egui::containers::menu::MenuButton::new(format!("Layers ({n_on})"))
                .config(config)
                .ui(ui, |ui| {
                    for spec in ALL.iter().filter(|s| matches!(s.home, Home::Layer)) {
                        match spec.option {
                            ViewOption::Surfaces => {
                                if let Some(text) = &controls.models_text {
                                    ui.label(
                                        egui::RichText::new(text)
                                            .small()
                                            .color(crate::theme::ui_palette().text_secondary),
                                    );
                                }
                            }
                            ViewOption::Occlusion if !controls.ssao_available => {
                                ui.add_enabled_ui(false, |ui| {
                                    novo_theme::widgets::filter_toggle(ui, spec.label, false)
                                        .on_disabled_hover_text(
                                            "Needs a discrete GPU; this one cannot draw ambient occlusion",
                                        );
                                });
                                continue;
                            }
                            _ => {}
                        }
                        let before = values.get(spec.option);
                        toggle(ui, values, spec.option);
                        // These five rebuild the scene; the rest are drawn.
                        if before != values.get(spec.option)
                            && matches!(
                                spec.option,
                                ViewOption::Enclosure
                                    | ViewOption::Door
                                    | ViewOption::Wires
                                    | ViewOption::DeviceShapes
                                    | ViewOption::SlottedDucts
                            )
                        {
                            changed.layers = true;
                        }
                    }
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Walk through")
                            .small()
                            .color(crate::theme::ui_palette().text_secondary),
                    );
                    for spec in ALL.iter().filter(|s| matches!(s.home, Home::Walk)) {
                        toggle(ui, values, spec.option);
                    }
                    ui.separator();
                    for spec in ALL.iter().filter(|s| matches!(s.home, Home::Preference)) {
                        toggle(ui, values, spec.option);
                    }
                });
            ui.separator();
            // Modes and overlays: what the pointer does, what sits at the side.
            // A mode, not a layer: keeps the copper wash of a tool.
            let moving = values.get(ViewOption::Move);
            let mode = if moving { "✔ Move" } else { "Move" };
            if ui
                .selectable_label(moving, mode)
                .on_hover_text(ViewOption::Move.spec().hint)
                .clicked()
            {
                values.set(ViewOption::Move, !moving);
            }
            let heat = toggle(ui, values, ViewOption::Heat);
            super::cabinet_bar::note_ctx(ui.ctx(), "view3d.heat", "Heat button", &heat);
            toggle(ui, values, ViewOption::Emc);
            changed.views = super::view3d_views::menu(ui, views);
            command = selection_menu(ui, selected, controls.hidden.len());
            section_menu(ui, values, section, bounds);
            toggle(ui, values, ViewOption::Dimensions);
            // A tool, not a layer: next to the other measuring control.
            toggle(ui, values, ViewOption::Measure);
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
                && values.get(ViewOption::Door)
            {
                // The slider does not wrap by itself: in a narrow window the door
                // gets its own row instead of running under the view cube.
                if width < TOOLBAR_WIDTH {
                    ui.end_row();
                } else {
                    ui.separator();
                }
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
    values: &mut Values,
    section: &mut Option<Section>,
    bounds: &dyn Fn() -> Option<novo_view3d::Aabb>,
) {
    let title = if values.get(ViewOption::Section) {
        "✔ Section"
    } else {
        "Section"
    };
    ui.menu_button(title, |ui| {
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
    });
}

/// A switch of the registry: neutral, with a tick when on, so it never relies
/// on colour alone and never reads as copper. It edits the snapshot; the
/// host writes the change to the option's owner.
fn toggle(ui: &mut egui::Ui, values: &mut Values, option: ViewOption) -> egui::Response {
    let on = values.get(option);
    let spec = option.spec();
    let mut response =
        novo_theme::widgets::filter_toggle(ui, spec.label, on).on_hover_text(format!(
            "{}: {}. {}",
            if on { "On" } else { "Off" },
            spec.hint,
            if on {
                "Click to turn it off."
            } else {
                "Click to turn it on."
            }
        ));
    if response.clicked() {
        values.set(option, !on);
        response.mark_changed();
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The content rect of a window of this height, after one frame.
    fn ctx_of_height(height: f32) -> egui::Context {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1280.0, height),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |_| {});
        out.textures_delta.clear();
        ctx
    }

    #[test]
    fn a_short_window_gets_the_compact_toolbar_and_panels_start_higher() {
        let short = ctx_of_height(688.0);
        let tall = ctx_of_height(768.0);
        assert!(short_window(&short) && !short_window(&tall));
        assert!(below(&short) < below(&tall));
        assert_eq!(below(&tall), BELOW);
        // The panels start under the one-row toolbar that the camera is fitted for.
        assert!(below(&short) > first_inset(&short));
    }

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
