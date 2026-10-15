//! Short-circuit currents along a feeder and what they ask of the
//! protective device and the cable (IEC 60909-0 in its low-voltage
//! simplification, IEC 60364-4-43 §434 and 5-54).
//!
//! The model is a radial feeder: a source (transformer or a stated
//! short-circuit current at the incoming terminals) and cable segments in
//! series. Impedances are positive sequence per phase; the single-phase
//! (earth-fault) loop doubles the conductor part and adds the protective
//! conductor, which is what the minimum current for disconnection needs.

use super::{Finding, Insulation, Rule, copper_resistivity, num, within};
use crate::calc::drop::REACTANCE_OHM_PER_M;

/// Voltage factor `c` for the maximum short-circuit current on low voltage
/// (IEC 60909-0 Table 1, tolerance +6 %).
pub const C_MAX: f64 = 1.05;
/// Voltage factor `c` for the minimum short-circuit current.
pub const C_MIN: f64 = 0.95;

/// A resistance and reactance, in ohm.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Impedance {
    pub r: f64,
    pub x: f64,
}

impl Impedance {
    pub fn new(r: f64, x: f64) -> Self {
        Impedance { r, x }
    }

    pub fn magnitude(self) -> f64 {
        self.r.hypot(self.x)
    }

    pub fn scaled(self, factor: f64) -> Impedance {
        Impedance {
            r: self.r * factor,
            x: self.x * factor,
        }
    }
}

impl std::ops::Add for Impedance {
    type Output = Impedance;
    fn add(self, other: Impedance) -> Impedance {
        Impedance {
            r: self.r + other.r,
            x: self.x + other.x,
        }
    }
}

/// Splits an impedance magnitude into resistance and reactance with
/// `R = 0.1·X` (IEC 60909-0 §3.3.1 for a network feeder).
fn from_magnitude(z: f64, r_over_x: f64) -> Impedance {
    let x = z / (1.0 + r_over_x * r_over_x).sqrt();
    Impedance { r: x * r_over_x, x }
}

/// Impedance of the supply seen from the point where the feeder starts,
/// per phase, referred to the low-voltage side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    pub impedance: Impedance,
    /// Nominal line-to-line voltage, V.
    pub voltage: f64,
}

impl Source {
    /// From a stated three-phase short-circuit current `ik_ka` (kA) at the
    /// incoming point, as a distribution board's label or the supplier
    /// gives it: `Z = c·Un / (√3·Ik)`, `R = 0.1·X`.
    pub fn from_ik(ik_ka: f64, voltage: f64) -> Option<Source> {
        if !(ik_ka.is_finite() && ik_ka > 0.0 && voltage.is_finite() && voltage > 0.0) {
            return None;
        }
        let z = C_MAX * voltage / (3.0_f64.sqrt() * ik_ka * 1000.0);
        Some(Source {
            impedance: from_magnitude(z, 0.1),
            voltage,
        })
    }

    /// From a transformer of `sn_kva` and short-circuit voltage `uk_percent`
    /// on a stiff network: `Z = uk/100 · Un²/Sn`, `R = 0.2·X` (typical
    /// distribution transformer; the correction factor `KT` of IEC 60909 is
    /// left out, which errs on the side of the larger current).
    pub fn from_transformer(sn_kva: f64, uk_percent: f64, voltage: f64) -> Option<Source> {
        if !(sn_kva.is_finite()
            && sn_kva > 0.0
            && uk_percent.is_finite()
            && uk_percent > 0.0
            && voltage.is_finite()
            && voltage > 0.0)
        {
            return None;
        }
        let z = uk_percent / 100.0 * voltage * voltage / (sn_kva * 1000.0);
        Some(Source {
            impedance: from_magnitude(z, 0.2),
            voltage,
        })
    }
}

/// Impedance of a copper cable of `length_m` and `section_mm2`, one
/// conductor, with the resistance taken at `celsius`.
pub fn cable_impedance(section_mm2: f64, length_m: f64, celsius: f64) -> Option<Impedance> {
    if !(section_mm2.is_finite() && section_mm2 > 0.0 && length_m.is_finite() && length_m >= 0.0) {
        return None;
    }
    Some(Impedance {
        r: copper_resistivity(celsius) * length_m / section_mm2,
        x: REACTANCE_OHM_PER_M * length_m,
    })
}

/// Maximum three-phase short-circuit current in amperes at the end of the
/// segments in `phase`, which are the phase conductors of the cables from
/// the source onwards. Resistances are taken at 20 °C by the caller (cold
/// cables carry the most current).
pub fn ik3_max(source: &Source, phase: &[Impedance]) -> f64 {
    let z = phase.iter().fold(source.impedance, |acc, z| acc + *z);
    C_MAX * source.voltage / (3.0_f64.sqrt() * z.magnitude())
}

