//! The 3D view's options: one registry, one stored value each.
//!
//! Every option the person can switch in the 3D workspace (a layer such as
//! the door, an overlay such as Heat or EMC, a mode such as Measure) is one
//! [`ViewOption`] with one owner of its state. The toolbar, the commands
//! (Ctrl K and the menus) and the lessons read and write the option through
//! [`NovoApp::view3d_option`] and [`NovoApp::set_view3d_option`] and keep no
//! copy of their own, so two buttons for one thing cannot disagree.
//! Docs: `docs/design/visual-redesign-3d.md`.

use std::collections::BTreeSet;

use super::NovoApp;
use super::emc_view::EmcState;
use super::heat::HeatState;
use super::view3d::View3dMode;
use super::view3d_controls::Controls;
use super::view3d_measure::Measure;

/// What kind of control an option is. The Layers popover shows `Layer` and
/// `Preference` rows, the toolbar shows the rest; an option is in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Home {
    /// Something drawn or not.
    Layer,
    /// A view that comes with its own panel, legend or strip.
    Overlay,
    /// What the pointer does.
    Mode,
    /// A walk through the cabinet with a strip under the view (Build order,
    /// Wire-it). It sits in the Layers popover for now.
    Walk,
    /// How the view is handled, kept in the settings.
    Preference,
}

/// A switch of the 3D view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum ViewOption {
    Enclosure,
    Door,
    Wires,
    DeviceShapes,
    SlottedDucts,
    Surfaces,
    DuctFill,
    ConnectionPoints,
    Shadows,
    MetalFinish,
    Occlusion,
    MissingData,
    Dimensions,
    Heat,
    Emc,
    WireRuns,
    Move,
    Measure,
    Section,
    BuildOrder,
    WireIt,
    TrackpadPans,
}

/// What the registry knows about an option.
pub(crate) struct Spec {
    pub option: ViewOption,
    pub label: &'static str,
    pub hint: &'static str,
    pub home: Home,
    /// The Ctrl K command that toggles it (also its menu row, where it has one).
    pub command: &'static str,
}

use Home::{Layer, Mode, Overlay, Preference, Walk};

