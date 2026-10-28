//! Frequency drive (lane L1): ramps, V/f law, DC bus with braking chopper, overload models,
//! terminals and macros, faults, sizing rules and the drive state machine.
//!
//! Averaged model: no PWM and no harmonics, no manufacturer firmware. Exact by definition:
//! ramp arithmetic and the V/f law as configured. Model: current limit action, I2t curves
//! away from the entered point, DC bus and resistor heating, torque limit in vector mode,
//! mains current. See [`LEAVES_OUT`].

pub mod bus;
pub mod engine;
pub mod protection;
pub mod ramp;
pub mod sizing;
pub mod terminals;
pub mod vf;

pub use bus::{BusConfig, BusEnergy, BusStep, DcBus};
pub use engine::{
    ControlMode, Drive, DriveConfig, DriveInputs, DriveOutputs, DriveState, MotorNameplate,
};
pub use protection::{Fault, I2tOverload};
pub use ramp::{Ramp, SRamp};
pub use sizing::{
    BrakingDuty, BrakingResistor, DriveClass, ResistorCheck, ResistorFlag, SizingCheck,
    check_braking_resistor, check_drive_sizing, kinetic_energy_j, required_drive_current_a,
};
pub use terminals::{Commands, DigitalInput, ReferenceSource, TerminalFunction, TerminalMacro};
pub use vf::{VfCurve, VfShape};

use crate::Fidelity;

/// Fidelity of each quantity, shown next to numbers in the UI.
pub const FIDELITY: &[(&str, Fidelity)] = &[
    ("Ramp arithmetic", Fidelity::Exact),
    ("V/f law", Fidelity::Exact),
    ("Energy balance of the DC bus", Fidelity::Exact),
    ("Overload trip at the entered point", Fidelity::Exact),
    ("Overload trip time at other currents", Fidelity::Model),
    ("Current limit action", Fidelity::Model),
    ("Torque limit (vector)", Fidelity::Model),
    ("DC bus voltage, rectifier, chopper", Fidelity::Model),
    ("Braking resistor heating", Fidelity::Model),
    ("Mains current", Fidelity::Model),
];

/// What this model leaves out, for the "What this model leaves out" panel.
pub const LEAVES_OUT: &[&str] = &[
    "No PWM switching and no harmonics: the output is the averaged fundamental.",
    "No manufacturer firmware: parameters and behaviour are generic, not an exact replica of any drive.",
    "Sensorless vector is a steady-state torque limit, not the dynamic current loops.",
    "Mains harmonics, line reactors and mains impedance are not modelled; mains current assumes a stated power factor.",
    "Drive thermal derating by switching frequency and ambient is not modelled.",
    "Not a safety or compliance statement.",
];
