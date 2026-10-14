//! Cable types: how many cores, which colours or numbers, which
//! cross-section, shielded or not.
//!
//! A type is named the way cables are ordered: `4G1.5` is four cores of
//! 1.5 mm² of which one is green-yellow, `7x1` is seven cores of 1 mm²
//! without one. Colour-coded types follow HD 308 S2 (BN, BK, GY for the
//! lines, BU for neutral, GNYE for PE) in the core order of the starter
//! catalog; larger types are numbered (`1`, `2`, ... plus GNYE).

use novo_core::{Cable, CableCore, Project, Value};
use novo_parts::{Catalog, CatalogPart, keys};

/// Name of the green-yellow protective core.
pub const GNYE: &str = "GNYE";
/// Name of a shield core.
pub const SHIELD: &str = "SH";

/// Standard conductor cross-sections in mm², smallest first.
pub const CROSS_SECTIONS: [f64; 16] = [
    0.5, 0.75, 1.0, 1.5, 2.5, 4.0, 6.0, 10.0, 16.0, 25.0, 35.0, 50.0, 70.0, 95.0, 120.0, 150.0,
];

/// Core counts numbered cables come in.
pub const NUMBERED_SIZES: [usize; 9] = [3, 4, 5, 7, 12, 18, 25, 34, 41];

/// Whether a core name is the protective conductor (`GNYE`, `PE`).
pub fn is_protective(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "GNYE" | "YEGN" | "GN/YE" | "PE" | "PEN"
    )
}

/// The smallest standard cross-section of at least `mm2`. Larger values
/// than the table are kept as they are.
pub fn standard_cross_section(mm2: f64) -> f64 {
    CROSS_SECTIONS
        .iter()
        .copied()
        .find(|&s| s >= mm2 - 1e-9)
        .unwrap_or(mm2)
}

/// One cable type.
#[derive(Debug, Clone, PartialEq)]
pub struct CableType {
    /// Conductor names in core order, without the shield.
    pub cores: Vec<String>,
    /// Cross-section of each conductor in mm².
    pub cross_section_mm2: f64,
    /// An overall shield (a `SH` core marked as shield).
    pub shield: bool,
}

/// Colour-coded types: core names for a type with (`G`) and without (`x`)
/// a green-yellow core.
fn colour_cores(count: usize, pe: bool) -> Option<&'static [&'static str]> {
    Some(match (count, pe) {
        (3, true) => &["BN", "BU", GNYE],
        (4, true) => &["BN", "BK", "GY", GNYE],
        (5, true) => &["BU", "BN", "BK", "GY", GNYE],
        (2, false) => &["BN", "BU"],
        (3, false) => &["BN", "BK", "GY"],
        (4, false) => &["BU", "BN", "BK", "GY"],
        _ => return None,
    })
}

impl CableType {
    /// The usual type with `count` conductors: colour-coded up to five
    /// cores, numbered above (the last core green-yellow when `pe`).
    pub fn standard(count: usize, pe: bool, cross_section_mm2: f64) -> Self {
        match colour_cores(count, pe) {
            Some(names) => Self::from_names(names, cross_section_mm2),
            None => Self::numbered(count, pe, cross_section_mm2),
        }
    }

    /// A numbered type: cores `1`..`n`, or `1`..`n-1` plus GNYE when `pe`.
    pub fn numbered(count: usize, pe: bool, cross_section_mm2: f64) -> Self {
        let numbered = if pe { count.saturating_sub(1) } else { count };
        let mut cores: Vec<String> = (1..=numbered).map(|n| n.to_string()).collect();
        if pe {
            cores.push(GNYE.into());
        }
        CableType {
            cores,
            cross_section_mm2,
            shield: false,
        }
    }

    fn from_names(names: &[&str], cross_section_mm2: f64) -> Self {
        CableType {
            cores: names.iter().map(|n| (*n).to_owned()).collect(),
            cross_section_mm2,
            shield: false,
        }
    }

    /// The same type with an overall shield, builder style.
    pub fn shielded(mut self) -> Self {
        self.shield = true;
        self
    }

    /// Whether one core is green-yellow.
    pub fn has_pe(&self) -> bool {
        self.cores.iter().any(|c| is_protective(c))
    }

    /// Whether the cores are numbered rather than colour-coded.
    pub fn is_numbered(&self) -> bool {
        self.cores
            .iter()
            .any(|c| c.chars().all(|ch| ch.is_ascii_digit()))
    }

    /// The order name: `4G1.5`, `7x1`, with ` shielded` for a shield.
    pub fn name(&self) -> String {
        let letter = if self.has_pe() { 'G' } else { 'x' };
        let mut name = format!(
            "{}{letter}{}",
            self.cores.len(),
            format_mm2(self.cross_section_mm2)
        );
        if self.shield {
            name.push_str(crate::strings::SHIELDED_SUFFIX);
        }
        name
    }

