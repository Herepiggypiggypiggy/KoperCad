//! Forced-convection building blocks for device surfaces (lane H8).
//!
//! Plain functions of air speed, size and temperature, no grid. Each one
//! says which part is a textbook correlation (exact within its stated range)
//! and which part is a planning model; the Heat panel repeats that behind its
//! "?" icon.
//!
//! - [`plate_nusselt`]: flat plate, laminar (Blasius) and turbulent, joined
//!   within 1.2 % at the transition. Textbook.
//! - [`plate_nusselt_with_turbulence`]: free-stream or wake turbulence lifts
//!   the laminar value by up to 50 %. **Model**: the clean end is exact, the
//!   lift is a planning size, not measured.
//! - [`wake_turbulence`]: turbulence level behind a blocking part. **Model**:
//!   the decay exponent of a plane wake (-1/2) is theory, the starting level
//!   is a planning value.
//! - [`jet_nusselt`]: round jet blowing straight at a surface, Martin's
//!   correlation as printed in Incropera. Textbook inside its range
//!   (`2000 <= Re <= 400000`, `2.5 <= r/D <= 7.5`, `2 <= H/D <= 12`).
//! - [`Heatsink`]: parallel fins in a channel: Elenbaas natural convection,
//!   fully developed laminar forced flow with a developing-flow term, fin
//!   efficiency. Elenbaas and fin efficiency are textbook; the developing
//!   term and the way natural and forced are blended are **model**.
//! - [`Mixed`]: how natural and forced film coefficients combine.

/// Prandtl number of air near room temperature.
pub const PR: f64 = 0.71;
/// Conductivity of air, W/mK.
pub const K_AIR: f64 = 0.026;

/// Air properties at film temperature `tf_k` (kelvin): kinematic viscosity
/// and thermal diffusivity, m2/s. Same scaling the 2D engine always used.
pub fn air_nu_alpha(tf_k: f64) -> (f64, f64) {
    let scale = (tf_k / 293.15).powf(1.7);
    (1.5e-5 * scale, 2.1e-5 * scale)
}

/// Reynolds number above which a flat plate boundary layer is taken as
/// turbulent (leading edge start, no trip).
pub const RE_TRANSITION: f64 = 5.0e5;

/// Average Nusselt number of an isothermal flat plate of length `L`.
/// Laminar: `0.664 Re^1/2 Pr^1/3` (Blasius). Mixed above `Re = 5e5`:
/// `(0.037 Re^4/5 - 871) Pr^1/3`, which meets the laminar value at the
/// transition within 1.2 %.
pub fn plate_nusselt(re: f64) -> f64 {
    let re = re.max(0.0);
    let pr13 = PR.powf(1.0 / 3.0);
    if re < RE_TRANSITION {
        0.664 * re.sqrt() * pr13
    } else {
        (0.037 * re.powf(0.8) - 871.0) * pr13
    }
}

/// Average Nusselt number on a plate that is fully turbulent from the
/// leading edge: `0.037 Re^4/5 Pr^1/3`. The upper limit of any turbulence
/// enhancement.
pub fn plate_nusselt_turbulent(re: f64) -> f64 {
    0.037 * re.max(0.0).powf(0.8) * PR.powf(1.0 / 3.0)
}

/// Most the free-stream turbulence is allowed to lift the clean plate value
/// in this model (at `tu >= TU_FULL`), as a factor.
pub const TU_MAX_GAIN: f64 = 1.5;
/// Turbulence intensity at which that cap is reached.
pub const TU_FULL: f64 = 0.25;

/// Flat plate Nusselt number with free-stream turbulence `tu` (0 to 1, the
/// rms of the fluctuation over the mean speed). It rises linearly from the
/// clean plate value at `tu = 0` to `TU_MAX_GAIN` times it at `tu = TU_FULL`,
/// and the fully turbulent plate value is a floor where that is higher (high
/// Re). Planning model: the size of the lift is of the order reported for
/// turbulence levels of 5 to 25 % (tens of percent), not fitted to a case; the
/// clean end is exact (Blasius).
pub fn plate_nusselt_with_turbulence(re: f64, tu: f64) -> f64 {
    let clean = plate_nusselt(re);
    let w = (tu / TU_FULL).clamp(0.0, 1.0);
    let lifted = clean * (1.0 + (TU_MAX_GAIN - 1.0) * w);
    lifted.max(clean + (plate_nusselt_turbulent(re) - clean).max(0.0) * w)
}

