//! Fit of the equivalent circuit to a nameplate.
//!
//! Unknowns: rotor resistance, magnetising reactance and total leakage reactance (and the
//! stator resistance when a breakdown ratio is given). Equations at the rated point: stator
//! current, power factor and shaft power. The stator-to-rotor resistance ratio, the split of
//! the leakage between stator and rotor, friction and core loss are assumptions the caller can
//! change. A Levenberg-Marquardt iteration in log space solves the equations; the report
//! lists how closely the fitted circuit reproduces the nameplate and what it implies.

use super::circuit::{Circuit, InductionMotor, Supply};
use crate::drive::MotorNameplate;
use crate::error::{self, InputError};
use crate::motor::Direction;

/// Assumptions of the fit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitOptions {
    /// Stator resistance over rotor resistance (referred), used unless a breakdown ratio is
    /// given. Typical 0.8 to 1.5.
    pub rs_over_rr: f64,
    /// Share of the total leakage reactance in the stator (0.5 = equal split).
    pub stator_leakage_share: f64,
    /// Breakdown torque (air gap) over rated shaft torque. When given, the stator resistance
    /// becomes an unknown instead of following `rs_over_rr`. When not given and the
    /// nameplate losses cannot be matched with `rs_over_rr`, the fit frees the stator
    /// resistance and assumes one of [`ASSUMED_BREAKDOWN_RATIOS`] (a note says so).
    pub breakdown_ratio: Option<f64>,
    /// Core loss as a fraction of rated output power.
    pub core_loss_fraction: f64,
    /// Friction and windage as a fraction of rated output power.
    pub rot_loss_fraction: f64,
}

impl Default for FitOptions {
    /// No core loss and no friction: the pure circuit.
    fn default() -> Self {
        Self {
            rs_over_rr: 1.0,
            stator_leakage_share: 0.5,
            breakdown_ratio: None,
            core_loss_fraction: 0.0,
            rot_loss_fraction: 0.0,
        }
    }
}

impl FitOptions {
    /// Typical small-motor values: core loss 2 % and friction 1 % of rated power.
    pub fn typical() -> Self {
        Self {
            core_loss_fraction: 0.02,
            rot_loss_fraction: 0.01,
            ..Self::default()
        }
    }
}

/// How well the fit reproduces the nameplate and what it implies.
#[derive(Debug, Clone, PartialEq)]
pub struct FitReport {
    /// Worst relative miss of the fitted equations (current, power factor, power).
    pub residual: f64,
    pub iterations: u32,
    /// Efficiency at the rated point.
    pub rated_efficiency: f64,
    /// Air-gap breakdown torque over rated shaft torque.
    pub breakdown_ratio: f64,
    /// Standstill torque at rated voltage and frequency over rated torque.
    pub starting_torque_ratio: f64,
    /// Standstill current over rated current.
    pub starting_current_ratio: f64,
    /// Core loss plus friction and windage of the fitted motor at the rated point, W.
    pub core_and_friction_w: f64,
    /// The breakdown ratio the fit assumed, when it had to (see [`FitOptions::breakdown_ratio`]).
    pub assumed_breakdown_ratio: Option<f64>,
    /// Things the user should look at.
    pub notes: Vec<&'static str>,
}

struct Target {
    poles: u32,
    v_ll: f64,
    f: f64,
    rpm: f64,
    i_n: f64,
    pf: f64,
    p_out: f64,
    opts: FitOptions,
    /// Breakdown ratio the fit must reproduce.
    breakdown: Option<f64>,
    /// The stator resistance is an unknown (otherwise `rs_over_rr` times the rotor resistance).
    free_rs: bool,
    /// Losses beyond the entered core and friction fractions are an unknown, split by
    /// [`EXTRA_LOSS_CORE_SHARE`] into core loss and friction.
    extra_loss: bool,
}

impl Target {
    fn circuit(&self, x: &[f64]) -> Circuit {
        let rr = x[0].exp();
        let xm = x[1].exp();
        let xl = x[2].exp();
        let mut next = 3;
        let rs = if self.free_rs {
            next += 1;
            x[3].exp()
        } else {
            self.opts.rs_over_rr * rr
        };
        let extra = if self.extra_loss { x[next].exp() } else { 0.0 };
        let share = self.opts.stator_leakage_share;
        Circuit {
            rs,
            xs: share * xl,
            rr,
            xr: (1.0 - share) * xl,
            xm,
            core_loss_w: self.opts.core_loss_fraction * self.p_out + EXTRA_LOSS_CORE_SHARE * extra,
            rot_loss_w: self.opts.rot_loss_fraction * self.p_out
                + (1.0 - EXTRA_LOSS_CORE_SHARE) * extra,
            saturation: None,
        }
    }

