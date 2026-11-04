//! Closed-form mechanical estimates: busbar short-circuit forces and the
//! deflection of rails and mounting plates under device load.
//!
//! Pure functions on plain numbers (newtons, metres, pascals; the public
//! inputs of the busbar and the beam use millimetres where an electrician
//! reads drawings, and say so). No model access, no I/O.
//!
//! # What is exact and what is a model
//!
//! * The force between parallel conductors, the beam formulas (bending
//!   moment, reactions, deflection, natural frequency) and the plate series
//!   (Navier double sine series) are textbook closed forms. Tests compare
//!   them with hand-checked numbers and with the published plate
//!   coefficients.
//! * The busbar model is quasi-static: peak current `ip = κ·√2·Ik″`
//!   (peak factor of IEC 60909-0), force per metre from Biot-Savart for
//!   thin parallel filaments (the formula in IEC 60865-1 for the force
//!   between conductors), then bending of an elastic beam. The dynamic
//!   factor of IEC 60865-1 (swinging of the bar after the first peak) is
//!   NOT computed: it is an input that defaults to 1, and the natural
//!   frequency of the span is reported so a resonance risk can be flagged.
//! * Filament forces overestimate the force on wide bars at small
//!   distance (the shape factor is below 1), so the result is on the safe
//!   side for the force and unsafe for nothing else here.
//! * One conductor per phase. No sub-conductors, no bar ends or bends, no
//!   thermal derating of the yield stress, no supports that tilt.
//! * Material values are typical planning values: enter the yield stress
//!   from the maker's certificate. Clause numbers of the standards are not
//!   quoted (unverified); the result is a design aid, never a certificate.

use std::f64::consts::PI;

/// Magnetic constant μ0, H/m.
pub const MU0: f64 = 4.0e-7 * PI;

// ---------------------------------------------------------------------------
// Busbar short-circuit force
// ---------------------------------------------------------------------------

/// Peak factor κ from the R/X ratio of the short-circuit path
/// (`κ = 1.02 + 0.98·e^(−3R/X)`, IEC 60909-0).
pub fn peak_factor(r_over_x: f64) -> f64 {
    1.02 + 0.98 * (-3.0 * r_over_x.max(0.0)).exp()
}

/// Peak short-circuit current `ip = κ·√2·Ik″`, A.
pub fn peak_current(ik_a: f64, kappa: f64) -> f64 {
    kappa * std::f64::consts::SQRT_2 * ik_a
}

/// Which fault the force is counted for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusFault {
    /// Two conductors carry the current in opposite directions
    /// (single phase, DC, or a two-phase fault).
    TwoConductor,
    /// Three-phase fault on three coplanar, equally spaced conductors;
    /// the worst force acts on the middle one.
    ThreePhase,
}

/// Peak force per metre of conductor, N/m, between parallel thin
/// conductors `a_m` metres apart.
///
/// Two conductors: `μ0/(2π)·ip²/a`. Three phases: the factor `√3/2` for the
/// middle conductor (IEC 60865-1).
pub fn force_per_metre(ip_a: f64, a_m: f64, fault: BusFault) -> f64 {
    let k = match fault {
        BusFault::TwoConductor => 1.0,
        BusFault::ThreePhase => 3.0_f64.sqrt() / 2.0,
    };
    MU0 / (2.0 * PI) * k * ip_a * ip_a / a_m
}

/// How the bar lies on its supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spans {
    /// One span, free (simply supported) at both supports.
    Simple,
    /// One span clamped at both supports. Optimistic when the supports
    /// are insulators that can tilt; use only for rigid clamps.
    FixedBoth,
    /// Two equal spans over three supports (continuous bar).
    TwoSpan,
}

impl Spans {
    /// Largest bending moment per `F'·l²`.
    fn moment_coefficient(self) -> f64 {
        match self {
            Spans::Simple | Spans::TwoSpan => 1.0 / 8.0,
            Spans::FixedBoth => 1.0 / 12.0,
        }
    }

    /// Largest support reaction per `F'·l`.
    fn reaction_coefficient(self) -> f64 {
        match self {
            Spans::Simple | Spans::FixedBoth => 0.5,
            Spans::TwoSpan => 1.25,
        }
    }

    /// `(βl)²` of the first mode, so `f1 = (βl)²/(2π)·√(EI/(m'·l⁴))`.
    fn mode_constant(self) -> f64 {
        match self {
            Spans::Simple => PI * PI,
            Spans::FixedBoth => 22.373_3,
            Spans::TwoSpan => 15.418_2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Spans::Simple => "one span, free ends",
            Spans::FixedBoth => "one span, clamped ends",
            Spans::TwoSpan => "two equal spans",
        }
    }
}

