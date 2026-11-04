//! Busbar and terminal Joule heating: current in, temperature along the bar
//! out (plan `/handoff/heat-research/ROADMAP.md`, research item "busbar
//! electro-thermal").
//!
//! One straight bar of constant rectangular section, discretised into nodes
//! along its length (1D finite volume). Per node the heat balance is
//!
//! ```text
//! k A (T[i-1] - 2 T[i] + T[i+1]) / dx  +  s(T[i]) dx  +  joints
//!      -  h(T[i]) P dx (T[i] - T_air)  =  0
//! ```
//!
//! * `s(T) = I² ρ(T) Fr(T) kp / A` is the Joule heat per metre. `ρ(T)` is the
//!   linear resistivity law `ρ20 (1 + α (T − 20))`; `Fr` the skin-effect
//!   factor of a flat conductor; `kp` a user-given proximity factor.
//! * The film `h` is either a fixed number or natural convection plus
//!   linearised radiation to surroundings at air temperature (the same
//!   [`crate::device_h`] the devices use).
//! * Each end is held at a temperature (a cooled lug), adiabatic, or tied to
//!   a terminal through a thermal conductance.
//! * Joints (bolted or clamped connections) add `I² R` at their position.
//!
//! Exact against hand solutions (unit tests): uniform bar with a temperature
//! dependent source, the fin equation with fixed end temperatures, the
//! lumped heat-up curve, the skin factor series. A **model** everywhere
//! else: the film coefficient, radiation to the cabinet, the proximity
//! factor (neighbouring phases are not solved), the skin factor of a thin
//! flat bar (edges ignored) and the end conditions. Not validated against a
//! measured busbar, not a compliance statement. The ampere limit it reports
//! is a planning number for a chosen temperature, not an IEC 61439 verdict.

use crate::device_h;

/// Conductor material with the properties the solver needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Resistivity at 20 °C, Ω·m.
    pub rho20: f64,
    /// Temperature coefficient of resistivity at 20 °C, 1/K.
    pub alpha: f64,
    /// Thermal conductivity, W/(m·K).
    pub k: f64,
    /// Volumetric heat capacity, J/(m³·K).
    pub rho_cp: f64,
    /// Melting point, °C: no steady state is reported above it.
    pub melt_c: f64,
}

impl Material {
    /// Electrolytic copper (annealed standard, 100 % IACS).
    pub const COPPER: Material = Material {
        rho20: 1.7241e-8,
        alpha: 0.003_93,
        k: 390.0,
        rho_cp: 8960.0 * 385.0,
        melt_c: 1085.0,
    };
    /// Electrical-grade aluminium (about 61 % IACS).
    pub const ALUMINIUM: Material = Material {
        rho20: 2.826e-8,
        alpha: 0.004_03,
        k: 237.0,
        rho_cp: 2700.0 * 897.0,
        melt_c: 660.0,
    };

    /// Resistivity at `t_c` degrees Celsius.
    pub fn rho(&self, t_c: f64) -> f64 {
        self.rho20 * (1.0 + self.alpha * (t_c - 20.0))
    }

    /// Skin depth at `t_c` and `f` hertz, metres (infinite for `f <= 0`).
    pub fn skin_depth(&self, t_c: f64, f_hz: f64) -> f64 {
        if f_hz <= 0.0 {
            return f64::INFINITY;
        }
        let mu0 = 4.0e-7 * std::f64::consts::PI;
        (self.rho(t_c) / (std::f64::consts::PI * f_hz * mu0)).sqrt()
    }
}

/// Ratio of AC to DC resistance of a flat conductor of thickness
/// `thickness_m` carrying current along its length (slab solution
/// `Δ/2 · (sinh Δ + sin Δ) / (cosh Δ − cos Δ)` with `Δ = t / δ`). Equal to 1
/// for thin bars and `Δ/2` for thick ones. Edges and neighbours are ignored.
pub fn skin_factor(thickness_m: f64, skin_depth_m: f64) -> f64 {
    let d = thickness_m / skin_depth_m;
    if !d.is_finite() {
        return 1.0;
    }
    if d < 0.05 {
        // Series 1 + d^4/180: the closed form loses digits to cancellation.
        return 1.0 + d.powi(4) / 180.0;
    }
    if d > 20.0 {
        return d / 2.0;
    }
    d / 2.0 * (d.sinh() + d.sin()) / (d.cosh() - d.cos())
}

