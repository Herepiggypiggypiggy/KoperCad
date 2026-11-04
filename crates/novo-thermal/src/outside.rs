//! Heat transfer on the OUTSIDE of an enclosure (lane H3, plan
//! `handoff/heat-research/ROADMAP.md`).
//!
//! Pure correlations, no dependency on the solver or on the shell data
//! contract (lane H1). The wiring into the solver waits for that contract.
//! What it answers: given the heat that reaches a wall's outer surface from
//! inside, how warm does that surface get, how much leaves by convection and
//! radiation, and what do sun, wind and mounting (a wall behind the cabinet,
//! a neighbour beside it) change.
//!
//! Status of every number: MODEL. Churchill-Chu (vertical), McAdams or
//! Lloyd-Moran (horizontal), flat plate forced convection, Churchill's
//! cubic blend for mixed convection, an Elenbaas channel for a gap. They are
//! textbook correlations with 10 to 25 percent scatter; the tests check
//! them against hand calculations, limits and energy balance, not against a
//! measured cabinet. IEC 60890 stays the sealed-box reference: its k is an
//! air-to-air overall value, so never add these films on top of it.
//!
//! Units: metres, kelvin or degrees C as named, watts.

const G: f64 = 9.81;
/// Stefan-Boltzmann constant, W/(m2 K4).
pub const SIGMA: f64 = 5.670_374e-8;

fn k_of(c: f64) -> f64 {
    c + 273.15
}

/// Air properties at one temperature (1 atm, ideal gas).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Air {
    /// Thermal conductivity, W/(m K).
    pub k: f64,
    /// Kinematic viscosity, m2/s.
    pub nu: f64,
    /// Thermal diffusivity, m2/s.
    pub alpha: f64,
    /// Prandtl number.
    pub pr: f64,
    /// Volumetric expansion 1/T, 1/K.
    pub beta: f64,
}

/// Air at film temperature `t_c` (degrees C, valid about -20 to 120).
/// Sutherland viscosity, linear conductivity, cp 1007 J/(kg K).
pub fn air(t_c: f64) -> Air {
    let t = k_of(t_c);
    let rho = 101_325.0 / (287.05 * t);
    let mu = 1.716e-5 * (t / 273.15).powf(1.5) * (273.15 + 110.4) / (t + 110.4);
    let k = 0.0241 + 7.6e-5 * (t - 273.15);
    let cp = 1007.0;
    let nu = mu / rho;
    let alpha = k / (rho * cp);
    Air {
        k,
        nu,
        alpha,
        pr: nu / alpha,
        beta: 1.0 / t,
    }
}

/// Rayleigh number for surface `ts_c`, air `ta_c`, length `l_m`.
pub fn rayleigh(ts_c: f64, ta_c: f64, l_m: f64) -> f64 {
    let a = air(0.5 * (ts_c + ta_c));
    G * a.beta * (ts_c - ta_c).abs() * l_m.powi(3) / (a.nu * a.alpha)
}

/// Orientation of an outer face for natural convection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    /// Front, back, sides.
    Vertical,
    /// Roof (faces up).
    Up,
    /// Underside (faces down).
    Down,
}

/// Natural convection coefficient, W/(m2 K), of a face of size `w` by `h`
/// metres at surface temperature `ts_c` in air at `ta_c`.
///
/// Vertical: Churchill-Chu over the full Rayleigh range, length = height.
/// Horizontal: McAdams and Lloyd-Moran, length = area/perimeter; a hot face
/// that looks up (or a cold one that looks down) uses the strong branch.
pub fn h_natural(facing: Facing, w: f64, h: f64, ts_c: f64, ta_c: f64) -> f64 {
    let dt = ts_c - ta_c;
    let a = air(0.5 * (ts_c + ta_c));
    match facing {
        Facing::Vertical => {
            let ra = rayleigh(ts_c, ta_c, h);
            let nu = (0.825
                + 0.387 * ra.powf(1.0 / 6.0)
                    / (1.0 + (0.492 / a.pr).powf(9.0 / 16.0)).powf(8.0 / 27.0))
            .powi(2);
            nu * a.k / h
        }
        Facing::Up | Facing::Down => {
            let l = w * h / (2.0 * (w + h));
            let ra = rayleigh(ts_c, ta_c, l);
            let strong = (facing == Facing::Up) == (dt >= 0.0);
            let nu = if strong {
                if ra < 1.0e7 {
                    0.54 * ra.powf(0.25)
                } else {
                    0.15 * ra.powf(1.0 / 3.0)
                }
            } else {
                0.27 * ra.powf(0.25)
            };
            nu * a.k / l
        }
    }
}

