//! Conductor sizes and ampacities in AWG, for the NFPA 79 and UL 508A rules.
//!
//! The model stores cross-sections in mm². North American rules speak in AWG
//! and in ampacity tables, so a wire is rated as the largest AWG size whose
//! area it covers (a 2.5 mm² wire as 14 AWG, 4 mm² as 12 AWG): conservative,
//! and the message says so. Values are the usual copper conductor figures
//! (60 °C and 75 °C columns) and the usual equipment-grounding-conductor
//! steps, written here as plain numbers for checking. They are not the text
//! of any table, and the rule table marks them for verification in the
//! engineer's licensed copy.

/// One conductor size.
#[derive(Debug, PartialEq)]
pub struct Awg {
    pub label: &'static str,
    /// Conductor area in mm².
    pub mm2: f64,
    /// Ampacity at 60 °C terminations, A.
    pub a60: f64,
    /// Ampacity at 75 °C terminations, A.
    pub a75: f64,
}

const fn awg(label: &'static str, mm2: f64, a60: f64, a75: f64) -> Awg {
    Awg {
        label,
        mm2,
        a60,
        a75,
    }
}

/// Smallest to largest.
pub(crate) static SIZES: [Awg; 15] = [
    awg("18 AWG", 0.823, 7.0, 7.0),
    awg("16 AWG", 1.31, 10.0, 10.0),
    awg("14 AWG", 2.08, 15.0, 20.0),
    awg("12 AWG", 3.31, 20.0, 25.0),
    awg("10 AWG", 5.26, 30.0, 35.0),
    awg("8 AWG", 8.37, 40.0, 50.0),
    awg("6 AWG", 13.3, 55.0, 65.0),
    awg("4 AWG", 21.2, 70.0, 85.0),
    awg("3 AWG", 26.7, 85.0, 100.0),
    awg("2 AWG", 33.6, 95.0, 115.0),
    awg("1 AWG", 42.4, 110.0, 130.0),
    awg("1/0 AWG", 53.5, 125.0, 150.0),
    awg("2/0 AWG", 67.4, 145.0, 175.0),
    awg("3/0 AWG", 85.0, 165.0, 200.0),
    awg("4/0 AWG", 107.2, 195.0, 230.0),
];

fn by_label(label: &str) -> &'static Awg {
    SIZES
        .iter()
        .find(|s| s.label == label)
        .expect("size in table")
}

/// How far below a size a metric wire may be and still count as that size
/// (rounding of catalogue sections).
const AREA_TOLERANCE: f64 = 0.03;

/// The largest size whose area the wire covers; `None` for a wire thinner
/// than the smallest size.
pub fn awg_for_area(mm2: f64) -> Option<&'static Awg> {
    SIZES
        .iter()
        .rev()
        .find(|s| s.mm2 <= mm2 * (1.0 + AREA_TOLERANCE))
}

/// What a wire of `mm2` carries at the terminal temperature (60 or 75 °C).
pub(crate) fn ampacity(mm2: f64, terminal_c: f64) -> Option<f64> {
    let size = awg_for_area(mm2)?;
    Some(if terminal_c >= 75.0 {
        size.a75
    } else {
        size.a60
    })
}

/// Whether a wire of `wire_mm2` is at least as large as `size`, allowing for
/// catalogue rounding.
pub(crate) fn covers(wire_mm2: f64, size: &Awg) -> bool {
    wire_mm2 * (1.0 + AREA_TOLERANCE) >= size.mm2
}

/// The smallest equipment grounding conductor for an overcurrent device of
/// `rating_a`. `None` above the steps the check knows.
pub(crate) fn grounding_conductor(rating_a: f64) -> Option<&'static Awg> {
    let label = match rating_a {
        r if r <= 15.0 => "14 AWG",
        r if r <= 20.0 => "12 AWG",
        r if r <= 60.0 => "10 AWG",
        r if r <= 100.0 => "8 AWG",
        r if r <= 200.0 => "6 AWG",
        r if r <= 300.0 => "4 AWG",
        r if r <= 400.0 => "3 AWG",
        r if r <= 500.0 => "2 AWG",
        r if r <= 600.0 => "1 AWG",
        r if r <= 800.0 => "1/0 AWG",
        r if r <= 1000.0 => "2/0 AWG",
        r if r <= 1200.0 => "3/0 AWG",
        r if r <= 1600.0 => "4/0 AWG",
        _ => return None,
    };
    Some(by_label(label))
}

/// Standard ratings of fuses and breakers, A.
const STANDARD_RATINGS: [f64; 40] = [
    1.0, 3.0, 6.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0, 60.0, 70.0, 80.0, 90.0,
    100.0, 110.0, 125.0, 150.0, 175.0, 200.0, 225.0, 250.0, 300.0, 350.0, 400.0, 450.0, 500.0,
    600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0, 2500.0, 3000.0, 4000.0, 5000.0,
];

/// The next standard rating at or above `amps`: a motor branch device may be
/// taken up to it when the computed limit falls between two ratings.
pub(crate) fn next_standard_rating(amps: f64) -> f64 {
    STANDARD_RATINGS
        .iter()
        .copied()
        .find(|r| *r + 1e-9 >= amps)
        .unwrap_or(amps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_wires_rate_as_the_size_they_cover() {
        assert_eq!(awg_for_area(2.5).unwrap().label, "14 AWG");
        assert_eq!(awg_for_area(4.0).unwrap().label, "12 AWG");
        assert_eq!(awg_for_area(2.08).unwrap().label, "14 AWG");
        assert_eq!(awg_for_area(0.75).map(|s| s.label), None);
        assert_eq!(awg_for_area(1.5).unwrap().label, "16 AWG");
        assert_eq!(awg_for_area(200.0).unwrap().label, "4/0 AWG");
    }

    #[test]
    fn ampacity_follows_the_terminal_column() {
        assert_eq!(ampacity(2.5, 60.0), Some(15.0));
        assert_eq!(ampacity(2.5, 75.0), Some(20.0));
        assert_eq!(ampacity(0.5, 60.0), None);
    }

    #[test]
    fn grounding_steps() {
        assert_eq!(grounding_conductor(15.0).unwrap().label, "14 AWG");
        assert_eq!(grounding_conductor(16.0).unwrap().label, "12 AWG");
        assert_eq!(grounding_conductor(60.0).unwrap().label, "10 AWG");
        assert_eq!(grounding_conductor(61.0).unwrap().label, "8 AWG");
        assert_eq!(grounding_conductor(1600.0).unwrap().label, "4/0 AWG");
        assert!(grounding_conductor(2000.0).is_none());
        assert!(covers(2.5, grounding_conductor(15.0).unwrap()));
        assert!(!covers(1.5, grounding_conductor(15.0).unwrap()));
    }

    #[test]
    fn standard_ratings_round_up() {
        assert_eq!(next_standard_rating(22.5), 25.0);
        assert_eq!(next_standard_rating(25.0), 25.0);
        assert_eq!(next_standard_rating(26.0), 30.0);
        assert_eq!(next_standard_rating(6.0), 6.0);
    }
}