    fn motor(&self, x: &[f64]) -> InductionMotor {
        InductionMotor::unchecked(self.circuit(x), self.poles, self.v_ll, self.f, self.rpm)
    }

    fn residuals(&self, x: &[f64]) -> Option<Vec<f64>> {
        let m = self.motor(x);
        let s_n = m.rated_slip();
        let supply = Supply::new(self.f, self.v_ll, Direction::Positive).ok()?;
        let p = m.at_slip(&supply, s_n).ok()?;
        let mut r = vec![
            (p.stator_current_a - self.i_n) / self.i_n,
            (p.power_factor - self.pf) / self.pf,
            (p.shaft_power_w - self.p_out) / self.p_out,
        ];
        if let Some(k) = self.breakdown {
            let t_n = self.p_out / (std::f64::consts::TAU * self.rpm / 60.0);
            r.push(m.breakdown(&supply).motoring_nm / t_n / k - 1.0);
        }
        r.iter().all(|v| v.is_finite()).then_some(r)
    }
}

fn cost(r: &[f64]) -> f64 {
    r.iter().map(|v| v * v).sum()
}

/// Solves `a x = b` by Gaussian elimination with partial pivoting; `None` when singular.
fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let piv = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[piv][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, piv);
        b.swap(col, piv);
        let pivot_row = a[col].clone();
        let pivot_b = b[col];
        for row in col + 1..n {
            let m = a[row][col] / pivot_row[col];
            for (k, p) in pivot_row.iter().enumerate().skip(col) {
                a[row][k] -= m * p;
            }
            b[row] -= m * pivot_b;
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let s: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - s) / a[row][row];
    }
    Some(x)
}

/// Levenberg-Marquardt on the log parameters; returns the best point, its residuals and the
/// iteration count.
fn levenberg_marquardt(t: &Target, mut x: Vec<f64>) -> Option<(Vec<f64>, Vec<f64>, u32)> {
    let n = x.len();
    let mut r = t.residuals(&x)?;
    let mut c = cost(&r);
    let mut lambda = 1e-3;
    let mut iterations = 0;
    for it in 0..400 {
        iterations = it + 1;
        if r.iter().all(|v| v.abs() < 1e-12) {
            break;
        }
        // Forward-difference Jacobian in log space.
        let mut jac = vec![vec![0.0; n]; r.len()];
        for j in 0..n {
            let mut xp = x.clone();
            let h = 1e-7;
            xp[j] += h;
            let rp = t.residuals(&xp)?;
            for i in 0..r.len() {
                jac[i][j] = (rp[i] - r[i]) / h;
            }
        }
        let mut jtj = vec![vec![0.0; n]; n];
        let mut g = vec![0.0; n];
        for i in 0..r.len() {
            for a in 0..n {
                g[a] += jac[i][a] * r[i];
                for b in 0..n {
                    jtj[a][b] += jac[i][a] * jac[i][b];
                }
            }
        }
        let mut accepted = false;
        for _ in 0..30 {
            let mut a = jtj.clone();
            for (d, row) in a.iter_mut().enumerate() {
                row[d] += lambda * jtj[d][d].max(1e-12);
            }
            let rhs: Vec<f64> = g.iter().map(|v| -v).collect();
            if let Some(step) = solve_linear(a, rhs) {
                let xn: Vec<f64> = x
                    .iter()
                    .zip(&step)
                    .map(|(a, d)| a + d.clamp(-1.0, 1.0))
                    .collect();
                if let Some(rn) = t.residuals(&xn) {
                    let cn = cost(&rn);
                    if cn < c {
                        x = xn;
                        r = rn;
                        c = cn;
                        lambda = (lambda / 4.0).max(1e-12);
                        accepted = true;
                        break;
                    }
                }
            }
            lambda *= 4.0;
        }
        if !accepted {
            break;
        }
    }
    Some((x, r, iterations))
}

/// Breakdown ratios tried, in this order, when the nameplate alone cannot be matched with a
/// stator resistance equal to the rotor resistance.
pub const ASSUMED_BREAKDOWN_RATIOS: [f64; 7] = [2.5, 2.2, 2.8, 2.0, 3.2, 1.8, 3.6];

/// Share of unexplained nameplate loss given to the core; the rest is friction and windage.
pub const EXTRA_LOSS_CORE_SHARE: f64 = 0.6;

/// Most poles derived from a nameplate speed; a motor slower than this needs the pole count entered.
pub const MAX_DERIVED_POLES: u32 = 24;