/// Rectangular bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bar {
    pub material: Material,
    /// Width (the long side of the section), m.
    pub width_m: f64,
    /// Thickness (the short side, the way the skin effect acts), m.
    pub thickness_m: f64,
    /// Length between the two ends, m.
    pub length_m: f64,
}

impl Bar {
    pub fn area_m2(&self) -> f64 {
        self.width_m * self.thickness_m
    }
    /// Perimeter of the section, m.
    pub fn perimeter_m(&self) -> f64 {
        2.0 * (self.width_m + self.thickness_m)
    }
    /// DC resistance at `t_c`, ohm.
    pub fn r_dc(&self, t_c: f64) -> f64 {
        self.material.rho(t_c) * self.length_m / self.area_m2()
    }
}

/// How the bar loses heat to the air around it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Film {
    /// A fixed total coefficient (convection and radiation), W/(m²·K).
    Fixed(f64),
    /// Natural convection plus linearised radiation to surroundings at air
    /// temperature, optionally with forced air.
    Natural {
        /// Surface emissivity (bare copper about 0.1 to 0.3, painted or
        /// oxidised up to 0.9).
        emissivity: f64,
        /// Air speed along the bar, m/s (0 for still air).
        air_speed_m_s: f64,
        /// The bar stands upright: the buoyant boundary layer grows over
        /// the whole length. Otherwise the width is the length scale.
        vertical: bool,
    },
}

/// What the bar is tied to at one end.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum End {
    /// Held at a temperature, °C (an ideal heat sink: a big lug or a cooled
    /// device terminal).
    Fixed(f64),
    /// No heat leaves through the end.
    Adiabatic,
    /// Tied through a thermal conductance `conductance_w_k` to a body at
    /// `temp_c` (a device terminal).
    Terminal { conductance_w_k: f64, temp_c: f64 },
}

/// A connection with a contact resistance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Joint {
    /// Position from the first end, m.
    pub x_m: f64,
    /// Contact resistance, ohm (microohms to hundreds of microohms).
    pub resistance_ohm: f64,
}

/// All inputs of one run.
#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub bar: Bar,
    /// RMS current through the bar, A.
    pub current_a: f64,
    /// Frequency, Hz; 0 for DC (no skin effect).
    pub frequency_hz: f64,
    /// Extra loss factor from neighbouring phases (proximity effect), ≥ 1.
    /// Not computed: the user states it.
    pub proximity_factor: f64,
    /// Air temperature around the bar, °C.
    pub ambient_c: f64,
    pub film: Film,
    /// Fraction of the perimeter that is open to the air (0 to 1): a bar
    /// lying on a support loses less.
    pub exposed_fraction: f64,
    pub ends: [End; 2],
    pub joints: Vec<Joint>,
    /// Number of nodes along the bar (at least 3).
    pub nodes: usize,
}

impl Input {
    /// A bar in free air at 35 °C, 50 Hz, both ends adiabatic, 101 nodes.
    pub fn new(bar: Bar, current_a: f64) -> Input {
        Input {
            bar,
            current_a,
            frequency_hz: 50.0,
            proximity_factor: 1.0,
            ambient_c: 35.0,
            film: Film::Natural {
                emissivity: 0.4,
                air_speed_m_s: 0.0,
                vertical: false,
            },
            exposed_fraction: 1.0,
            ends: [End::Adiabatic, End::Adiabatic],
            joints: Vec::new(),
            nodes: 101,
        }
    }
}

/// Why a run gave no answer.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    #[error("invalid busbar input: {0}")]
    Invalid(String),
    #[error(
        "no steady state: the bar heats up past the melting point of the material (the heat it makes grows faster than it can lose)"
    )]
    Runaway,
}

/// Result of a steady run.
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    /// Node positions, m.
    pub x_m: Vec<f64>,
    /// Node temperatures, °C.
    pub temp_c: Vec<f64>,
    pub peak_c: f64,
    pub peak_x_m: f64,
    pub mean_c: f64,
    /// Joule heat in the bar itself (skin and proximity included), W.
    pub bar_loss_w: f64,
    /// Joule heat in the joints, W.
    pub joint_loss_w: f64,
    /// Heat that leaves through the surface into the air, W. This is what a
    /// cabinet heat model takes as the bar's contribution.
    pub heat_to_air_w: f64,
    /// Heat that leaves through the two ends, W (negative: heat flows in).
    pub heat_to_ends_w: [f64; 2],
    /// Skin-effect factor at the hottest node (1 for DC and thin bars).
    pub skin_factor: f64,
    /// DC resistance of the bar at the mean temperature, ohm.
    pub r_dc_hot_ohm: f64,
    pub iterations: usize,
    pub converged: bool,
}