/// Every option, in the order the toolbar draws them.
pub(crate) const ALL: &[Spec] = &[
    Spec {
        option: ViewOption::Enclosure,
        label: "Enclosure",
        hint: "Show the enclosure box",
        home: Layer,
        command: "view.3d_shell",
    },
    Spec {
        option: ViewOption::Door,
        label: "Door",
        hint: "Show the door",
        home: Layer,
        command: "view.3d_door_layer",
    },
    Spec {
        option: ViewOption::Wires,
        label: "Wires",
        hint: "Show the routed wires",
        home: Layer,
        command: "view.3d_wires",
    },
    Spec {
        option: ViewOption::DeviceShapes,
        label: "Device shapes",
        hint: "Draw each device with the typical form of its family (breaker, contactor, \
               terminal block, PLC, drive, power supply) at the part's real width, height \
               and depth. The details are typical, not the maker's. Off draws plain boxes.",
        home: Layer,
        command: "view.3d_device_shapes",
    },
    Spec {
        option: ViewOption::SlottedDucts,
        label: "Slotted ducts",
        hint: "Draw each duct with slotted walls (fingers) and mounting slots in the floor, \
               at the real width, height and length. The finger pitch (about 9 mm) and the \
               slot width are a generic drawing, not the data of a catalogue duct, and \
               the heat and fill figures do not read them. Far out the duct is drawn \
               solid. Off draws every duct solid.",
        home: Layer,
        command: "view.3d_slotted_ducts",
    },
    Spec {
        option: ViewOption::Surfaces,
        label: "Surfaces",
        hint: "Show the door, walls, roof, floor and extra plates as faint panes. \
               The surface picked in the Cabinet header is highlighted.",
        home: Layer,
        command: "view.3d_surfaces",
    },
    Spec {
        option: ViewOption::DuctFill,
        label: "Duct fill",
        hint: "Tint each duct by how full the routed wires leave it: green up to 80 % \
               of the allowed fill, amber above, red over the limit. A duct nothing runs in \
               keeps its normal colour; grey means the duct has no cross-section yet. The \
               figure is the routing engine's (wire section and the project's fill limit).",
        home: Layer,
        command: "view.3d_duct_fill",
    },
    Spec {
        option: ViewOption::ConnectionPoints,
        label: "Connection points",
        hint: "Mark where each terminal of a part is: green when measured or \
               typed, amber when estimated. Set them in the Parts window > \
               Connection points.",
        home: Layer,
        command: "view.3d_pins",
    },
    Spec {
        option: ViewOption::Shadows,
        label: "Shadows",
        hint: "Soft shadows from the key light",
        home: Layer,
        command: "view.3d_shadows",
    },
    Spec {
        option: ViewOption::MetalFinish,
        label: "Metal finish",
        hint: "A faint brushed grain and soft patches on the plate and streaks \
               along the rails. It is subtle by design: broad patches show in the \
               overview, the fine grain only when you zoom in. Off: plain flat colours.",
        home: Layer,
        command: "view.3d_finish",
    },
    Spec {
        option: ViewOption::Occlusion,
        label: "Occlusion",
        hint: "Ambient occlusion: corners and contacts darken. The 3D view turns it off \
               by itself when it runs slowly; \"Restore quality\" or switching it off and \
               on asks for it back.",
        home: Layer,
        command: "view.3d_ssao",
    },
    Spec {
        option: ViewOption::MissingData,
        label: "Missing data",
        hint: "Mark every device that has no part data with a \"?\". Off: \
               the \"?\" shows on the device under the pointer.",
        home: Layer,
        command: "view.3d_missing_data",
    },
    Spec {
        option: ViewOption::Dimensions,
        label: "Dimensions",
        hint: "Measuring lines in mm: the enclosure, and for one selected part its \
               size and its distances to neighbours and the plate edge, and a dashed \
               rectangle around it: the free space it asks for",
        home: Layer,
        command: "view.3d_dimensions",
    },
    Spec {
        option: ViewOption::TrackpadPans,
        label: "Trackpad pans",
        hint: "Two-finger swipes pan instead of zoom. For Windows touchpads, which \
               send swipes as mouse-wheel steps; a real mouse wheel then pans too. \
               Zoom with Ctrl + swipe or a pinch.",
        home: Preference,
        command: "view.3d_trackpad_pans",
    },
    Spec {
        option: ViewOption::Heat,
        label: "Heat",
        hint: "The heat view: its panel, strip and the picture in the cabinet. One switch \
               for the Cabinet and the 3D view.",
        home: Overlay,
        command: "heat.toggle",
    },
    Spec {
        option: ViewOption::Emc,
        label: "EMC",
        hint: "Electromagnetic compatibility: cable classes, keep-clear zones round noisy \
               runs and numbered hot spots, as an indicative model",
        home: Overlay,
        command: "view.emc",
    },
    Spec {
        option: ViewOption::WireRuns,
        label: "Wire runs",
        hint: "Lines and a card for how the wires run through the ducts",
        home: Overlay,
        command: "view.3d_wire_runs",
    },
    Spec {
        option: ViewOption::Move,
        label: "Move",
        hint: "Move mode. Drag a device, rail or duct to move it on the plate (grid \
               and checks as in the Cabinet). R turns the selection, Delete takes it \
               off. Off: a drag draws a selection box.",
        home: Mode,
        command: "view.3d_move",
    },
    Spec {
        option: ViewOption::Measure,
        label: "Measure",
        hint: "Click two points to read the distance, and the gap between two devices",
        home: Mode,
        command: "view.3d_measure",
    },
    Spec {
        option: ViewOption::Section,
        label: "Section",
        hint: "Cut the picture with a plane to see inside",
        home: Mode,
        command: "view.3d_section",
    },
    Spec {
        option: ViewOption::BuildOrder,
        label: "Build order",
        hint: "Step through the build order of this cabinet: rails, ducts, devices, \
               then wires, the same order as the printed assembly pack. The part a \
               step adds is shown in blue.",
        home: Walk,
        command: "view.3d_assembly",
    },
    Spec {
        option: ViewOption::WireIt,
        label: "Wire-it",
        hint: "Run the wires one at a time in bench order (earth, power, 24 V, \
               signals): both ends and the path are lit, with the cut, strip, \
               ferrule, label and how full the duct is after each wire. Ticks \
               are saved in the project.",
        home: Walk,
        command: "view.3d_wire_it",
    },
];