/// Forced convection over a flat plate of length `l` along the flow, wind
/// speed `v` (m/s): laminar 0.664 Re^0.5 Pr^1/3, mixed laminar-turbulent
/// (0.037 Re^0.8 - 871) Pr^1/3 above Re 5e5.
pub fn h_forced(l: f64, v: f64, ts_c: f64, ta_c: f64) -> f64 {
    if v <= 0.0 {
        return 0.0;
    }
    let a = air(0.5 * (ts_c + ta_c));
    let re = v * l / a.nu;
    let nu = if re < 5.0e5 {
        0.664 * re.sqrt() * a.pr.cbrt()
    } else {
        (0.037 * re.powf(0.8) - 871.0) * a.pr.cbrt()
    };
    nu * a.k / l
}

/// Mixed convection: Churchill's cubic blend of natural and forced.
pub fn h_mixed(h_nat: f64, h_for: f64) -> f64 {
    (h_nat.powi(3) + h_for.powi(3)).cbrt()
}

/// Linearised radiation coefficient from a surface at `ts_c` to surroundings
/// at `tsur_c`, emissivity `eps` (view factor 1 to the surroundings).
pub fn h_rad(eps: f64, ts_c: f64, tsur_c: f64) -> f64 {
    let (a, b) = (k_of(ts_c), k_of(tsur_c));
    eps * SIGMA * (a * a + b * b) * (a + b)
}

/// What a face looks at on the outside.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Surroundings {
    /// Open room or open air: convection to `ambient_c`, radiation to
    /// `radiant_c` (room walls, or the sky for an outdoor roof).
    Open { radiant_c: f64 },
    /// A parallel surface at `gap_m` (wall behind the cabinet, a neighbour in
    /// a row). `gap_m` of 0 means touching: no heat leaves the face. The
    /// other surface is taken at ambient with emissivity `other_eps`.
    Parallel { gap_m: f64, other_eps: f64 },
}

/// Natural convection in a vertical channel with one heated wall (Elenbaas,
/// Bar-Cohen and Rohsenow form for isothermal plates, used for one heated
/// side), capped by the isolated plate value: a wide gap is no worse than
/// no wall. For a narrow gap the heated wall's coefficient drops.
pub fn h_gap(h_m: f64, gap_m: f64, ts_c: f64, ta_c: f64) -> f64 {
    let free = h_natural(Facing::Vertical, 1.0, h_m, ts_c, ta_c);
    if gap_m <= 1.0e-4 {
        return 0.0;
    }
    let a = air(0.5 * (ts_c + ta_c));
    let ra_s = G * a.beta * (ts_c - ta_c).abs() * gap_m.powi(3) / (a.nu * a.alpha) * (gap_m / h_m);
    if ra_s <= 0.0 {
        return 0.0;
    }
    let nu = ra_s / 24.0 * (1.0 - (-35.0 / ra_s).exp()).powf(0.75);
    (nu * a.k / gap_m).min(free)
}

/// Sun on a face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    /// Irradiance on the face plane, W/m2 (already includes angle and
    /// shading; roof at noon in summer about 900, a sunlit wall 300 to 700).
    pub irradiance_w_m2: f64,
    /// Solar absorptance of the paint, 0..1 (white 0.25, RAL 7035 grey 0.6,
    /// dark 0.9, bare galvanised 0.5, stainless 0.4).
    pub absorptance: f64,
}