/// Turbulence intensity a distance `x` downstream of a blocking part of
/// width `d` (same unit as `x`), in a stream of intensity `tu_free`.
///
/// Model: `tu = tu0 (x/d)^-1/2` added in quadrature to the free-stream level,
/// with `tu0 = 0.35` at one width downstream. The exponent is the similarity
/// result for the centre line of a plane wake; the starting level is a
/// planning value, not measured. Capped at 0.5.
pub fn wake_turbulence(tu_free: f64, x: f64, d: f64) -> f64 {
    if d.is_nan() || x.is_nan() || d <= 0.0 || x <= 0.0 {
        return tu_free.clamp(0.0, 0.5);
    }
    let tu_w = 0.35 * (x / d).max(1.0).powf(-0.5);
    tu_free.hypot(tu_w).min(0.5)
}

/// Average Nusselt number (on the nozzle diameter `D`) over a circle of
/// radius `r` around the stagnation point of a single round air jet at
/// nozzle distance `H`: Martin's correlation as given by Incropera,
///
/// `Nu = Pr^0.42 G F`, `G = (D/r)(1 - 1.1 D/r) / (1 + 0.1 (H/D - 6) D/r)`,
/// `F = 2 Re^1/2 (1 + 0.005 Re^0.55)^1/2`.
///
/// Returns `None` outside the range the correlation was fitted on.
pub fn jet_nusselt(re: f64, r_over_d: f64, h_over_d: f64) -> Option<f64> {
    if !((2000.0..=400_000.0).contains(&re)
        && (2.5..=7.5).contains(&r_over_d)
        && (2.0..=12.0).contains(&h_over_d))
    {
        return None;
    }
    let dr = 1.0 / r_over_d;
    let g = dr * (1.0 - 1.1 * dr) / (1.0 + 0.1 * (h_over_d - 6.0) * dr);
    let f = 2.0 * re.sqrt() * (1.0 + 0.005 * re.powf(0.55)).sqrt();
    Some(PR.powf(0.42) * g * f)
}

/// Film coefficient, W/m2K, under a round jet of diameter `d_m` and speed
/// `speed` at the nozzle, on a disc of radius `r_m`, nozzle distance `h_m`
/// (air at film temperature `tf_k`). `None` outside the correlation's range.
pub fn jet_h(speed: f64, d_m: f64, r_m: f64, h_m: f64, tf_k: f64) -> Option<f64> {
    let (nu, _) = air_nu_alpha(tf_k);
    let re = speed * d_m / nu;
    jet_nusselt(re, r_m / d_m, h_m / d_m).map(|n| n * K_AIR / d_m)
}

/// Decay constant of a round jet: after the core `u_c = B u0 d / (x + x_v)`.
/// Literature value for the free round jet (`B = 5.8`); the spreading rate
/// of the half-width is `S = 0.094` (Wygnanski and Fiedler type fits).
pub const JET_DECAY_B: f64 = 5.8;
pub const JET_SPREAD_S: f64 = 0.094;
/// Length, in diameters, over which the jet of a cabinet axial fan keeps its
/// exit speed on the axis. A clean nozzle keeps it for 5.8 diameters; an axial
/// fan's discharge (swirl, hub wake, blade turbulence) mixes sooner. The 2 is
/// a planning value, not measured. The virtual origin of the decay moves with
/// it so the curve stays continuous.
pub const JET_CORE_D: f64 = 2.0;

/// Speed in a round jet from a fan or nozzle of diameter `d` and exit speed
/// `u0`, at axial distance `x` and radius `r` from the axis (same unit as
/// `d`). Gaussian profile with the half-width `S (x + x_v)`; on the axis it
/// keeps `u0` up to `x = JET_CORE_D d` and falls as `B u0 d / (x + x_v)`
/// after, with `x_v = (B - JET_CORE_D) d`. Beyond the core the momentum of
/// the profile is the same everywhere (within 14 % of the exit momentum with
/// these constants). Free jet only: walls, parts in the way and a surface the
/// jet is blown along are not in it (model for a cabinet).
pub fn round_jet_speed(u0: f64, d: f64, x: f64, r: f64) -> f64 {
    if d.is_nan() || x.is_nan() || d <= 0.0 || x <= 0.0 {
        return 0.0;
    }
    let xv = (JET_DECAY_B - JET_CORE_D) * d;
    let uc = if x <= JET_CORE_D * d {
        u0
    } else {
        u0 * JET_DECAY_B * d / (x + xv)
    };
    let half = (JET_SPREAD_S * (x + xv)).max(0.5 * d);
    uc * (-std::f64::consts::LN_2 * (r / half).powi(2)).exp()
}