/// Minimum single-phase short-circuit (earth-fault) current in amperes at
/// the end of the run: the loop goes out on the phase conductor and back on
/// the protective conductor. `phase` and `protective` are the cable
/// impedances with resistance at the hot conductor temperature; the source
/// counts twice (a transformer's zero-sequence impedance is about its
/// positive-sequence one). `U0 = Un/√3`.
pub fn ik1_min(source: &Source, phase: &[Impedance], protective: &[Impedance]) -> f64 {
    let loop_z = phase
        .iter()
        .chain(protective)
        .fold(source.impedance.scaled(2.0), |acc, z| acc + *z);
    C_MIN * source.voltage / 3.0_f64.sqrt() / loop_z.magnitude()
}

/// Breaking capacity check: a device must break the current at the point
/// where it sits. Equal passes.
pub fn check_breaking_capacity(device_icu_ka: f64, ik_a: f64) -> Option<Finding> {
    if !(device_icu_ka.is_finite() && device_icu_ka > 0.0 && ik_a.is_finite()) {
        return None;
    }
    if within(ik_a, device_icu_ka * 1000.0) {
        return None;
    }
    Some(Finding::error(
        Rule::BreakingCapacity,
        format!(
            "the prospective short-circuit current here is {} kA but the device only breaks {} kA",
            num(ik_a / 1000.0),
            num(device_icu_ka)
        ),
    ))
}

/// How a device trips on a short circuit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Trip {
    /// Miniature breaker curve B: certain to trip at 5 × `In`.
    B,
    /// Curve C: 10 × `In`.
    C,
    /// Curve D: 20 × `In`.
    D,
    /// Motor protection breaker or any device with a stated factor times
    /// its rating (its magnetic trip at the top of the tolerance band).
    Factor(f64),
}

impl Trip {
    /// The multiple of the rated current at which the device is certain to
    /// trip instantly.
    pub fn factor(self) -> f64 {
        match self {
            Trip::B => 5.0,
            Trip::C => 10.0,
            Trip::D => 20.0,
            Trip::Factor(f) => f,
        }
    }

    /// Reads `B`, `C` or `D` as printed on a breaker, any case.
    pub fn parse(text: &str) -> Option<Trip> {
        match text.trim().to_ascii_uppercase().as_str() {
            "B" => Some(Trip::B),
            "C" => Some(Trip::C),
            "D" => Some(Trip::D),
            _ => None,
        }
    }
}

/// Checks that the smallest fault current at the end of a cable still
/// trips the device instantly (the usual way to meet the disconnection
/// time of IEC 60364-4-41 on a long cable). Error when it does not.
pub fn check_disconnection(ik_min_a: f64, rated_a: f64, trip: Trip) -> Option<Finding> {
    if !(rated_a.is_finite() && rated_a > 0.0 && ik_min_a.is_finite()) {
        return None;
    }
    let needed = trip.factor() * rated_a;
    if within(needed, ik_min_a) {
        return None;
    }
    Some(Finding::error(
        Rule::DisconnectionTime,
        format!(
            "the smallest fault current at the end of the cable is {} A but the {} A device needs {} A to trip at once; shorten the cable, use a thicker one or a lower curve",
            num(ik_min_a),
            num(rated_a),
            num(needed)
        ),
    ))
}

/// The longest time in seconds a short circuit of `ik_a` may last before a
/// copper conductor of `section_mm2` is damaged: `t = k²S² / I²`
/// (adiabatic, valid up to about 5 s).
pub fn max_clearing_time(section_mm2: f64, insulation: Insulation, ik_a: f64) -> Option<f64> {
    if !(section_mm2.is_finite() && section_mm2 > 0.0 && ik_a.is_finite() && ik_a > 0.0) {
        return None;
    }
    let k = insulation.k_copper();
    Some(k * k * section_mm2 * section_mm2 / (ik_a * ik_a))
}

