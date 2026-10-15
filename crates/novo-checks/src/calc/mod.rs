//! Electrical calculations behind the design checks (WS-14, engine side).
//!
//! Pure functions on numbers: no [`Project`](novo_core::Project), no
//! netlist, no I/O. Each module takes plain inputs (amperes, mm², metres,
//! volts) and returns either a number or a [`Finding`], so every formula can
//! be tested alone and the same code serves the checks, the property sheet
//! and the Python API. Reading the model into these inputs is a separate
//! step (the adapter in `checks.rs`), kept thin on purpose.
//!
//! | Module | Answers |
//! | --- | --- |
//! | [`cable`] | What current a cable carries for its installation, and whether a protective device protects it against overload |
//! | [`drop`] | Voltage drop over length and current, as volts and percent of the supply |
//! | [`short_circuit`] | Short-circuit current along a feeder: breaking capacity, minimum current for disconnection, thermal withstand of the conductor |
//! | [`motor`] | Basic motor starter coordination: overload relay setting, contactor rating, short-circuit protection |
//! | [`pe`] | Protective conductor size, and loop impedance for automatic disconnection |
//!
//! # Sources and limits
//!
//! Formulas follow IEC 60364-4-43 / 5-52 / 5-54 (cable rating, protection,
//! protective conductors), IEC 60909-0 (short-circuit currents) and IEC
//! 60947-4-1 (starter coordination). Tables are stored as numbers in the
//! module that uses them, each with the table it comes from. They cover the
//! common case (copper, PVC and XLPE, 1.5 to 120 mm², 50 Hz, TN systems) and
//! say so by returning `None` outside it. A result is guidance for the
//! designer, never a certificate: tables must be checked against the
//! edition of the standard in force before numbers are quoted to a customer
//! (open item in `handoff/ws-calc/HANDOFF.md`).
//!
//! Units: amperes, volts, ohms, millimetres squared for sections, metres
//! for lengths, kilovoltamperes for transformers, degrees Celsius.

pub mod cable;
pub mod circuit;
pub mod curves;
pub mod drop;
pub mod motor;
pub mod network;
pub mod pe;
pub mod sccr;
pub mod selectivity;
pub mod short_circuit;
pub mod spec;
pub mod starter;
pub mod tables;

use crate::Severity;

/// Which calculation a [`Finding`] came from. Stable: used as the key for
/// waivers and in saved reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Rule {
    /// Cable ampacity against the protective device (`Ib ≤ In ≤ Iz`).
    CableRating,
    /// Voltage drop above the allowed limit.
    VoltageDrop,
    /// Breaking capacity below the prospective short-circuit current.
    BreakingCapacity,
    /// Short-circuit current too low for the device to disconnect in time.
    DisconnectionTime,
    /// Conductor cannot take the let-through energy of the device.
    ThermalWithstand,
    /// Motor starter parts that do not fit each other.
    MotorStarter,
    /// Protective conductor too small or loop impedance too high.
    ProtectiveConductor,
}

impl Rule {
    /// Every rule, in the order of the table above.
    pub const ALL: [Rule; 7] = [
        Rule::CableRating,
        Rule::VoltageDrop,
        Rule::BreakingCapacity,
        Rule::DisconnectionTime,
        Rule::ThermalWithstand,
        Rule::MotorStarter,
        Rule::ProtectiveConductor,
    ];

    /// A stable key, `check.<snake_case>`, for waivers and saved reports.
    /// Never renamed once released: waivers are stored under it.
    pub fn key(self) -> &'static str {
        match self {
            Rule::CableRating => "check.cable_rating",
            Rule::VoltageDrop => "check.voltage_drop",
            Rule::BreakingCapacity => "check.breaking_capacity",
            Rule::DisconnectionTime => "check.disconnection_time",
            Rule::ThermalWithstand => "check.thermal_withstand",
            Rule::MotorStarter => "check.motor_starter",
            Rule::ProtectiveConductor => "check.protective_conductor",
        }
    }

    /// A short plain name for lists and filter chips.
    pub fn label(self) -> &'static str {
        match self {
            Rule::CableRating => "Cable rating",
            Rule::VoltageDrop => "Voltage drop",
            Rule::BreakingCapacity => "Breaking capacity",
            Rule::DisconnectionTime => "Disconnection time",
            Rule::ThermalWithstand => "Thermal withstand",
            Rule::MotorStarter => "Motor starter",
            Rule::ProtectiveConductor => "Protective conductor",
        }
    }
}

/// One result of a calculation that is worth showing: a rule that is
/// broken or close to broken, in one plain sentence with the numbers in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub rule: Rule,
    pub severity: Severity,
    pub message: String,
}

impl Finding {
    pub(crate) fn error(rule: Rule, message: String) -> Finding {
        Finding {
            rule,
            severity: Severity::Error,
            message,
        }
    }

    pub(crate) fn warning(rule: Rule, message: String) -> Finding {
        Finding {
            rule,
            severity: Severity::Warning,
            message,
        }
    }
}

/// Conductor insulation, which sets the operating temperature (PVC 70 °C,
/// XLPE and EPR 90 °C).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Insulation {
    Pvc,
    Xlpe,
}

impl Insulation {
    /// Maximum conductor temperature in normal operation, °C.
    pub fn max_temperature(self) -> f64 {
        match self {
            Insulation::Pvc => 70.0,
            Insulation::Xlpe => 90.0,
        }
    }

    /// Short-circuit temperature limit, °C (IEC 60364-4-43 Table 43.1).
    pub fn short_circuit_temperature(self) -> f64 {
        match self {
            Insulation::Pvc => 160.0,
            Insulation::Xlpe => 250.0,
        }
    }

    /// Factor `k` of the adiabatic equation `I²t ≤ k²S²` for copper
    /// (IEC 60364-4-43 Table 43.1).
    pub fn k_copper(self) -> f64 {
        match self {
            Insulation::Pvc => 115.0,
            Insulation::Xlpe => 143.0,
        }
    }
}

/// Resistivity of copper at `celsius`, in ohm·mm²/m: 0.017241 at 20 °C
/// (IEC 60228), rising 0.393 % per kelvin.
pub fn copper_resistivity(celsius: f64) -> f64 {
    0.017_241 * (1.0 + 0.003_93 * (celsius - 20.0))
}

/// Compares floats with a relative tolerance, so a value exactly on its
/// limit passes in spite of rounding.
pub(crate) fn within(value: f64, limit: f64) -> bool {
    value <= limit * (1.0 + 1e-9) + 1e-12
}

/// `16`, `0.75`, `12.2`: at most two decimals, no trailing zeros.
pub(crate) fn num(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resistivity_matches_the_standard_values() {
        assert!((copper_resistivity(20.0) - 0.017_241).abs() < 1e-9);
        // 70 °C: about 0.0206 (0.0225 is the rounded-up figure used in practice).
        assert!((copper_resistivity(70.0) - 0.0206).abs() < 0.0005);
    }

    #[test]
    fn keys_are_unique_and_stable() {
        let keys: std::collections::BTreeSet<_> = Rule::ALL.iter().map(|r| r.key()).collect();
        assert_eq!(keys.len(), Rule::ALL.len());
        assert!(keys.iter().all(|k| k.starts_with("check.")));
        assert_eq!(Rule::BreakingCapacity.key(), "check.breaking_capacity");
    }

    #[test]
    fn num_trims() {
        assert_eq!(num(16.0), "16");
        assert_eq!(num(0.75), "0.75");
        assert_eq!(num(12.2049), "12.2");
    }
}
