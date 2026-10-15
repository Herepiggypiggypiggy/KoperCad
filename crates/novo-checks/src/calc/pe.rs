//! Protective conductors: the smallest section for a phase conductor and
//! the loop impedance that lets the device disconnect in time
//! (IEC 60364-5-54 §543, IEC 60364-4-41, TN system).

use super::{Finding, Insulation, Rule, num, within};

/// Smallest copper protective conductor in mm² for a phase conductor of
/// `phase_mm2` when both are the same metal and the PE is not part of the
/// same cable (IEC 60364-5-54 Table 54.2): the same section up to 16 mm²,
/// 16 mm² up to 35, half above. A PE inside the same multicore cable as the
/// phases follows the same rule for the usual cables; the adiabatic
/// calculation (`min_pe_adiabatic`) allows smaller.
pub fn min_pe_section(phase_mm2: f64) -> Option<f64> {
    if !(phase_mm2.is_finite() && phase_mm2 > 0.0) {
        return None;
    }
    Some(if phase_mm2 <= 16.0 {
        phase_mm2
    } else if phase_mm2 <= 35.0 {
        16.0
    } else {
        phase_mm2 / 2.0
    })
}

/// Smallest copper protective conductor by the adiabatic equation (IEC
/// 60364-5-54 §543.1.2): `S = √(I²t) / k`, for the fault current and the
/// time the device takes to clear it.
pub fn min_pe_adiabatic(fault_a: f64, clearing_s: f64, insulation: Insulation) -> Option<f64> {
    if !(fault_a.is_finite() && fault_a > 0.0 && clearing_s.is_finite() && clearing_s > 0.0) {
        return None;
    }
    Some(fault_a * clearing_s.sqrt() / insulation.k_copper())
}

/// Checks the protective conductor against its phase conductor: smaller
/// than [`min_pe_section`] is a warning (the adiabatic calculation may
/// still allow it, which only the fault current can say).
pub fn check_pe_section(phase_mm2: f64, pe_mm2: f64) -> Option<Finding> {
    let needed = min_pe_section(phase_mm2)?;
    if !(pe_mm2.is_finite() && pe_mm2 > 0.0) || within(needed, pe_mm2) {
        return None;
    }
    Some(Finding::warning(
        Rule::ProtectiveConductor,
        format!(
            "the {} mm² protective conductor is thinner than the {} mm² it needs next to a {} mm² phase conductor",
            num(pe_mm2),
            num(needed),
            num(phase_mm2)
        ),
    ))
}

/// Maximum disconnection time in seconds for final circuits up to 32 A in
/// a TN system (IEC 60364-4-41 Table 41.1) by line-to-earth voltage `u0`:
/// 0.8 s up to 120 V, 0.4 s up to 230 V, 0.2 s up to 400 V, 0.1 s above.
/// Distribution circuits may take up to 5 s.
pub fn max_disconnection_time(u0: f64) -> f64 {
    if u0 <= 120.0 {
        0.8
    } else if u0 <= 230.0 {
        0.4
    } else if u0 <= 400.0 {
        0.2
    } else {
        0.1
    }
}

/// Checks automatic disconnection in a TN system: `Zs · Ia ≤ U0`, where
/// `loop_ohm` is the impedance of the whole fault loop at its hot value and
/// `trip_a` the current that makes the device disconnect within the
/// required time. Error when the loop is too long for the device.
pub fn check_loop(loop_ohm: f64, trip_a: f64, u0: f64) -> Option<Finding> {
    if !(loop_ohm.is_finite() && loop_ohm >= 0.0 && trip_a.is_finite() && trip_a > 0.0 && u0 > 0.0)
    {
        return None;
    }
    if within(loop_ohm * trip_a, u0) {
        return None;
    }
    Some(Finding::error(
        Rule::ProtectiveConductor,
        format!(
            "the fault loop is {} Ω, so only {} A flows on an earth fault, but the device needs {} A to disconnect in time (largest loop {} Ω)",
            num(loop_ohm),
            num(u0 / loop_ohm),
            num(trip_a),
            num(u0 / trip_a)
        ),
    ))
}