impl ViewOption {
    pub(crate) fn spec(self) -> &'static Spec {
        ALL.iter()
            .find(|s| s.option == self)
            .expect("every option has a spec")
    }

    /// The option a command toggles.
    pub(crate) fn from_command(id: &str) -> Option<Self> {
        ALL.iter().find(|s| s.command == id).map(|s| s.option)
    }
}

/// The options that are on, as one snapshot a surface can read and edit.
/// The host writes the differences back through
/// [`NovoApp::set_view3d_option`], so a surface never owns a value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Values(BTreeSet<ViewOption>);

impl Values {
    pub(crate) fn get(&self, option: ViewOption) -> bool {
        self.0.contains(&option)
    }

    pub(crate) fn set(&mut self, option: ViewOption, on: bool) {
        if on {
            self.0.insert(option);
        } else {
            self.0.remove(&option);
        }
    }

    /// The options whose value is different in `other`, with that value.
    pub(crate) fn changes(&self, other: &Values) -> Vec<(ViewOption, bool)> {
        ALL.iter()
            .map(|s| s.option)
            .filter(|o| self.get(*o) != other.get(*o))
            .map(|o| (o, other.get(o)))
            .collect()
    }
}

/// Whether the option is on, read from its owner.
pub(crate) fn read(
    mode: &View3dMode,
    heat: &HeatState,
    trackpad_pans: bool,
    option: ViewOption,
) -> bool {
    let c = &mode.controls;
    match option {
        ViewOption::Enclosure => c.layers.shell,
        ViewOption::Door => c.layers.door,
        ViewOption::Wires => c.layers.wires,
        ViewOption::DeviceShapes => c.layers.shapes,
        ViewOption::SlottedDucts => c.layers.slots,
        ViewOption::Surfaces => c.surfaces,
        ViewOption::DuctFill => c.duct_fill,
        ViewOption::ConnectionPoints => c.pins,
        ViewOption::Shadows => c.shadows,
        ViewOption::MetalFinish => c.finish,
        ViewOption::Occlusion => c.ssao,
        ViewOption::MissingData => c.missing_data,
        ViewOption::Dimensions => c.dimensions,
        ViewOption::TrackpadPans => trackpad_pans,
        ViewOption::Heat => heat.on,
        ViewOption::Emc => mode.emc.on,
        ViewOption::WireRuns => mode.wire_runs,
        ViewOption::Move => c.move_items,
        ViewOption::Measure => mode.measure.on,
        ViewOption::Section => mode.view.state.section.is_some(),
        ViewOption::BuildOrder => c.assembly.is_some(),
        ViewOption::WireIt => c.wire_it.is_some(),
    }
}

/// Every option's value now.
pub(crate) fn snapshot(mode: &View3dMode, heat: &HeatState, trackpad_pans: bool) -> Values {
    let mut values = Values::default();
    for s in ALL {
        values.set(s.option, read(mode, heat, trackpad_pans, s.option));
    }
    values
}