/// One outer face for the surface balance.
#[derive(Debug, Clone, Copy)]
pub struct OuterFace {
    pub facing: Facing,
    /// Face width and height, m.
    pub w_m: f64,
    pub h_m: f64,
    /// Long-wave emissivity of the outer paint.
    pub eps: f64,
    pub ambient_c: f64,
    pub surroundings: Surroundings,
    /// Wind or room air speed along the face, m/s.
    pub wind_m_s: f64,
    pub sun: Option<Sun>,
}

/// Result of the outer surface balance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OuterResult {
    /// Outer surface temperature, degrees C.
    pub ts_c: f64,
    pub h_conv: f64,
    pub h_rad: f64,
    /// Heat to the outside per m2, W/m2 (convection + radiation).
    pub q_out_w_m2: f64,
    /// Heat in from the inside per m2, W/m2.
    pub q_in_w_m2: f64,
    /// Absorbed solar heat per m2, W/m2.
    pub q_sun_w_m2: f64,
}

/// Coefficients of the outer film at a given surface temperature.
pub fn outer_film(f: &OuterFace, ts_c: f64) -> (f64, f64) {
    match f.surroundings {
        Surroundings::Open { radiant_c } => {
            let hn = h_natural(f.facing, f.w_m, f.h_m, ts_c, f.ambient_c);
            let l = if f.facing == Facing::Vertical {
                f.w_m
            } else {
                f.w_m.max(f.h_m)
            };
            let hf = h_forced(l, f.wind_m_s, ts_c, f.ambient_c);
            (h_mixed(hn, hf), h_rad(f.eps, ts_c, radiant_c))
        }
        Surroundings::Parallel { gap_m, other_eps } => {
            if gap_m <= 1.0e-4 {
                return (0.0, 0.0);
            }
            let e = 1.0 / (1.0 / f.eps + 1.0 / other_eps - 1.0);
            (
                h_gap(f.h_m, gap_m, ts_c, f.ambient_c),
                h_rad(e, ts_c, f.ambient_c),
            )
        }
    }
}

/// Solves the outer surface temperature of a wall that receives heat from
/// the inside through conductance `u_in` (W/(m2 K), inner air to outer
/// surface, wall layers and inner film included) with inner air at `ti_c`.
///
/// Balance per m2: `u_in (Ti - Ts) + sun = (h_c + h_r)(Ts - Ta)` with the
/// film coefficients taken at `Ts`. Bisection, always converges.
pub fn solve_outer_surface(f: &OuterFace, u_in: f64, ti_c: f64) -> OuterResult {
    let sun = f.sun.map_or(0.0, |s| s.absorptance * s.irradiance_w_m2);
    let net = |ts: f64| {
        let (hc, hr) = outer_film(f, ts);
        u_in * (ti_c - ts) + sun - (hc + hr) * (ts - f.ambient_c)
    };
    let mut lo = f.ambient_c.min(ti_c) - 60.0;
    let mut hi = f.ambient_c.max(ti_c) + 150.0;
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if net(mid) > 0.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let ts = 0.5 * (lo + hi);
    let (hc, hr) = outer_film(f, ts);
    OuterResult {
        ts_c: ts,
        h_conv: hc,
        h_rad: hr,
        q_out_w_m2: (hc + hr) * (ts - f.ambient_c),
        q_in_w_m2: u_in * (ti_c - ts),
        q_sun_w_m2: sun,
    }
}

/// Overall air-to-air conductance seen from the inside, W/(m2 K), for the
/// same face without sun: `q_in / (Ti - Ta)`. Compare with the IEC 60890 k
/// (about 5.5 W/(m2 K) for sheet steel, which includes both films).
pub fn overall_u(f: &OuterFace, u_in: f64, ti_c: f64) -> f64 {
    let mut g = *f;
    g.sun = None;
    let r = solve_outer_surface(&g, u_in, ti_c);
    r.q_in_w_m2 / (ti_c - f.ambient_c)
}

