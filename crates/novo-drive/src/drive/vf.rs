//! Voltage/frequency law of the drive output.

/// Shape of the V/f law below the base frequency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VfShape {
    /// Constant volts per hertz (constant torque load).
    ConstantTorque,
    /// Volts follow the square of frequency (fan and pump loads).
    Quadratic,
}

/// V/f law: rises from 0 (plus boost) to `rated_voltage_v` at `base_hz`, constant above.
///
/// Exact by definition: the law itself. The boost is the entered voltage at 0 Hz that fades
/// linearly to nothing at `boost_end_hz` (its purpose is to cover the stator drop, which is
/// a model: see the motor lane).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VfCurve {
    pub rated_voltage_v: f64,
    pub base_hz: f64,
    pub shape: VfShape,
    /// Extra volts at 0 Hz.
    pub boost_v: f64,
    /// Frequency at which the boost has faded out.
    pub boost_end_hz: f64,
}

impl VfCurve {
    pub fn new(rated_voltage_v: f64, base_hz: f64) -> Self {
        Self {
            rated_voltage_v,
            base_hz,
            shape: VfShape::ConstantTorque,
            boost_v: 0.0,
            boost_end_hz: base_hz / 2.0,
        }
    }

    /// Output volts (line to line, rms) for an output frequency; sign of the frequency is ignored.
    pub fn volts_for_frequency(&self, frequency_hz: f64) -> f64 {
        let f = frequency_hz.abs();
        if !f.is_finite() || (self.base_hz.is_nan() || self.base_hz <= 0.0) {
            return 0.0;
        }
        let x = (f / self.base_hz).min(1.0);
        let shaped = match self.shape {
            VfShape::ConstantTorque => x,
            VfShape::Quadratic => x * x,
        };
        let mut v = self.rated_voltage_v * shaped;
        if self.boost_v > 0.0 && self.boost_end_hz > 0.0 && f < self.boost_end_hz {
            v += self.boost_v * (1.0 - f / self.boost_end_hz);
        }
        v.clamp(0.0, self.rated_voltage_v)
    }

    /// Air-gap flux relative to rated flux (`(V/V_rated) / (f/f_base)`); 1 up to the base
    /// frequency (without boost) and `f_base/f` above it.
    pub fn flux_ratio(&self, frequency_hz: f64) -> f64 {
        let f = frequency_hz.abs();
        if f <= 0.0 || (self.rated_voltage_v.is_nan() || self.rated_voltage_v <= 0.0) {
            return 1.0;
        }
        (self.volts_for_frequency(f) / self.rated_voltage_v) / (f / self.base_hz)
    }

    /// Breakdown torque capability relative to rated: `flux^2` (exact for the constant-flux law
    /// and the field-weakening region; the stator drop at low speed is a motor-model matter).
    pub fn torque_capability_ratio(&self, frequency_hz: f64) -> f64 {
        let r = self.flux_ratio(frequency_hz);
        r * r
    }
}
