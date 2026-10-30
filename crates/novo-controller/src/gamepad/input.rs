//! The state of one input device, by stable names.

use std::collections::BTreeMap;

/// Axis names every device uses when it has the part. Sticks run -1 to +1 with up and right
/// positive; triggers run 0 to 1; the d-pad is two axes of -1, 0, +1.
pub const AXES: [&str; 8] = [
    "left_x",
    "left_y",
    "right_x",
    "right_y",
    "left_trigger",
    "right_trigger",
    "dpad_x",
    "dpad_y",
];

/// Button names (the positions of an Xbox pad; `south` is the lower face button, `a` on Xbox).
pub const BUTTONS: [&str; 15] = [
    "south",
    "east",
    "west",
    "north",
    "left_bumper",
    "right_bumper",
    "select",
    "start",
    "mode",
    "left_thumb",
    "right_thumb",
    "dpad_up",
    "dpad_down",
    "dpad_left",
    "dpad_right",
];

/// An axis counts as a pressed button above this value (a trigger used as a dead-man switch).
pub const TRIGGER_DOWN: f64 = 0.5;

fn numbered(prefix: &str, name: &str) -> bool {
    name.strip_prefix(prefix)
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// A standard axis name, or `axis3`: a device that has no standard layout reports numbered ones.
pub fn is_axis_name(name: &str) -> bool {
    AXES.contains(&name) || numbered("axis", name)
}

/// A standard button name, or `button7`.
pub fn is_button_name(name: &str) -> bool {
    BUTTONS.contains(&name) || numbered("button", name)
}

/// What one device reports at one moment. Missing names read as zero and released.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InputState {
    pub connected: bool,
    pub axes: BTreeMap<String, f64>,
    pub buttons: BTreeMap<String, bool>,
}

impl InputState {
    /// A connected device with everything at rest.
    pub fn connected() -> Self {
        InputState {
            connected: true,
            ..Self::default()
        }
    }

    /// No device (unplugged, or never there).
    pub fn disconnected() -> Self {
        Self::default()
    }

    pub fn with_axis(mut self, name: &str, value: f64) -> Self {
        self.axes.insert(name.to_owned(), value);
        self
    }

    pub fn with_button(mut self, name: &str, down: bool) -> Self {
        self.buttons.insert(name.to_owned(), down);
        self
    }

    /// The raw value of an axis; zero when missing, not a number or infinite.
    pub fn axis(&self, name: &str) -> f64 {
        match self.axes.get(name) {
            Some(v) if v.is_finite() => *v,
            _ => 0.0,
        }
    }

    pub fn button(&self, name: &str) -> bool {
        self.buttons.get(name).copied().unwrap_or(false)
    }

    /// Whether the named input counts as pressed: a button that is down, or an axis (a trigger)
    /// beyond [`TRIGGER_DOWN`]. A disconnected device presses nothing.
    pub fn is_down(&self, name: &str) -> bool {
        self.connected && (self.button(name) || self.axis(name) > TRIGGER_DOWN)
    }
}