/// How natural and forced convection add up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mixed {
    /// Exponent `n` of `h = (h_nat^n + h_forced^n)^(1/n)`. 3 is the classic
    /// choice for a vertical plate (Churchill); the value 3 is what the
    /// engine has always used.
    pub n: f64,
}

impl Default for Mixed {
    fn default() -> Self {
        Mixed { n: 3.0 }
    }
}

impl Mixed {
    pub fn combine(&self, h_nat: f64, h_forced: f64) -> f64 {
        (h_nat.powf(self.n) + h_forced.powf(self.n)).powf(1.0 / self.n)
    }
}

/// Fin efficiency of a straight fin of thickness `t_m` and length `l_m` with
/// film coefficient `h` and conductivity `k` (tip adiabatic):
/// `tanh(mL)/(mL)`, `m = sqrt(2h/(k t))`. Exact for that fin.
pub fn fin_efficiency(h: f64, k: f64, t_m: f64, l_m: f64) -> f64 {
    let m = (2.0 * h / (k * t_m)).sqrt();
    let ml = m * l_m;
    if ml < 1e-9 { 1.0 } else { ml.tanh() / ml }
}

/// Elenbaas average Nusselt number of isothermal parallel plates, on the
/// plate spacing `S`: `Nu = (Ra'/24) [1 - exp(-35/Ra')]^(3/4)`,
/// `Ra' = Ra_S S/L`. Tends to `Ra'/24` (fully developed channel) for small
/// `Ra'` and to `0.60 Ra'^1/4` (single plate) for large.
pub fn elenbaas_nusselt(ra_prime: f64) -> f64 {
    let ra = ra_prime.max(1e-12);
    ra / 24.0 * (1.0 - (-35.0 / ra).exp()).powf(0.75)
}

/// A heatsink: parallel fins on a base, air flowing along the fins.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heatsink {
    /// Number of fins.
    pub fins: u32,
    /// Fin height (out of the base), mm.
    pub fin_height_mm: f64,
    /// Fin thickness, mm.
    pub fin_thickness_mm: f64,
    /// Gap between two fins, mm.
    pub gap_mm: f64,
    /// Length along the flow, mm.
    pub length_mm: f64,
    /// Fin material conductivity, W/mK (aluminium about 200).
    pub k_w_mk: f64,
}

/// What a heatsink does at one operating point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatsinkFilm {
    /// Coefficient on the fin and base surfaces, W/m2K.
    pub h: f64,
    /// Fin efficiency at that coefficient.
    pub efficiency: f64,
    /// Heat transfer area over the bare base area, with the fins weighted by
    /// their efficiency: multiply the bare-base film by this.
    pub area_gain: f64,
}

impl Heatsink {
    /// Base width covered by the fins, mm.
    pub fn width_mm(&self) -> f64 {
        let n = self.fins as f64;
        n * self.fin_thickness_mm + (n - 1.0).max(0.0) * self.gap_mm
    }

