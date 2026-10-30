//! Windows gamepads and joysticks through `gilrs` (Windows Gaming Input): it ships
//! with Windows, needs no driver install and no administrator rights, and works offline.
//! Devices with the standard layout (Xbox style pads and everything SDL knows a mapping for)
//! are reported by the names of `novo_controller::gamepad`; a device without a known mapping
//! shows up connected with every value at zero.
//!
//! Not linked on other systems: on Linux `gilrs` needs the system library `libudev`, which
//! the CI image does not have (see `input_devices.rs`).

use std::collections::BTreeMap;

use gilrs::{Axis, Button, Gilrs};
use novo_controller::gamepad::InputState;

use super::input_devices::{Device, InputBackend};

pub(super) struct GilrsBackend {
    gilrs: Gilrs,
    /// Devices seen so far, by id, so one that is unplugged stays listed as disconnected.
    known: BTreeMap<usize, String>,
}

impl GilrsBackend {
    pub(super) fn new() -> Result<Self, String> {
        Gilrs::new()
            .map(|gilrs| GilrsBackend {
                gilrs,
                known: BTreeMap::new(),
            })
            .map_err(|e| e.to_string())
    }
}

/// `(our axis name, gilrs axis)` for the sticks and the d-pad.
const AXES: [(&str, Axis); 6] = [
    ("left_x", Axis::LeftStickX),
    ("left_y", Axis::LeftStickY),
    ("right_x", Axis::RightStickX),
    ("right_y", Axis::RightStickY),
    ("dpad_x", Axis::DPadX),
    ("dpad_y", Axis::DPadY),
];

const BUTTONS: [(&str, Button); 15] = [
    ("south", Button::South),
    ("east", Button::East),
    ("west", Button::West),
    ("north", Button::North),
    ("left_bumper", Button::LeftTrigger),
    ("right_bumper", Button::RightTrigger),
    ("select", Button::Select),
    ("start", Button::Start),
    ("mode", Button::Mode),
    ("left_thumb", Button::LeftThumb),
    ("right_thumb", Button::RightThumb),
    ("dpad_up", Button::DPadUp),
    ("dpad_down", Button::DPadDown),
    ("dpad_left", Button::DPadLeft),
    ("dpad_right", Button::DPadRight),
];

impl InputBackend for GilrsBackend {
    fn label(&self) -> &str {
        "Windows Gaming Input"
    }

    fn poll(&mut self) -> Vec<Device> {
        // Reading the events brings the cached state of every pad up to date.
        while self.gilrs.next_event().is_some() {}
        let mut now: BTreeMap<usize, InputState> = BTreeMap::new();
        for (id, pad) in self.gilrs.gamepads() {
            let id = usize::from(id);
            self.known.insert(id, pad.name().to_owned());
            let mut state = InputState::connected();
            for (name, axis) in AXES {
                state
                    .axes
                    .insert(name.to_owned(), f64::from(pad.value(axis)));
            }
            for (name, button) in [
                ("left_trigger", Button::LeftTrigger2),
                ("right_trigger", Button::RightTrigger2),
            ] {
                let analog = pad.button_data(button).map_or(0.0, |d| d.value());
                let value = if pad.is_pressed(button) {
                    analog.max(1.0)
                } else {
                    analog
                };
                state.axes.insert(name.to_owned(), f64::from(value));
            }
            for (name, button) in BUTTONS {
                state
                    .buttons
                    .insert(name.to_owned(), pad.is_pressed(button));
            }
            now.insert(id, state);
        }
        self.known
            .iter()
            .map(|(id, name)| Device {
                id: format!("pad{id}"),
                name: name.clone(),
                state: now.remove(id).unwrap_or_else(InputState::disconnected),
            })
            .collect()
    }
}
