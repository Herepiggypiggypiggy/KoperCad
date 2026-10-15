//! Cable rating: what a copper cable carries for its installation, and
//! whether a protective device protects it against overload
//! (IEC 60364-4-43 §433, IEC 60364-5-52).

use super::{Finding, Insulation, Rule, num, within};

/// How the cable is installed (IEC 60364-5-52 Table B.52.1 reference
/// methods).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// B1: single-core cables in conduit on a wall (or in a trunking).
    B1,
    /// B2: multicore cable in conduit on a wall.
    B2,
    /// C: cable clipped direct to a wall or on an open tray (the usual
    /// method for control cabinets with open ducts).
    C,
}

impl Method {
    pub const ALL: [Method; 3] = [Method::B1, Method::B2, Method::C];

    pub fn label(self) -> &'static str {
        match self {
            Method::B1 => "B1 (single cores in conduit)",
            Method::B2 => "B2 (multicore in conduit)",
            Method::C => "C (clipped or on a tray)",
        }
    }

    /// Reads `B1`, `B2` or `C` (any case), as written in a property.
    pub fn parse(text: &str) -> Option<Method> {
        match text.trim().to_ascii_uppercase().as_str() {
            "B1" => Some(Method::B1),
            "B2" => Some(Method::B2),
            "C" => Some(Method::C),
            _ => None,
        }
    }
}

/// Loaded conductors: two for single-phase and DC, three for three-phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Loaded {
    Two,
    Three,
}

/// Sections the tables below list, mm².
const SECTIONS: [f64; 12] = [
    1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0,
];

/// Current-carrying capacity in amperes, copper, PVC insulation, 30 °C
/// ambient, one circuit: IEC 60364-5-52 Tables B.52.2 to B.52.5, columns
/// B1, B2, C, for two and three loaded conductors.
const PVC_TWO: [[f64; 12]; 3] = [
    [
        15.5, 21.0, 28.0, 36.0, 50.0, 68.0, 89.0, 110.0, 134.0, 171.0, 207.0, 239.0,
    ],
    [
        15.0, 20.0, 27.0, 34.0, 46.0, 62.0, 80.0, 99.0, 118.0, 149.0, 179.0, 206.0,
    ],
    [
        19.5, 27.0, 36.0, 46.0, 63.0, 85.0, 112.0, 138.0, 168.0, 213.0, 258.0, 299.0,
    ],
];
const PVC_THREE: [[f64; 12]; 3] = [
    [
        13.5, 18.0, 24.0, 31.0, 42.0, 56.0, 73.0, 89.0, 108.0, 136.0, 164.0, 188.0,
    ],
    [
        13.0, 17.5, 23.0, 29.0, 39.0, 52.0, 68.0, 83.0, 99.0, 125.0, 150.0, 172.0,
    ],
    [
        17.5, 24.0, 32.0, 41.0, 57.0, 76.0, 96.0, 119.0, 144.0, 184.0, 223.0, 259.0,
    ],
];
/// XLPE/EPR, rows B1 and C only (Tables B.52.2 to B.52.5, XLPE columns).
/// Method B2 is not listed for XLPE until its column has been checked
/// against the standard; [`ampacity`] returns `None` for it.
const XLPE_TWO: [[f64; 12]; 2] = [
    [
        19.0, 26.0, 35.0, 45.0, 61.0, 81.0, 106.0, 131.0, 158.0, 200.0, 241.0, 278.0,
    ],
    [
        24.0, 33.0, 45.0, 58.0, 80.0, 107.0, 138.0, 171.0, 209.0, 269.0, 328.0, 382.0,
    ],
];
const XLPE_THREE: [[f64; 12]; 2] = [
    [
        17.0, 23.0, 31.0, 40.0, 54.0, 73.0, 95.0, 117.0, 141.0, 179.0, 216.0, 249.0,
    ],
    [
        22.0, 30.0, 40.0, 52.0, 71.0, 96.0, 119.0, 147.0, 179.0, 229.0, 278.0, 322.0,
    ],
];

