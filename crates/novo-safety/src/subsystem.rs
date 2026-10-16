//! Subsystem model: category, MTTFd, DC, CCF, PFHd and combination.

use crate::components::{Component, channel_mttfd};
use crate::pl::PerformanceLevel;

pub const HOURS_PER_YEAR: f64 = 8760.0;
/// Proof test / mission time T_M in hours (20 years, ISO 13849-1).
const MISSION_H: f64 = 175_200.0;
/// Mean repair/diagnostic reaction time used in the model (hours).
const MTTR_H: f64 = 8.0;
/// Common cause factor with a passed CCF score.
const BETA: f64 = 0.02;
/// Common cause factor when the CCF score is below 65 (not allowed for cat 2 to 4).
const BETA_FAILED: f64 = 0.1;
/// Required CCF score for categories 2, 3 and 4.
pub const CCF_REQUIRED: u32 = 65;
/// MTTFd per channel is capped at 100 years.
const MTTFD_CAP: f64 = 100.0;

/// Designated architecture category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    B,
    C1,
    C2,
    C3,
    C4,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Self::B => "B",
            Self::C1 => "1",
            Self::C2 => "2",
            Self::C3 => "3",
            Self::C4 => "4",
        }
    }

    /// Highest PL the category can support.
    pub fn max_pl(self) -> PerformanceLevel {
        match self {
            Self::B => PerformanceLevel::B,
            Self::C1 => PerformanceLevel::C,
            Self::C2 | Self::C3 => PerformanceLevel::D,
            Self::C4 => PerformanceLevel::E,
        }
    }
}

/// MTTFd level of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mttfd {
    Low,
    Medium,
    High,
}

impl Mttfd {
    pub fn from_years(y: f64) -> Option<Self> {
        if y < 3.0 {
            None
        } else if y < 10.0 {
            Some(Self::Low)
        } else if y < 30.0 {
            Some(Self::Medium)
        } else {
            Some(Self::High)
        }
    }
}

/// Average diagnostic coverage level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Dc {
    None,
    Low,
    Medium,
    High,
}

impl Dc {
    pub fn from_percent(p: f64) -> Self {
        if p >= 99.0 {
            Self::High
        } else if p >= 90.0 {
            Self::Medium
        } else if p >= 60.0 {
            Self::Low
        } else {
            Self::None
        }
    }

    /// Representative value used in the model.
    fn value(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.60,
            Self::Medium => 0.90,
            Self::High => 0.99,
        }
    }
}

/// A finding about a subsystem (shown as a Problem by the UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub message: String,
}

/// One safety subsystem (sensor, logic or actuator part of a function).
#[derive(Debug, Clone, PartialEq)]
pub struct Subsystem {
    pub name: String,
    pub category: Category,
    /// Parts of channel 1.
    pub channel1: Vec<Component>,
    /// Parts of channel 2 (cat 3, 4 and the test channel of cat 2); empty means identical to channel 1.
    pub channel2: Vec<Component>,
    /// DCavg in percent (fold per-part values with [`dc_avg`]).
    pub dc_avg_percent: f64,
    /// CCF score out of 100 (65 needed for categories 2 to 4).
    pub ccf_score: u32,
}

/// Weighted DCavg in percent from `(mttfd_years, dc_percent)` of each part.
pub fn dc_avg(parts: &[(f64, f64)]) -> f64 {
    let denom: f64 = parts.iter().map(|(m, _)| 1.0 / m).sum();
    if denom <= 0.0 {
        return 0.0;
    }
    parts.iter().map(|(m, dc)| dc / m).sum::<f64>() / denom
}

/// Result of a subsystem calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct SubsystemResult {
    pub mttfd_years: f64,
    pub mttfd_level: Option<Mttfd>,
    pub dc: Dc,
    pub pfhd: f64,
    /// PL from PFHd alone.
    pub pl_from_pfhd: PerformanceLevel,
    /// PL after applying the category limit and requirement checks.
    pub pl: PerformanceLevel,
    pub problems: Vec<Problem>,
}

