//! Induction motor at variable frequency (lane L2b): the per-phase equivalent circuit with
//! reactances scaled by frequency, the voltage from the drive's V/f law, the torque-speed curve
//! for any (frequency, voltage), breakdown torque in closed form, the current-limited torque,
//! the field-weakening region, losses and efficiency, shaft dynamics and the fit from a
//! nameplate.
//!
//! Exact (closed form of the stated circuit): synchronous speed, the Thevenin breakdown
//! torque and slip, the energy balance of the operating point (input power = copper + core +
//! air-gap power). Model: the circuit parameters fitted from a nameplate, linear iron, the
//! frequency law of the core loss, friction and windage, saturation (off unless entered),
//! cooling derating, the DC hold below 0.01 Hz and quasi-static dynamics (no flux or rotor
//! current transients). See [`LEAVES_OUT`].

mod capability;
mod circuit;
mod complex;
mod fit;
mod shaft;

pub use capability::{FrequencyRow, Region};
pub use circuit::{
    Breakdown, Circuit, InductionMotor, OperatingPoint, Saturation, SelfVentilation, Supply,
    TorqueLimit, TorqueLimitedBy,
};
pub use fit::{FitOptions, FitReport};
pub use shaft::ShaftStep;

use crate::Fidelity;

/// Fidelity of each quantity, shown next to numbers in the UI.
pub const FIDELITY: &[(&str, Fidelity)] = &[
    ("Synchronous speed", Fidelity::Exact),
    (
        "Breakdown torque and slip of the entered circuit (Thevenin closed form)",
        Fidelity::Exact,
    ),
    (
        "Energy balance of an operating point (input = losses + air gap)",
        Fidelity::Exact,
    ),
    (
        "Circuit parameters fitted from a nameplate (resistance ratio and leakage split are assumptions)",
        Fidelity::Model,
    ),
    (
        "Torque-speed curve away from the nameplate point",
        Fidelity::Model,
    ),
    ("Stator current and power factor", Fidelity::Model),
    (
        "Core loss (frequency law) and friction/windage",
        Fidelity::Model,
    ),
    ("Magnetic saturation (off unless entered)", Fidelity::Model),
    (
        "Cooling derating of a self-ventilated motor",
        Fidelity::Model,
    ),
    ("Behaviour near 0 Hz (DC hold)", Fidelity::Model),
    (
        "Shaft dynamics (quasi-static torque, no flux transients)",
        Fidelity::Model,
    ),
];

/// What this model leaves out, for the "What this model leaves out" panel.
pub const LEAVES_OUT: &[&str] = &[
    "Parameters are fitted from the nameplate under stated assumptions; a measured no-load and locked-rotor test gives better values.",
    "No flux or rotor-current transients: torque follows the steady-state curve instantly.",
    "No deep-bar or skin effect: the rotor resistance does not change with slip frequency.",
    "No saturation unless entered, no temperature effect on resistances.",
    "Supply is an averaged sine at the fundamental: no PWM ripple, no harmonic losses, no dead-time voltage error.",
    "Cooling derating is a simple loss law, not a thermal network.",
    "Not a safety or compliance statement.",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_are_not_empty() {
        assert!(!FIDELITY.is_empty());
        assert!(!LEAVES_OUT.is_empty());
    }
}
