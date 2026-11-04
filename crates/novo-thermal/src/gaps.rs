//! Leaks and filters: the air paths a sealed cabinet still has, and the
//! resistance a filter mat puts into the paths it is meant to have.
//!
//! Two parts, both pure and independent of the grid:
//!
//! * **Leakage by IP class.** A door seal and the cable glands pass a little
//!   air. The leak is described the way building envelopes are: a flow at
//!   100 Pa per metre of seal (per gland) and a power law with exponent 2/3,
//!   `Q = q100 * (dp / 100 Pa)^(2/3)`. It is turned into an *effective
//!   leakage area* at 4 Pa (the ASHRAE definition, `A = Q4 / sqrt(2 * 4 / rho)`)
//!   and handed to the solver as small passive vents: the door's top and
//!   bottom edges and its two sides, and the glands at the bottom. So the
//!   stack effect works through the leaks too (air in low, out high).
//! * **Filters.** A mat has a pressure drop `dp = a * v + b * v^2` (viscous
//!   plus inertial) at its face speed `v`. Clogging adds dust, and the drop
//!   rises linearly with the dust load from the clean value to the value at
//!   which the maker says to change it. The mat in series with a grille is
//!   one quadratic; a fan behind a mat loses flow where the fan line meets the
//!   mat curve. Both have closed forms, used by the app and checked in tests.
//!
//! Honest status: every number in the class tables is a **planning value**
//! chosen to be typical, not a measurement, and the leak model is a power law
//! standing in for a gap whose real shape depends on door sag and gasket
//! compression. Use the maker's curve (`Filter::custom`) when you have it.
//! IEC 60529 gives no leakage rate: the IP code is a test of what gets in, not
//! of how much air passes, so the class only orders the tightness.

use crate::model::{Enclosure, Opening, Params, Placement, Vent};

/// Air density used for the equivalent areas, kg/m3 (20 degrees C, sea level).
pub const RHO_AIR: f64 = 1.2;
/// Pressure at which the effective leakage area is taken, Pa (ASHRAE).
pub const ELA_REF_PA: f64 = 4.0;
/// Flow exponent of a leak, 0.5 (orifice) to 1 (laminar crack); 2/3 is the
/// value used for windows and doors (EN 12207, EN 1026).
pub const LEAK_EXPONENT: f64 = 2.0 / 3.0;

/// The tightness class of the enclosure's door seal and cable entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum IpClass {
    /// No gasket: a bare door with a fold (IP20 to IP30).
    Ip20,
    /// A thin foam strip (IP43).
    Ip43,
    /// Dust-protected, splash water (IP54).
    Ip54,
    /// Dust-protected, water jets (IP55).
    Ip55,
    /// Dust-tight, water jets (IP65): a continuous poured gasket.
    Ip65,
    /// Dust-tight, strong jets (IP66).
    Ip66,
}

impl IpClass {
    pub const ALL: [IpClass; 6] = [
        IpClass::Ip20,
        IpClass::Ip43,
        IpClass::Ip54,
        IpClass::Ip55,
        IpClass::Ip65,
        IpClass::Ip66,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IpClass::Ip20 => "IP20",
            IpClass::Ip43 => "IP43",
            IpClass::Ip54 => "IP54",
            IpClass::Ip55 => "IP55",
            IpClass::Ip65 => "IP65",
            IpClass::Ip66 => "IP66",
        }
    }

    /// Door seal: air through one metre of seal at 100 Pa, m3/h. Planning
    /// value.
    pub fn door_q100_m3h_per_m(self) -> f64 {
        match self {
            IpClass::Ip20 => 80.0,
            IpClass::Ip43 => 30.0,
            IpClass::Ip54 => 10.0,
            IpClass::Ip55 => 6.0,
            IpClass::Ip65 => 2.0,
            IpClass::Ip66 => 1.0,
        }
    }

    /// One cable gland (or entry plate seal) at 100 Pa, m3/h. Planning value.
    pub fn gland_q100_m3h(self) -> f64 {
        match self {
            IpClass::Ip20 => 15.0,
            IpClass::Ip43 => 8.0,
            IpClass::Ip54 => 2.0,
            IpClass::Ip55 => 1.5,
            IpClass::Ip65 => 0.5,
            IpClass::Ip66 => 0.3,
        }
    }
}

