//! The per-phase equivalent circuit and everything computed from it.
//!
//! ```text
//!   V_ph --- Rs --- jXs --+--------+------------+
//!                         |        |            |
//!                       Rfe      jXm      Rr'/s + jXr'
//!                         |        |            |
//!                         +--------+------------+
//! ```
//!
//! Reactances are given at the rated frequency and scale with `f / f_rated`; resistances do not.
//! Everything is per phase of the equivalent star, line voltage divided by sqrt(3).
//! Torque is air-gap power over synchronous speed (`T = 3 I_r^2 Rr' / (s w_s)`); the shaft
//! torque is that minus friction and windage.

use std::f64::consts::TAU;

use super::complex::C;
use crate::error::{self, InputError};
use crate::motor::Direction;

/// Below this output frequency the field does not turn: the model returns the DC hold state.
pub const DC_HOLD_BELOW_HZ: f64 = 0.01;

/// Exponent of the core loss against frequency at constant flux (hysteresis `f`, eddy `f^2`).
pub const CORE_LOSS_FREQUENCY_EXPONENT: f64 = 1.5;

/// Smallest slip magnitude used, so the rotor branch stays finite at synchronous speed.
const MIN_SLIP: f64 = 1e-9;

/// Optional magnetic saturation: above `knee_flux_pu` of the rated flux the magnetising
/// reactance falls as `(knee / flux)^exponent`. Model, user entered; off by default.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Saturation {
    /// Flux (per unit of rated) where saturation starts, at least 1.
    pub knee_flux_pu: f64,
    /// How fast the magnetising reactance falls beyond the knee, greater than 0.
    pub exponent: f64,
}

/// Equivalent circuit values in ohm per phase, reactances at the rated frequency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circuit {
    pub rs: f64,
    pub xs: f64,
    pub rr: f64,
    pub xr: f64,
    pub xm: f64,
    /// Core loss in W at rated voltage and frequency; 0 = none.
    pub core_loss_w: f64,
    /// Friction and windage in W at rated speed; scales with speed squared; 0 = none.
    pub rot_loss_w: f64,
    pub saturation: Option<Saturation>,
}

impl Circuit {
    /// A circuit without core loss, friction or saturation.
    pub fn new(rs: f64, xs: f64, rr: f64, xr: f64, xm: f64) -> Self {
        Self {
            rs,
            xs,
            rr,
            xr,
            xm,
            core_loss_w: 0.0,
            rot_loss_w: 0.0,
            saturation: None,
        }
    }

    fn validate(&self) -> Result<(), InputError> {
        error::non_negative("rs", self.rs)?;
        error::non_negative("xs", self.xs)?;
        error::positive("rr", self.rr)?;
        error::non_negative("xr", self.xr)?;
        error::positive("xm", self.xm)?;
        error::non_negative("core_loss_w", self.core_loss_w)?;
        error::non_negative("rot_loss_w", self.rot_loss_w)?;
        if let Some(s) = self.saturation {
            error::finite("knee_flux_pu", s.knee_flux_pu)?;
            error::positive("saturation exponent", s.exponent)?;
            if s.knee_flux_pu < 1.0 {
                return Err(InputError::OutOfRange {
                    field: "knee_flux_pu",
                    value: s.knee_flux_pu,
                    min: 1.0,
                    max: f64::MAX,
                });
            }
        }
        Ok(())
    }
}

/// What the drive (or the mains) feeds the motor: field frequency, line voltage and sense.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supply {
    frequency_hz: f64,
    volts_ll: f64,
    direction: Direction,
}

impl Supply {
    /// `frequency_hz` and `volts_ll` (rms, line to line) must be finite and not negative; the
    /// sense of rotation is the `direction`, not a negative frequency.
    pub fn new(frequency_hz: f64, volts_ll: f64, direction: Direction) -> Result<Self, InputError> {
        error::non_negative("frequency_hz", frequency_hz)?;
        error::non_negative("volts_ll", volts_ll)?;
        Ok(Self {
            frequency_hz,
            volts_ll,
            direction,
        })
    }

