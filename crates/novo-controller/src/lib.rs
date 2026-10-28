//! Controller engine skeleton (lane C0): a virtual radio, pendant, joystick or PLC test panel.
//!
//! A controller is plain data: controls (buttons, rockers, axes, selectors, E-stop) whose
//! positions make named signals high or low. It simulates wiring and logic only: no radio
//! protocol, receiver firmware, contact bounce or compliance claim. Profiles are generic and
//! neutral; vendor profiles are made by the user from their own data.

pub mod import;

/// Kind of operator control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Button,
    Latch,
    Rocker2,
    Rocker3,
    Axis,
    Selector,
    EStop,
    Deadman,
    Key,
}

/// Positions of a spring-return two-step rocker in one direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Off,
    One,
    Two,
}

/// Contacts closed by a two-step rocker position: `(slow, fast)`.
/// With `step2_includes_step1` the slow contact stays closed at step two, as on a real
/// two-step switch; otherwise it drops when the fast contact closes.
pub fn two_step_contacts(step: Step, step2_includes_step1: bool) -> (bool, bool) {
    match step {
        Step::Off => (false, false),
        Step::One => (true, false),
        Step::Two => (step2_includes_step1, true),
    }
}

#[cfg(test)]
mod tests {
    use super::{Step, two_step_contacts};

    #[test]
    fn two_step_truth_table() {
        assert_eq!(two_step_contacts(Step::Off, true), (false, false));
        assert_eq!(two_step_contacts(Step::One, true), (true, false));
        assert_eq!(two_step_contacts(Step::Two, true), (true, true));
        assert_eq!(two_step_contacts(Step::Two, false), (false, true));
    }
}