impl Output {
    /// Heat generated in the whole run, W.
    pub fn generated_w(&self) -> f64 {
        self.bar_loss_w + self.joint_loss_w
    }
    /// Generated heat minus what leaves, W. Zero up to the solver tolerance.
    pub fn balance_w(&self) -> f64 {
        self.generated_w() - self.heat_to_air_w - self.heat_to_ends_w[0] - self.heat_to_ends_w[1]
    }
}

fn validate(inp: &Input) -> Result<(), Error> {
    let b = &inp.bar;
    let bad = |m: &str| Err(Error::Invalid(m.to_owned()));
    if !(b.width_m > 0.0 && b.thickness_m > 0.0 && b.length_m > 0.0) {
        return bad("width, thickness and length must be positive");
    }
    if !(inp.current_a.is_finite() && inp.current_a >= 0.0) {
        return bad("current must be zero or positive");
    }
    if inp.nodes < 3 {
        return bad("at least 3 nodes");
    }
    if !(0.0..=1.0).contains(&inp.exposed_fraction) {
        return bad("exposed fraction must be between 0 and 1");
    }
    if inp.proximity_factor < 1.0 {
        return bad("proximity factor must be at least 1");
    }
    if let Film::Fixed(h) = inp.film
        && h < 0.0
    {
        return bad("film coefficient must not be negative");
    }
    for j in &inp.joints {
        if !(0.0..=b.length_m).contains(&j.x_m) || j.resistance_ohm < 0.0 {
            return bad("a joint lies off the bar or has a negative resistance");
        }
    }
    for e in &inp.ends {
        if let End::Terminal {
            conductance_w_k, ..
        } = e
            && *conductance_w_k < 0.0
        {
            return bad("terminal conductance must not be negative");
        }
    }
    Ok(())
}

/// Shared geometry of the discretisation.
struct Grid {
    n: usize,
    dx: f64,
    /// Control volume length of each node (half at the ends), m.
    cell: Vec<f64>,
}

impl Grid {
    fn new(inp: &Input) -> Grid {
        let n = inp.nodes;
        let dx = inp.bar.length_m / (n - 1) as f64;
        let mut cell = vec![dx; n];
        cell[0] = 0.5 * dx;
        cell[n - 1] = 0.5 * dx;
        Grid { n, dx, cell }
    }
}

/// Joint heat per node, W: split linearly between the two bracketing nodes.
fn joint_heat(inp: &Input, g: &Grid) -> Vec<f64> {
    let mut q = vec![0.0; g.n];
    let i2 = inp.current_a * inp.current_a;
    for j in &inp.joints {
        let pos = j.x_m / g.dx;
        let lo = (pos.floor() as usize).min(g.n - 2);
        let f = pos - lo as f64;
        q[lo] += (1.0 - f) * i2 * j.resistance_ohm;
        q[lo + 1] += f * i2 * j.resistance_ohm;
    }
    q
}

/// Joule heat per metre at `t_c`, W/m, and its skin factor.
fn source_per_m(inp: &Input, t_c: f64) -> (f64, f64) {
    let m = &inp.bar.material;
    let fr = skin_factor(inp.bar.thickness_m, m.skin_depth(t_c, inp.frequency_hz));
    let s =
        inp.current_a * inp.current_a * m.rho(t_c) * fr * inp.proximity_factor / inp.bar.area_m2();
    (s, fr)
}

/// Film coefficient at surface temperature `t_c`.
fn film_h(inp: &Input, t_c: f64) -> f64 {
    match inp.film {
        Film::Fixed(h) => h,
        Film::Natural {
            emissivity,
            air_speed_m_s,
            vertical,
        } => {
            let len = if vertical {
                inp.bar.length_m
            } else {
                inp.bar.width_m
            };
            device_h(len, t_c, inp.ambient_c, air_speed_m_s, emissivity, 1.0)
        }
    }
}

/// Solves a tridiagonal system (sub `a`, diagonal `b`, super `c`).
fn thomas(a: &[f64], b: &[f64], c: &[f64], d: &[f64]) -> Vec<f64> {
    let n = b.len();
    let mut cp = vec![0.0; n];
    let mut dp = vec![0.0; n];
    cp[0] = c[0] / b[0];
    dp[0] = d[0] / b[0];
    for i in 1..n {
        let m = b[i] - a[i] * cp[i - 1];
        cp[i] = c[i] / m;
        dp[i] = (d[i] - a[i] * dp[i - 1]) / m;
    }
    let mut x = vec![0.0; n];
    x[n - 1] = dp[n - 1];
    for i in (0..n - 1).rev() {
        x[i] = dp[i] - cp[i] * x[i + 1];
    }
    x
}