/// Bar material with typical planning values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarMaterial {
    pub name: &'static str,
    /// Young's modulus, Pa.
    pub e_pa: f64,
    /// Density, kg/m³.
    pub density: f64,
    /// Yield stress `Rp0.2`, Pa. A typical minimum, not a certificate.
    pub rp02_pa: f64,
}

/// Hard copper (typical minimum yield).
pub const COPPER: BarMaterial = BarMaterial {
    name: "Copper (hard)",
    e_pa: 110.0e9,
    density: 8900.0,
    rp02_pa: 200.0e6,
};

/// Aluminium alloy, artificially aged (typical minimum yield).
pub const ALUMINIUM: BarMaterial = BarMaterial {
    name: "Aluminium (alloy)",
    e_pa: 70.0e9,
    density: 2700.0,
    rp02_pa: 150.0e6,
};

/// Shape factor of a rectangular section: the bar may be stressed to `q·Rp0.2`
/// because the outer fibres yield before the section does.
pub const Q_RECTANGULAR: f64 = 1.5;

/// A busbar run to check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusbarInput {
    /// Initial symmetrical short-circuit current `Ik″`, A.
    pub ik_a: f64,
    /// R/X ratio of the fault path; sets the peak factor.
    pub r_over_x: f64,
    pub fault: BusFault,
    /// Distance between conductor centres, mm.
    pub spacing_mm: f64,
    /// Distance between supports, mm.
    pub span_mm: f64,
    /// Size of the bar in the direction of the force, mm.
    pub depth_mm: f64,
    /// Size of the bar across the force, mm.
    pub breadth_mm: f64,
    pub spans: Spans,
    pub material: BarMaterial,
    /// Dynamic factor `Vσ·Vr` of the swinging bar. 1 is quasi-static.
    pub dynamic_factor: f64,
    /// Rated cantilever force of one support (insulator), N, if known.
    pub support_rating_n: Option<f64>,
    /// System frequency, Hz.
    pub frequency_hz: f64,
}

impl BusbarInput {
    /// 50 Hz, three-phase, copper, 100 × 10 mm on edge, 100 mm apart, a
    /// 500 mm span between rigid supports: a starting point to edit.
    pub fn typical() -> Self {
        BusbarInput {
            ik_a: 25_000.0,
            r_over_x: 0.1,
            fault: BusFault::ThreePhase,
            spacing_mm: 100.0,
            span_mm: 500.0,
            depth_mm: 100.0,
            breadth_mm: 10.0,
            spans: Spans::Simple,
            material: COPPER,
            dynamic_factor: 1.0,
            support_rating_n: None,
            frequency_hz: 50.0,
        }
    }
}

/// Verdict of one line of a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    /// Cannot be judged, or needs the user's attention.
    Note,
}

/// Result of the busbar check.
#[derive(Debug, Clone, PartialEq)]
pub struct BusbarResult {
    pub kappa: f64,
    pub peak_a: f64,
    /// Peak force per metre, N/m.
    pub force_per_m: f64,
    /// Largest bending stress including the dynamic factor, Pa.
    pub stress_pa: f64,
    /// Stress the bar may carry, `q·Rp0.2`, Pa.
    pub allowed_pa: f64,
    /// Stress over allowed.
    pub utilisation: f64,
    /// Longest span that keeps the stress within the allowed value, mm.
    pub max_span_mm: f64,
    /// Largest force on one support including the dynamic factor, N.
    pub support_force_n: f64,
    /// First natural frequency of the span, Hz.
    pub natural_hz: f64,
    /// The natural frequency lies near the system frequency or its double.
    pub resonance_risk: bool,
    pub bending: Verdict,
    pub support: Verdict,
    pub resonance: Verdict,
}

/// Why an input cannot be calculated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError(pub &'static str);

impl std::fmt::Display for InputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Section modulus of a rectangle `breadth × depth` bending about the
/// axis across the force, m³.
pub fn rectangle_section_modulus(breadth_m: f64, depth_m: f64) -> f64 {
    breadth_m * depth_m * depth_m / 6.0
}