impl Subsystem {
    pub fn calculate(&self) -> SubsystemResult {
        let mut problems = Vec::new();
        let m1 = channel_mttfd(&self.channel1);
        let m2 = if self.channel2.is_empty() {
            m1
        } else {
            channel_mttfd(&self.channel2)
        };
        // Symmetrisation for two unequal channels (ISO 13849-1 annex D.2).
        let mut m = match self.category {
            Category::C3 | Category::C4 => {
                if (m1 - m2).abs() < f64::EPSILON {
                    m1
                } else {
                    2.0 / 3.0 * (m1 + m2 - 1.0 / (1.0 / m1 + 1.0 / m2))
                }
            }
            _ => m1,
        };
        if m > MTTFD_CAP {
            m = MTTFD_CAP;
        }
        let level = Mttfd::from_years(m);
        let dc = Dc::from_percent(self.dc_avg_percent);
        if level.is_none() {
            problems.push(Problem {
                message: format!("MTTFd {m:.1} years is below 3 years: not allowed"),
            });
        }
        let ccf_ok = self.ccf_score >= CCF_REQUIRED;
        let needs_ccf = matches!(self.category, Category::C2 | Category::C3 | Category::C4);
        if needs_ccf && !ccf_ok {
            problems.push(Problem {
                message: format!(
                    "CCF score {} is below the required {CCF_REQUIRED}",
                    self.ccf_score
                ),
            });
        }
        match self.category {
            Category::B | Category::C1 if dc != Dc::None => problems.push(Problem {
                message: "categories B and 1 have no diagnostics: DCavg must be none".into(),
            }),
            Category::C2 if dc < Dc::Low => problems.push(Problem {
                message: "category 2 needs DCavg of at least low (60 percent)".into(),
            }),
            Category::C3 if dc < Dc::Low => problems.push(Problem {
                message: "category 3 needs DCavg of at least low (60 percent)".into(),
            }),
            Category::C4 if dc < Dc::High => problems.push(Problem {
                message: "category 4 needs DCavg high (99 percent)".into(),
            }),
            _ => {}
        }
        if self.category == Category::C1 && level != Some(Mttfd::High) {
            problems.push(Problem {
                message: "category 1 needs high MTTFd (30 years or more)".into(),
            });
        }

        let pfhd = pfhd_model(
            self.category,
            m,
            (self.dc_avg_percent / 100.0).clamp(0.0, 0.999),
            ccf_ok,
        );
        let pl_from_pfhd = PerformanceLevel::from_pfhd(pfhd);
        let mut pl = pl_from_pfhd.min(self.category.max_pl());
        if !problems.is_empty() {
            pl = PerformanceLevel::None;
        }
        SubsystemResult {
            mttfd_years: m,
            mttfd_level: level,
            dc,
            pfhd,
            pl_from_pfhd,
            pl,
            problems,
        }
    }
}

/// Closed-form PFHd estimate in 1/h.
///
/// B and 1: single channel, `1/(MTTFd * 8760)`. Categories 3 and 4: two channels
/// with diagnostics and common cause; categories 2: single channel with test
/// channel, undetected fraction of the dangerous failure rate.
fn pfhd_model(cat: Category, mttfd_years: f64, dc: f64, ccf_ok: bool) -> f64 {
    let lambda_d = 1.0 / (mttfd_years * HOURS_PER_YEAR);
    match cat {
        Category::B | Category::C1 => lambda_d,
        Category::C2 => lambda_d * (1.0 - dc),
        Category::C3 | Category::C4 => {
            let beta = if ccf_ok { BETA } else { BETA_FAILED };
            let ldd = dc * lambda_d;
            let ldu = (1.0 - dc) * lambda_d;
            // Mean time a channel stays dangerous before it is noticed or repaired.
            let t_ce = (ldu / lambda_d) * (MISSION_H / 2.0 + MTTR_H) + (ldd / lambda_d) * MTTR_H;
            let a = (1.0 - beta / 2.0) * ldd + (1.0 - beta) * ldu;
            let b = (1.0 - beta) * lambda_d;
            2.0 * a * b * t_ce + beta * lambda_d
        }
    }
}

/// Combined result of several subsystems.
#[derive(Debug, Clone, PartialEq)]
pub struct Combined {
    pub pfhd: f64,
    pub pl: PerformanceLevel,
}

/// Series combination: PFHd values add; the lowest subsystem PL still limits the result
/// (ISO 13849-1 section 6.3).
pub fn combine_series(parts: &[SubsystemResult]) -> Combined {
    let pfhd: f64 = parts.iter().map(|p| p.pfhd).sum();
    let mut pl = PerformanceLevel::from_pfhd(pfhd);
    if let Some(min) = parts.iter().map(|p| p.pl).min() {
        pl = pl.min(min);
    }
    Combined { pfhd, pl }
}

/// Parallel (redundant, independent) combination of two subsystems with a common
/// cause fraction `beta`. This goes beyond the standard's simplified method and is a
/// modelling estimate: `PFH = beta * min + 2 * a * b * T/2` style terms are reduced to
/// the dominant `beta * min(PFH) + PFH1 * PFH2 * T_M / 2`.
pub fn combine_parallel(a: &SubsystemResult, b: &SubsystemResult, beta: f64) -> Combined {
    let min = a.pfhd.min(b.pfhd);
    let pfhd = beta * min + (1.0 - beta) * a.pfhd * b.pfhd * MISSION_H / 2.0;
    let mut pl = PerformanceLevel::from_pfhd(pfhd);
    pl = pl.min(PerformanceLevel::E);
    Combined { pfhd, pl }
}
