//! Overload (I2t) models and the neutral fault list.

/// I2t overload accumulator.
///
/// The user enters one point of the trip curve: `trip_ratio` times the rated current trips
/// after `trip_time_s`. The model trips when the integral of `(I/In)^2 - 1` over time reaches
/// `K = (trip_ratio^2 - 1) * trip_time_s`. Only the entered point is exact; other times are
/// the model. Below rated current the accumulator cools with `cooling` times the same square
/// law (0 = no memory loss, 1 = symmetric).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct I2tOverload {
    pub rated_a: f64,
    pub trip_ratio: f64,
    pub trip_time_s: f64,
    pub cooling: f64,
    /// Accumulated `K` in (I/In)^2 s.
    pub accumulated: f64,
}

impl I2tOverload {
    pub fn new(rated_a: f64, trip_ratio: f64, trip_time_s: f64) -> Self {
        Self {
            rated_a,
            trip_ratio,
            trip_time_s,
            cooling: 1.0,
            accumulated: 0.0,
        }
    }

    /// Trip threshold `K`.
    pub fn threshold(&self) -> f64 {
        (self.trip_ratio * self.trip_ratio - 1.0) * self.trip_time_s
    }

    /// Time to trip at a constant ratio of rated current from a cold start (model);
    /// `None` at or below rated current.
    pub fn time_to_trip_s(&self, ratio: f64) -> Option<f64> {
        (ratio > 1.0).then(|| self.threshold() / (ratio * ratio - 1.0))
    }

    /// Fraction of the trip threshold used, 0..1+.
    pub fn used(&self) -> f64 {
        let k = self.threshold();
        if k > 0.0 { self.accumulated / k } else { 0.0 }
    }

    /// Advance with the present current; returns true once tripped.
    pub fn step(&mut self, current_a: f64, dt_s: f64) -> bool {
        if (self.rated_a.is_nan() || self.rated_a <= 0.0)
            || !current_a.is_finite()
            || (dt_s.is_nan() || dt_s <= 0.0)
        {
            return self.used() >= 1.0;
        }
        let r = current_a.abs() / self.rated_a;
        let d = (r * r - 1.0) * dt_s;
        self.accumulated = if d >= 0.0 {
            self.accumulated + d
        } else {
            (self.accumulated + d * self.cooling).max(0.0)
        };
        self.accumulated >= self.threshold() && self.threshold() > 0.0
    }

    pub fn reset(&mut self) {
        self.accumulated = 0.0;
    }
}

/// Faults of the drive (generic list; codes are neutral and user-aliased in a drive profile).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fault {
    /// Output current above the hardware limit.
    OverCurrent,
    /// Short circuit or earth fault on the output (input flag).
    OutputShort,
    /// Motor thermal model (I2t) tripped.
    MotorOverload,
    /// Drive thermal model (I2t) tripped.
    DriveOverload,
    /// DC bus above the trip level.
    BusOverVoltage,
    /// DC bus below the trip level while running.
    BusUnderVoltage,
    /// Mains phase loss.
    MainsPhaseLoss,
    /// Braking resistor thermal model tripped.
    ResistorOverload,
    /// External fault input active.
    External,
    /// 4-20 mA reference below the loss-of-signal level.
    ReferenceLost,
}

impl Fault {
    /// Neutral short code.
    pub fn code(self) -> &'static str {
        match self {
            Fault::OverCurrent => "OC",
            Fault::OutputShort => "SC",
            Fault::MotorOverload => "MOL",
            Fault::DriveOverload => "DOL",
            Fault::BusOverVoltage => "OBV",
            Fault::BusUnderVoltage => "UBV",
            Fault::MainsPhaseLoss => "MPL",
            Fault::ResistorOverload => "BRO",
            Fault::External => "EXT",
            Fault::ReferenceLost => "RLS",
        }
    }

    /// Plain-language description.
    pub fn description(self) -> &'static str {
        match self {
            Fault::OverCurrent => "Output current above the hardware limit",
            Fault::OutputShort => "Short circuit or earth fault on the output",
            Fault::MotorOverload => "Motor overload (thermal model)",
            Fault::DriveOverload => "Drive overload (thermal model)",
            Fault::BusOverVoltage => "DC bus overvoltage",
            Fault::BusUnderVoltage => "DC bus undervoltage",
            Fault::MainsPhaseLoss => "Mains phase loss",
            Fault::ResistorOverload => "Braking resistor overload (thermal model)",
            Fault::External => "External fault input",
            Fault::ReferenceLost => "Analog reference lost",
        }
    }
}