/// Checks a busbar span against the short-circuit force.
pub fn check_busbar(i: &BusbarInput) -> Result<BusbarResult, InputError> {
    let pos = |v: f64| v.is_finite() && v > 0.0;
    if !pos(i.ik_a) {
        return Err(InputError("The short-circuit current must be above zero"));
    }
    if !pos(i.spacing_mm) || !pos(i.span_mm) {
        return Err(InputError("Spacing and span must be above zero"));
    }
    if !pos(i.depth_mm) || !pos(i.breadth_mm) {
        return Err(InputError("The bar size must be above zero"));
    }
    if !pos(i.dynamic_factor) || !pos(i.frequency_hz) {
        return Err(InputError(
            "The dynamic factor and frequency must be above zero",
        ));
    }
    let kappa = peak_factor(i.r_over_x);
    let peak = peak_current(i.ik_a, kappa);
    let a = i.spacing_mm / 1000.0;
    let l = i.span_mm / 1000.0;
    let b = i.breadth_mm / 1000.0;
    let d = i.depth_mm / 1000.0;
    let f_line = force_per_metre(peak, a, i.fault);
    let z = rectangle_section_modulus(b, d);
    let inertia = b * d * d * d / 12.0;
    let stress = i.dynamic_factor * i.spans.moment_coefficient() * f_line * l * l / z;
    let allowed = Q_RECTANGULAR * i.material.rp02_pa;
    let max_span =
        (allowed * z / (i.dynamic_factor * i.spans.moment_coefficient() * f_line)).sqrt() * 1000.0;
    let support_force = i.dynamic_factor * i.spans.reaction_coefficient() * f_line * l;
    let mass_per_m = i.material.density * b * d;
    let natural = i.spans.mode_constant() / (2.0 * PI)
        * (i.material.e_pa * inertia / (mass_per_m * l.powi(4))).sqrt();
    let near = |target: f64| natural > 0.7 * target && natural < 1.3 * target;
    let resonance_risk = near(i.frequency_hz) || near(2.0 * i.frequency_hz);
    let utilisation = stress / allowed;
    Ok(BusbarResult {
        kappa,
        peak_a: peak,
        force_per_m: f_line,
        stress_pa: stress,
        allowed_pa: allowed,
        utilisation,
        max_span_mm: max_span,
        support_force_n: support_force,
        natural_hz: natural,
        resonance_risk,
        bending: if utilisation <= 1.0 {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
        support: match i.support_rating_n {
            Some(r) if support_force <= r => Verdict::Pass,
            Some(_) => Verdict::Fail,
            None => Verdict::Note,
        },
        resonance: if resonance_risk {
            Verdict::Note
        } else {
            Verdict::Pass
        },
    })
}

// ---------------------------------------------------------------------------
// Beams: DIN rails and mounting rails
// ---------------------------------------------------------------------------

/// Second moments of a thin-walled section built from rectangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Section {
    /// Area, m².
    pub area: f64,
    /// Second moment about the axis across the width (bending when the load
    /// pulls out of the plate), m⁴.
    pub i_out_of_plate: f64,
    /// Second moment about the axis out of the plate (bending when the load
    /// lies in the plate plane, such as a device weighing on a horizontal
    /// rail), m⁴.
    pub i_in_plate: f64,
}

/// One rectangle of a section: centre `(y, z)` and size `(width, height)`,
/// mm. `y` runs across the rail along the plate, `z` out of the plate.
pub type Rect = (f64, f64, f64, f64);

/// Section properties from rectangles that do not overlap.
pub fn section_from_rects(rects: &[Rect]) -> Section {
    let area_mm2: f64 = rects.iter().map(|r| r.2 * r.3).sum();
    let yc = rects.iter().map(|r| r.2 * r.3 * r.0).sum::<f64>() / area_mm2;
    let zc = rects.iter().map(|r| r.2 * r.3 * r.1).sum::<f64>() / area_mm2;
    let mut iy = 0.0;
    let mut iz = 0.0;
    for &(y, z, w, h) in rects {
        let a = w * h;
        iy += w * h * h * h / 12.0 + a * (z - zc) * (z - zc);
        iz += h * w * w * w / 12.0 + a * (y - yc) * (y - yc);
    }
    Section {
        area: area_mm2 * 1e-6,
        i_out_of_plate: iy * 1e-12,
        i_in_plate: iz * 1e-12,
    }
}

/// An idealised top-hat rail (sharp corners): 27 mm web on the plate,
/// walls of height `height_mm`, 4 mm lips outward, 35 mm over all.
pub fn top_hat_35(height_mm: f64, thickness_mm: f64) -> Section {
    let t = thickness_mm;
    let h = height_mm;
    section_from_rects(&[
        (0.0, t / 2.0, 27.0 - 2.0 * t, t),
        (-(13.5 - t / 2.0), h / 2.0, t, h),
        (13.5 - t / 2.0, h / 2.0, t, h),
        (-(13.5 + 2.0), h - t / 2.0, 4.0, t),
        (13.5 + 2.0, h - t / 2.0, 4.0, t),
    ])
}

/// Steel, Young's modulus, Pa.
pub const STEEL_E_PA: f64 = 210.0e9;

/// How the beam is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hold {
    /// Rail between two fixings that do not resist rotation.
    Simple,
    /// Rail clamped at both fixings.
    FixedBoth,
    /// Held at one end only.
    Cantilever,
}

impl Hold {
    pub fn label(self) -> &'static str {
        match self {
            Hold::Simple => "between two fixings",
            Hold::FixedBoth => "clamped at both ends",
            Hold::Cantilever => "held at one end",
        }
    }
}