/// The door seal and cable entries of a cabinet.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Seal {
    pub ip: IpClass,
    /// Number of cable glands and plate entries (each one a leak).
    pub glands: u32,
}

impl Default for Seal {
    fn default() -> Self {
        Seal {
            ip: IpClass::Ip54,
            glands: 0,
        }
    }
}

/// Flow through a leak with flow `q100` at 100 Pa, at pressure `dp_pa`, in
/// the unit of `q100`.
pub fn leak_flow(q100: f64, dp_pa: f64) -> f64 {
    q100 * (dp_pa.abs() / 100.0).powf(LEAK_EXPONENT)
}

/// Effective leakage area of a leak with `q100_m3h` at 100 Pa, m2: the
/// orifice (discharge coefficient 1) that passes the same flow at
/// [`ELA_REF_PA`].
pub fn effective_leakage_area_m2(q100_m3h: f64) -> f64 {
    let q4 = leak_flow(q100_m3h, ELA_REF_PA) / 3600.0;
    q4 / (2.0 * ELA_REF_PA / RHO_AIR).sqrt()
}

/// Length of the door seal, m: the perimeter of the door, taken as the
/// interior width and height.
pub fn door_seal_length_m(e: &Enclosure) -> f64 {
    2.0 * (e.interior_width_mm() + e.interior_height_mm()) / 1000.0
}

/// Total leakage of a seal on enclosure `e`, flow at 100 Pa, m3/h.
pub fn total_q100_m3h(e: &Enclosure, seal: &Seal) -> f64 {
    seal.ip.door_q100_m3h_per_m() * door_seal_length_m(e)
        + seal.ip.gland_q100_m3h() * f64::from(seal.glands)
}

/// Leak flow of the whole seal at `dp_pa`, m3/h.
pub fn total_leak_m3h(e: &Enclosure, seal: &Seal, dp_pa: f64) -> f64 {
    leak_flow(total_q100_m3h(e, seal), dp_pa)
}

/// The leaks as passive vents for the solver: the door's four edges on the
/// top, bottom, left and right walls, and the glands at the bottom. Each
/// takes the whole wall as its opening and a free share that gives the leak's
/// effective area (so it does not depend on how the grid cuts a thin slot).
/// Empty when there is nothing to leak.
pub fn leak_vents(e: &Enclosure, seal: &Seal, params: &Params) -> Vec<Vent> {
    let (w, h, d) = (
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    );
    let cd = params.discharge_coefficient.max(0.05);
    let q = seal.ip.door_q100_m3h_per_m();
    let glands = seal.ip.gland_q100_m3h() * f64::from(seal.glands);
    let make = |placement: Placement, wid: f64, hei: f64, q100: f64| -> Option<Vent> {
        if q100 <= 0.0 || wid <= 0.0 || hei <= 0.0 {
            return None;
        }
        let opening = Opening {
            placement,
            width_mm: wid,
            height_mm: hei,
        };
        let frac = effective_leakage_area_m2(q100) / (cd * opening.area_m2());
        Some(Vent {
            opening,
            free_area_fraction: frac.clamp(1e-6, 1.0),
        })
    };
    [
        make(Placement::Top { x_mm: 0.5 * w }, w, d, q * w / 1000.0),
        make(
            Placement::Bottom { x_mm: 0.5 * w },
            w,
            d,
            q * w / 1000.0 + glands,
        ),
        make(Placement::Left { y_mm: 0.5 * h }, h, d, q * h / 1000.0),
        make(Placement::Right { y_mm: 0.5 * h }, h, d, q * h / 1000.0),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Filter mat grades (the EN 779 names; the grades in ISO 16890 are
/// close neighbours).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FilterClass {
    G3,
    G4,
    M5,
    F7,
}

impl FilterClass {
    pub const ALL: [FilterClass; 4] = [
        FilterClass::G3,
        FilterClass::G4,
        FilterClass::M5,
        FilterClass::F7,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FilterClass::G3 => "G3 coarse mat",
            FilterClass::G4 => "G4 coarse mat",
            FilterClass::M5 => "M5 medium",
            FilterClass::F7 => "F7 fine",
        }
    }

    /// Clean drop, drop at change-over (both at [`REF_SPEED_MS`]), dust
    /// holding capacity g/m2 and share of dust caught. Planning values for a
    /// cabinet mat 10 to 20 mm thick.
    fn table(self) -> (f64, f64, f64, f64) {
        match self {
            FilterClass::G3 => (10.0, 60.0, 250.0, 0.80),
            FilterClass::G4 => (20.0, 100.0, 350.0, 0.90),
            FilterClass::M5 => (40.0, 150.0, 250.0, 0.95),
            FilterClass::F7 => (70.0, 200.0, 150.0, 0.98),
        }
    }
}

/// Face speed at which the table drops are given, m/s.
pub const REF_SPEED_MS: f64 = 1.5;
/// Share of the drop that is viscous (linear in speed); the rest is inertial.
pub const VISCOUS_SHARE: f64 = 0.7;

/// A filter mat with its dust load.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Filter {
    /// Drop of the clean mat at `v_ref_ms`, Pa.
    pub dp_clean_pa: f64,
    /// Drop at the change-over load at `v_ref_ms`, Pa.
    pub dp_final_pa: f64,
    pub v_ref_ms: f64,
    /// Dust the mat holds up to change-over, g/m2.
    pub capacity_g_m2: f64,
    /// Share of the dust in the air the mat catches, 0 to 1.
    pub arrestance: f64,
    /// Dust load as a share of the capacity, 0 (new) to 1 (change it).
    pub loading: f64,
}

