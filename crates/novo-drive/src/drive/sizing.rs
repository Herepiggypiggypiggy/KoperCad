//! Sizing rules and braking resistor checks (pure arithmetic).

use std::f64::consts::PI;

/// Default factor of the sizing planning rule (user-entered; not a standard and not a
/// manufacturer's statement).
pub const DEFAULT_SIZING_FACTOR: f64 = 2.0;

/// Planning rule: drive output current at least `factor x` the sum of the motor nominal
/// currents (default [`DEFAULT_SIZING_FACTOR`]). Returns the required current in A.
pub fn required_drive_current_a(motor_currents_a: &[f64], factor: f64) -> f64 {
    factor
        * motor_currents_a
            .iter()
            .copied()
            .filter(|i| i.is_finite() && *i > 0.0)
            .sum::<f64>()
}

/// Which drive class a required current falls in, by an entered boundary current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveClass {
    Compact,
    Process,
}

/// Result of the sizing rule check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SizingCheck {
    pub required_a: f64,
    pub drive_a: f64,
    pub ok: bool,
    /// Class the required current points to (against the entered boundary).
    pub class: DriveClass,
}

/// Check a drive rating against the rule; `compact_limit_a` is the entered boundary between
/// the compact and the process class.
pub fn check_drive_sizing(
    drive_a: f64,
    motor_currents_a: &[f64],
    factor: f64,
    compact_limit_a: f64,
) -> SizingCheck {
    let required_a = required_drive_current_a(motor_currents_a, factor);
    SizingCheck {
        required_a,
        drive_a,
        ok: drive_a >= required_a,
        class: if required_a <= compact_limit_a {
            DriveClass::Compact
        } else {
            DriveClass::Process
        },
    }
}

/// One braking event: a rotating inertia stopped in a given time, repeated every cycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakingDuty {
    pub inertia_kg_m2: f64,
    pub speed_rpm: f64,
    pub stop_time_s: f64,
    pub cycle_time_s: f64,
}

/// Braking resistor data (entered by the user).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakingResistor {
    pub ohm: f64,
    pub continuous_w: f64,
    /// Thermal capacity for short overload in J (0 = none).
    pub thermal_capacity_j: f64,
}

/// What is wrong with a resistor choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResistorFlag {
    /// Peak braking power above `V_bus^2 / R`: the bus rises beyond the chopper threshold.
    PeakExceedsCapacity,
    /// Average power over the cycle above the continuous rating.
    DutyExceedsContinuous,
    /// Resistance below the drive's minimum value (chopper current too high).
    BelowMinimumOhm,
    /// An input number (inertia, speed, times, bus level) is not finite: nothing can be computed.
    InvalidInput,
    /// Resistance is zero, negative or not a number: nothing can be computed.
    InvalidResistance,
    /// Energy of one stop above the thermal capacity.
    EnergyExceedsThermalCapacity,
}

/// Result of [`check_braking_resistor`].
#[derive(Debug, Clone, PartialEq)]
pub struct ResistorCheck {
    pub energy_j: f64,
    /// Average power during the stop (`E / t_stop`).
    pub average_stop_w: f64,
    /// Peak at the start of a constant-torque stop (twice the average; upper bound, losses ignored).
    pub peak_w: f64,
    /// Average over the whole cycle.
    pub duty_average_w: f64,
    /// Highest power the resistor can take from the bus: `V^2 / R`.
    pub capacity_w: f64,
    pub flags: Vec<ResistorFlag>,
}

/// Kinetic energy `0.5 J w^2` of the duty in J.
pub fn kinetic_energy_j(inertia_kg_m2: f64, speed_rpm: f64) -> f64 {
    let w = 2.0 * PI * speed_rpm / 60.0;
    0.5 * inertia_kg_m2 * w * w
}

/// Check a braking resistor against a duty. Model: constant deceleration torque, so the power
/// falls linearly from the peak to zero; motor and drive losses are ignored (upper bound).
/// `bus_threshold_v` is the chopper level, `min_ohm` the drive's minimum resistance (0 = unchecked).
pub fn check_braking_resistor(
    duty: &BrakingDuty,
    resistor: &BrakingResistor,
    bus_threshold_v: f64,
    min_ohm: f64,
) -> ResistorCheck {
    let energy_j = kinetic_energy_j(duty.inertia_kg_m2, duty.speed_rpm);
    let average_stop_w = if duty.stop_time_s > 0.0 {
        energy_j / duty.stop_time_s
    } else {
        f64::INFINITY
    };
    let peak_w = 2.0 * average_stop_w;
    let duty_average_w = if duty.cycle_time_s > 0.0 {
        energy_j / duty.cycle_time_s
    } else {
        f64::INFINITY
    };
    let valid = resistor.ohm.is_finite() && resistor.ohm > 0.0;
    let capacity_w = if valid {
        bus_threshold_v * bus_threshold_v / resistor.ohm
    } else {
        0.0
    };
    let mut flags = Vec::new();
    let inputs = [
        duty.inertia_kg_m2,
        duty.speed_rpm,
        duty.stop_time_s,
        duty.cycle_time_s,
        bus_threshold_v,
    ];
    if inputs.iter().any(|x| !x.is_finite()) {
        flags.push(ResistorFlag::InvalidInput);
    }
    if !valid {
        flags.push(ResistorFlag::InvalidResistance);
    }
    if peak_w > capacity_w {
        flags.push(ResistorFlag::PeakExceedsCapacity);
    }
    if duty_average_w > resistor.continuous_w {
        flags.push(ResistorFlag::DutyExceedsContinuous);
    }
    if valid && resistor.ohm < min_ohm {
        flags.push(ResistorFlag::BelowMinimumOhm);
    }
    if resistor.thermal_capacity_j > 0.0 && energy_j > resistor.thermal_capacity_j {
        flags.push(ResistorFlag::EnergyExceedsThermalCapacity);
    }
    ResistorCheck {
        energy_j,
        average_stop_w,
        peak_w,
        duty_average_w,
        capacity_w,
        flags,
    }
}