/// One linear solve with the source and film frozen at `t`. `storage` is
/// `(ρ·cp·A·cell/dt, previous temperatures)` for a transient step.
fn linear_step(inp: &Input, g: &Grid, t: &[f64], storage: Option<(f64, &[f64])>) -> Vec<f64> {
    let bar = &inp.bar;
    let kad = bar.material.k * bar.area_m2() / g.dx;
    let p_exp = bar.perimeter_m() * inp.exposed_fraction;
    let jq = joint_heat(inp, g);
    let n = g.n;
    let (mut a, mut b, mut c, mut d) = (vec![0.0; n], vec![0.0; n], vec![0.0; n], vec![0.0; n]);
    for i in 0..n {
        let (s, _) = source_per_m(inp, t[i]);
        let hp = film_h(inp, t[i]) * p_exp * g.cell[i];
        if i > 0 {
            a[i] = -kad;
            b[i] += kad;
        }
        if i + 1 < n {
            c[i] = -kad;
            b[i] += kad;
        }
        b[i] += hp;
        d[i] = s * g.cell[i] + jq[i] + hp * inp.ambient_c;
        if let Some((c_dt, prev)) = storage {
            let m = c_dt * g.cell[i];
            b[i] += m;
            d[i] += m * prev[i];
        }
    }
    for (side, e) in inp.ends.iter().enumerate() {
        let i = if side == 0 { 0 } else { n - 1 };
        match *e {
            End::Adiabatic => {}
            End::Fixed(tc) => {
                a[i] = 0.0;
                c[i] = 0.0;
                b[i] = 1.0;
                d[i] = tc;
            }
            End::Terminal {
                conductance_w_k,
                temp_c,
            } => {
                b[i] += conductance_w_k;
                d[i] += conductance_w_k * temp_c;
            }
        }
    }
    // A fixed end must not keep the neighbour's link to it in the neighbour's
    // row as a missing term: the neighbour row already holds `-kad` for it.
    thomas(&a, &b, &c, &d)
}

fn finish(inp: &Input, g: &Grid, t: Vec<f64>, iterations: usize, converged: bool) -> Output {
    let bar = &inp.bar;
    let n = g.n;
    let kad = bar.material.k * bar.area_m2() / g.dx;
    let p_exp = bar.perimeter_m() * inp.exposed_fraction;
    let jq = joint_heat(inp, g);
    let mut bar_loss = 0.0;
    let mut air = 0.0;
    let mut skin = 1.0;
    let mut peak = f64::NEG_INFINITY;
    let mut peak_i = 0;
    let mut sum = 0.0;
    let mut gen_node = vec![0.0; n];
    let mut loss_node = vec![0.0; n];
    for i in 0..n {
        let (s, fr) = source_per_m(inp, t[i]);
        gen_node[i] = s * g.cell[i];
        bar_loss += gen_node[i];
        loss_node[i] = film_h(inp, t[i]) * p_exp * g.cell[i] * (t[i] - inp.ambient_c);
        air += loss_node[i];
        sum += t[i] * g.cell[i];
        if t[i] > peak {
            peak = t[i];
            peak_i = i;
            skin = fr;
        }
    }
    let ends = |i: usize, nb: usize, e: End| -> f64 {
        match e {
            End::Adiabatic => 0.0,
            End::Fixed(_) => kad * (t[nb] - t[i]) + gen_node[i] + jq[i] - loss_node[i],
            End::Terminal {
                conductance_w_k,
                temp_c,
            } => conductance_w_k * (t[i] - temp_c),
        }
    };
    let e0 = ends(0, 1, inp.ends[0]);
    let e1 = ends(n - 1, n - 2, inp.ends[1]);
    // A fixed end's own half cell is counted in the end flow, not in the air
    // loss, so the bookkeeping adds up.
    let fixed_adjust = |i: usize, e: End| match e {
        End::Fixed(_) => loss_node[i],
        _ => 0.0,
    };
    let air = air - fixed_adjust(0, inp.ends[0]) - fixed_adjust(n - 1, inp.ends[1]);
    let joint_loss: f64 = jq.iter().sum();
    let mean = sum / bar.length_m;
    Output {
        x_m: (0..n).map(|i| i as f64 * g.dx).collect(),
        peak_c: peak,
        peak_x_m: peak_i as f64 * g.dx,
        mean_c: mean,
        bar_loss_w: bar_loss,
        joint_loss_w: joint_loss,
        heat_to_air_w: air,
        heat_to_ends_w: [e0, e1],
        skin_factor: skin,
        r_dc_hot_ohm: bar.r_dc(mean),
        temp_c: t,
        iterations,
        converged,
    }
}