/// The owners of the options, borrowed field by field so a surface can write
/// while the scene (which lives in the same mode) is still borrowed.
pub(crate) struct Owners<'a> {
    pub controls: &'a mut Controls,
    pub emc: &'a mut EmcState,
    pub wire_runs: &'a mut bool,
    pub measure: &'a mut Measure,
    pub section: &'a mut Option<novo_view3d::Section>,
    pub heat: &'a mut HeatState,
    pub trackpad_pans: &'a mut bool,
}

impl Owners<'_> {
    /// Sets the option in its owner. Turning Build order on turns Wire-it
    /// off and the other way round (one strip below the view); Wire runs
    /// needs the wires drawn.
    pub(crate) fn write(&mut self, option: ViewOption, on: bool) {
        let c = &mut *self.controls;
        match option {
            ViewOption::Enclosure => c.layers.shell = on,
            ViewOption::Door => c.layers.door = on,
            ViewOption::Wires => c.layers.wires = on,
            ViewOption::DeviceShapes => c.layers.shapes = on,
            ViewOption::SlottedDucts => c.layers.slots = on,
            ViewOption::Surfaces => c.surfaces = on,
            ViewOption::DuctFill => c.duct_fill = on,
            ViewOption::ConnectionPoints => c.pins = on,
            ViewOption::Shadows => c.shadows = on,
            ViewOption::MetalFinish => c.finish = on,
            ViewOption::Occlusion => c.ssao = on,
            ViewOption::MissingData => c.missing_data = on,
            ViewOption::Dimensions => c.dimensions = on,
            ViewOption::TrackpadPans => *self.trackpad_pans = on,
            ViewOption::Heat => self.heat.on = on,
            ViewOption::Emc => self.emc.on = on,
            ViewOption::WireRuns => {
                *self.wire_runs = on;
                if on {
                    c.layers.wires = true;
                }
            }
            ViewOption::Move => c.move_items = on,
            ViewOption::Measure => {
                if on != self.measure.on {
                    self.measure.toggle();
                }
            }
            ViewOption::Section => {
                if on != self.section.is_some() {
                    *self.section = on.then(novo_view3d::Section::default);
                }
            }
            ViewOption::BuildOrder => {
                if on != c.assembly.is_some() {
                    c.assembly = on.then_some(0);
                }
                if on {
                    c.wire_it = None;
                }
            }
            ViewOption::WireIt => {
                if on != c.wire_it.is_some() {
                    c.wire_it = on.then_some(super::view3d_wire_it::RESUME);
                }
                if on {
                    c.assembly = None;
                }
            }
        }
    }
}

impl NovoApp {
    /// Whether the option is on, read from its owner.
    pub(crate) fn view3d_option(&self, option: ViewOption) -> bool {
        read(
            &self.view3d,
            &self.cabinet.heat,
            self.settings.trackpad_pans,
            option,
        )
    }

    /// Sets the option in its owner.
    pub(crate) fn set_view3d_option(&mut self, option: ViewOption, on: bool) {
        let mode = &mut self.view3d;
        Owners {
            controls: &mut mode.controls,
            emc: &mut mode.emc,
            wire_runs: &mut mode.wire_runs,
            measure: &mut mode.measure,
            section: &mut mode.view.state.section,
            heat: &mut self.cabinet.heat,
            trackpad_pans: &mut self.settings.trackpad_pans,
        }
        .write(option, on);
    }

