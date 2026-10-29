//! Closed-loop (encoder) hoist drive and winch (lane L2c).
//!
//! A hoist drive with an encoder on the motor shaft controls torque even at zero speed. The
//! brake is a holding brake: the drive builds torque first, proves it, then releases the brake
//! through its relay; at stop it holds zero speed until the brake has taken the load. This
//! module is the averaged model of that sequence with the monitoring a hoist drive has: speed
//! deviation, rollback, encoder plausibility, brake feedback and safe torque off.
//!
//! Exact by definition: encoder counts per revolution (`4 ppr`), speed resolution of the
//! count window, ramp arithmetic, STO gating (one low channel removes torque at once), gravity
//! and reduction arithmetic. Model: the PI tuning response, the torque loop (first order),
//! current and torque limit, field weakening, losses and the DC bus, brake delays and torque
//! entered by the user, the encoder plausibility check, thermal state. Not modelled: see
//! [`LEAVES_OUT`].

pub mod controller;
pub mod encoder;
pub mod findings;
pub mod sto;
pub mod unit;

pub use controller::{
    ClFault, ClInputs, ClOutputs, ClState, ClosedLoopConfig, ClosedLoopDrive, PreTorque, RunMode,
    ShaftSense,
};
pub use encoder::{Encoder, EncoderHealth};
pub use findings::{Finding, Severity, check_winch};
pub use sto::{StoMonitor, StoStatus};
pub use unit::{ClosedLoopWinch, WinchSnapshot};

use crate::Fidelity;

/// Fidelity of each quantity, shown next to numbers in the UI.
pub const FIDELITY: &[(&str, Fidelity)] = &[
    (
        "Encoder counts per revolution and speed resolution",
        Fidelity::Exact,
    ),
    (
        "STO: one low channel removes torque at once",
        Fidelity::Exact,
    ),
    (
        "Gravity and reduction arithmetic of the hoist",
        Fidelity::Exact,
    ),
    (
        "Speed loop response (PI) and torque loop lag",
        Fidelity::Model,
    ),
    (
        "Torque limit from the current limit, constant power above rated speed (no breakdown torque bound)",
        Fidelity::Model,
    ),
    ("Motor current and losses, DC bus voltage", Fidelity::Model),
    (
        "Brake release and apply delays and torque (entered by the user)",
        Fidelity::Model,
    ),
    (
        "Encoder plausibility check against the motor model",
        Fidelity::Model,
    ),
    ("Motor thermal state (I2t)", Fidelity::Model),
    (
        "Rope stretch and load sharing of two winches",
        Fidelity::Model,
    ),
];

/// What this model leaves out, for the "What this model leaves out" panel.
pub const LEAVES_OUT: &[&str] = &[
    "Averaged drive: no PWM, no harmonics, no current loops. Torque follows its command through one first order lag.",
    "Field orientation is assumed perfect: a wrong or lost encoder angle is detected and tripped, its effect on the field angle is not simulated.",
    "The plausibility check uses the true shaft speed above a speed threshold; a real drive uses a sensorless estimate that is less exact.",
    "Generic parameters: no manufacturer firmware, no parameter codes, no exact replica of any drive.",
    "Brake release and apply are fixed delays; the coil current, air gap and wear are not modelled.",
    "Rope stretch is linear with damping; rope swing, drum grooving and rope slip are not modelled.",
    "Not a safety or compliance statement: STO and the brake sequence are behaviour, not a PL or SIL assessment.",
];