/// Whether `a` and `b` are the same standard section.
fn same_section(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Current-carrying capacity `Iz0` in amperes of a copper cable at 30 °C
/// ambient and no grouping. `None` for a section the tables do not list
/// (use a standard one: 1.5, 2.5, 4, 6, 10, 16, 25, 35, 50, 70, 95, 120).
pub fn ampacity(
    section_mm2: f64,
    insulation: Insulation,
    method: Method,
    loaded: Loaded,
) -> Option<f64> {
    let column = SECTIONS
        .iter()
        .position(|s| same_section(*s, section_mm2))?;
    let row = match (insulation, method) {
        (_, Method::B1) => 0,
        (Insulation::Pvc, Method::B2) => 1,
        (Insulation::Pvc, Method::C) => 2,
        (Insulation::Xlpe, Method::C) => 1,
        (Insulation::Xlpe, Method::B2) => return None,
    };
    let table: &[[f64; 12]] = match (insulation, loaded) {
        (Insulation::Pvc, Loaded::Two) => &PVC_TWO,
        (Insulation::Pvc, Loaded::Three) => &PVC_THREE,
        (Insulation::Xlpe, Loaded::Two) => &XLPE_TWO,
        (Insulation::Xlpe, Loaded::Three) => &XLPE_THREE,
    };
    Some(table[row][column])
}

/// The smallest standard section that carries `current` for the
/// conditions, or `None` above 120 mm².
pub fn smallest_section(
    current: f64,
    insulation: Insulation,
    method: Method,
    loaded: Loaded,
    derating: f64,
) -> Option<f64> {
    SECTIONS.iter().copied().find(|s| {
        ampacity(*s, insulation, method, loaded).is_some_and(|a| within(current, a * derating))
    })
}

/// Correction factor `k1` for an ambient air temperature other than 30 °C
/// (IEC 60364-5-52 Table B.52.14). Between listed temperatures the value is
/// interpolated. `None` outside 10 to 60 °C (PVC) or 10 to 80 °C (XLPE).
pub fn ambient_factor(celsius: f64, insulation: Insulation) -> Option<f64> {
    const PVC: [(f64, f64); 11] = [
        (10.0, 1.22),
        (15.0, 1.17),
        (20.0, 1.12),
        (25.0, 1.06),
        (30.0, 1.0),
        (35.0, 0.94),
        (40.0, 0.87),
        (45.0, 0.79),
        (50.0, 0.71),
        (55.0, 0.61),
        (60.0, 0.50),
    ];
    const XLPE: [(f64, f64); 15] = [
        (10.0, 1.15),
        (15.0, 1.12),
        (20.0, 1.08),
        (25.0, 1.04),
        (30.0, 1.0),
        (35.0, 0.96),
        (40.0, 0.91),
        (45.0, 0.87),
        (50.0, 0.82),
        (55.0, 0.76),
        (60.0, 0.71),
        (65.0, 0.65),
        (70.0, 0.58),
        (75.0, 0.50),
        (80.0, 0.41),
    ];
    let table: &[(f64, f64)] = match insulation {
        Insulation::Pvc => &PVC,
        Insulation::Xlpe => &XLPE,
    };
    let (first, last) = (table[0], table[table.len() - 1]);
    if celsius.is_nan() || celsius < first.0 || celsius > last.0 {
        return None;
    }
    let upper = table.iter().position(|(t, _)| *t >= celsius)?;
    if upper == 0 {
        return Some(first.1);
    }
    let ((t0, k0), (t1, k1)) = (table[upper - 1], table[upper]);
    Some(k0 + (k1 - k0) * (celsius - t0) / (t1 - t0))
}

/// Grouping factor `k2` for `circuits` loaded circuits bunched together in
/// method B1, B2 or C on a wall (IEC 60364-5-52 Table B.52.17, line 1).
/// Beyond 20 circuits the value for 20 applies. `circuits` below one count
/// as one.
pub fn grouping_factor(circuits: u32) -> f64 {
    const TABLE: [(u32, f64); 12] = [
        (1, 1.0),
        (2, 0.80),
        (3, 0.70),
        (4, 0.65),
        (5, 0.60),
        (6, 0.57),
        (7, 0.54),
        (8, 0.52),
        (9, 0.50),
        (12, 0.45),
        (16, 0.41),
        (20, 0.38),
    ];
    let n = circuits.max(1);
    TABLE
        .iter()
        .rev()
        .find(|(listed, _)| *listed <= n)
        .map_or(0.38, |(_, k)| *k)
}

/// Everything that sets the load and the rating of one circuit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circuit {
    /// Design current `Ib`: what the load draws, A.
    pub design_current: f64,
    /// Rated current `In` (or setting `Ir`) of the protective device, A.
    pub device_rating: f64,
    pub section_mm2: f64,
    pub insulation: Insulation,
    pub method: Method,
    pub loaded: Loaded,
    /// Ambient air temperature, °C. 30 is the reference.
    pub ambient_c: f64,
    /// Circuits bunched together, this one included.
    pub grouped_circuits: u32,
}