/// A load on a beam.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Load {
    /// Total force spread evenly over the whole span, N.
    Spread(f64),
    /// A force at `pos_mm` from the left/fixed end, N.
    Point { pos_mm: f64, newtons: f64 },
}

/// A beam to check.
#[derive(Debug, Clone, PartialEq)]
pub struct BeamInput {
    pub span_mm: f64,
    pub hold: Hold,
    pub e_pa: f64,
    /// Second moment of the section for the load direction, m⁴.
    pub inertia_m4: f64,
    pub loads: Vec<Load>,
    /// Allowed deflection as span divided by this number (200 is common).
    pub limit_ratio: f64,
}

/// Result of a beam check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeamResult {
    /// Largest deflection, mm.
    pub deflection_mm: f64,
    /// Where it occurs, mm from the left/fixed end.
    pub at_mm: f64,
    /// Allowed deflection, mm.
    pub limit_mm: f64,
    pub verdict: Verdict,
}

/// Deflection of a beam of length `l` at `x` (all metres) from one load.
fn deflection_at(hold: Hold, l: f64, x: f64, ei: f64, load: Load) -> f64 {
    match (hold, load) {
        (Hold::Simple, Load::Spread(f)) => {
            let w = f / l;
            w * x * (l.powi(3) - 2.0 * l * x * x + x.powi(3)) / (24.0 * ei)
        }
        (Hold::Simple, Load::Point { pos_mm, newtons }) => {
            let a = pos_mm / 1000.0;
            let b = l - a;
            // Symmetric formula: mirror when x lies right of the load.
            let (x, _a, b) = if x <= a { (x, a, b) } else { (l - x, b, a) };
            newtons * b * x * (l * l - b * b - x * x) / (6.0 * l * ei)
        }
        (Hold::FixedBoth, Load::Spread(f)) => {
            let w = f / l;
            w * x * x * (l - x) * (l - x) / (24.0 * ei)
        }
        (Hold::FixedBoth, Load::Point { pos_mm, newtons }) => {
            let a = pos_mm / 1000.0;
            let b = l - a;
            let (x, a, b) = if x <= a { (x, a, b) } else { (l - x, b, a) };
            newtons * b * b * x * x / (6.0 * l.powi(3) * ei) * (3.0 * a * l - x * (3.0 * a + b))
        }
        (Hold::Cantilever, Load::Spread(f)) => {
            let w = f / l;
            w * x * x * (6.0 * l * l - 4.0 * l * x + x * x) / (24.0 * ei)
        }
        (Hold::Cantilever, Load::Point { pos_mm, newtons }) => {
            let a = pos_mm / 1000.0;
            if x <= a {
                newtons * x * x * (3.0 * a - x) / (6.0 * ei)
            } else {
                newtons * a * a * (3.0 * x - a) / (6.0 * ei)
            }
        }
    }
}

/// Largest deflection of a beam, by superposition of the closed forms and a
/// scan along the span.
pub fn check_beam(i: &BeamInput) -> Result<BeamResult, InputError> {
    let pos = |v: f64| v.is_finite() && v > 0.0;
    if !pos(i.span_mm) || !pos(i.e_pa) || !pos(i.inertia_m4) || !pos(i.limit_ratio) {
        return Err(InputError(
            "Span, stiffness and the limit must be above zero",
        ));
    }
    if i.loads.is_empty() {
        return Err(InputError("Enter at least one load"));
    }
    for load in &i.loads {
        match *load {
            Load::Spread(f) if !f.is_finite() || f < 0.0 => {
                return Err(InputError("A load cannot be negative"));
            }
            Load::Point { pos_mm, newtons }
                if !newtons.is_finite()
                    || newtons < 0.0
                    || !pos_mm.is_finite()
                    || pos_mm < 0.0
                    || pos_mm > i.span_mm =>
            {
                return Err(InputError("A point load must lie on the span"));
            }
            _ => {}
        }
    }
    let l = i.span_mm / 1000.0;
    let ei = i.e_pa * i.inertia_m4;
    const STEPS: usize = 2000;
    let (mut best, mut at) = (0.0_f64, 0.0_f64);
    for k in 0..=STEPS {
        let x = l * k as f64 / STEPS as f64;
        let y: f64 = i
            .loads
            .iter()
            .map(|&load| deflection_at(i.hold, l, x, ei, load))
            .sum();
        if y.abs() > best {
            best = y.abs();
            at = x;
        }
    }
    let limit_mm = i.span_mm / i.limit_ratio;
    let deflection_mm = best * 1000.0;
    Ok(BeamResult {
        deflection_mm,
        at_mm: at * 1000.0,
        limit_mm,
        verdict: if deflection_mm <= limit_mm {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    })
}

// ---------------------------------------------------------------------------
// Plates
// ---------------------------------------------------------------------------

/// A rectangular plate simply supported on all four edges (Navier series).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateInput {
    /// Side lengths, mm.
    pub a_mm: f64,
    pub b_mm: f64,
    pub thickness_mm: f64,
    pub e_pa: f64,
    pub poisson: f64,
    /// Load spread evenly over the plate, N.
    pub spread_n: f64,
    /// A force at the plate centre, N.
    pub centre_n: f64,
    /// Allowed deflection as the shorter side divided by this number.
    pub limit_ratio: f64,
}