    /// Flips the option; what a command or a key does.
    pub(crate) fn toggle_view3d_option(&mut self, option: ViewOption) {
        let on = !self.view3d_option(option);
        self.set_view3d_option(option, on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::view3d_tests::drive_with_cabinet;
    use crate::app::workspace::Workspace;
    use eframe::egui;

    fn visible(ctx: &egui::Context, panel: &str) -> bool {
        let layer = egui::LayerId::new(egui::Order::Middle, super::super::panel::id(panel));
        ctx.memory(|m| m.areas().visible_last_frame(&layer))
    }

    #[test]
    fn every_option_has_one_label_one_command_and_the_app_runs_it() {
        let mut labels = BTreeSet::new();
        let mut commands = BTreeSet::new();
        for s in ALL {
            assert!(labels.insert(s.label), "{} is listed twice", s.label);
            assert!(commands.insert(s.command), "{} is listed twice", s.command);
            assert!(
                novo_workflow::commands::COMMANDS
                    .iter()
                    .any(|c| c.id == s.command),
                "{} is not a command",
                s.command
            );
            assert!(
                super::super::workbench::app_runs(s.command),
                "{} is not run by the app",
                s.command
            );
            assert_eq!(ViewOption::from_command(s.command), Some(s.option));
        }
        // Every variant is in the table once.
        assert_eq!(ALL.len(), 22);
    }

    #[test]
    fn the_command_of_each_option_flips_the_one_value_and_a_frame_keeps_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = drive_with_cabinet(dir.path());
        let ctx = d.ctx.clone();
        for s in ALL {
            let first = d.app.view3d_option(s.option);
            d.app.run_command(&ctx, s.command, None);
            d.frame();
            assert_eq!(d.app.workspace(), Workspace::View3d, "{}", s.label);
            assert_eq!(d.app.view3d_option(s.option), !first, "{} on", s.label);
            // The snapshot every surface reads says the same.
            let values = snapshot(
                &d.app.view3d,
                &d.app.cabinet.heat,
                d.app.settings.trackpad_pans,
            );
            assert_eq!(values.get(s.option), !first, "{}", s.label);
            d.app.run_command(&ctx, s.command, None);
            d.frame();
            assert_eq!(d.app.view3d_option(s.option), first, "{} off", s.label);
        }
    }

    #[test]
    fn emc_turns_off_and_stays_off_through_frames_and_a_selection() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = drive_with_cabinet(dir.path());
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "view.emc", None);
        for _ in 0..4 {
            d.frame();
        }
        assert!(d.app.view3d_option(ViewOption::Emc), "on");
        // The old toolbar wrote the value back from the panel every frame.
        d.app.set_view3d_option(ViewOption::Emc, false);
        for _ in 0..4 {
            d.frame();
        }
        assert!(!d.app.view3d_option(ViewOption::Emc), "off");
        assert!(!visible(&ctx, "view3d_emc_panel"), "its panel is gone");
        d.app.view3d.view.state.selection.clear();
        d.frame();
        assert!(!d.app.view3d_option(ViewOption::Emc), "still off");
    }

    #[test]
    fn heat_is_one_switch_for_the_panels_and_the_command() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = drive_with_cabinet(dir.path());
        let ctx = d.ctx.clone();
        assert!(!d.app.view3d_option(ViewOption::Heat));
        d.app.run_command(&ctx, "heat.toggle", None);
        for _ in 0..3 {
            d.frame();
        }
        assert_eq!(d.app.workspace(), Workspace::View3d, "stays in 3D");
        assert!(
            d.app.cabinet.heat.on,
            "the Cabinet's switch is the same one"
        );
        assert!(visible(&ctx, "view3d_heat_panel"));
        d.app.set_view3d_option(ViewOption::Heat, false);
        for _ in 0..3 {
            d.frame();
        }
        assert!(!d.app.cabinet.heat.on);
        assert!(!visible(&ctx, "view3d_heat_panel"), "the panel goes");
        assert!(!visible(&ctx, "view3d_vents"), "the vents panel goes");
    }

    #[test]
    fn the_layers_popover_and_the_toolbar_do_not_both_carry_an_option() {
        // The Layers popover shows layers and preferences; the toolbar shows
        // overlays and modes. An option has one home.
        for s in ALL {
            let in_popover = matches!(s.home, Home::Layer | Home::Walk | Home::Preference);
            let on_toolbar = matches!(s.home, Home::Overlay | Home::Mode);
            assert!(in_popover != on_toolbar, "{}", s.label);
        }
    }
}