    pub fn frequency_hz(&self) -> f64 {
        self.frequency_hz
    }

    pub fn volts_ll(&self) -> f64 {
        self.volts_ll
    }

    pub fn direction(&self) -> Direction {
        self.direction
    }

    fn phase_volts(&self) -> f64 {
        self.volts_ll / 3f64.sqrt()
    }
}

/// Self-ventilated motor: the fan slows with the shaft. `standstill_loss_fraction` is the
/// share of the rated losses the motor can dissipate at standstill (about 0.3 to 0.5 for
/// a totally enclosed fan-cooled motor). Model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelfVentilation {
    pub standstill_loss_fraction: f64,
}

impl SelfVentilation {
    pub fn typical() -> Self {
        Self {
            standstill_loss_fraction: 0.4,
        }
    }
}

/// One steady operating point. Torque and speed are signed in the shaft sense (positive =
/// the positive sense of the shaft, which is the field sense when the direction is positive);
/// powers are signed electrically (input power is negative when the motor feeds back).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OperatingPoint {
    /// Slip in the field sense (negative above synchronous speed: generating).
    pub slip: f64,
    pub speed_rpm: f64,
    pub sync_rpm: f64,
    /// Slip frequency in Hz.
    pub slip_hz: f64,
    /// Shaft torque in N m (air-gap torque minus friction and windage).
    pub torque_nm: f64,
    /// Air-gap torque in N m.
    pub air_gap_torque_nm: f64,
    /// Stator current (rms, line) in A.
    pub stator_current_a: f64,
    /// Rotor current referred to the stator, A.
    pub rotor_current_a: f64,
    /// Magnetising current, A.
    pub magnetising_current_a: f64,
    /// Power factor; negative when feeding back.
    pub power_factor: f64,
    /// Electrical power at the motor terminals in W.
    pub input_power_w: f64,
    pub air_gap_power_w: f64,
    pub shaft_power_w: f64,
    pub loss_stator_w: f64,
    pub loss_rotor_w: f64,
    pub loss_core_w: f64,
    pub loss_rot_w: f64,
    /// Shaft over input power when motoring, input over shaft power when generating.
    pub efficiency: Option<f64>,
    /// Air-gap flux per unit of the rated flux (`E / f` against rated).
    pub flux_pu: f64,
}

impl OperatingPoint {
    /// All losses in W.
    pub fn losses_w(&self) -> f64 {
        self.loss_stator_w + self.loss_rotor_w + self.loss_core_w + self.loss_rot_w
    }

    fn zero(slip: f64, sync_rpm: f64) -> Self {
        Self {
            slip,
            speed_rpm: 0.0,
            sync_rpm,
            slip_hz: 0.0,
            torque_nm: 0.0,
            air_gap_torque_nm: 0.0,
            stator_current_a: 0.0,
            rotor_current_a: 0.0,
            magnetising_current_a: 0.0,
            power_factor: 1.0,
            input_power_w: 0.0,
            air_gap_power_w: 0.0,
            shaft_power_w: 0.0,
            loss_stator_w: 0.0,
            loss_rotor_w: 0.0,
            loss_core_w: 0.0,
            loss_rot_w: 0.0,
            efficiency: None,
            flux_pu: 0.0,
        }
    }
}

/// Maximum air-gap torque of the curve at a supply, motoring and generating side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Breakdown {
    /// Motoring breakdown torque in N m (positive, field sense).
    pub motoring_nm: f64,
    /// Slip at motoring breakdown.
    pub motoring_slip: f64,
    /// Generating breakdown torque in N m (positive magnitude, field sense reversed).
    pub generating_nm: f64,
    /// Slip at generating breakdown (negative).
    pub generating_slip: f64,
    /// Motoring breakdown at a slip of at most 1. When false the peak lies at standstill or
    /// beyond: a motor cannot reach it, the largest motoring torque is the standstill torque.
    pub motoring_reachable: bool,
}