/// Result of a plate check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateResult {
    /// Deflection at the centre, mm.
    pub deflection_mm: f64,
    pub limit_mm: f64,
    pub verdict: Verdict,
}

/// Bending stiffness `D = E·t³/(12·(1−ν²))`, N·m.
pub fn plate_stiffness(e_pa: f64, t_m: f64, poisson: f64) -> f64 {
    e_pa * t_m.powi(3) / (12.0 * (1.0 - poisson * poisson))
}

/// Centre deflection of a simply supported plate, m: a uniform pressure
/// `q` (Pa) plus a force `p` (N) at the centre.
pub fn plate_centre_deflection(a: f64, b: f64, d: f64, q: f64, p: f64) -> f64 {
    let mut uniform = 0.0;
    let mut point = 0.0;
    for m in (1..400).step_by(2) {
        for n in (1..400).step_by(2) {
            let (mf, nf) = (m as f64, n as f64);
            let sign = if ((m - 1) / 2 + (n - 1) / 2) % 2 == 0 {
                1.0
            } else {
                -1.0
            };
            let k = (mf * mf / (a * a) + nf * nf / (b * b)).powi(2);
            uniform += sign / (mf * nf * k);
            point += 1.0 / k;
        }
    }
    16.0 * q / (PI.powi(6) * d) * uniform + 4.0 * p / (PI.powi(4) * d * a * b) * point
}