struct Found {
    x: Vec<f64>,
    r: Vec<f64>,
    iterations: u32,
}

impl Found {
    fn worst(&self) -> f64 {
        self.r.iter().fold(0.0f64, |a, v| a.max(v.abs()))
    }
}

/// Tries several starting points and keeps the best result.
fn search(t: &Target, base: &[f64; 3]) -> Option<Found> {
    let mut best: Option<Found> = None;
    for scale in [
        [1.0, 1.0, 1.0],
        [0.5, 1.5, 0.7],
        [2.0, 0.7, 1.5],
        [1.0, 2.0, 0.4],
        [0.3, 1.0, 2.0],
        [3.0, 1.0, 1.0],
    ] {
        let mut x: Vec<f64> = base.iter().zip(scale).map(|(b, s)| (b * s).ln()).collect();
        if t.free_rs {
            x.push((t.opts.rs_over_rr * base[0] * scale[0]).ln());
        }
        if t.extra_loss {
            x.push((0.02 * t.p_out * scale[0]).ln());
        }
        if let Some((x, r, iterations)) = levenberg_marquardt(t, x) {
            let found = Found { x, r, iterations };
            if best.as_ref().is_none_or(|b| found.worst() < b.worst()) {
                best = Some(found);
            }
            if best.as_ref().is_some_and(|b| b.worst() < 1e-9) {
                break;
            }
        }
    }
    best
}