/// What limits the torque delivered at a current limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TorqueLimitedBy {
    /// The current limit.
    Current,
    /// The breakdown torque (the current limit is above the current at breakdown).
    Breakdown,
}

/// Largest torque within a current limit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TorqueLimit {
    /// Air-gap torque in N m, positive magnitude in the field sense (motoring) or reversed
    /// (generating).
    pub torque_nm: f64,
    pub slip: f64,
    pub limited_by: TorqueLimitedBy,
    /// The operating point at that torque.
    pub point: OperatingPoint,
}

/// The induction motor: circuit plus its rated frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InductionMotor {
    circuit: Circuit,
    poles: u32,
    rated_hz: f64,
    rated_v_ll: f64,
    rated_rpm: f64,
    /// Air-gap emf per phase at the rated point, the flux reference for saturation.
    e_ref: f64,
    /// Rated shaft torque in N m from the model.
    rated_torque_nm: f64,
}

struct Solved {
    i1: C,
    ir: C,
    e: C,
    rfe: f64,
    xm_eff: f64,
}

impl InductionMotor {
    /// A motor from circuit values. `rated_rpm` is the speed at rated torque on rated voltage
    /// and frequency (it fixes the rated slip, the friction speed and the rated torque).
    pub fn new(
        circuit: Circuit,
        poles: u32,
        rated_v_ll: f64,
        rated_hz: f64,
        rated_rpm: f64,
    ) -> Result<Self, InputError> {
        circuit.validate()?;
        error::poles(poles)?;
        error::positive("rated_v_ll", rated_v_ll)?;
        error::positive("rated_hz", rated_hz)?;
        error::positive("rated_rpm", rated_rpm)?;
        let sync = 120.0 * rated_hz / f64::from(poles);
        if rated_rpm >= sync {
            return Err(InputError::OutOfRange {
                field: "rated_rpm",
                value: rated_rpm,
                min: 0.0,
                max: sync,
            });
        }
        let mut m = Self::unchecked(circuit, poles, rated_v_ll, rated_hz, rated_rpm);
        let s_n = 1.0 - rated_rpm / sync;
        let rated = m.solve_at(rated_hz, rated_v_ll / 3f64.sqrt(), s_n, false);
        m.e_ref = rated.e.abs();
        let op = m.point(rated_hz, rated_v_ll / 3f64.sqrt(), s_n);
        m.rated_torque_nm = op.torque_nm;
        Ok(m)
    }

    pub(super) fn unchecked(
        circuit: Circuit,
        poles: u32,
        rated_v_ll: f64,
        rated_hz: f64,
        rated_rpm: f64,
    ) -> Self {
        Self {
            circuit,
            poles,
            rated_hz,
            rated_v_ll,
            rated_rpm,
            e_ref: 1.0,
            rated_torque_nm: 0.0,
        }
    }

    pub fn circuit(&self) -> &Circuit {
        &self.circuit
    }

    pub fn poles(&self) -> u32 {
        self.poles
    }

    pub fn rated_hz(&self) -> f64 {
        self.rated_hz
    }

    pub fn rated_v_ll(&self) -> f64 {
        self.rated_v_ll
    }

    pub fn rated_rpm(&self) -> f64 {
        self.rated_rpm
    }

    /// Rated shaft torque in N m, from the circuit at the rated point.
    pub fn rated_torque_nm(&self) -> f64 {
        self.rated_torque_nm
    }

    /// Rated slip.
    pub fn rated_slip(&self) -> f64 {
        1.0 - self.rated_rpm / self.sync_rpm_at(self.rated_hz)
    }

    /// Synchronous speed in rpm at a frequency: `120 f / p` (exact).
    pub fn sync_rpm_at(&self, frequency_hz: f64) -> f64 {
        120.0 * frequency_hz / f64::from(self.poles)
    }

    fn omega_sync(&self, f: f64) -> f64 {
        TAU * f / f64::from(self.poles / 2)
    }