fn initial(inp: &Input, g: &Grid) -> Vec<f64> {
    vec![inp.ambient_c; g.n]
}

/// Steady state: temperature along the bar for a constant current.
pub fn solve_steady(inp: &Input) -> Result<Output, Error> {
    validate(inp)?;
    let g = Grid::new(inp);
    let mut t = initial(inp, &g);
    let limit = inp.bar.material.melt_c;
    let mut converged = false;
    let mut it = 0;
    for k in 1..=500 {
        it = k;
        let next = linear_step(inp, &g, &t, None);
        let change = next
            .iter()
            .zip(&t)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max);
        t = next;
        if t.iter().any(|v| !v.is_finite() || *v > limit) {
            return Err(Error::Runaway);
        }
        if change < 1.0e-7 {
            converged = true;
            break;
        }
    }
    if !converged {
        // Still creeping upwards after 500 passes means the loop gain is
        // about 1: treat a rising, unconverged bar as having no steady state.
        let again = linear_step(inp, &g, &t, None);
        let rising = again.iter().zip(&t).any(|(a, b)| a > b);
        if rising && t.iter().any(|v| *v > inp.ambient_c + 300.0) {
            return Err(Error::Runaway);
        }
    }
    Ok(finish(inp, &g, t, it, converged))
}

/// One point of a heat-up curve.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatUpPoint {
    pub time_s: f64,
    pub peak_c: f64,
    pub mean_c: f64,
}

/// Heat-up from a uniform start temperature (the ambient if `start_c` is
/// `None`) for `duration_s`, stepping `dt_s` (backward Euler, the source and
/// film updated every step). Returns the curve and the final steady run for
/// comparison when one exists.
pub fn heat_up(
    inp: &Input,
    start_c: Option<f64>,
    duration_s: f64,
    dt_s: f64,
) -> Result<Vec<HeatUpPoint>, Error> {
    validate(inp)?;
    if !(dt_s > 0.0 && duration_s >= 0.0) {
        return Err(Error::Invalid(
            "time step and duration must be positive".into(),
        ));
    }
    let g = Grid::new(inp);
    let mut t = vec![start_c.unwrap_or(inp.ambient_c); g.n];
    for (side, e) in inp.ends.iter().enumerate() {
        if let End::Fixed(tc) = e {
            t[if side == 0 { 0 } else { g.n - 1 }] = *tc;
        }
    }
    let c_dt = inp.bar.material.rho_cp * inp.bar.area_m2() / dt_s;
    let limit = inp.bar.material.melt_c;
    let stats = |time_s: f64, t: &[f64]| HeatUpPoint {
        time_s,
        peak_c: t.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        mean_c: t.iter().zip(&g.cell).map(|(v, w)| v * w).sum::<f64>() / inp.bar.length_m,
    };
    let mut out = vec![stats(0.0, &t)];
    let steps = (duration_s / dt_s).ceil() as usize;
    for k in 1..=steps {
        let prev = t.clone();
        for _ in 0..20 {
            let next = linear_step(inp, &g, &t, Some((c_dt, &prev)));
            let change = next
                .iter()
                .zip(&t)
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);
            t = next;
            if change < 1.0e-8 {
                break;
            }
        }
        if t.iter().any(|v| !v.is_finite() || *v > limit) {
            return Err(Error::Runaway);
        }
        out.push(stats(k as f64 * dt_s, &t));
    }
    Ok(out)
}

/// First time at which the heat-up curve's hottest node reaches `limit_c`,
/// by linear interpolation between steps; `None` if it never does.
pub fn time_to_reach(curve: &[HeatUpPoint], limit_c: f64) -> Option<f64> {
    curve.windows(2).find_map(|w| {
        (w[0].peak_c < limit_c && w[1].peak_c >= limit_c).then(|| {
            let f = (limit_c - w[0].peak_c) / (w[1].peak_c - w[0].peak_c);
            w[0].time_s + f * (w[1].time_s - w[0].time_s)
        })
    })
}

