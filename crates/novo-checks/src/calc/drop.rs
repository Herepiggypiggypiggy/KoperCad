//! Voltage drop over a cable (IEC 60364-5-52 Annex G).

use super::{Finding, Rule, copper_resistivity, num, within};

/// Reactance of a cable in ohm per metre. 0.08 mΩ/m is the usual figure
/// for multicore cables up to 50 Hz; it only matters above about 50 mm².
pub const REACTANCE_OHM_PER_M: f64 = 0.000_08;

/// The supply the drop is counted against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Supply {
    /// Two conductors carry the current (single phase, DC): the drop is
    /// over the way out and back.
    TwoWire,
    /// Three-phase: the drop is between phases, `√3` times one conductor's.
    ThreePhase,
}

/// Inputs of one cable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    /// One-way length of the cable, m.
    pub length_m: f64,
    pub section_mm2: f64,
    /// Load current, A.
    pub current: f64,
    /// Power factor `cos φ` of the load; 1 for DC and heaters.
    pub power_factor: f64,
    pub supply: Supply,
    /// Nominal voltage between the conductors the drop is counted on
    /// (400 V three-phase, 230 V single phase, 24 V DC).
    pub voltage: f64,
    /// Conductor temperature the resistance is taken at, °C. Operating
    /// temperature of the insulation (70 for PVC, 90 for XLPE) is the
    /// safe choice; 20 gives the cold value.
    pub temperature_c: f64,
}

impl Run {
    /// A copper run at 70 °C (PVC).
    pub fn new(
        length_m: f64,
        section_mm2: f64,
        current: f64,
        supply: Supply,
        voltage: f64,
    ) -> Self {
        Run {
            length_m,
            section_mm2,
            current,
            power_factor: 1.0,
            supply,
            voltage,
            temperature_c: 70.0,
        }
    }

    /// Resistance of one conductor over the run, ohm.
    pub fn resistance(&self) -> f64 {
        copper_resistivity(self.temperature_c) * self.length_m / self.section_mm2
    }

    /// Reactance of one conductor over the run, ohm.
    pub fn reactance(&self) -> f64 {
        REACTANCE_OHM_PER_M * self.length_m
    }

    /// The voltage drop in volts: `k·I·(R·cos φ + X·sin φ)` with `k` 2 for
    /// two wires and √3 for three phases. `None` for a length, section,
    /// current or voltage that is not a positive number, or a power factor
    /// outside 0 to 1.
    pub fn drop_volts(&self) -> Option<f64> {
        let positive = |v: f64| v.is_finite() && v > 0.0;
        if !positive(self.section_mm2)
            || !positive(self.voltage)
            || !self.length_m.is_finite()
            || self.length_m < 0.0
            || !self.current.is_finite()
            || self.current < 0.0
            || !(0.0..=1.0).contains(&self.power_factor)
        {
            return None;
        }
        let sin = (1.0 - self.power_factor * self.power_factor).sqrt();
        let per_conductor =
            self.current * (self.resistance() * self.power_factor + self.reactance() * sin);
        Some(match self.supply {
            Supply::TwoWire => 2.0 * per_conductor,
            Supply::ThreePhase => 3.0_f64.sqrt() * per_conductor,
        })
    }

    /// The drop as a percentage of the nominal voltage.
    pub fn drop_percent(&self) -> Option<f64> {
        self.drop_volts().map(|v| 100.0 * v / self.voltage)
    }

    /// Longest one-way length in metres that keeps the drop within
    /// `limit_percent`. `None` when the inputs are not valid or the load
    /// draws no current.
    pub fn longest_length(&self, limit_percent: f64) -> Option<f64> {
        let per_metre = Run {
            length_m: 1.0,
            ..*self
        }
        .drop_percent()?;
        (per_metre > 0.0 && limit_percent > 0.0).then(|| limit_percent / per_metre)
    }
}

/// Default limit, percent of nominal voltage: IEC 60364-5-52 Annex G gives
/// 3 % for lighting and 5 % for other uses on a public supply (as recalled;
/// verify against the edition in force). A single cable gets the whole
/// limit, so a cable that fails alone is certain to be a problem.
pub const DEFAULT_LIMIT_PERCENT: f64 = 5.0;

/// Limit for 24 V DC control circuits, percent: a coil or sensor on 24 V has
/// little room (24 V -10 % is the usual lower bound, and the power supply
/// itself takes part of it), so a cable gets less than the default.
pub const DC_CONTROL_LIMIT_PERCENT: f64 = 3.0;

