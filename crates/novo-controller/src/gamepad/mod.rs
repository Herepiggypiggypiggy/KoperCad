//! Gamepad and joystick input for the controller (lane C5): a device-independent state of one
//! input device, the per-controller [`GamepadMap`] that says which axis or button drives which
//! control, the [`PadMapper`] that turns the state into holds and presses, and the
//! [`Calibrator`] behind the "move the stick to all corners" wizard.
//!
//! Like the rest of the crate it is pure: `std` only, no device access, no clock, no files, so it
//! also compiles to the add-on `engine.wasm`. The host (`novo-app`) reads the real device, or a
//! fake one in tests, and hands the state in once per frame. Nothing here claims more than the
//! wiring and logic of the profile: a pad is just another way to hold the same controls.
//!
//! Safety behaviour that is part of the contract (each has tests):
//! - the dead-man gate: with a dead-man input mapped, nothing that moves (axes, hold buttons) acts
//!   while it is not held, and after it is grasped again a deflected stick must first return to
//!   neutral;
//! - the E-stop button acts on the press edge, whatever the dead-man says;
//! - a device that disappears releases every control at once and then applies the policy of the
//!   map (nothing more, drop the radio link, or press the E-stop); reconnecting never presses or
//!   resets anything by itself;
//! - whatever is held when the device connects must be let go first.

mod calibrate;
mod input;
mod map;
mod run;

pub use calibrate::{AxisCal, AxisProgress, CalError, CalPhase, Calibrator};
pub use input::{AXES, BUTTONS, InputState, TRIGGER_DOWN, is_axis_name, is_button_name};
pub use map::{
    AxisMap, ButtonAction, ButtonMap, DEFAULT_DEADZONE, DEFAULT_HYSTERESIS, DeadmanMap, GamepadMap,
    LinkLoss, normalise,
};
pub use run::{AxisReading, PadMapper, PadOutput, band_of, detent_counts, step_edges, step_of};