/// Resistance in ohm of a protective path from the protective earth
/// terminal to a device (continuity test, IEC 61439-1 §10.5.2, 0.1 Ω is a
/// common limit for the enclosure): the sum of its segments.
pub fn path_resistance(segments: &[(f64, f64)], celsius: f64) -> Option<f64> {
    segments.iter().try_fold(0.0, |acc, &(section, length)| {
        super::short_circuit::cable_impedance(section, length, celsius).map(|z| acc + z.r)
    })
}

/// Checks that a PE path resistance stays within `limit_ohm`.
pub fn check_continuity(resistance_ohm: f64, limit_ohm: f64) -> Option<Finding> {
    if !(resistance_ohm.is_finite() && limit_ohm > 0.0) || within(resistance_ohm, limit_ohm) {
        return None;
    }
    Some(Finding::warning(
        Rule::ProtectiveConductor,
        format!(
            "the protective path is {} Ω, above the {} Ω limit",
            num(resistance_ohm),
            num(limit_ohm)
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Severity;

    #[test]
    fn section_rule() {
        assert_eq!(min_pe_section(2.5), Some(2.5));
        assert_eq!(min_pe_section(16.0), Some(16.0));
        assert_eq!(min_pe_section(25.0), Some(16.0));
        assert_eq!(min_pe_section(35.0), Some(16.0));
        assert_eq!(min_pe_section(50.0), Some(25.0));
        assert_eq!(min_pe_section(95.0), Some(47.5));
        assert_eq!(min_pe_section(0.0), None);
    }

    #[test]
    fn adiabatic() {
        // 1000 A for 0.1 s on PVC: 1000·√0.1/115 ≈ 2.75 mm².
        let s = min_pe_adiabatic(1000.0, 0.1, Insulation::Pvc).unwrap();
        assert!((s - 2.75).abs() < 0.01, "{s}");
        assert!(min_pe_adiabatic(0.0, 0.1, Insulation::Pvc).is_none());
    }

    #[test]
    fn thin_pe_warns() {
        assert!(check_pe_section(10.0, 10.0).is_none());
        let f = check_pe_section(10.0, 6.0).unwrap();
        assert_eq!(f.severity, Severity::Warning);
        assert!(f.message.contains("6 mm²") && f.message.contains("10 mm²"));
        assert!(check_pe_section(50.0, 25.0).is_none());
        assert!(check_pe_section(50.0, 16.0).is_some());
        // An unknown PE section is not judged.
        assert!(check_pe_section(10.0, 0.0).is_none());
    }

    #[test]
    fn disconnection_times() {
        assert_eq!(max_disconnection_time(230.0), 0.4);
        assert_eq!(max_disconnection_time(400.0), 0.2);
    }

    #[test]
    fn loop_impedance() {
        // 230 V, 16 A curve C trips at 160 A: loop may be up to 1.4375 Ω.
        assert!(check_loop(1.4, 160.0, 230.0).is_none());
        assert!(check_loop(1.4375, 160.0, 230.0).is_none());
        let f = check_loop(1.6, 160.0, 230.0).unwrap();
        assert_eq!(f.severity, Severity::Error);
        assert!(f.message.contains("1.44 Ω"), "{}", f.message);
        assert!(check_loop(1.0, 0.0, 230.0).is_none());
    }

    #[test]
    fn continuity() {
        // 10 m of 2.5 mm² at 20 °C ≈ 0.069 Ω.
        let r = path_resistance(&[(2.5, 10.0)], 20.0).unwrap();
        assert!((r - 0.069).abs() < 0.001, "{r}");
        assert!(check_continuity(r, 0.1).is_none());
        let r2 = path_resistance(&[(2.5, 10.0), (1.5, 10.0)], 20.0).unwrap();
        assert!(check_continuity(r2, 0.1).is_some());
        assert!(path_resistance(&[(0.0, 10.0)], 20.0).is_none());
        assert_eq!(path_resistance(&[], 20.0), Some(0.0));
    }
}
