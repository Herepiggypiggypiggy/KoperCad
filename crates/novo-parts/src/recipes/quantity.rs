//! Reads the quantities makers print in catalog files: `24 A`, `0,08 ... 4 mm²`,
//! `1.5 kW`, `-25 ... +60 °C`.

use novo_core::Value;

/// A number or a range with the unit it was written in.
#[derive(Debug, Clone, PartialEq)]
pub struct Quantity {
    pub low: f64,
    pub high: f64,
    /// The unit as written, lower-cased with `²` spelled `2` (`mm2`, `ka`,
    /// `°c`). Empty when the text had none.
    pub unit: String,
    /// Whether the text was a range.
    pub range: bool,
}

/// What a value measures; decides which units convert and to what.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    Current,
    CurrentKa,
    Voltage,
    PowerW,
    PowerKw,
    Length,
    Area,
    Temperature,
    ResistancePerKm,
    Count,
}

impl Dim {
    /// The factor from `unit` to this dimension's stored unit (A, kA, V, W,
    /// kW, mm, mm2, °C, ohm/km), or `None` for a unit that does not belong.
    /// A missing unit counts as the stored unit: catalog files often put the
    /// unit in the feature name instead.
    pub fn factor(self, unit: &str) -> Option<f64> {
        let u = unit;
        Some(match (self, u) {
            (_, "") => 1.0,
            (Dim::Current, "a") => 1.0,
            (Dim::Current, "ma") => 1e-3,
            (Dim::Current, "ka") => 1e3,
            (Dim::CurrentKa, "ka") => 1.0,
            (Dim::CurrentKa, "a") => 1e-3,
            (Dim::CurrentKa, "ma") => 1e-6,
            (Dim::Voltage, "v") => 1.0,
            (Dim::Voltage, "mv") => 1e-3,
            (Dim::Voltage, "kv") => 1e3,
            (Dim::PowerW, "w") => 1.0,
            (Dim::PowerW, "kw") => 1e3,
            (Dim::PowerW, "mw") => 1e-3,
            (Dim::PowerKw, "kw") => 1.0,
            (Dim::PowerKw, "w") => 1e-3,
            (Dim::PowerKw, "mw") => 1e3,
            (Dim::PowerKw, "hp") => 0.7457,
            (Dim::Length, "mm") => 1.0,
            (Dim::Length, "cm") => 10.0,
            (Dim::Length, "m") => 1000.0,
            (Dim::Length, "in" | "inch") => 25.4,
            (Dim::Area, "mm2" | "qmm") => 1.0,
            (Dim::Area, "cm2") => 100.0,
            (Dim::Temperature, "°c" | "c" | "degc") => 1.0,
            (Dim::ResistancePerKm, "ω/km" | "ohm/km" | "ohm/1000m" | "mω/m" | "mohm/m") => 1.0,
            (Dim::Count, _) => 1.0,
            _ => return None,
        })
    }
}

/// Reads a quantity from text. Decimal commas are accepted; a range may be
/// written `a ... b`, `a - b`, `a to b`, `a…b` or `a bis b`. Returns `None`
/// for text without a number.
pub fn parse_quantity(text: &str) -> Option<Quantity> {
    let t = text
        .trim()
        .replace('²', "2")
        .replace(['\u{a0}', '\u{202f}'], " ");
    let (first, after_first) = take_number(&t)?;
    let rest = after_first.trim_start();
    // A range: another number after a separator word or sign.
    for sep in [
        "...", "…", "\u{2013}", "\u{2014}", " to ", " bis ", "-", "~",
    ] {
        if let Some(tail) = rest.strip_prefix(sep.trim_start())
            && let Some((second, after_second)) = take_number(tail.trim_start())
        {
            let unit = unit_of(after_second.trim_start());
            return Some(Quantity {
                low: first,
                high: second,
                unit,
                range: true,
            });
        }
    }
    let unit = unit_of(rest);
    Some(Quantity {
        low: first,
        high: first,
        unit,
        range: false,
    })
}

/// Reads a catalog value (number or text) as a quantity.
pub fn value_quantity(value: &Value) -> Option<Quantity> {
    match value {
        Value::Int(i) => Some(Quantity {
            low: *i as f64,
            high: *i as f64,
            unit: String::new(),
            range: false,
        }),
        Value::Float(f) => Some(Quantity {
            low: *f,
            high: *f,
            unit: String::new(),
            range: false,
        }),
        Value::Text(t) => parse_quantity(t),
        _ => None,
    }
}

/// A number at the start of `s` (with optional sign, decimal comma or dot),
/// and what follows it.
fn take_number(s: &str) -> Option<(f64, &str)> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    let mut i = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let digits_start = i;
    let mut seen_sep = false;
    while i < bytes.len() {
        match bytes[i] {
            b'0'..=b'9' => i += 1,
            b',' | b'.' if !seen_sep && bytes.get(i + 1).is_some_and(u8::is_ascii_digit) => {
                seen_sep = true;
                i += 1;
            }
            _ => break,
        }
    }
    if i == digits_start {
        return None;
    }
    let n: f64 = s[..i].replace(',', ".").parse().ok()?;
    Some((n, &s[i..]))
}

/// The unit token at the start of `s`.
fn unit_of(s: &str) -> String {
    let s = s.trim_start();
    let end = s
        .find(|c: char| c.is_whitespace() || c == ';' || c == ')' || c == '(')
        .unwrap_or(s.len());
    let raw = &s[..end];
    // Keep only text that looks like a unit: letters, degree, slash, omega.
    if raw.is_empty()
        || !raw
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '°' | '/' | 'Ω' | 'ω' | 'µ'))
    {
        return String::new();
    }
    raw.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_values_with_units() {
        let q = parse_quantity("24 A").unwrap();
        assert_eq!(
            (q.low, q.high, q.unit.as_str(), q.range),
            (24.0, 24.0, "a", false)
        );
        assert_eq!(parse_quantity("2,5 mm²").unwrap().low, 2.5);
        assert_eq!(parse_quantity("2,5 mm²").unwrap().unit, "mm2");
        assert_eq!(parse_quantity("800V").unwrap().unit, "v");
        assert_eq!(parse_quantity("6").unwrap().unit, "");
        assert!(parse_quantity("push-in").is_none());
    }

    #[test]
    fn ranges() {
        let q = parse_quantity("0,08 ... 4 mm²").unwrap();
        assert_eq!(
            (q.low, q.high, q.unit.as_str(), q.range),
            (0.08, 4.0, "mm2", true)
        );
        let q = parse_quantity("0.2-6 mm2").unwrap();
        assert_eq!((q.low, q.high), (0.2, 6.0));
        let q = parse_quantity("-25 ... 60 °C").unwrap();
        assert_eq!((q.low, q.high, q.unit.as_str()), (-25.0, 60.0, "°c"));
        let q = parse_quantity("1.4 bis 2 A").unwrap();
        assert_eq!((q.low, q.high), (1.4, 2.0));
    }

    #[test]
    fn conversion_factors() {
        assert_eq!(Dim::Current.factor("ma"), Some(1e-3));
        assert_eq!(Dim::CurrentKa.factor("a"), Some(1e-3));
        assert_eq!(Dim::Length.factor("cm"), Some(10.0));
        assert_eq!(Dim::Current.factor("v"), None);
        assert_eq!(Dim::Area.factor("mm2"), Some(1.0));
    }
}
