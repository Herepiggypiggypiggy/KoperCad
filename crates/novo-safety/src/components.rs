//! Component level inputs: B10d to MTTFd.

/// Operations per year from duty parameters (ISO 13849-1 formula C.2).
/// `days` operating days per year, `hours` per day, `cycle_s` seconds per cycle.
pub fn operations_per_year(days: f64, hours: f64, cycle_s: f64) -> f64 {
    if cycle_s <= 0.0 {
        return 0.0;
    }
    days * hours * 3600.0 / cycle_s
}

/// MTTFd in years from B10d (cycles) and operations per year:
/// `MTTFd = B10d / (0.1 * nop)`.
pub fn mttfd_from_b10d(b10d: f64, nop_per_year: f64) -> f64 {
    if nop_per_year <= 0.0 {
        return f64::INFINITY;
    }
    b10d / (0.1 * nop_per_year)
}

/// One component of a channel with its MTTFd in years.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub mttfd_years: f64,
}

impl Component {
    pub fn new(name: impl Into<String>, mttfd_years: f64) -> Self {
        Self {
            name: name.into(),
            mttfd_years,
        }
    }

    pub fn from_b10d(name: impl Into<String>, b10d: f64, nop_per_year: f64) -> Self {
        Self::new(name, mttfd_from_b10d(b10d, nop_per_year))
    }
}

/// Channel MTTFd: `1/M = sum 1/M_i` (parts count method).
pub fn channel_mttfd(parts: &[Component]) -> f64 {
    let s: f64 = parts.iter().map(|c| 1.0 / c.mttfd_years).sum();
    if s <= 0.0 { f64::INFINITY } else { 1.0 / s }
}