/// Checks the let-through energy `i2t` (A²s) of the protective device,
/// taken at the short-circuit current of the cable's start, against what
/// the conductor withstands, `k²S²`.
pub fn check_withstand(section_mm2: f64, insulation: Insulation, i2t: f64) -> Option<Finding> {
    if !(section_mm2.is_finite() && section_mm2 > 0.0 && i2t.is_finite() && i2t >= 0.0) {
        return None;
    }
    let k = insulation.k_copper();
    let allowed = k * k * section_mm2 * section_mm2;
    if within(i2t, allowed) {
        return None;
    }
    Some(Finding::error(
        Rule::ThermalWithstand,
        format!(
            "the device lets through {} A²s on a short circuit but a {} mm² cable withstands {} A²s",
            num(i2t),
            num(section_mm2),
            num(allowed)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    fn near(a: f64, b: f64, rel: f64) {
        assert!((a - b).abs() <= rel * b.abs().max(1e-12), "{a} vs {b}");
    }

    #[test]
    fn transformer_secondary_current() {
        // 630 kVA, uk 4 %, 400 V: In = 909 A, Ik ≈ 22.7 kA (without c);
        // with c 1.05 about 23.8 kA.
        let source = Source::from_transformer(630.0, 4.0, 400.0).unwrap();
        near(
            source.impedance.magnitude(),
            0.04 * 400.0 * 400.0 / 630_000.0,
            1e-9,
        );
        let ik = ik3_max(&source, &[]);
        near(ik, 1.05 * 22_746.0, 0.01);
    }

    #[test]
    fn stated_ik_is_reproduced_at_the_incoming_point() {
        let source = Source::from_ik(10.0, 400.0).unwrap();
        near(ik3_max(&source, &[]), 10_000.0, 1e-9);
    }

    #[test]
    fn cable_lowers_the_current() {
        let source = Source::from_ik(10.0, 400.0).unwrap();
        let cable = cable_impedance(2.5, 30.0, 20.0).unwrap();
        let ik = ik3_max(&source, &[cable]);
        assert!(ik < 10_000.0 && ik > 1_000.0, "{ik}");
        // Twice the length, lower again.
        let longer = cable_impedance(2.5, 60.0, 20.0).unwrap();
        assert!(ik3_max(&source, &[longer]) < ik);
        // Segments add in series: two halves equal one whole.
        let half = cable_impedance(2.5, 15.0, 20.0).unwrap();
        near(ik3_max(&source, &[half, half]), ik, 1e-9);
    }

    #[test]
    fn earth_fault_loop_is_smaller_than_three_phase() {
        let source = Source::from_ik(10.0, 400.0).unwrap();
        let phase = [cable_impedance(2.5, 40.0, 80.0).unwrap()];
        let pe = [cable_impedance(2.5, 40.0, 80.0).unwrap()];
        let ik3 = ik3_max(&source, &[cable_impedance(2.5, 40.0, 20.0).unwrap()]);
        let ik1 = ik1_min(&source, &phase, &pe);
        assert!(ik1 < ik3);
        // Loop of 2 · 40 m of 2.5 mm² hot ≈ 2·0.02063·... about 0.8 Ω → ~ 270 A.
        assert!(ik1 > 200.0 && ik1 < 350.0, "{ik1}");
    }

    #[test]
    fn breaking_capacity() {
        assert!(check_breaking_capacity(6.0, 5_999.0).is_none());
        assert!(check_breaking_capacity(6.0, 6_000.0).is_none());
        let f = check_breaking_capacity(6.0, 8_500.0).unwrap();
        assert_eq!(f.rule, Rule::BreakingCapacity);
        assert_eq!(f.severity, Severity::Error);
        assert!(f.message.contains("8.5 kA") && f.message.contains("6 kA"));
        assert!(check_breaking_capacity(0.0, 8_500.0).is_none());
    }

    #[test]
    fn disconnection_by_curve() {
        // 16 A curve C trips at 160 A.
        assert!(check_disconnection(160.0, 16.0, Trip::C).is_none());
        let f = check_disconnection(159.0, 16.0, Trip::C).unwrap();
        assert_eq!(f.rule, Rule::DisconnectionTime);
        assert!(f.message.contains("160 A"));
        // The same fault current is enough with curve B (80 A).
        assert!(check_disconnection(159.0, 16.0, Trip::B).is_none());
        assert!(check_disconnection(300.0, 16.0, Trip::D).is_some());
        assert!(check_disconnection(300.0, 16.0, Trip::Factor(13.0)).is_none());
    }

    #[test]
    fn withstand() {
        // 2.5 mm² PVC: 115²·2.5² = 82 656 A²s.
        assert_eq!(
            max_clearing_time(2.5, Insulation::Pvc, 1000.0).map(|t| (t * 1e3).round()),
            Some(83.0)
        );
        assert!(check_withstand(2.5, Insulation::Pvc, 80_000.0).is_none());
        let f = check_withstand(2.5, Insulation::Pvc, 100_000.0).unwrap();
        assert_eq!(f.rule, Rule::ThermalWithstand);
        assert!(check_withstand(2.5, Insulation::Xlpe, 100_000.0).is_none());
    }

    #[test]
    fn invalid_inputs() {
        assert!(Source::from_ik(0.0, 400.0).is_none());
        assert!(Source::from_transformer(630.0, f64::NAN, 400.0).is_none());
        assert!(cable_impedance(0.0, 10.0, 20.0).is_none());
        assert!(cable_impedance(1.5, -1.0, 20.0).is_none());
        assert!(max_clearing_time(2.5, Insulation::Pvc, 0.0).is_none());
    }

    #[test]
    fn trip_parses() {
        assert_eq!(Trip::parse(" c "), Some(Trip::C));
        assert_eq!(Trip::parse("K"), None);
    }
}