/// Checks a plate under device load.
pub fn check_plate(i: &PlateInput) -> Result<PlateResult, InputError> {
    let pos = |v: f64| v.is_finite() && v > 0.0;
    if !pos(i.a_mm) || !pos(i.b_mm) || !pos(i.thickness_mm) || !pos(i.e_pa) || !pos(i.limit_ratio) {
        return Err(InputError(
            "Plate size, thickness, stiffness and the limit must be above zero",
        ));
    }
    if !(0.0..0.5).contains(&i.poisson) {
        return Err(InputError("Poisson's ratio must lie between 0 and 0.5"));
    }
    if i.spread_n < 0.0 || i.centre_n < 0.0 || !i.spread_n.is_finite() || !i.centre_n.is_finite() {
        return Err(InputError("A load cannot be negative"));
    }
    let (a, b) = (i.a_mm / 1000.0, i.b_mm / 1000.0);
    let d = plate_stiffness(i.e_pa, i.thickness_mm / 1000.0, i.poisson);
    let w = plate_centre_deflection(a, b, d, i.spread_n / (a * b), i.centre_n);
    let deflection_mm = w * 1000.0;
    let limit_mm = i.a_mm.min(i.b_mm) / i.limit_ratio;
    Ok(PlateResult {
        deflection_mm,
        limit_mm,
        verdict: if deflection_mm <= limit_mm {
            Verdict::Pass
        } else {
            Verdict::Fail
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) {
        assert!(
            (a - b).abs() <= rel * b.abs(),
            "{a} is not within {rel} of {b}"
        );
    }

    #[test]
    fn peak_factor_limits() {
        // R/X → 0 gives 2.0; very resistive paths approach 1.02.
        close(peak_factor(0.0), 2.0, 1e-12);
        close(peak_factor(10.0), 1.02, 1e-6);
        // Hand value at R/X = 0.1: 1.02 + 0.98·e^-0.3 = 1.7457.
        close(peak_factor(0.1), 1.7457, 1e-3);
    }

    #[test]
    fn three_phase_force_hand_value() {
        // ip = 1.8·√2·50 kA = 127 279 A; a = 0.1 m.
        let ip = peak_current(50_000.0, 1.8);
        close(ip, 127_279.2, 1e-5);
        // F' = 2e-7·(√3/2)·ip²/a = 28 062 N/m (hand: 2e-7·0.866025·1.62e10/0.1).
        close(
            force_per_metre(ip, 0.1, BusFault::ThreePhase),
            28_059.0,
            1e-3,
        );
        // The two-conductor force is 1/(√3/2) = 1.1547 times larger.
        close(
            force_per_metre(ip, 0.1, BusFault::TwoConductor)
                / force_per_metre(ip, 0.1, BusFault::ThreePhase),
            2.0 / 3.0_f64.sqrt(),
            1e-12,
        );
    }

    #[test]
    fn busbar_hand_check() {
        // 25 kA, R/X 0.1, 100 mm spacing, 500 mm span, copper 100×10 on edge.
        let r = check_busbar(&BusbarInput::typical()).unwrap();
        close(r.kappa, 1.7457, 1e-3);
        // ip = 1.7457·√2·25000 = 61 722 A.
        close(r.peak_a, 61_722.0, 1e-3);
        // F' = 2e-7·0.866025·ip²/0.1 = 6 598 N/m.
        close(r.force_per_m, 6_598.0, 2e-3);
        // Z = 0.01·0.1²/6 = 1.6667e-5 m³; σ = F'l²/(8Z) = 6598·0.25/8/1.6667e-5.
        close(r.stress_pa, 12.37e6, 3e-3);
        close(r.allowed_pa, 300.0e6, 1e-12);
        assert_eq!(r.bending, Verdict::Pass);
        // Reaction = F'·l/2 = 1 649.5 N.
        close(r.support_force_n, 1_649.5, 3e-3);
        // Longest span: l = √(8·σ_allow·Z/F') = √(8·300e6·1.6667e-5/6598) = 2.462 m.
        close(r.max_span_mm, 2_462.0, 3e-3);
    }

    #[test]
    fn busbar_natural_frequency_hand_check() {
        // 100×10 copper, 500 mm, free ends: I = 0.01·0.1³/12 = 8.333e-7 m⁴,
        // m' = 8900·0.001 = 8.9 kg/m, f = (π/2)√(110e9·8.333e-7/(8.9·0.0625)).
        // = 1.5708·√(164 794) = 1.5708·405.95 = 637.7 Hz.
        let r = check_busbar(&BusbarInput::typical()).unwrap();
        close(r.natural_hz, 637.7, 2e-3);
        assert!(!r.resonance_risk);
        // f falls with the square of the span: 1.8 m gives 637.7·(0.5/1.8)² =
        // 49.2 Hz, right on the system frequency.
        let mut i = BusbarInput::typical();
        i.span_mm = 1_800.0;
        let r = check_busbar(&i).unwrap();
        assert!(r.resonance_risk, "{} Hz", r.natural_hz);
        assert_eq!(r.resonance, Verdict::Note);
    }

    #[test]
    fn busbar_fails_when_the_span_is_too_long_or_the_bar_too_thin() {
        let mut i = BusbarInput::typical();
        i.span_mm = 3_000.0;
        assert_eq!(check_busbar(&i).unwrap().bending, Verdict::Fail);
        let mut i = BusbarInput::typical();
        i.ik_a = 80_000.0;
        i.depth_mm = 20.0;
        i.breadth_mm = 5.0;
        assert_eq!(check_busbar(&i).unwrap().bending, Verdict::Fail);
    }

    #[test]
    fn support_rating_decides_the_support_verdict() {
        let mut i = BusbarInput::typical();
        assert_eq!(check_busbar(&i).unwrap().support, Verdict::Note);
        i.support_rating_n = Some(2_000.0);
        assert_eq!(check_busbar(&i).unwrap().support, Verdict::Pass);
        i.support_rating_n = Some(1_000.0);
        assert_eq!(check_busbar(&i).unwrap().support, Verdict::Fail);
    }

    #[test]
    fn span_layouts_follow_beam_theory() {
        let mut i = BusbarInput::typical();
        let simple = check_busbar(&i).unwrap();
        i.spans = Spans::FixedBoth;
        let fixed = check_busbar(&i).unwrap();
        i.spans = Spans::TwoSpan;
        let two = check_busbar(&i).unwrap();
        // Clamped ends: M = F'l²/12 (2/3 of the simple moment).
        close(fixed.stress_pa / simple.stress_pa, 8.0 / 12.0, 1e-12);
        // The continuous bar has the simple bar's moment and a 2.5 times
        // larger middle reaction.
        close(two.stress_pa, simple.stress_pa, 1e-12);
        close(two.support_force_n / simple.support_force_n, 2.5, 1e-12);
    }

    #[test]
    fn a_dynamic_factor_scales_stress_and_support_force() {
        let mut i = BusbarInput::typical();
        let base = check_busbar(&i).unwrap();
        i.dynamic_factor = 2.0;
        let dynamic = check_busbar(&i).unwrap();
        close(dynamic.stress_pa / base.stress_pa, 2.0, 1e-12);
        close(dynamic.support_force_n / base.support_force_n, 2.0, 1e-12);
        close(
            base.max_span_mm / dynamic.max_span_mm,
            2.0_f64.sqrt(),
            1e-12,
        );
    }

    #[test]
    fn bad_busbar_inputs_are_refused() {
        let mut i = BusbarInput::typical();
        i.ik_a = 0.0;
        assert!(check_busbar(&i).is_err());
        let mut i = BusbarInput::typical();
        i.spacing_mm = -1.0;
        assert!(check_busbar(&i).is_err());
        let mut i = BusbarInput::typical();
        i.depth_mm = f64::NAN;
        assert!(check_busbar(&i).is_err());
    }

    #[test]
    fn rectangle_inertia_hand_value() {
        // 20 × 10 mm: Iy = 20·10³/12 = 1666.7 mm⁴, Iz = 10·20³/12 = 6666.7 mm⁴.
        let s = section_from_rects(&[(0.0, 0.0, 20.0, 10.0)]);
        close(s.area, 200e-6, 1e-12);
        close(s.i_out_of_plate, 1666.67e-12, 1e-5);
        close(s.i_in_plate, 6666.67e-12, 1e-5);
    }

    #[test]
    fn top_hat_section_matches_a_hand_sum() {
        // TH35×7.5, 1 mm. Rectangles (mm): web 25×1 at z .5; walls 1×7.5 at
        // y ±13, z 3.75; lips 4×1 at y ±15.5, z 7. Area = 25+15+8 = 48 mm².
        let s = top_hat_35(7.5, 1.0);
        close(s.area, 48.0e-6, 1e-12);
        // zc = (25·.5 + 15·3.75 + 8·7)/48 = (12.5+56.25+56)/48 = 2.5990 mm.
        // Iy: web 25/12 + 25·(2.099)² = 2.083+110.15; walls 2·(7.5³/12 +
        // 7.5·(1.151)²) = 2·(35.16+9.94) = 90.2; lips 2·(4/12 + 4·4.401²)
        // = 2·(0.333+77.47) = 155.6. Sum ≈ 358.0 mm⁴.
        close(s.i_out_of_plate, 358.0e-12, 5e-3);
        // Iz: web 1·25³/12 = 1302.1; walls 2·(7.5·1/12 + 7.5·169) = 2535.6;
        // lips 2·(1·64/12 + 4·240.25) = 1932.7. Sum = 5770.4 mm⁴.
        close(s.i_in_plate, 5770.4e-12, 1e-3);
    }

    #[test]
    fn beam_matches_textbook_maxima() {
        let ei = 200.0e9 * 1.0e-8; // 2000 N·m²
        let input = |hold, load| BeamInput {
            span_mm: 1000.0,
            hold,
            e_pa: 200.0e9,
            inertia_m4: 1.0e-8,
            loads: vec![load],
            limit_ratio: 200.0,
        };
        // Simple, uniform W: 5Wl³/(384EI) at the middle.
        let r = check_beam(&input(Hold::Simple, Load::Spread(100.0))).unwrap();
        close(r.deflection_mm, 5.0 * 100.0 / (384.0 * ei) * 1000.0, 1e-4);
        close(r.at_mm, 500.0, 1e-3);
        // Simple, centre point load P: Pl³/(48EI).
        let r = check_beam(&input(
            Hold::Simple,
            Load::Point {
                pos_mm: 500.0,
                newtons: 100.0,
            },
        ))
        .unwrap();
        close(r.deflection_mm, 100.0 / (48.0 * ei) * 1000.0, 1e-4);
        // Fixed-fixed, uniform W: Wl³/(384EI).
        let r = check_beam(&input(Hold::FixedBoth, Load::Spread(100.0))).unwrap();
        close(r.deflection_mm, 100.0 / (384.0 * ei) * 1000.0, 1e-4);
        // Fixed-fixed, centre point load: Pl³/(192EI).
        let r = check_beam(&input(
            Hold::FixedBoth,
            Load::Point {
                pos_mm: 500.0,
                newtons: 100.0,
            },
        ))
        .unwrap();
        close(r.deflection_mm, 100.0 / (192.0 * ei) * 1000.0, 1e-4);
        // Cantilever, tip load: Pl³/(3EI); uniform: Wl³/(8EI).
        let r = check_beam(&input(
            Hold::Cantilever,
            Load::Point {
                pos_mm: 1000.0,
                newtons: 100.0,
            },
        ))
        .unwrap();
        close(r.deflection_mm, 100.0 / (3.0 * ei) * 1000.0, 1e-4);
        close(r.at_mm, 1000.0, 1e-9);
        let r = check_beam(&input(Hold::Cantilever, Load::Spread(100.0))).unwrap();
        close(r.deflection_mm, 100.0 / (8.0 * ei) * 1000.0, 1e-4);
    }

    #[test]
    fn off_centre_point_load_on_a_simple_beam() {
        // P = 100 N at a = 0.3 m of l = 1 m: the maximum is
        // P·a·(l²−a²)^1.5/(9√3·l·EI), with a the shorter distance.
        let ei = 2000.0;
        let r = check_beam(&BeamInput {
            span_mm: 1000.0,
            hold: Hold::Simple,
            e_pa: 200.0e9,
            inertia_m4: 1.0e-8,
            loads: vec![Load::Point {
                pos_mm: 300.0,
                newtons: 100.0,
            }],
            limit_ratio: 200.0,
        })
        .unwrap();
        let a = 0.3_f64;
        let expect = 100.0 * a * (1.0 - a * a).powf(1.5) / (9.0 * 3.0_f64.sqrt() * ei) * 1000.0;
        close(r.deflection_mm, expect, 1e-3);
        // The maximum lies on the longer side, at √((l²−a²)/3) = 0.5508 m
        // from the far support, so 449.2 mm from the left.
        close(r.at_mm, 449.2, 2e-3);
    }

    #[test]
    fn beam_loads_add_and_the_limit_decides() {
        let mut b = BeamInput {
            span_mm: 1000.0,
            hold: Hold::Simple,
            e_pa: 200.0e9,
            inertia_m4: 1.0e-8,
            loads: vec![Load::Spread(100.0)],
            limit_ratio: 200.0,
        };
        let one = check_beam(&b).unwrap();
        b.loads.push(Load::Spread(100.0));
        let two = check_beam(&b).unwrap();
        close(two.deflection_mm / one.deflection_mm, 2.0, 1e-9);
        assert_eq!(one.limit_mm, 5.0);
        b.loads = vec![Load::Spread(1.0e5)];
        assert_eq!(check_beam(&b).unwrap().verdict, Verdict::Fail);
    }

    #[test]
    fn bad_beam_inputs_are_refused() {
        let ok = BeamInput {
            span_mm: 1000.0,
            hold: Hold::Simple,
            e_pa: 200.0e9,
            inertia_m4: 1.0e-8,
            loads: vec![Load::Spread(10.0)],
            limit_ratio: 200.0,
        };
        let mut b = ok.clone();
        b.loads.clear();
        assert!(check_beam(&b).is_err());
        let mut b = ok.clone();
        b.loads = vec![Load::Point {
            pos_mm: 1500.0,
            newtons: 1.0,
        }];
        assert!(check_beam(&b).is_err());
        let mut b = ok;
        b.inertia_m4 = 0.0;
        assert!(check_beam(&b).is_err());
    }

    #[test]
    fn a_hot_rail_is_stiffer_in_the_plate_plane() {
        let s = top_hat_35(7.5, 1.0);
        assert!(s.i_in_plate > 10.0 * s.i_out_of_plate);
    }

    #[test]
    fn plate_matches_timoshenko_coefficients() {
        // Square plate, simply supported, ν = 0.3 (Timoshenko, Theory of
        // Plates and Shells): uniform load 0.00406·q·a⁴/D, centre force
        // 0.01160·P·a²/D.
        let a = 0.5;
        let d = plate_stiffness(200.0e9, 0.002, 0.3);
        let q = 1000.0;
        close(
            plate_centre_deflection(a, a, d, q, 0.0),
            0.00406 * q * a.powi(4) / d,
            2e-3,
        );
        let p = 200.0;
        close(
            plate_centre_deflection(a, a, d, 0.0, p),
            0.01160 * p * a * a / d,
            2e-3,
        );
    }

    #[test]
    fn plate_check_uses_the_shorter_side_for_the_limit() {
        let r = check_plate(&PlateInput {
            a_mm: 600.0,
            b_mm: 400.0,
            thickness_mm: 2.0,
            e_pa: 210.0e9,
            poisson: 0.3,
            spread_n: 300.0,
            centre_n: 0.0,
            limit_ratio: 200.0,
        })
        .unwrap();
        assert_eq!(r.limit_mm, 2.0);
        assert!(r.deflection_mm > 0.0);
        // Doubling the thickness divides the deflection by eight.
        let thick = check_plate(&PlateInput {
            a_mm: 600.0,
            b_mm: 400.0,
            thickness_mm: 4.0,
            e_pa: 210.0e9,
            poisson: 0.3,
            spread_n: 300.0,
            centre_n: 0.0,
            limit_ratio: 200.0,
        })
        .unwrap();
        close(r.deflection_mm / thick.deflection_mm, 8.0, 1e-9);
    }

    #[test]
    fn bad_plate_inputs_are_refused() {
        let ok = PlateInput {
            a_mm: 600.0,
            b_mm: 400.0,
            thickness_mm: 2.0,
            e_pa: 210.0e9,
            poisson: 0.3,
            spread_n: 300.0,
            centre_n: 0.0,
            limit_ratio: 200.0,
        };
        let mut p = ok;
        p.poisson = 0.7;
        assert!(check_plate(&p).is_err());
        let mut p = ok;
        p.thickness_mm = 0.0;
        assert!(check_plate(&p).is_err());
        let mut p = ok;
        p.spread_n = -1.0;
        assert!(check_plate(&p).is_err());
    }
}