    /// The cable's cores: the conductors, then the shield.
    pub fn build_cores(&self) -> Vec<CableCore> {
        let mut cores: Vec<CableCore> = self
            .cores
            .iter()
            .map(|name| {
                let mut core = CableCore::new(name.clone());
                core.cross_section_mm2 = Some(self.cross_section_mm2);
                core
            })
            .collect();
        if self.shield {
            let mut shield = CableCore::new(SHIELD);
            shield.shield = true;
            cores.push(shield);
        }
        cores
    }

    /// The type of an existing cable, read from its cores.
    pub fn of_cable(cable: &Cable) -> Self {
        let conductors: Vec<&CableCore> = cable.cores.iter().filter(|c| !c.shield).collect();
        CableType {
            cores: conductors.iter().map(|c| c.name.clone()).collect(),
            cross_section_mm2: conductors
                .iter()
                .filter_map(|c| c.cross_section_mm2)
                .fold(0.0, f64::max),
            shield: cable.cores.iter().any(|c| c.shield),
        }
    }

    /// A catalog cable part of this type: letter code `W`, the same core
    /// names in order and the same cross-section. Shielded types only match
    /// parts whose data says `shield = true`.
    pub fn find_part<'c>(&self, catalog: &'c Catalog) -> Option<&'c CatalogPart> {
        catalog.parts().iter().find(|part| {
            let cores = part.cores();
            let section = match part.data.get(keys::CORE_CROSS_SECTION_MM2) {
                Some(Value::Float(v)) => Some(*v),
                Some(Value::Int(v)) => Some(*v as f64),
                _ => None,
            };
            let shield = matches!(part.data.get(SHIELD_KEY), Some(Value::Bool(true)));
            part.fits("W")
                && !cores.is_empty()
                && cores.len() == self.cores.len()
                && cores
                    .iter()
                    .zip(&self.cores)
                    .all(|(a, b)| a.eq_ignore_ascii_case(b))
                && section.is_some_and(|s| (s - self.cross_section_mm2).abs() < 1e-9)
                && shield == self.shield
        })
    }
}

/// Catalog data key marking a shielded cable part.
pub const SHIELD_KEY: &str = "shield";

fn format_mm2(mm2: f64) -> String {
    let text = format!("{mm2:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The types offered when choosing a cable by hand: colour-coded power
/// cables, numbered control cables and shielded variants.
pub fn default_types() -> Vec<CableType> {
    let mut types = Vec::new();
    for section in [1.5, 2.5, 4.0] {
        for count in [3, 4, 5] {
            types.push(CableType::standard(count, true, section));
        }
    }
    for count in [7, 12, 18, 25] {
        types.push(CableType::numbered(count, true, 1.5));
    }
    for count in [2, 3, 4] {
        types.push(CableType::standard(count, false, 0.75));
    }
    for count in [7, 12, 18] {
        types.push(CableType::numbered(count, false, 1.0));
    }
    // Shielded: motor cables on drives and analogue signals.
    for count in [4, 5] {
        types.push(CableType::standard(count, true, 1.5).shielded());
        types.push(CableType::standard(count, true, 2.5).shielded());
    }
    for count in [2, 3, 4] {
        types.push(CableType::standard(count, false, 0.75).shielded());
    }
    types.push(CableType::numbered(7, false, 0.75).shielded());
    types
}

/// A project cable's type name: its part's type number when it has a part,
/// else the type read from its cores.
pub fn type_label(project: &Project, cable: &Cable) -> String {
    if let Some(part) = cable.part.and_then(|p| project.part(p))
        && !part.type_number.is_empty()
    {
        return part.type_number.clone();
    }
    CableType::of_cable(cable).name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(CableType::standard(4, true, 1.5).name(), "4G1.5");
        assert_eq!(CableType::standard(5, true, 2.5).name(), "5G2.5");
        assert_eq!(CableType::numbered(7, false, 1.0).name(), "7x1");
        assert_eq!(CableType::standard(3, false, 0.75).name(), "3x0.75");
        assert_eq!(
            CableType::standard(4, true, 1.5).shielded().name(),
            "4G1.5 shielded"
        );
    }

    #[test]
    fn numbered_above_five() {
        let t = CableType::standard(7, true, 1.5);
        assert_eq!(t.cores, ["1", "2", "3", "4", "5", "6", "GNYE"]);
        assert!(t.is_numbered());
        let shielded = t.shielded().build_cores();
        assert_eq!(shielded.len(), 8);
        assert!(shielded[7].shield);
    }

    #[test]
    fn starter_catalog_parts() {
        let catalog = Catalog::starter();
        let part = CableType::standard(4, true, 1.5).find_part(&catalog);
        assert_eq!(part.map(|p| p.order_number.as_str()), Some("1119304"));
        let part = CableType::standard(5, true, 1.5).find_part(&catalog);
        assert_eq!(part.map(|p| p.order_number.as_str()), Some("1119305"));
        assert!(
            CableType::standard(4, true, 1.5)
                .shielded()
                .find_part(&catalog)
                .is_none()
        );
    }

    #[test]
    fn sections_round_up() {
        assert_eq!(standard_cross_section(1.2), 1.5);
        assert_eq!(standard_cross_section(2.5), 2.5);
        assert_eq!(standard_cross_section(300.0), 300.0);
    }
}