/// Checks a run against `limit_percent`. A drop above the limit is a
/// warning; above twice the limit an error (the load will not start).
/// Returns `None` when the run is fine or cannot be calculated.
pub fn check_drop(run: &Run, limit_percent: f64) -> Option<Finding> {
    let percent = run.drop_percent()?;
    if within(percent, limit_percent) {
        return None;
    }
    let longest = run
        .longest_length(limit_percent)
        .map(|m| format!("; this section reaches {} m", num(m.floor())))
        .unwrap_or_default();
    let message = format!(
        "voltage drop is {} % ({} V of {} V) over {} m of {} mm² at {} A, limit {} %{longest}",
        num(percent),
        num(run.drop_volts()?),
        num(run.voltage),
        num(run.length_m),
        num(run.section_mm2),
        num(run.current),
        num(limit_percent),
    );
    Some(if percent > 2.0 * limit_percent {
        Finding::error(Rule::VoltageDrop, message)
    } else {
        Finding::warning(Rule::VoltageDrop, message)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    fn near(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() <= tol, "{a} vs {b}");
    }

    #[test]
    fn single_phase_resistive() {
        // 50 m of 2.5 mm² at 16 A, 230 V, 70 °C:
        // R = 0.02063·50/2.5 = 0.4126 Ω per conductor; ΔU = 2·16·0.4126 plus X.
        let run = Run::new(50.0, 2.5, 16.0, Supply::TwoWire, 230.0);
        let v = run.drop_volts().unwrap();
        near(v, 2.0 * 16.0 * 0.4126, 0.02);
        near(run.drop_percent().unwrap(), 100.0 * v / 230.0, 1e-9);
    }

    #[test]
    fn three_phase_uses_root_three() {
        let one = Run::new(100.0, 10.0, 20.0, Supply::TwoWire, 400.0);
        let three = Run::new(100.0, 10.0, 20.0, Supply::ThreePhase, 400.0);
        near(
            three.drop_volts().unwrap() / one.drop_volts().unwrap(),
            3.0_f64.sqrt() / 2.0,
            1e-9,
        );
    }

    #[test]
    fn power_factor_moves_towards_reactance() {
        let mut run = Run::new(100.0, 95.0, 100.0, Supply::ThreePhase, 400.0);
        let resistive = run.drop_volts().unwrap();
        run.power_factor = 0.8;
        let inductive = run.drop_volts().unwrap();
        // R = 0.02063·100/95 Ω, X = 0.008 Ω: R·0.8 + X·0.6 against R.
        let r = copper_resistivity(70.0) * 100.0 / 95.0;
        near(inductive / resistive, (0.8 * r + 0.6 * 0.008) / r, 1e-9);
        assert!(inductive > resistive);
    }

    #[test]
    fn linear_in_length_and_current_inverse_in_section() {
        let base = Run::new(30.0, 4.0, 10.0, Supply::TwoWire, 24.0)
            .drop_volts()
            .unwrap();
        let twice_length = Run::new(60.0, 4.0, 10.0, Supply::TwoWire, 24.0)
            .drop_volts()
            .unwrap();
        near(twice_length, 2.0 * base, 1e-9);
        let twice_current = Run::new(30.0, 4.0, 20.0, Supply::TwoWire, 24.0)
            .drop_volts()
            .unwrap();
        near(twice_current, 2.0 * base, 1e-9);
    }

    #[test]
    fn invalid_inputs_give_nothing() {
        assert!(
            Run::new(10.0, 0.0, 1.0, Supply::TwoWire, 24.0)
                .drop_volts()
                .is_none()
        );
        assert!(
            Run::new(-1.0, 1.5, 1.0, Supply::TwoWire, 24.0)
                .drop_volts()
                .is_none()
        );
        assert!(
            Run::new(10.0, 1.5, f64::NAN, Supply::TwoWire, 24.0)
                .drop_volts()
                .is_none()
        );
        let mut run = Run::new(10.0, 1.5, 1.0, Supply::TwoWire, 24.0);
        run.power_factor = 1.2;
        assert!(run.drop_volts().is_none());
        assert!(check_drop(&run, 4.0).is_none());
    }

    #[test]
    fn zero_length_or_current_has_no_drop() {
        let run = Run::new(0.0, 2.5, 16.0, Supply::TwoWire, 230.0);
        assert_eq!(run.drop_volts(), Some(0.0));
        assert!(check_drop(&run, 4.0).is_none());
        let idle = Run::new(100.0, 2.5, 0.0, Supply::TwoWire, 230.0);
        assert_eq!(idle.drop_volts(), Some(0.0));
        assert!(idle.longest_length(4.0).is_none());
    }

    #[test]
    fn check_reports_warning_then_error() {
        // 24 V, 1.5 mm², 3 A, 40 m: 2·3·0.02063·40/1.5 ≈ 3.3 V = 13.7 %.
        let run = Run::new(40.0, 1.5, 3.0, Supply::TwoWire, 24.0);
        let f = check_drop(&run, 10.0).unwrap();
        assert_eq!(f.rule, Rule::VoltageDrop);
        assert_eq!(f.severity, Severity::Warning);
        assert!(f.message.contains("13.7"), "{}", f.message);

        let f = check_drop(&run, 5.0).unwrap();
        assert_eq!(f.severity, Severity::Error);
    }

    #[test]
    fn longest_length_agrees_with_the_check() {
        let run = Run::new(10.0, 2.5, 16.0, Supply::ThreePhase, 400.0);
        let longest = run.longest_length(3.0).unwrap();
        let at_limit = Run {
            length_m: longest,
            ..run
        };
        near(at_limit.drop_percent().unwrap(), 3.0, 1e-9);
        assert!(check_drop(&at_limit, 3.0).is_none());
        let beyond = Run {
            length_m: longest * 1.01,
            ..run
        };
        assert!(check_drop(&beyond, 3.0).is_some());
    }
}