/// The largest steady current (A) whose hottest node stays at or below
/// `limit_c` with everything else as in `inp` (bisection on the current;
/// `inp.current_a` is ignored). `None` if even a tiny current is too hot,
/// that is the ambient or an end is already above the limit.
pub fn max_current_for_limit(inp: &Input, limit_c: f64) -> Option<f64> {
    let hot = |i: f64| -> Option<bool> {
        let mut x = inp.clone();
        x.current_a = i;
        match solve_steady(&x) {
            Ok(o) => Some(o.peak_c > limit_c),
            Err(Error::Runaway) => Some(true),
            Err(_) => None,
        }
    };
    if hot(0.0)? {
        return None;
    }
    let (mut lo, mut hi) = (0.0, 100.0);
    while !hot(hi)? {
        lo = hi;
        hi *= 2.0;
        if hi > 1.0e7 {
            return Some(lo);
        }
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if hot(mid)? {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    Some(lo)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A material without temperature dependence, so the closed forms hold.
    const FLAT: Material = Material {
        rho20: 1.7241e-8,
        alpha: 0.0,
        k: 390.0,
        rho_cp: 8960.0 * 385.0,
        melt_c: 1085.0,
    };

    fn bar(material: Material) -> Bar {
        Bar {
            material,
            width_m: 0.05,
            thickness_m: 0.005,
            length_m: 1.0,
        }
    }

    fn input(material: Material, amps: f64) -> Input {
        let mut i = Input::new(bar(material), amps);
        i.frequency_hz = 0.0;
        i.film = Film::Fixed(10.0);
        i.ambient_c = 30.0;
        i
    }

    #[test]
    fn skin_factor_matches_hand_values() {
        // Delta = 2: (2/2)(sinh2 + sin2)/(cosh2 - cos2)
        //  = (3.626860 + 0.909297)/(3.762196 + 0.416147) = 1.085636
        let f = skin_factor(2.0, 1.0);
        assert!((f - 1.085_636).abs() < 1.0e-5, "{f}");
        assert!((skin_factor(1.0e-4, 1.0) - 1.0).abs() < 1.0e-9);
        // Thick bar: Delta/2.
        assert!((skin_factor(30.0, 1.0) - 15.0).abs() < 1.0e-9);
        assert_eq!(skin_factor(5.0, f64::INFINITY), 1.0);
    }

    #[test]
    fn skin_depth_of_copper_at_50_hz_is_about_9_mm() {
        let d = Material::COPPER.skin_depth(20.0, 50.0);
        // sqrt(1.7241e-8 / (pi * 50 * 4e-7 pi)) = 9.35 mm.
        assert!((d - 0.009_35).abs() < 1.0e-4, "{d}");
    }

    #[test]
    fn uniform_bar_matches_the_closed_form_with_temperature_dependent_resistance() {
        // Adiabatic ends: s(T) = hP (T - Ta) with s = s0 (1 + a (T - 20)).
        // T (hP - s0 a) = hP Ta + s0 (1 - 20 a).
        let m = Material::COPPER;
        let mut inp = input(m, 800.0);
        inp.nodes = 11;
        let out = solve_steady(&inp).unwrap();
        let area = inp.bar.area_m2();
        let s0 = 800.0_f64.powi(2) * m.rho20 / area;
        let hp = 10.0 * inp.bar.perimeter_m();
        let want = (hp * 30.0 + s0 * (1.0 - 20.0 * m.alpha)) / (hp - s0 * m.alpha);
        for t in &out.temp_c {
            assert!((t - want).abs() < 1.0e-5, "{t} vs {want}");
        }
        assert!(out.converged);
        assert!(out.balance_w().abs() < 1.0e-6, "{}", out.balance_w());
        // Heat equals I^2 R at the bar temperature.
        let r = inp.bar.r_dc(want);
        assert!((out.bar_loss_w - 800.0 * 800.0 * r).abs() < 1.0e-3);
    }

    #[test]
    fn cooled_ends_follow_the_fin_equation() {
        // alpha = 0, both ends at the air temperature:
        // theta(x) = (s/hP) (1 - cosh(m (x - L/2)) / cosh(m L / 2)).
        let mut inp = input(FLAT, 600.0);
        inp.ends = [End::Fixed(30.0), End::Fixed(30.0)];
        inp.nodes = 401;
        let out = solve_steady(&inp).unwrap();
        let b = inp.bar;
        let s = 600.0_f64.powi(2) * FLAT.rho20 / b.area_m2();
        let hp = 10.0 * b.perimeter_m();
        let m = (hp / (FLAT.k * b.area_m2())).sqrt();
        let theta = |x: f64| {
            s / hp * (1.0 - (m * (x - 0.5 * b.length_m)).cosh() / (0.5 * m * b.length_m).cosh())
        };
        for (x, t) in out.x_m.iter().zip(&out.temp_c) {
            assert!((t - 30.0 - theta(*x)).abs() < 2.0e-4 * theta(0.5) + 1.0e-6);
        }
        assert!((out.peak_x_m - 0.5).abs() < 2.0e-3);
        // Heat balance: what leaves the ends plus the air equals the source.
        assert!(out.balance_w().abs() < 1.0e-6);
        // Both ends carry the same, and by symmetry nothing is lost to a
        // negative flow: heat flows out of the bar at the ends.
        assert!(out.heat_to_ends_w[0] > 0.0);
        assert!((out.heat_to_ends_w[0] - out.heat_to_ends_w[1]).abs() < 1.0e-6);
        // End flow equals k A theta'(0) = k A (s / hP) m tanh(m L / 2).
        let flow = FLAT.k * b.area_m2() * s / hp * m * (0.5 * m * b.length_m).tanh();
        assert!((out.heat_to_ends_w[0] - flow).abs() < 2.0e-3 * flow);
    }

    #[test]
    fn a_joint_adds_its_i_squared_r() {
        let mut inp = input(FLAT, 500.0);
        inp.joints = vec![Joint {
            x_m: 0.5,
            resistance_ohm: 100.0e-6,
        }];
        let out = solve_steady(&inp).unwrap();
        assert!((out.joint_loss_w - 500.0 * 500.0 * 100.0e-6).abs() < 1.0e-9);
        // Adiabatic ends: everything leaves through the surface.
        assert!((out.heat_to_air_w - out.generated_w()).abs() < 1.0e-6);
        // The joint is the hottest place.
        assert!((out.peak_x_m - 0.5).abs() < 1.0e-9);
        let plain = solve_steady(&input(FLAT, 500.0)).unwrap();
        assert!(out.peak_c > plain.peak_c);
    }

    #[test]
    fn a_joint_between_nodes_is_split_without_losing_heat() {
        let mut inp = input(FLAT, 300.0);
        inp.nodes = 21;
        inp.joints = vec![Joint {
            x_m: 0.512,
            resistance_ohm: 50.0e-6,
        }];
        let out = solve_steady(&inp).unwrap();
        assert!((out.joint_loss_w - 300.0 * 300.0 * 50.0e-6).abs() < 1.0e-9);
        assert!(out.balance_w().abs() < 1.0e-6);
    }

    #[test]
    fn terminal_end_draws_heat_by_its_conductance() {
        let mut inp = input(FLAT, 0.0);
        inp.ambient_c = 20.0;
        inp.ends = [
            End::Terminal {
                conductance_w_k: 1000.0,
                temp_c: 80.0,
            },
            End::Adiabatic,
        ];
        inp.film = Film::Fixed(0.0);
        let out = solve_steady(&inp).unwrap();
        // No loss anywhere: the bar sits at the terminal temperature and the
        // flow is zero.
        for t in &out.temp_c {
            assert!((t - 80.0).abs() < 1.0e-6);
        }
        assert!(out.heat_to_ends_w[0].abs() < 1.0e-6);
    }

    #[test]
    fn skin_effect_raises_the_loss_of_a_thick_bar_at_50_hz() {
        let thick = Bar {
            material: FLAT,
            width_m: 0.1,
            thickness_m: 0.03,
            length_m: 1.0,
        };
        let mut ac = Input::new(thick, 1000.0);
        ac.film = Film::Fixed(10.0);
        let a = solve_steady(&ac).unwrap();
        let dc_loss = 1000.0_f64.powi(2) * a.r_dc_hot_ohm;
        let delta = FLAT.skin_depth(a.mean_c, 50.0);
        let want = skin_factor(0.03, delta);
        assert!(want > 1.2, "{want}");
        assert!((a.bar_loss_w / dc_loss - want).abs() < 1.0e-9);
        assert!((a.skin_factor - want).abs() < 1.0e-9);
    }

    #[test]
    fn proximity_factor_scales_the_loss() {
        let mut a = input(FLAT, 400.0);
        let base = solve_steady(&a).unwrap().bar_loss_w;
        a.proximity_factor = 1.25;
        let out = solve_steady(&a).unwrap().bar_loss_w;
        assert!((out / base - 1.25).abs() < 1.0e-9);
    }

    #[test]
    fn natural_film_gives_a_plausible_rise_for_a_copper_bar() {
        // 50 x 5 mm copper, 400 A, still air at 35 C, emissivity 0.4: the
        // textbook rise for a free bar is a few tens of kelvin. Pin the
        // band, not a decimal: this is a model, not a measurement.
        let mut inp = Input::new(bar(Material::COPPER), 400.0);
        inp.frequency_hz = 50.0;
        let out = solve_steady(&inp).unwrap();
        let rise = out.mean_c - 35.0;
        assert!((10.0..60.0).contains(&rise), "{rise}");
        assert!(out.balance_w().abs() < 1.0e-6);
        // More current, hotter; a cooler air, cooler bar.
        inp.current_a = 600.0;
        assert!(solve_steady(&inp).unwrap().mean_c > out.mean_c);
        inp.current_a = 400.0;
        inp.ambient_c = 20.0;
        assert!(solve_steady(&inp).unwrap().mean_c < out.mean_c);
    }

    #[test]
    fn a_bar_with_no_way_to_lose_heat_runs_away() {
        let mut inp = Input::new(bar(Material::COPPER), 3000.0);
        inp.film = Film::Fixed(0.5);
        assert_eq!(solve_steady(&inp), Err(Error::Runaway));
    }

    #[test]
    fn bad_input_is_refused() {
        let mut inp = input(FLAT, 100.0);
        inp.nodes = 2;
        assert!(matches!(solve_steady(&inp), Err(Error::Invalid(_))));
        let mut inp = input(FLAT, 100.0);
        inp.joints = vec![Joint {
            x_m: 2.0,
            resistance_ohm: 1.0e-5,
        }];
        assert!(matches!(solve_steady(&inp), Err(Error::Invalid(_))));
        let mut inp = input(FLAT, 100.0);
        inp.proximity_factor = 0.5;
        assert!(matches!(solve_steady(&inp), Err(Error::Invalid(_))));
    }

    #[test]
    fn heat_up_follows_the_lumped_exponential() {
        // alpha = 0, adiabatic ends: T = Ta + dT (1 - exp(-t / tau)),
        // tau = rho cp A / (h P), dT = s / (h P).
        let inp = input(FLAT, 700.0);
        let b = inp.bar;
        let hp = 10.0 * b.perimeter_m();
        let tau = FLAT.rho_cp * b.area_m2() / hp;
        let dt_k = 700.0_f64.powi(2) * FLAT.rho20 / b.area_m2() / hp;
        let curve = heat_up(&inp, None, 2.0 * tau, tau / 400.0).unwrap();
        for p in curve.iter().step_by(40) {
            let want = 30.0 + dt_k * (1.0 - (-p.time_s / tau).exp());
            assert!(
                (p.peak_c - want).abs() < 0.01 * dt_k + 1.0e-9,
                "{p:?} {want}"
            );
        }
        // Time to 63.2 % of the rise is tau.
        let t63 = time_to_reach(&curve, 30.0 + dt_k * (1.0 - (-1.0_f64).exp())).unwrap();
        assert!((t63 - tau).abs() < 0.01 * tau, "{t63} vs {tau}");
        assert_eq!(time_to_reach(&curve, 30.0 + 2.0 * dt_k), None);
    }

    #[test]
    fn heat_up_ends_at_the_steady_state() {
        let inp = input(Material::COPPER, 500.0);
        let steady = solve_steady(&inp).unwrap();
        let b = inp.bar;
        let tau = b.material.rho_cp * b.area_m2() / (10.0 * b.perimeter_m());
        let curve = heat_up(&inp, None, 8.0 * tau, tau / 20.0).unwrap();
        let last = curve.last().unwrap();
        assert!((last.peak_c - steady.peak_c).abs() < 0.05, "{last:?}");
    }

    #[test]
    fn max_current_lands_on_the_limit() {
        let mut inp = Input::new(bar(Material::COPPER), 0.0);
        inp.nodes = 21;
        let amps = max_current_for_limit(&inp, 105.0).unwrap();
        inp.current_a = amps;
        let peak = solve_steady(&inp).unwrap().peak_c;
        assert!((peak - 105.0).abs() < 0.01, "{peak} at {amps} A");
        assert!(amps > 200.0 && amps < 2000.0, "{amps}");
        // Cooler air allows more.
        let mut cool = inp.clone();
        cool.ambient_c = 20.0;
        assert!(max_current_for_limit(&cool, 105.0).unwrap() > amps);
        // An ambient above the limit allows nothing.
        let mut hot = inp.clone();
        hot.ambient_c = 120.0;
        assert_eq!(max_current_for_limit(&hot, 105.0), None);
    }
}