impl Circuit {
    /// A circuit in the reference conditions: 30 °C, alone, method C, PVC.
    pub fn new(design_current: f64, device_rating: f64, section_mm2: f64, loaded: Loaded) -> Self {
        Circuit {
            design_current,
            device_rating,
            section_mm2,
            insulation: Insulation::Pvc,
            method: Method::C,
            loaded,
            ambient_c: 30.0,
            grouped_circuits: 1,
        }
    }

    /// The cable's rating `Iz` in these conditions: table value times the
    /// ambient and grouping factors. `None` for a section or temperature
    /// outside the tables.
    pub fn rating(&self) -> Option<f64> {
        let base = ampacity(self.section_mm2, self.insulation, self.method, self.loaded)?;
        let k1 = ambient_factor(self.ambient_c, self.insulation)?;
        Some(base * k1 * grouping_factor(self.grouped_circuits))
    }
}

/// Checks overload protection, `Ib ≤ In ≤ Iz` (IEC 60364-4-43 §433.1):
///
/// - a load above the device rating trips it in normal use (error: the
///   device is too small);
/// - a device rating above the cable's rating leaves the cable unprotected
///   (error: the wire is too thin or the device too large).
///
/// Returns the findings in that order; empty when the circuit is fine or
/// the section is outside the tables (nothing can be said).
pub fn check_overload(circuit: &Circuit) -> Vec<Finding> {
    let mut out = Vec::new();
    let Some(iz) = circuit.rating() else {
        return out;
    };
    let (ib, inn) = (circuit.design_current, circuit.device_rating);
    if ib > 0.0 && inn > 0.0 && !within(ib, inn) {
        out.push(Finding::error(
            Rule::CableRating,
            format!(
                "the load draws {} A but its protective device is only rated {} A",
                num(ib),
                num(inn)
            ),
        ));
    }
    if inn > 0.0 && !within(inn, iz) {
        out.push(Finding::error(
            Rule::CableRating,
            format!(
                "the {} mm² cable carries {} A here but is protected by a {} A device",
                num(circuit.section_mm2),
                num(iz),
                num(inn)
            ),
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_values() {
        // 2.5 mm² PVC, method C, three loaded: 24 A; two loaded: 27 A.
        assert_eq!(
            ampacity(2.5, Insulation::Pvc, Method::C, Loaded::Three),
            Some(24.0)
        );
        assert_eq!(
            ampacity(2.5, Insulation::Pvc, Method::C, Loaded::Two),
            Some(27.0)
        );
        assert_eq!(
            ampacity(16.0, Insulation::Xlpe, Method::B2, Loaded::Three),
            None
        );
        assert_eq!(
            ampacity(16.0, Insulation::Xlpe, Method::C, Loaded::Three),
            Some(96.0)
        );
        assert_eq!(
            ampacity(3.0, Insulation::Pvc, Method::C, Loaded::Three),
            None
        );
    }

    #[test]
    fn tables_rise_with_section_and_fall_with_loaded_conductors() {
        for insulation in [Insulation::Pvc, Insulation::Xlpe] {
            for method in Method::ALL {
                if ampacity(1.5, insulation, method, Loaded::Three).is_none() {
                    continue;
                }
                let mut last_three = 0.0;
                for s in SECTIONS {
                    let three = ampacity(s, insulation, method, Loaded::Three).unwrap();
                    let two = ampacity(s, insulation, method, Loaded::Two).unwrap();
                    assert!(three > last_three, "{s} mm² {method:?}");
                    assert!(two > three, "{s} mm² {method:?}");
                    last_three = three;
                }
            }
        }
        // XLPE carries more than PVC in every cell.
        for s in SECTIONS {
            for m in [Method::B1, Method::C] {
                assert!(
                    ampacity(s, Insulation::Xlpe, m, Loaded::Three)
                        > ampacity(s, Insulation::Pvc, m, Loaded::Three)
                );
            }
        }
    }

    #[test]
    fn ambient_factor_interpolates_and_is_one_at_30() {
        assert_eq!(ambient_factor(30.0, Insulation::Pvc), Some(1.0));
        assert_eq!(ambient_factor(40.0, Insulation::Pvc), Some(0.87));
        let mid = ambient_factor(42.5, Insulation::Pvc).unwrap();
        assert!((mid - 0.83).abs() < 1e-9);
        assert_eq!(ambient_factor(5.0, Insulation::Pvc), None);
        assert_eq!(ambient_factor(65.0, Insulation::Pvc), None);
        assert_eq!(ambient_factor(65.0, Insulation::Xlpe), Some(0.65));
        assert_eq!(ambient_factor(f64::NAN, Insulation::Xlpe), None);
    }

    #[test]
    fn grouping_steps_down() {
        assert_eq!(grouping_factor(0), 1.0);
        assert_eq!(grouping_factor(1), 1.0);
        assert_eq!(grouping_factor(3), 0.70);
        // Between listed counts the lower count's factor is kept (safe side).
        assert_eq!(grouping_factor(10), 0.50);
        assert_eq!(grouping_factor(40), 0.38);
    }

    #[test]
    fn rating_applies_both_factors() {
        let mut c = Circuit::new(10.0, 16.0, 2.5, Loaded::Three);
        c.ambient_c = 40.0;
        c.grouped_circuits = 3;
        // 24 A · 0.87 · 0.70
        assert!((c.rating().unwrap() - 14.616).abs() < 1e-9);
    }

    #[test]
    fn overload_protection() {
        // 16 A breaker on 2.5 mm² three loaded: Iz 24 A, fine.
        let fine = Circuit::new(14.0, 16.0, 2.5, Loaded::Three);
        assert!(check_overload(&fine).is_empty());

        // 25 A breaker on 2.5 mm²: cable unprotected.
        let thin = Circuit::new(14.0, 25.0, 2.5, Loaded::Three);
        let found = check_overload(&thin);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].rule, Rule::CableRating);
        assert!(found[0].message.contains("2.5 mm²"));
        assert!(found[0].message.contains("25 A"));

        // Load above the device: nuisance trips.
        let hot = Circuit::new(18.0, 16.0, 2.5, Loaded::Three);
        assert_eq!(check_overload(&hot).len(), 1);

        // Derating makes the same circuit fail.
        let mut warm = Circuit::new(14.0, 16.0, 2.5, Loaded::Three);
        warm.ambient_c = 50.0;
        warm.grouped_circuits = 4;
        assert_eq!(check_overload(&warm).len(), 1);
    }

    #[test]
    fn device_exactly_at_the_rating_passes() {
        // 4 mm² PVC C three-loaded is 32 A: a 32 A device is allowed.
        let c = Circuit::new(30.0, 32.0, 4.0, Loaded::Three);
        assert!(check_overload(&c).is_empty());
    }

    #[test]
    fn unknown_section_says_nothing() {
        let c = Circuit::new(10.0, 16.0, 3.0, Loaded::Three);
        assert!(check_overload(&c).is_empty());
    }

    #[test]
    fn smallest_section_for_a_load() {
        let s = smallest_section(30.0, Insulation::Pvc, Method::C, Loaded::Three, 1.0);
        assert_eq!(s, Some(4.0));
        let s = smallest_section(30.0, Insulation::Pvc, Method::C, Loaded::Three, 0.7);
        assert_eq!(s, Some(10.0));
        assert_eq!(
            smallest_section(900.0, Insulation::Pvc, Method::C, Loaded::Three, 1.0),
            None
        );
    }

    #[test]
    fn method_parses() {
        assert_eq!(Method::parse(" b2 "), Some(Method::B2));
        assert_eq!(Method::parse("E"), None);
    }
}