/// Sky temperature for an outdoor roof or wall seen against the sky, degrees C
/// (Swinbank: Tsky = 0.0552 Ta^1.5, kelvin; clear sky).
pub fn sky_temperature_c(ambient_c: f64) -> f64 {
    0.0552 * k_of(ambient_c).powf(1.5) - 273.15
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs()
    }

    fn face(facing: Facing) -> OuterFace {
        OuterFace {
            facing,
            w_m: 0.8,
            h_m: 2.0,
            eps: 0.9,
            ambient_c: 25.0,
            surroundings: Surroundings::Open { radiant_c: 25.0 },
            wind_m_s: 0.0,
            sun: None,
        }
    }

    #[test]
    fn air_at_300k_matches_tables() {
        let a = air(26.85);
        assert!(near(a.k, 0.0263, 0.02));
        assert!(near(a.nu, 1.57e-5, 0.03));
        assert!(near(a.pr, 0.707, 0.03));
        assert!(near(a.alpha, 2.22e-5, 0.04));
    }

    #[test]
    fn vertical_plate_hand_calculation() {
        // 1 m plate, 50 C in 20 C air: Ra about 2.5e9, Nu about 160, h 4.3.
        let h = h_natural(Facing::Vertical, 1.0, 1.0, 50.0, 20.0);
        assert!(h > 4.0 && h < 4.7, "h = {h}");
    }

    #[test]
    fn hot_roof_beats_hot_underside() {
        let up = h_natural(Facing::Up, 0.8, 0.4, 45.0, 25.0);
        let down = h_natural(Facing::Down, 0.8, 0.4, 45.0, 25.0);
        assert!(up > 1.5 * down, "{up} vs {down}");
    }

    #[test]
    fn horizontal_hand_calculation() {
        // 1 m x 1 m, L = 0.25 m, dT 20 K: Ra = 1.1e7 * ... use branch limit.
        let h = h_natural(Facing::Up, 1.0, 1.0, 45.0, 25.0);
        assert!(h > 3.5 && h < 6.5, "h = {h}");
    }

    #[test]
    fn forced_laminar_hand_calculation() {
        // L 1 m, 2 m/s, Re about 1.3e5: Nu = 0.664 sqrt(Re) Pr^(1/3) = 213, h 5.5.
        let h = h_forced(1.0, 2.0, 30.0, 25.0);
        assert!(h > 5.0 && h < 6.2, "h = {h}");
        assert_eq!(h_forced(1.0, 0.0, 30.0, 25.0), 0.0);
    }

    #[test]
    fn forced_turbulent_branch_is_continuous_enough() {
        let lo = h_forced(2.0, 3.6, 30.0, 25.0);
        let hi = h_forced(2.0, 3.8, 30.0, 25.0);
        assert!(hi > lo && (hi - lo) / lo < 0.15);
    }

    #[test]
    fn radiation_matches_linear_rule_of_thumb() {
        // eps 0.9 near 300 K: about 5.5 W/(m2 K).
        let h = h_rad(0.9, 30.0, 25.0);
        assert!(h > 5.0 && h < 6.0, "h = {h}");
    }

    #[test]
    fn gap_zero_is_adiabatic_and_wide_gap_is_free() {
        assert_eq!(h_gap(2.0, 0.0, 45.0, 25.0), 0.0);
        let free = h_natural(Facing::Vertical, 1.0, 2.0, 45.0, 25.0);
        let wide = h_gap(2.0, 5.0, 45.0, 25.0);
        assert!(wide <= free && wide > 0.6 * free, "{wide} vs {free}");
        let narrow = h_gap(2.0, 0.01, 45.0, 25.0);
        assert!(narrow < 0.6 * free, "{narrow} vs {free}");
        // Rises with the gap up to the cap.
        assert!(h_gap(2.0, 0.05, 45.0, 25.0) >= narrow);
    }

    #[test]
    fn balance_closes_and_no_load_means_ambient() {
        let f = face(Facing::Vertical);
        let r = solve_outer_surface(&f, 8.0, 25.0);
        assert!((r.ts_c - 25.0).abs() < 1e-6);
        let r = solve_outer_surface(&f, 8.0, 45.0);
        assert!(r.ts_c > 25.0 && r.ts_c < 45.0);
        assert!((r.q_in_w_m2 + r.q_sun_w_m2 - r.q_out_w_m2).abs() < 1e-6);
    }

    #[test]
    fn overall_u_is_in_the_iec_range_for_sheet_steel() {
        // Thin steel, inner film about 5 W/(m2 K): air-to-air about 2.5 to 5.5.
        let f = face(Facing::Vertical);
        let u = overall_u(&f, 5.0, 45.0);
        assert!(u > 2.0 && u < 5.5, "u = {u}");
    }

    #[test]
    fn wind_cools_the_surface_and_raises_the_loss() {
        let still = solve_outer_surface(&face(Facing::Vertical), 8.0, 50.0);
        let mut w = face(Facing::Vertical);
        w.wind_m_s = 5.0;
        let windy = solve_outer_surface(&w, 8.0, 50.0);
        assert!(windy.ts_c < still.ts_c);
        assert!(windy.q_out_w_m2 > still.q_out_w_m2);
    }

    #[test]
    fn sun_heats_a_cabinet_with_no_internal_loss_above_ambient() {
        let mut roof = face(Facing::Up);
        roof.sun = Some(Sun {
            irradiance_w_m2: 900.0,
            absorptance: 0.6,
        });
        let r = solve_outer_surface(&roof, 5.0, 25.0);
        assert!(r.ts_c > 30.0 && r.ts_c < 60.0, "ts = {}", r.ts_c);
        // Heat flows INTO the cabinet: negative inner heat flow.
        assert!(r.q_in_w_m2 < 0.0);
        // White paint is much cooler than dark paint.
        let mut white = roof;
        white.sun = Some(Sun {
            irradiance_w_m2: 900.0,
            absorptance: 0.25,
        });
        let mut dark = roof;
        dark.sun = Some(Sun {
            irradiance_w_m2: 900.0,
            absorptance: 0.9,
        });
        assert!(
            solve_outer_surface(&white, 5.0, 25.0).ts_c + 8.0
                < solve_outer_surface(&dark, 5.0, 25.0).ts_c
        );
    }

    #[test]
    fn touching_wall_stops_the_heat_and_a_gap_helps() {
        let mut rear = face(Facing::Vertical);
        rear.surroundings = Surroundings::Parallel {
            gap_m: 0.0,
            other_eps: 0.9,
        };
        let touching = overall_u(&rear, 8.0, 50.0);
        assert!(touching < 1e-9);
        rear.surroundings = Surroundings::Parallel {
            gap_m: 0.02,
            other_eps: 0.9,
        };
        let gap20 = overall_u(&rear, 8.0, 50.0);
        rear.surroundings = Surroundings::Parallel {
            gap_m: 0.2,
            other_eps: 0.9,
        };
        let gap200 = overall_u(&rear, 8.0, 50.0);
        let free = overall_u(&face(Facing::Vertical), 8.0, 50.0);
        assert!(gap20 > 0.0 && gap20 < gap200 && gap200 <= free * 1.001);
    }

    #[test]
    fn sky_is_colder_than_air() {
        let s = sky_temperature_c(25.0);
        assert!(s < 20.0 && s > 5.0, "sky = {s}");
    }

    #[test]
    fn mixed_convection_limits() {
        assert!(near(h_mixed(4.0, 0.0), 4.0, 1e-12));
        assert!(h_mixed(4.0, 4.0) < 8.0 && h_mixed(4.0, 4.0) > 4.0);
    }
}
