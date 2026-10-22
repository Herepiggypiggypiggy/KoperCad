//! Heat of the wires lying in cable ducts: I²R of each conductor over the
//! length it runs in a duct.
//!
//! Pure numbers, no project types. The caller (the app) finds which wires
//! lie in which duct, how long, and what current they carry; this module
//! turns that into watts per duct, which the bridge puts on the duct
//! obstacle ([`crate::Obstacle::loss_w`]).
//!
//! # What the number is
//!
//! `P = n · I² · ρ(T) · L / S` for `n` identical conductors of length `L`
//! and cross-section `S` carrying `I`. `ρ` is copper at the conductor
//! temperature `T` ([`copper_resistivity`], IEC 60228 at 20 °C with the usual
//! 0.393 %/K). Taking `T` as the insulation's maximum operating
//! temperature gives the upper end: a wire in a cool cabinet is cooler and
//! its resistance lower. The current is the load current of the run, with no
//! diversity unless the caller scales it. Wire heat grows with the square of
//! the current, so a run that is lightly loaded most of the time heats far
//! less than its rated current says.

use std::collections::BTreeMap;

/// Resistivity of copper at 20 °C, ohm·mm²/m (IEC 60228).
pub const COPPER_RHO_20C: f64 = 0.017_241;
/// Temperature coefficient of copper resistance, per kelvin.
pub const COPPER_ALPHA: f64 = 0.003_93;

/// Resistivity of copper at `celsius`, ohm·mm²/m. The same formula as
/// `novo_checks::calc::copper_resistivity`, kept here so this engine stays
/// free of the checks crate.
pub fn copper_resistivity(celsius: f64) -> f64 {
    COPPER_RHO_20C * (1.0 + COPPER_ALPHA * (celsius - 20.0))
}

/// One wire (or several identical ones) as far as it lies in one duct.
#[derive(Debug, Clone, PartialEq)]
pub struct DuctWire {
    /// The duct (rail) id.
    pub duct: u64,
    /// Length of the wire inside the duct, m.
    pub length_m: f64,
    pub section_mm2: f64,
    /// Current in each conductor, A.
    pub current_a: f64,
    /// Conductor temperature the resistance is taken at, degrees C.
    pub temperature_c: f64,
    /// Identical conductors this entry stands for (3 for the phases of a
    /// balanced three-phase run, 2 for out and back).
    pub conductors: u32,
}

impl DuctWire {
    /// Resistance of one conductor over its length in the duct, ohm.
    pub fn resistance_ohm(&self) -> f64 {
        copper_resistivity(self.temperature_c) * self.length_m / self.section_mm2
    }

    /// Heat of all its conductors, W. 0 for a length, section or current
    /// that is not a positive finite number.
    pub fn loss_w(&self) -> f64 {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !(ok(self.length_m) && ok(self.section_mm2) && ok(self.current_a))
            || !self.temperature_c.is_finite()
        {
            return 0.0;
        }
        f64::from(self.conductors)
            * self.current_a
            * self.current_a
            * self.resistance_ohm().max(0.0)
    }
}

/// Watts per duct, summed over the wires. Ducts without heat are absent.
pub fn loss_by_duct(wires: &[DuctWire]) -> BTreeMap<u64, f64> {
    let mut out: BTreeMap<u64, f64> = BTreeMap::new();
    for w in wires {
        let p = w.loss_w();
        if p > 0.0 {
            *out.entry(w.duct).or_default() += p;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(length_m: f64, section: f64, current: f64, conductors: u32) -> DuctWire {
        DuctWire {
            duct: 1,
            length_m,
            section_mm2: section,
            current_a: current,
            temperature_c: 20.0,
            conductors,
        }
    }

    #[test]
    fn ten_amps_in_a_metre_of_one_and_a_half_square_at_20_c() {
        // R = 0.017241 * 1 / 1.5 = 0.011494 ohm; P = 100 * R = 1.1494 W.
        let p = wire(1.0, 1.5, 10.0, 1).loss_w();
        assert!((p - 1.1494).abs() < 1e-3, "{p}");
    }

    #[test]
    fn hot_copper_has_more_resistance_than_cold() {
        // 70 C: 0.017241 * (1 + 0.00393 * 50) = 0.020629.
        assert!((copper_resistivity(70.0) - 0.020_629).abs() < 1e-5);
        let mut w = wire(1.0, 1.5, 10.0, 1);
        let cold = w.loss_w();
        w.temperature_c = 70.0;
        assert!((w.loss_w() / cold - 1.19650).abs() < 1e-3);
    }

    #[test]
    fn three_conductors_make_three_times_the_heat() {
        let one = wire(2.0, 2.5, 16.0, 1).loss_w();
        let three = wire(2.0, 2.5, 16.0, 3).loss_w();
        assert!((three - 3.0 * one).abs() < 1e-12);
    }

    #[test]
    fn heat_goes_with_the_square_of_the_current() {
        let a = wire(1.0, 4.0, 5.0, 1).loss_w();
        let b = wire(1.0, 4.0, 10.0, 1).loss_w();
        assert!((b / a - 4.0).abs() < 1e-12);
    }

    #[test]
    fn nonsense_inputs_make_no_heat() {
        assert_eq!(wire(0.0, 1.5, 10.0, 1).loss_w(), 0.0);
        assert_eq!(wire(1.0, 0.0, 10.0, 1).loss_w(), 0.0);
        assert_eq!(wire(1.0, 1.5, f64::NAN, 1).loss_w(), 0.0);
        assert_eq!(wire(1.0, 1.5, -3.0, 1).loss_w(), 0.0);
    }

    #[test]
    fn wires_add_up_per_duct() {
        let mut a = wire(1.0, 1.5, 10.0, 1);
        a.duct = 7;
        let mut b = wire(1.0, 1.5, 10.0, 1);
        b.duct = 7;
        let mut c = wire(1.0, 1.5, 10.0, 1);
        c.duct = 9;
        let by = loss_by_duct(&[a.clone(), b, c]);
        assert!((by[&7] - 2.0 * a.loss_w()).abs() < 1e-12);
        assert!((by[&9] - a.loss_w()).abs() < 1e-12);
        assert_eq!(by.len(), 2);
    }
}