    /// Speed at rated torque by the constant slip speed convention: the slip speed (synchronous
    /// minus rated rpm) stays the same at every frequency in the constant-flux region
    /// (model; it neglects the stator drop, so it is least exact at a few hertz). Scaling the
    /// slip fraction instead is the usual mistake.
    pub fn rated_rpm_at(&self, frequency_hz: f64) -> f64 {
        let slip_rpm = self.sync_rpm_at(self.rated_hz) - self.rated_rpm;
        (self.sync_rpm_at(frequency_hz) - slip_rpm).max(0.0)
    }

    fn rfe_at(&self, f: f64, v_ph: f64) -> f64 {
        if self.circuit.core_loss_w <= 0.0 {
            return f64::INFINITY;
        }
        let v_n = self.rated_v_ll / 3f64.sqrt();
        let rfe_n = 3.0 * v_n * v_n / self.circuit.core_loss_w;
        rfe_n * (v_ph / v_n).powi(2) * (f / self.rated_hz).powf(-CORE_LOSS_FREQUENCY_EXPONENT)
    }

    /// Phasor solution at `f` (> 0), phase voltage and slip. `saturate` false ignores the
    /// saturation (used for the rated flux reference).
    fn solve_at(&self, f: f64, v_ph: f64, slip: f64, saturate: bool) -> Solved {
        let c = &self.circuit;
        let k = f / self.rated_hz;
        let s = if slip.abs() < MIN_SLIP {
            MIN_SLIP.copysign(if slip.is_sign_negative() { -1.0 } else { 1.0 })
        } else {
            slip
        };
        let rfe = self.rfe_at(f, v_ph);
        let v = C::new(v_ph, 0.0);
        let zs = C::new(c.rs, c.xs * k);
        let yr = C::new(c.rr / s, c.xr * k).recip();
        let g = if rfe.is_finite() { 1.0 / rfe } else { 0.0 };
        let xm_lin = c.xm * k;
        let mut xm_eff = xm_lin;
        let mut result;
        let mut guard = 0;
        loop {
            let ym = C::new(g, -1.0 / xm_eff);
            let zpar = (ym + yr).recip();
            let i1 = v / (zs + zpar);
            let e = i1 * zpar;
            let ir = e * yr;
            result = Solved {
                i1,
                ir,
                e,
                rfe,
                xm_eff,
            };
            let Some(sat) = c.saturation.filter(|_| saturate) else {
                break;
            };
            let psi = e.abs() / k / self.e_ref;
            let target = if psi > sat.knee_flux_pu {
                xm_lin * (sat.knee_flux_pu / psi).powf(sat.exponent)
            } else {
                xm_lin
            };
            guard += 1;
            if (target - xm_eff).abs() <= 1e-12 * xm_lin || guard >= 80 {
                break;
            }
            xm_eff = 0.5 * (xm_eff + target);
        }
        result
    }