impl Filter {
    pub fn from_class(class: FilterClass) -> Filter {
        let (dp_clean_pa, dp_final_pa, capacity_g_m2, arrestance) = class.table();
        Filter {
            dp_clean_pa,
            dp_final_pa,
            v_ref_ms: REF_SPEED_MS,
            capacity_g_m2,
            arrestance,
            loading: 0.0,
        }
    }

    /// A mat from the maker's numbers: drop new and at change-over, both at
    /// `v_ref_ms`.
    pub fn custom(dp_clean_pa: f64, dp_final_pa: f64, v_ref_ms: f64) -> Filter {
        Filter {
            dp_clean_pa,
            dp_final_pa: dp_final_pa.max(dp_clean_pa),
            v_ref_ms: v_ref_ms.max(1e-3),
            capacity_g_m2: 250.0,
            arrestance: 0.9,
            loading: 0.0,
        }
    }

    pub fn with_loading(mut self, loading: f64) -> Filter {
        self.loading = loading.clamp(0.0, 1.5);
        self
    }

    /// Drop at the reference speed with the present dust load, Pa. Linear in
    /// the load (past 1 it keeps rising: a mat left in too long).
    pub fn dp_ref_pa(&self) -> f64 {
        self.dp_clean_pa + (self.dp_final_pa - self.dp_clean_pa) * self.loading
    }

    /// `dp = a * v + b * v^2`: the coefficients, Pa s/m and Pa s2/m2.
    pub fn coefficients(&self) -> (f64, f64) {
        let dp = self.dp_ref_pa();
        let v = self.v_ref_ms;
        (VISCOUS_SHARE * dp / v, (1.0 - VISCOUS_SHARE) * dp / (v * v))
    }

    /// Pressure drop at face speed `v_ms`, Pa.
    pub fn dp_pa(&self, v_ms: f64) -> f64 {
        let (a, b) = self.coefficients();
        let v = v_ms.abs();
        a * v + b * v * v
    }

    /// Dust load after `hours` in air with `dust_mg_m3` at `flow_m3h` through
    /// a mat of `area_m2`, as a share of the capacity. The flow is taken as
    /// constant (a clogging mat passes less, so this runs a little fast).
    pub fn loading_after(&self, hours: f64, dust_mg_m3: f64, flow_m3h: f64, area_m2: f64) -> f64 {
        if area_m2 <= 0.0 || self.capacity_g_m2 <= 0.0 {
            return 0.0;
        }
        let grams = self.arrestance * dust_mg_m3 / 1000.0 * flow_m3h * hours;
        (grams / (area_m2 * self.capacity_g_m2)).max(0.0)
    }

