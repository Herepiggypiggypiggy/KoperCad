//! Controller engine (lane C1): a virtual radio, pendant, joystick or PLC test panel.
//!
//! A controller is plain data (a [`Profile`], `*.controller.toml`): controls (buttons, rockers,
//! axes, selectors, E-stop) whose positions make named signals high or low. It simulates wiring
//! and logic only: no radio protocol, receiver firmware, contact bounce or compliance claim.
//! Profiles are generic and neutral; vendor profiles are made by the user from their own data.
//!
//! Layers:
//! - [`profile`] and [`validate`]: the file format, parsing and the validator.
//! - [`engine`]: one [`Controller`] (state machines, interlocks, link effects, signals).
//! - [`set`]: several named instances per project ([`ControllerSet`]).
//! - [`binding`]: signal to target bindings keyed `controller/signal`, with checks.
//! - [`sequence`]: deterministic record and replay of timed presses.
//!
//! The crate has no dependency on the data model or the simulator, uses only `std` (no files,
//! clock, threads or global state) and a deterministic order everywhere, so it also compiles to
//! the add-on `engine.wasm`. Time is an integer number of milliseconds driven by the caller.

pub mod binding;
pub mod engine;
pub mod import;
pub mod profile;
pub mod sequence;
pub mod set;
pub mod validate;

pub use binding::{
    Binding, BindingFinding, BindingProblem, BindingTable, SignalKey, Target, TargetOwnership,
};
pub use engine::{Applied, Block, ControlError, Controller, SignalChange, SignalState, State};
pub use profile::{
    Combine, Condition, ControlDef, ControlKind, ControllerKind, ControllerMeta, Effects,
    InterlockDef, InterlockMode, Profile, ProfileError, SignalDef, SignalKind, Supply, SupplyKind,
};
pub use sequence::{Action, Player, Recorder, Sequence, TimedAction, Trace, TraceSample};
pub use set::{ControllerSet, InstanceRecord, ProfileLibrary, ProjectControllers, SetError};
pub use validate::{Issue, Severity, validate};

/// The JSON schema of the profile format (kept in step with the types by a test).
pub const PROFILE_SCHEMA: &str = include_str!("../schema/controller-profile.schema.json");

/// Positions of a spring-return rocker in one direction.
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

/// Whether contact number `contact` (1 = slowest) is closed when the rocker sits at step
/// `at` (0 = off). `includes` says that a higher step keeps the lower contacts closed.
pub fn step_contact(at: u8, contact: u8, includes: bool) -> bool {
    at == contact || (includes && at > contact)
}

#[cfg(test)]
mod tests {
    use super::{Step, step_contact, two_step_contacts};

    #[test]
    fn two_step_truth_table() {
        assert_eq!(two_step_contacts(Step::Off, true), (false, false));
        assert_eq!(two_step_contacts(Step::One, true), (true, false));
        assert_eq!(two_step_contacts(Step::Two, true), (true, true));
        assert_eq!(two_step_contacts(Step::Two, false), (false, true));
    }

    #[test]
    fn step_contact_agrees_with_two_step_contacts() {
        for (step, at) in [(Step::Off, 0), (Step::One, 1), (Step::Two, 2)] {
            for includes in [true, false] {
                assert_eq!(
                    two_step_contacts(step, includes),
                    (step_contact(at, 1, includes), step_contact(at, 2, includes))
                );
            }
        }
    }

    #[test]
    fn three_step_includes_lower_contacts() {
        let closed = |at| [1, 2, 3].map(|c| step_contact(at, c, true));
        assert_eq!(closed(0), [false, false, false]);
        assert_eq!(closed(2), [true, true, false]);
        assert_eq!(closed(3), [true, true, true]);
        let only = |at| [1, 2, 3].map(|c| step_contact(at, c, false));
        assert_eq!(only(3), [false, false, true]);
    }
}