    /// Film of the heatsink with channel air speed `speed` (m/s, along the
    /// fins), base temperature `t_base_c` and air `t_air_c`.
    ///
    /// Forced: channel of hydraulic diameter `2 S`, fully developed laminar
    /// `Nu = 7.54` plus a developing-flow term of the Hausen form,
    /// `0.03 Gz / (1 + 0.016 Gz^2/3)`, `Gz = Re Pr D_h / L` (the constants of
    /// the tube case; **model** for plates). Natural: Elenbaas. Joined with
    /// the cube blend.
    pub fn film(&self, speed: f64, t_base_c: f64, t_air_c: f64) -> HeatsinkFilm {
        let s = self.gap_mm / 1000.0;
        let l = (self.length_mm / 1000.0).max(1e-3);
        let hf = (self.fin_height_mm / 1000.0).max(0.0);
        let t = (self.fin_thickness_mm / 1000.0).max(1e-4);
        let n = self.fins as f64;
        let dt = (t_base_c - t_air_c).abs().max(0.5);
        let tf = 0.5 * (t_base_c + t_air_c) + 273.15;
        let (nu, alpha) = air_nu_alpha(tf);
        let ra_s = 9.81 / tf * dt * s.powi(3) / (nu * alpha);
        let nu_nat = elenbaas_nusselt(ra_s * s / l);
        let h_nat = nu_nat * K_AIR / s;
        let dh = 2.0 * s;
        let re = speed.max(0.0) * dh / nu;
        let gz = re * PR * dh / l;
        let nu_forced = 7.54 + 0.03 * gz / (1.0 + 0.016 * gz.powf(2.0 / 3.0));
        let h_forced = if speed > 0.0 {
            nu_forced * K_AIR / dh
        } else {
            0.0
        };
        let h = Mixed::default().combine(h_nat, h_forced);
        let eff = fin_efficiency(h, self.k_w_mk, t, hf);
        let w = self.width_mm() / 1000.0;
        let base = (w * l).max(1e-9);
        // Base between the fins plus fin sides (tip counted too).
        let a_base = (n - 1.0).max(0.0) * s * l;
        let a_fin = n * (2.0 * hf * l + t * l);
        let area_gain = (a_base + eff * a_fin) / base;
        HeatsinkFilm {
            h,
            efficiency: eff,
            area_gain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs().max(1e-12)
    }

    #[test]
    fn blasius_value_at_re_100000() {
        // 0.664 sqrt(1e5) 0.71^(1/3) = 0.664 * 316.228 * 0.89221 = 187.3
        assert!(close(plate_nusselt(1.0e5), 187.3, 0.002));
    }

    #[test]
    fn the_plate_correlation_has_no_jump_at_transition() {
        let below = plate_nusselt(RE_TRANSITION - 1.0);
        let above = plate_nusselt(RE_TRANSITION + 1.0);
        // 0.664 sqrt(5e5) Pr^1/3 = 418.9 against (0.037 (5e5)^0.8 - 871) Pr^1/3 = 423.7: the
        // textbook mixed form joins the laminar one within about 1.2 %.
        assert!(close(below, above, 0.02), "{below} vs {above}");
    }

    #[test]
    fn turbulence_lifts_the_clean_value_by_a_bounded_amount() {
        for re in [5.0e3, 8.0e4, 3.0e5] {
            let clean = plate_nusselt(re);
            assert_eq!(plate_nusselt_with_turbulence(re, 0.0), clean);
            let mid = plate_nusselt_with_turbulence(re, 0.1);
            let full = plate_nusselt_with_turbulence(re, TU_FULL);
            assert!(mid > clean && full >= mid, "re {re}");
            assert!(full <= clean * TU_MAX_GAIN + (plate_nusselt_turbulent(re) - clean).max(0.0));
            assert_eq!(plate_nusselt_with_turbulence(re, 0.9), full);
        }
        // At high Re the fully turbulent plate is the floor.
        let re = 4.0e5;
        assert!(plate_nusselt_with_turbulence(re, TU_FULL) >= plate_nusselt_turbulent(re));
    }

    #[test]
    fn a_wake_decays_with_the_plane_wake_exponent_and_not_below_free_stream() {
        let a = wake_turbulence(0.0, 4.0, 1.0);
        let b = wake_turbulence(0.0, 16.0, 1.0);
        // x x4 -> intensity /2
        assert!(close(a / b, 2.0, 1e-9));
        assert!(wake_turbulence(0.05, 1000.0, 1.0) >= 0.05);
        assert_eq!(wake_turbulence(0.03, 5.0, 0.0), 0.03);
    }

    #[test]
    fn martin_jet_hand_calculation() {
        // Re 20000, r/D 5, H/D 6: G = 0.2 (1 - 0.22) = 0.156,
        // F = 2 sqrt(20000) sqrt(1 + 0.005 * 20000^0.55) = 415.8,
        // Pr^0.42 = 0.8660 -> Nu = 56.2
        let nu = jet_nusselt(20_000.0, 5.0, 6.0).unwrap();
        assert!(close(nu, 56.2, 0.01), "{nu}");
        assert!(jet_nusselt(500.0, 5.0, 6.0).is_none());
        assert!(jet_nusselt(20_000.0, 1.0, 6.0).is_none());
    }

    #[test]
    fn a_stronger_jet_cools_better_and_a_farther_one_less_at_the_edge() {
        let a = jet_h(5.0, 0.04, 0.12, 0.2, 310.0).unwrap();
        let b = jet_h(10.0, 0.04, 0.12, 0.2, 310.0).unwrap();
        assert!(b > a);
    }

    #[test]
    fn the_free_jet_keeps_its_momentum_to_within_the_fit() {
        // Momentum flux of the nozzle: (pi/4) u0^2 d^2. Beyond the core the Gaussian profile
        // carries 0.857 of it with the literature constants; allow 20 %.
        let (u0, d) = (4.0, 0.1);
        let nozzle = std::f64::consts::FRAC_PI_4 * u0 * u0 * d * d;
        for x in [0.5, 1.0, 2.0, 4.0] {
            let n = 4000;
            let rmax = 3.0 * (x + 0.4);
            let mut m = 0.0;
            for k in 0..n {
                let r = (k as f64 + 0.5) / n as f64 * rmax;
                let u = round_jet_speed(u0, d, x, r);
                m += u * u * 2.0 * std::f64::consts::PI * r * (rmax / n as f64);
            }
            assert!((m / nozzle - 1.0).abs() < 0.2, "x {x}: {}", m / nozzle);
        }
    }

    #[test]
    fn the_jet_axis_speed_follows_the_decay_law() {
        let (u0, d) = (3.0, 0.08);
        assert_eq!(round_jet_speed(u0, d, 1.5 * d, 0.0), u0);
        // continuous at the end of the core
        let at_core = round_jet_speed(u0, d, JET_CORE_D * d, 0.0);
        let after = round_jet_speed(u0, d, JET_CORE_D * d + 1e-9, 0.0);
        assert!(close(at_core, after, 1e-6));
        // x = 10 d: 5.8 / (10 + 3.8) = 0.4203
        assert!(close(
            round_jet_speed(u0, d, 10.0 * d, 0.0),
            5.8 / 13.8 * u0,
            1e-9
        ));
        // half speed at the half-width
        let x = 10.0 * d;
        let on = round_jet_speed(u0, d, x, 0.0);
        let half = round_jet_speed(u0, d, x, JET_SPREAD_S * (x + 3.8 * d));
        assert!(close(half / on, 0.5, 1e-9));
    }

    #[test]
    fn fin_efficiency_limits() {
        assert!(close(fin_efficiency(1e-6, 200.0, 0.002, 0.03), 1.0, 1e-6));
        // m L = 1 -> tanh(1) = 0.76159
        let h = 200.0 * 0.002 / 2.0 / (0.03 * 0.03); // m = 1/L -> m^2 = 2h/(k t)
        assert!(close(fin_efficiency(h, 200.0, 0.002, 0.03), 0.76159, 1e-4));
    }

    #[test]
    fn elenbaas_tends_to_both_known_limits() {
        // small Ra': Ra'/24
        assert!(close(elenbaas_nusselt(0.5), 0.5 / 24.0, 0.01));
        // large Ra': 35^0.75/24 Ra'^0.25 = 0.599 Ra'^0.25
        let ra = 1.0e7;
        assert!(close(
            elenbaas_nusselt(ra),
            35f64.powf(0.75) / 24.0 * ra.powf(0.25),
            0.01
        ));
    }

    fn sink() -> Heatsink {
        Heatsink {
            fins: 12,
            fin_height_mm: 30.0,
            fin_thickness_mm: 2.0,
            gap_mm: 6.0,
            length_mm: 100.0,
            k_w_mk: 200.0,
        }
    }

    #[test]
    fn a_heatsink_gains_area_and_forced_air_raises_its_film() {
        let still = sink().film(0.0, 70.0, 40.0);
        let blown = sink().film(2.0, 70.0, 40.0);
        assert!(blown.h > 2.0 * still.h, "{} vs {}", blown.h, still.h);
        assert!(still.area_gain > 3.0, "{}", still.area_gain);
        assert!(blown.efficiency < still.efficiency);
        assert!(blown.efficiency > 0.5 && blown.efficiency <= 1.0);
    }

    #[test]
    fn heatsink_forced_film_is_the_fully_developed_value_for_a_long_channel() {
        let mut s = sink();
        s.length_mm = 5000.0;
        let f = s.film(0.5, 50.0, 40.0);
        // Nu 7.54 on D_h = 12 mm -> h = 7.54 * 0.026 / 0.012 = 16.3; natural is small in
        // a 5 m channel, the cube blend adds little.
        assert!(close(f.h, 16.3, 0.05), "{}", f.h);
    }
}