    /// Operating point in the field sense at slip `slip` (f > 0 handled here).
    fn point(&self, f: f64, v_ph: f64, slip: f64) -> OperatingPoint {
        let c = &self.circuit;
        let sync_rpm = self.sync_rpm_at(f);
        if v_ph <= 0.0 {
            let mut z = OperatingPoint::zero(slip, sync_rpm);
            z.speed_rpm = sync_rpm * (1.0 - slip);
            return z;
        }
        let s = if slip.abs() < MIN_SLIP {
            MIN_SLIP
        } else {
            slip
        };
        let sol = self.solve_at(f, v_ph, s, true);
        let i1 = sol.i1.abs();
        let ir = sol.ir.abs();
        let w_s = self.omega_sync(f);
        let w = w_s * (1.0 - slip);
        let w_n = TAU * self.rated_rpm / 60.0;
        let p_in = 3.0 * v_ph * sol.i1.re;
        let p_gap = 3.0 * ir * ir * c.rr / s;
        let t_em = p_gap / w_s;
        let loss_rot = c.rot_loss_w * (w / w_n).powi(2);
        let t_rot = if w.abs() > 0.0 { loss_rot / w } else { 0.0 };
        let t_shaft = t_em - t_rot;
        let p_shaft = t_shaft * w;
        let loss_core = if sol.rfe.is_finite() {
            3.0 * sol.e.abs().powi(2) / sol.rfe
        } else {
            0.0
        };
        let efficiency = if p_in > 0.0 && p_shaft > 0.0 {
            Some(p_shaft / p_in)
        } else if p_in < 0.0 && p_shaft < 0.0 {
            Some(p_in / p_shaft)
        } else {
            None
        };
        let mag = sol.e.abs() / sol.xm_eff;
        OperatingPoint {
            slip,
            speed_rpm: sync_rpm * (1.0 - slip),
            sync_rpm,
            slip_hz: slip * f,
            torque_nm: t_shaft,
            air_gap_torque_nm: t_em,
            stator_current_a: i1,
            rotor_current_a: ir,
            magnetising_current_a: mag,
            power_factor: p_in / (3.0 * v_ph * i1),
            input_power_w: p_in,
            air_gap_power_w: p_gap,
            shaft_power_w: p_shaft,
            loss_stator_w: 3.0 * i1 * i1 * c.rs,
            loss_rotor_w: 3.0 * ir * ir * c.rr,
            loss_core_w: loss_core,
            loss_rot_w: loss_rot,
            efficiency,
            flux_pu: sol.e.abs() / (f / self.rated_hz) / self.e_ref,
        }
    }

    /// The field does not turn: DC hold. The stator carries `V / Rs` (rms-equivalent per phase)
    /// and produces no torque. Model.
    fn dc_hold(&self, supply: &Supply) -> OperatingPoint {
        let rs = self.circuit.rs.max(1e-9);
        let i = supply.phase_volts() / rs;
        let mut z = OperatingPoint::zero(1.0, 0.0);
        z.stator_current_a = i;
        z.input_power_w = 3.0 * i * i * rs;
        z.loss_stator_w = z.input_power_w;
        z
    }

    /// Operating point at a shaft speed `omega_rad_s` (signed, positive in the positive shaft
    /// sense). Torque and speed in the result are in the shaft sense: with a negative
    /// direction the field turns the negative way and the torque comes out negative.
    pub fn evaluate(
        &self,
        supply: &Supply,
        omega_rad_s: f64,
    ) -> Result<OperatingPoint, InputError> {
        error::finite("omega_rad_s", omega_rad_s)?;
        let f = supply.frequency_hz;
        if f < DC_HOLD_BELOW_HZ {
            return Ok(self.dc_hold(supply));
        }
        let sign = supply.direction.sign();
        let w_s = self.omega_sync(f);
        let slip = (w_s - sign * omega_rad_s) / w_s;
        Ok(self.in_shaft_sense(self.point(f, supply.phase_volts(), slip), sign))
    }

    /// Operating point at a slip in the field sense.
    pub fn at_slip(&self, supply: &Supply, slip: f64) -> Result<OperatingPoint, InputError> {
        error::finite("slip", slip)?;
        let f = supply.frequency_hz;
        if f < DC_HOLD_BELOW_HZ {
            return Ok(self.dc_hold(supply));
        }
        Ok(self.in_shaft_sense(
            self.point(f, supply.phase_volts(), slip),
            supply.direction.sign(),
        ))
    }

    fn in_shaft_sense(&self, mut p: OperatingPoint, sign: f64) -> OperatingPoint {
        p.torque_nm *= sign;
        p.air_gap_torque_nm *= sign;
        p.speed_rpm *= sign;
        p.shaft_power_w = p.torque_nm * p.speed_rpm * TAU / 60.0;
        p
    }