    /// Hours until the mat reaches change-over load; the inverse of
    /// [`Filter::loading_after`] from the present load.
    pub fn hours_to_change(&self, dust_mg_m3: f64, flow_m3h: f64, area_m2: f64) -> Option<f64> {
        let per_hour = self.loading_after(1.0, dust_mg_m3, flow_m3h, area_m2);
        (per_hour > 0.0).then(|| ((1.0 - self.loading) / per_hour).max(0.0))
    }
}

/// Typical dust in the air round a cabinet, mg/m3 (planning values).
pub const DUST_OFFICE: f64 = 0.05;
pub const DUST_WORKSHOP: f64 = 0.2;
pub const DUST_INDUSTRY: f64 = 1.0;
pub const DUST_HEAVY: f64 = 5.0;

/// Flow through a grille with effective free area `a_eff_m2` (discharge
/// coefficient already in) with a filter of face area `face_m2` in series,
/// at pressure difference `dp_pa`, m3/s. Without a filter it is the orifice
/// law.
pub fn series_flow_m3s(dp_pa: f64, a_eff_m2: f64, face_m2: f64, filter: Option<&Filter>) -> f64 {
    let dp = dp_pa.abs();
    if dp <= 0.0 || a_eff_m2 <= 0.0 {
        return 0.0;
    }
    let mut big_k = RHO_AIR / (2.0 * a_eff_m2 * a_eff_m2);
    let mut k1 = 0.0;
    if let Some(f) = filter {
        let (a, b) = f.coefficients();
        big_k += b / (face_m2 * face_m2);
        k1 = a / face_m2;
    }
    (-k1 + (k1 * k1 + 4.0 * big_k * dp).sqrt()) / (2.0 * big_k)
}

/// Free-area share of a grille that passes the same flow as the grille with
/// the filter, at the design pressure `dp_pa` (the pressure the cabinet runs
/// at: a few Pa from buoyancy, tens of Pa with a fan). The solver's vent is an
/// orifice; this is how a mat is put into it. Exact at `dp_pa` only: the mat
/// is partly linear, so away from it the real flow falls off a little
/// differently.
pub fn equivalent_free_fraction(
    grille_fraction: f64,
    opening: &Opening,
    filter: &Filter,
    dp_pa: f64,
    params: &Params,
) -> f64 {
    let area = opening.area_m2();
    let cd = params.discharge_coefficient.max(0.05);
    let a_eff = cd * grille_fraction * area;
    let q = series_flow_m3s(dp_pa, a_eff, area, Some(filter));
    let a_eq = q / (2.0 * dp_pa.abs().max(1e-9) / RHO_AIR).sqrt();
    (a_eq / (cd * area)).clamp(1e-6, grille_fraction)
}

/// Free-air flow (m3/h) a fan keeps when a filter of face area `face_m2` sits
/// in front of it: where the fan's straight line from (0, `p_max_pa`) to
/// (`q_free_m3h`, 0) meets the mat curve.
pub fn fan_free_flow_with_filter(
    q_free_m3h: f64,
    p_max_pa: f64,
    face_m2: f64,
    filter: &Filter,
) -> f64 {
    let q_free = q_free_m3h / 3600.0;
    let (a, b) = filter.coefficients();
    // p_max (1 - q / q_free) = a q / A + b q^2 / A^2
    let qa = b / (face_m2 * face_m2);
    let lin = a / face_m2 + p_max_pa / q_free;
    let q = (-lin + (lin * lin + 4.0 * qa * p_max_pa).sqrt()) / (2.0 * qa.max(1e-30));
    // With no inertial term the quadratic degenerates: use the linear root.
    let q = if qa < 1e-12 { p_max_pa / lin } else { q };
    q.min(q_free) * 3600.0
}

/// A word on how far the filter is gone, for the panel.
pub fn loading_word(loading: f64) -> &'static str {
    if loading < 0.25 {
        "clean"
    } else if loading < 0.75 {
        "dusty"
    } else if loading < 1.0 {
        "nearly due"
    } else {
        "change it"
    }
}