impl InductionMotor {
    /// Fits an equivalent circuit to a nameplate (line voltage, current, frequency, rpm,
    /// output power in kW, power factor). `poles` is derived from the rpm when `None`.
    /// Model: see [`FitOptions`] for the assumptions and [`FitReport`] for what the circuit implies.
    pub fn from_nameplate(
        np: &MotorNameplate,
        poles: Option<u32>,
        opts: &FitOptions,
    ) -> Result<(Self, FitReport), InputError> {
        error::positive("voltage_v", np.voltage_v)?;
        error::positive("current_a", np.current_a)?;
        error::positive("frequency_hz", np.frequency_hz)?;
        error::positive("rpm", np.rpm)?;
        error::positive("power_kw", np.power_kw)?;
        error::fraction("power_factor", np.power_factor)?;
        error::positive("rs_over_rr", opts.rs_over_rr)?;
        error::non_negative("core_loss_fraction", opts.core_loss_fraction)?;
        error::non_negative("rot_loss_fraction", opts.rot_loss_fraction)?;
        if !(0.0..=1.0).contains(&opts.stator_leakage_share) {
            return Err(InputError::OutOfRange {
                field: "stator_leakage_share",
                value: opts.stator_leakage_share,
                min: 0.0,
                max: 1.0,
            });
        }
        if let Some(k) = opts.breakdown_ratio {
            error::finite("breakdown_ratio", k)?;
            if !(1.0..=crate::motor::MAX_BREAKDOWN_RATIO).contains(&k) {
                return Err(InputError::OutOfRange {
                    field: "breakdown_ratio",
                    value: k,
                    min: 1.0,
                    max: crate::motor::MAX_BREAKDOWN_RATIO,
                });
            }
        }
        let poles = match poles {
            Some(p) => error::poles(p)?,
            None => {
                // Largest even pole count whose synchronous speed is not below the rated speed.
                let pairs = (60.0 * np.frequency_hz / np.rpm).floor().max(1.0);
                if pairs > f64::from(MAX_DERIVED_POLES / 2) {
                    return Err(InputError::OutOfRange {
                        field: "rpm",
                        value: np.rpm,
                        // Refused up to 60 f / (pairs + 1) rpm; the 2 pole synchronous speed bounds it above.
                        min: 60.0 * np.frequency_hz / f64::from(MAX_DERIVED_POLES / 2 + 1),
                        max: 120.0 * np.frequency_hz / 2.0,
                    });
                }
                2 * pairs as u32
            }
        };
        let sync = 120.0 * np.frequency_hz / f64::from(poles);
        if np.rpm >= sync {
            return Err(InputError::OutOfRange {
                field: "rpm",
                value: np.rpm,
                min: 0.0,
                max: sync,
            });
        }
        let p_out = np.power_kw * 1000.0;
        let p_in = 3f64.sqrt() * np.voltage_v * np.current_a * np.power_factor;
        if p_out >= p_in {
            return Err(InputError::Inconsistent {
                what: "rated power is not below the input power from voltage, current and power factor (efficiency above 100 %)",
            });
        }
        if p_out < 0.3 * p_in {
            return Err(InputError::Inconsistent {
                what: "rated power is below 30 % of the input power from voltage, current and power factor",
            });
        }
        let mut t = Target {
            poles,
            v_ll: np.voltage_v,
            f: np.frequency_hz,
            rpm: np.rpm,
            i_n: np.current_a,
            pf: np.power_factor,
            p_out,
            opts: *opts,
            breakdown: opts.breakdown_ratio,
            free_rs: false,
            extra_loss: false,
        };
        let v_ph = np.voltage_v / 3f64.sqrt();
        let zb = v_ph / np.current_a;
        let s_n = 1.0 - np.rpm / sync;
        let sin_phi = (1.0 - np.power_factor * np.power_factor).sqrt().max(0.05);
        let base = [
            s_n.max(0.005) * zb * np.power_factor,
            zb / sin_phi,
            0.2 * zb,
        ];
        let mut notes = Vec::new();
        t.free_rs = opts.breakdown_ratio.is_some();
        let mut attempt = search(&t, &base);
        let mut assumed_breakdown_ratio = None;
        let failed = |a: &Option<Found>| a.as_ref().is_none_or(|a| a.worst() > 1e-6);
        if opts.breakdown_ratio.is_none() && failed(&attempt) {
            // The nameplate losses are larger than copper and the entered core and friction
            // can explain (rounded nameplate values, stray loss). Give the surplus to core loss
            // and friction, and assume a breakdown ratio to keep the equations determined.
            t.extra_loss = true;
            for k in ASSUMED_BREAKDOWN_RATIOS {
                t.breakdown = Some(k);
                attempt = search(&t, &base);
                if !failed(&attempt) {
                    assumed_breakdown_ratio = Some(k);
                    notes.push("Nameplate losses beyond copper were assigned to core loss and friction and the breakdown torque ratio is assumed: enter the catalogue breakdown ratio and measured losses for a better curve");
                    break;
                }
            }
        }
        if opts.breakdown_ratio.is_none() && failed(&attempt) {
            // Last resort: let the stator resistance absorb the losses.
            t.extra_loss = false;
            t.free_rs = true;
            for k in ASSUMED_BREAKDOWN_RATIOS {
                t.breakdown = Some(k);
                attempt = search(&t, &base);
                if !failed(&attempt) {
                    assumed_breakdown_ratio = Some(k);
                    notes.push("Stator resistance was raised until the nameplate losses add up and the breakdown torque ratio is assumed: the stator resistance is probably too high, check the nameplate");
                    break;
                }
            }
        }
        let found = attempt.ok_or(InputError::NoFit { residual: f64::NAN })?;
        let worst = found.worst();
        if worst > 1e-6 {
            return Err(InputError::NoFit { residual: worst });
        }
        let (x, r, iterations) = (found.x, found.r, found.iterations);
        let _ = r;
        let motor = Self::new(t.circuit(&x), poles, np.voltage_v, np.frequency_hz, np.rpm)?;
        let supply = Supply::new(np.frequency_hz, np.voltage_v, Direction::Positive)?;
        let rated = motor.at_slip(&supply, motor.rated_slip())?;
        let start = motor.at_slip(&supply, 1.0)?;
        let t_n = motor.rated_torque_nm();
        let breakdown_ratio = motor.breakdown(&supply).motoring_nm / t_n;
        let starting_torque_ratio = start.air_gap_torque_nm / t_n;
        let starting_current_ratio = start.stator_current_a / np.current_a;
        if !(1.3..=4.5).contains(&breakdown_ratio) {
            notes.push("Breakdown torque ratio is outside the usual 1.3 to 4.5: check the nameplate or enter a breakdown ratio");
        }
        if !(3.0..=12.0).contains(&starting_current_ratio) {
            notes.push("Starting current ratio is outside the usual 3 to 12: check the nameplate");
        }
        if starting_torque_ratio < 0.8 {
            notes.push("Standstill torque is low: a constant rotor resistance underestimates deep-bar motors at standstill (drive operation at a few hertz of slip is not affected)");
        }
        if t.breakdown.is_none() {
            notes.push("Breakdown torque follows from the assumed resistance ratio and leakage split; enter the catalogue breakdown ratio for a better curve");
        }
        let report = FitReport {
            residual: worst,
            iterations,
            rated_efficiency: rated.efficiency.unwrap_or(0.0),
            breakdown_ratio,
            starting_torque_ratio,
            starting_current_ratio,
            core_and_friction_w: rated.loss_core_w + rated.loss_rot_w,
            assumed_breakdown_ratio,
            notes,
        };
        Ok((motor, report))
    }
}