    /// Torque-speed curve in the shaft sense from standstill to synchronous speed and a little
    /// beyond (`points` samples, at least 2): `(rpm, shaft torque N m)`.
    pub fn torque_speed_curve(
        &self,
        supply: &Supply,
        points: usize,
    ) -> Result<Vec<(f64, f64)>, InputError> {
        let n = points.max(2);
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            // From slip 1 to slip -0.15.
            let slip = 1.0 - 1.15 * i as f64 / (n - 1) as f64;
            let p = self.at_slip(supply, slip)?;
            out.push((p.speed_rpm, p.torque_nm));
        }
        Ok(out)
    }

    /// Breakdown torque and slip, motoring and generating. Closed form (Thevenin) for the
    /// linear circuit, exact for the entered values; a numeric search when saturation is on.
    /// The torque is the air-gap torque, in N m, positive magnitudes. At a frequency below
    /// [`DC_HOLD_BELOW_HZ`] everything is zero.
    pub fn breakdown(&self, supply: &Supply) -> Breakdown {
        let f = supply.frequency_hz;
        let v_ph = supply.phase_volts();
        if f < DC_HOLD_BELOW_HZ || v_ph <= 0.0 {
            return Breakdown {
                motoring_nm: 0.0,
                motoring_slip: 0.0,
                generating_nm: 0.0,
                generating_slip: 0.0,
                motoring_reachable: false,
            };
        }
        if self.circuit.saturation.is_some() {
            return self.breakdown_numeric(f, v_ph);
        }
        self.breakdown_closed_form(f, v_ph)
    }

    fn breakdown_closed_form(&self, f: f64, v_ph: f64) -> Breakdown {
        let c = &self.circuit;
        let k = f / self.rated_hz;
        let zs = C::new(c.rs, c.xs * k);
        let g = if self.rfe_at(f, v_ph).is_finite() {
            1.0 / self.rfe_at(f, v_ph)
        } else {
            0.0
        };
        let zm = C::new(g, -1.0 / (c.xm * k)).recip();
        let vth = C::new(v_ph, 0.0) * zm / (zm + zs);
        let zth = zm * zs / (zm + zs);
        let x_tot = zth.im + c.xr * k;
        let root = zth.re.hypot(x_tot);
        let w_s = self.omega_sync(f);
        let v2 = vth.abs().powi(2);
        let sb = c.rr / root;
        Breakdown {
            motoring_nm: 3.0 * v2 / (2.0 * w_s * (zth.re + root)),
            motoring_slip: sb,
            generating_nm: 3.0 * v2 / (2.0 * w_s * (root - zth.re)),
            generating_slip: -sb,
            motoring_reachable: sb <= 1.0,
        }
    }

    fn breakdown_numeric(&self, f: f64, v_ph: f64) -> Breakdown {
        let t = |s: f64| self.point(f, v_ph, s).air_gap_torque_nm;
        let search = |sign: f64| -> (f64, f64) {
            let mut best = (0.0, 0.0);
            let n: usize = 800;
            let (lo, hi) = (1e-4f64, 20.0f64);
            let mut at: usize = 0;
            for i in 0..=n {
                let s = lo * (hi / lo).powf(i as f64 / n as f64);
                let v = sign * t(sign * s);
                if v > best.1 {
                    best = (s, v);
                    at = i;
                }
            }
            let mut a = lo * (hi / lo).powf(at.saturating_sub(1) as f64 / n as f64);
            let mut b = lo * (hi / lo).powf((at + 1).min(n) as f64 / n as f64);
            let phi = (5f64.sqrt() - 1.0) / 2.0;
            for _ in 0..80 {
                let x1 = b - phi * (b - a);
                let x2 = a + phi * (b - a);
                if sign * t(sign * x1) > sign * t(sign * x2) {
                    b = x2;
                } else {
                    a = x1;
                }
            }
            let s = 0.5 * (a + b);
            (s, sign * t(sign * s))
        };
        let (sm, tm) = search(1.0);
        let (sg, tg) = search(-1.0);
        Breakdown {
            motoring_nm: tm,
            motoring_slip: sm,
            generating_nm: tg,
            generating_slip: -sg,
            motoring_reachable: sm <= 1.0,
        }
    }

    /// The steady operating point at which the motor delivers `torque_nm` on the shaft in the
    /// field sense (positive = motoring, negative = the load drives the motor and it feeds
    /// back). `None` when the torque is beyond the breakdown torque of the stable branch.
    pub fn steady_state(
        &self,
        supply: &Supply,
        torque_nm: f64,
    ) -> Result<Option<OperatingPoint>, InputError> {
        error::finite("torque_nm", torque_nm)?;
        let f = supply.frequency_hz;
        let v_ph = supply.phase_volts();
        if f < DC_HOLD_BELOW_HZ || v_ph <= 0.0 {
            return Ok(if torque_nm == 0.0 {
                Some(self.dc_hold(supply))
            } else {
                None
            });
        }
        let b = self.breakdown(supply);
        let t = |s: f64| self.point(f, v_ph, s).torque_nm;
        // The stable branch runs from the generating breakdown slip to the motoring one,
        // capped at standstill.
        let (mut lo, mut hi) = (b.generating_slip, b.motoring_slip.min(1.0));
        if torque_nm > t(hi) || torque_nm < t(lo) {
            return Ok(None);
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if t(mid) < torque_nm {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let slip = 0.5 * (lo + hi);
        Ok(Some(self.in_shaft_sense(
            self.point(f, v_ph, slip),
            supply.direction.sign(),
        )))
    }

    /// Largest torque within a stator current limit on the stable branch. `generating`
    /// asks for the braking side. Model (it is the circuit at the limit, not the drive's
    /// current-control behaviour).
    pub fn torque_at_current_limit(
        &self,
        supply: &Supply,
        current_limit_a: f64,
        generating: bool,
    ) -> Result<TorqueLimit, InputError> {
        error::positive("current_limit_a", current_limit_a)?;
        let f = supply.frequency_hz;
        let v_ph = supply.phase_volts();
        let sign = supply.direction.sign();
        if f < DC_HOLD_BELOW_HZ || v_ph <= 0.0 {
            return Ok(TorqueLimit {
                torque_nm: 0.0,
                slip: 0.0,
                limited_by: TorqueLimitedBy::Current,
                point: self.dc_hold(supply),
            });
        }
        let b = self.breakdown(supply);
        let s_max = if generating {
            b.generating_slip
        } else {
            b.motoring_slip.min(1.0)
        };
        let current = |s: f64| self.point(f, v_ph, s).stator_current_a;
        let (slip, limited_by) = if current(s_max) <= current_limit_a {
            (s_max, TorqueLimitedBy::Breakdown)
        } else if current(0.0) >= current_limit_a {
            (0.0, TorqueLimitedBy::Current)
        } else {
            let (mut lo, mut hi) = (0.0, s_max);
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                if current(mid) < current_limit_a {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (0.5 * (lo + hi), TorqueLimitedBy::Current)
        };
        let p = self.in_shaft_sense(self.point(f, v_ph, slip), sign);
        Ok(TorqueLimit {
            torque_nm: self.point(f, v_ph, slip).air_gap_torque_nm.abs(),
            slip,
            limited_by,
            point: p,
        })
    }

    /// Continuous torque in N m a self-ventilated motor can give at `speed_rpm` without
    /// exceeding its rated losses: copper loss goes as torque squared, the allowed loss as
    /// `a + (1 - a) n / n_rated`, so `T = T_rated sqrt(a + (1 - a) n / n_rated)`, with the ratio
    /// capped at 1. Model.
    pub fn continuous_torque_nm(&self, speed_rpm: f64, cooling: &SelfVentilation) -> f64 {
        let a = cooling.standstill_loss_fraction.clamp(0.0, 1.0);
        let x = (speed_rpm.abs() / self.rated_rpm).min(1.0);
        self.rated_torque_nm * (a + (1.0 - a) * x).sqrt()
    }
}
