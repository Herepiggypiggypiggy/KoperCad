//! Three-phase induction motor start model.
//!
//! The motor is the standard per-phase equivalent circuit (stator `r1 + jx1`,
//! magnetising `jxm` in parallel with the rotor `r2/s + jx2`), evaluated at
//! each slip. The start is the mechanical equation `J dω/dt = Te - Tl`
//! integrated in time while the electrical side follows at once (quasi-steady
//! state, which is the usual model for start-up studies).
//!
//! What this is not: the electromagnetic transient of the first cycles (the
//! asymmetric inrush peak, the spike when a star-delta starter re-closes) is
//! not modelled, so the peak currents reported are the RMS values at each
//! speed. Iron loss and saturation are not modelled either, so currents near
//! breakdown torque are slightly low for a saturating motor. Parameters
//! fitted from a nameplate are an estimate; enter measured values when known.

use std::f64::consts::PI;

use super::Complex;

/// How the three windings are connected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connection {
    Star,
    Delta,
}

/// The per-winding equivalent circuit and the nameplate it belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motor {
    pub r1: f64,
    pub x1: f64,
    pub r2: f64,
    pub x2: f64,
    pub xm: f64,
    pub hz: f64,
    pub pole_pairs: u32,
    /// Line voltage the winding is rated for in the connection it runs in.
    pub rated_ll: f64,
    /// Connection in normal running.
    pub connection: Connection,
    pub rated_power_w: f64,
    pub rated_rpm: f64,
}

/// The supply seen by the motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supply {
    /// Line-to-line voltage at the source.
    pub v_ll: f64,
    /// Impedance of the cable and transformer ahead of the motor, per phase.
    pub source: Complex,
}

impl Supply {
    pub fn stiff(v_ll: f64) -> Supply {
        Supply {
            v_ll,
            source: Complex::ZERO,
        }
    }
}

/// What the motor draws in one state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Voltage across one winding, RMS.
    pub winding_voltage: f64,
    /// Current in the supply line, RMS.
    pub line_current: f64,
    /// Electromagnetic torque in N·m.
    pub torque: f64,
    /// Power factor at the motor terminals.
    pub power_factor: f64,
}

impl Motor {
    /// Synchronous speed in rad/s (mechanical).
    pub fn omega_sync(&self) -> f64 {
        2.0 * PI * self.hz / f64::from(self.pole_pairs)
    }

    pub fn sync_rpm(&self) -> f64 {
        self.omega_sync() * 60.0 / (2.0 * PI)
    }

    pub fn rated_torque(&self) -> f64 {
        self.rated_power_w / (self.rated_rpm * 2.0 * PI / 60.0)
    }

    /// Rated slip from the nameplate speed.
    pub fn rated_slip(&self) -> f64 {
        1.0 - self.rated_rpm / self.sync_rpm()
    }

    /// Input impedance of one winding at slip `s`.
    pub fn z_in(&self, s: f64) -> Complex {
        let s = if s.abs() < 1e-9 { 1e-9 } else { s };
        let zr = Complex::new(self.r2 / s, self.x2);
        let zm = Complex::new(0.0, self.xm);
        Complex::new(self.r1, self.x1) + (zr * zm) / (zr + zm)
    }

    /// Torque for a winding voltage `vw` (RMS) at slip `s`, N·m (three phases).
    pub fn torque(&self, vw: f64, s: f64) -> f64 {
        let s = if s.abs() < 1e-9 { 1e-9 } else { s };
        let i1 = Complex::real(vw) / self.z_in(s);
        let zr = Complex::new(self.r2 / s, self.x2);
        let i2 = i1 * Complex::new(0.0, self.xm) / (zr + Complex::new(0.0, self.xm));
        3.0 * i2.abs().powi(2) * self.r2 / s / self.omega_sync()
    }

    /// The state at slip `s`: `connection` is how the windings are connected
    /// right now (a star-delta starter starts them in star), `ratio` the
    /// voltage reduction (1 = full), `transformer` whether it comes from an
    /// auto-transformer (the supply impedance is seen through it and the line
    /// current is lower by the ratio) or from a thyristor soft starter.
    pub fn point(
        &self,
        supply: Supply,
        connection: Connection,
        ratio: f64,
        transformer: bool,
        s: f64,
    ) -> Point {
        let zw = self.z_in(s);
        let zy = match connection {
            Connection::Star => zw,
            Connection::Delta => zw / Complex::real(3.0),
        };
        let a = ratio;
        let zs = if transformer {
            supply.source * (a * a)
        } else {
            supply.source
        };
        let vph = Complex::real(supply.v_ll / 3f64.sqrt() * a);
        let i_motor = vph / (zs + zy);
        let v_y = i_motor * zy;
        let vw = match connection {
            Connection::Star => v_y.abs(),
            Connection::Delta => v_y.abs() * 3f64.sqrt(),
        };
        let line = if transformer {
            i_motor.abs() * a
        } else {
            i_motor.abs()
        };
        let pf = (v_y / i_motor).arg().cos();
        Point {
            winding_voltage: vw,
            line_current: line,
            torque: self.torque(vw, s),
            power_factor: pf,
        }
    }

    /// Torque at standstill for a stiff supply at the rated voltage.
    pub fn locked_rotor_torque(&self) -> f64 {
        self.torque(self.winding_rated(), 1.0)
    }

    /// Locked-rotor line current for a stiff supply at the rated voltage.
    pub fn locked_rotor_current(&self) -> f64 {
        self.point(
            Supply::stiff(self.rated_ll),
            self.connection,
            1.0,
            false,
            1.0,
        )
        .line_current
    }

    pub fn rated_current(&self) -> f64 {
        self.point(
            Supply::stiff(self.rated_ll),
            self.connection,
            1.0,
            false,
            self.rated_slip(),
        )
        .line_current
    }

    /// Breakdown (pull-out) torque and the slip it occurs at, rated voltage.
    pub fn breakdown(&self) -> (f64, f64) {
        let vw = self.winding_rated();
        (1..=2000)
            .map(|i| {
                let s = f64::from(i) / 2000.0;
                (self.torque(vw, s), s)
            })
            .fold((0.0, 0.0), |m, x| if x.0 > m.0 { x } else { m })
    }

    fn winding_rated(&self) -> f64 {
        match self.connection {
            Connection::Star => self.rated_ll / 3f64.sqrt(),
            Connection::Delta => self.rated_ll,
        }
    }
}

/// Nameplate and catalogue values; what [`Motor::from_nameplate`] fits to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Nameplate {
    pub power_w: f64,
    pub v_ll: f64,
    pub hz: f64,
    pub rpm: f64,
    pub current: f64,
    pub power_factor: f64,
    pub efficiency: f64,
    /// Locked-rotor current as a multiple of rated current (I_LR / I_N, e.g. 7).
    pub lr_current_ratio: f64,
    pub connection: Connection,
}

/// Why a fit failed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FitError {
    /// The values contradict each other (the fit does not converge).
    Inconsistent { residual: f64 },
    /// A value is zero, negative or not a number.
    Invalid,
}

impl Motor {
    /// Estimates the equivalent circuit from nameplate values. Leakage is
    /// split 40 % stator and 60 % rotor (NEMA design B practice); the four
    /// remaining parameters are fitted so that rated current, power factor,
    /// shaft power and locked-rotor current are reproduced. This is an
    /// estimate: torque at standstill and breakdown torque come out of the
    /// model, they are not inputs.
    pub fn from_nameplate(n: Nameplate) -> Result<Motor, FitError> {
        let vals = [
            n.power_w,
            n.v_ll,
            n.hz,
            n.rpm,
            n.current,
            n.power_factor,
            n.efficiency,
            n.lr_current_ratio,
        ];
        if vals.iter().any(|v| !(v.is_finite() && *v > 0.0))
            || n.power_factor > 1.0
            || n.efficiency > 1.0
        {
            return Err(FitError::Invalid);
        }
        // Pole pairs from the nearest synchronous speed above the rated one.
        let pp = (1..=12u32)
            .rev()
            .find(|&p| 60.0 * n.hz / f64::from(p) > n.rpm)
            .ok_or(FitError::Invalid)?;
        let sync = 60.0 * n.hz / f64::from(pp);
        let slip = 1.0 - n.rpm / sync;
        // Per-winding quantities.
        let (vw, iw_n) = match n.connection {
            Connection::Star => (n.v_ll / 3f64.sqrt(), n.current),
            Connection::Delta => (n.v_ll, n.current / 3f64.sqrt()),
        };
        let p_out = n.power_w;
        let p_in = 3.0 * vw * iw_n * n.power_factor;
        // Efficiency, power factor and current must describe the same
        // operating point.
        let implied = p_out / p_in;
        if (implied / n.efficiency - 1.0).abs() > 0.03 {
            return Err(FitError::Inconsistent {
                residual: (implied / n.efficiency - 1.0).abs(),
            });
        }
        let target = [
            iw_n,
            -n.power_factor.acos(),
            p_out,
            iw_n * n.lr_current_ratio,
        ];
        // Starting point from the loss balance at rated load.
        let p_ag = p_out / (1.0 - slip);
        let r1_0 = ((p_in - p_ag).max(0.02 * p_in) / (3.0 * iw_n * iw_n)).max(1e-3);
        let r2_0 = (slip * p_ag / (3.0 * (0.95 * iw_n).powi(2))).max(1e-3);
        let z_lr = vw / (iw_n * n.lr_current_ratio);
        let x_lr0 = (z_lr * z_lr - (r1_0 + r2_0).powi(2))
            .max(0.05 * z_lr * z_lr)
            .sqrt();
        let xm0 = vw / (iw_n * (1.0 - n.power_factor * n.power_factor).sqrt()) * 1.1;
        let mut x: [f64; 4] = [r1_0, r2_0, x_lr0, xm0];
        let build = |x: &[f64; 4]| Motor {
            r1: x[0],
            r2: x[1],
            x1: 0.4 * x[2],
            x2: 0.6 * x[2],
            xm: x[3],
            hz: n.hz,
            pole_pairs: pp,
            rated_ll: n.v_ll,
            connection: n.connection,
            rated_power_w: n.power_w,
            rated_rpm: n.rpm,
        };
        let omega_m = n.rpm * 2.0 * PI / 60.0;
        let model = |x: &[f64; 4]| -> [f64; 4] {
            let m = build(x);
            let i1 = Complex::real(vw) / m.z_in(slip);
            let shaft = m.torque(vw, slip) * omega_m;
            // The nameplate power is shaft power; the model gives airgap
            // power minus nothing, so subtract the rotor copper loss share
            // already in the slip: shaft ≈ airgap * (1 - s).
            let _ = i1;
            [
                i1.abs(),
                i1.arg(),
                shaft,
                (Complex::real(vw) / m.z_in(1.0)).abs(),
            ]
        };
        let scale = [iw_n, 1.0, p_out, iw_n * n.lr_current_ratio];
        let residual = |x: &[f64; 4]| -> [f64; 4] {
            let y = model(x);
            std::array::from_fn(|i| (y[i] - target[i]) / scale[i])
        };
        let norm = |r: &[f64; 4]| r.iter().map(|v| v * v).sum::<f64>().sqrt();
        // Levenberg-Marquardt on the logarithms of the parameters, which
        // keeps every one of them positive.
        let at = |u: &[f64; 4]| -> [f64; 4] { u.map(f64::exp) };
        let mut u = x.map(f64::ln);
        let mut mu = 1e-2;
        for _ in 0..500 {
            let r = residual(&at(&u));
            if norm(&r) < 1e-12 {
                break;
            }
            let mut jac = [[0.0; 4]; 4];
            for c in 0..4 {
                let mut up = u;
                up[c] += 1e-7;
                let rp = residual(&at(&up));
                for row in 0..4 {
                    jac[row][c] = (rp[row] - r[row]) / 1e-7;
                }
            }
            // (J'J + mu I) step = -J' r
            let mut a = [[0.0; 4]; 4];
            let mut g = [0.0; 4];
            for i in 0..4 {
                for j in 0..4 {
                    a[i][j] = (0..4).map(|k| jac[k][i] * jac[k][j]).sum::<f64>();
                }
                a[i][i] += mu;
                g[i] = -(0..4).map(|k| jac[k][i] * r[k]).sum::<f64>();
            }
            let Some(step) = solve4(a, g) else {
                mu *= 10.0;
                continue;
            };
            let trial: [f64; 4] = std::array::from_fn(|i| u[i] + step[i].clamp(-1.0, 1.0));
            if norm(&residual(&at(&trial))) < norm(&r) {
                u = trial;
                mu = (mu * 0.3).max(1e-12);
            } else {
                mu *= 5.0;
                if mu > 1e12 {
                    break;
                }
            }
        }
        x = at(&u);
        let res = norm(&residual(&x));
        if res > 1e-3 {
            return Err(FitError::Inconsistent { residual: res });
        }
        Ok(build(&x))
    }
}

/// Gaussian elimination with partial pivoting for the 4x4 Newton step.
fn solve4(mut a: [[f64; 4]; 4], mut b: [f64; 4]) -> Option<[f64; 4]> {
    for k in 0..4 {
        let p = (k..4).max_by(|&i, &j| a[i][k].abs().total_cmp(&a[j][k].abs()))?;
        if a[p][k].abs() < 1e-14 {
            return None;
        }
        a.swap(k, p);
        b.swap(k, p);
        for r in k + 1..4 {
            let f = a[r][k] / a[k][k];
            for c in k..4 {
                a[r][c] -= f * a[k][c];
            }
            b[r] -= f * b[k];
        }
    }
    let mut x = [0.0; 4];
    for r in (0..4).rev() {
        let s: f64 = (r + 1..4).map(|c| a[r][c] * x[c]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    Some(x)
}

/// The mechanical load.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Load {
    /// Constant torque (conveyor, hoist, compressor): N·m at any speed.
    Constant(f64),
    /// Fans and centrifugal pumps: `torque = nm * (ω/ω_sync)²`, with `nm`
    /// the torque at synchronous speed.
    Quadratic(f64),
    /// Constant torque plus a breakaway torque that applies at standstill.
    Breakaway { running: f64, standstill: f64 },
}

impl Load {
    pub fn torque(&self, omega: f64, omega_sync: f64) -> f64 {
        match *self {
            Load::Constant(t) => t,
            Load::Quadratic(t) => t * (omega / omega_sync).powi(2),
            Load::Breakaway {
                running,
                standstill,
            } => {
                if omega < 0.02 * omega_sync {
                    standstill
                } else {
                    running
                }
            }
        }
    }
}

/// When a starter changes over from the first stage to the second.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Changeover {
    AtTime(f64),
    /// At this fraction of synchronous speed (0.85 is typical).
    AtSpeed(f64),
}

/// How the motor is connected to the supply.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Starter {
    Direct,
    /// Star first, then delta. `open_s` is the time the motor is off the
    /// supply during changeover (open transition).
    StarDelta {
        changeover: Changeover,
        open_s: f64,
    },
    /// Reduced voltage from an auto-transformer tap, then full voltage.
    AutoTransformer {
        tap: f64,
        changeover: Changeover,
    },
    /// Thyristor soft starter: voltage ramps from `initial` to 1 in `ramp_s`.
    SoftStart {
        initial: f64,
        ramp_s: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    First,
    /// Off the supply during changeover.
    Open,
    Run,
}

/// One sample of a start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub t: f64,
    pub rpm: f64,
    pub slip: f64,
    pub motor_torque: f64,
    pub load_torque: f64,
    pub line_current: f64,
    pub winding_voltage: f64,
    pub stage: Stage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StartResult {
    pub samples: Vec<Sample>,
    /// Time until the speed is within 2 % of its final value, if reached.
    pub start_time: Option<f64>,
    /// The motor never left standstill (load torque above motor torque).
    pub stalled: bool,
    pub final_rpm: f64,
    pub peak_line_current: f64,
    /// Integral of line current squared over the run, A²·s: the heating the
    /// protection and the motor see during the start.
    pub i2t: f64,
}

/// Runs a start with moment of inertia `inertia` (kg·m², motor and load
/// together) until `t_end` seconds, step `dt`.
pub fn start(
    motor: &Motor,
    supply: Supply,
    starter: Starter,
    load: Load,
    inertia: f64,
    t_end: f64,
    dt: f64,
) -> StartResult {
    let ws = motor.omega_sync();
    let mut w = 0.0f64;
    let mut samples = Vec::new();
    let mut stage = Stage::First;
    let mut open_until = 0.0;
    let mut i2t = 0.0;
    let mut peak = 0.0f64;
    let steps = (t_end / dt).round() as usize;
    // State of the supply connection at (t, w): returns the electrical point
    // and the stage.
    let electrical = |stage: Stage, t: f64, w: f64| -> Option<Point> {
        let s = 1.0 - w / ws;
        match (starter, stage) {
            (_, Stage::Open) => None,
            (Starter::Direct, _) => Some(motor.point(supply, motor.connection, 1.0, false, s)),
            (Starter::StarDelta { .. }, Stage::First) => {
                Some(motor.point(supply, Connection::Star, 1.0, false, s))
            }
            (Starter::StarDelta { .. }, Stage::Run) => {
                Some(motor.point(supply, Connection::Delta, 1.0, false, s))
            }
            (Starter::AutoTransformer { tap, .. }, Stage::First) => {
                Some(motor.point(supply, motor.connection, tap, true, s))
            }
            (Starter::AutoTransformer { .. }, Stage::Run) => {
                Some(motor.point(supply, motor.connection, 1.0, false, s))
            }
            (Starter::SoftStart { initial, ramp_s }, _) => {
                let a = (initial + (1.0 - initial) * (t / ramp_s)).min(1.0);
                Some(motor.point(supply, motor.connection, a, false, s))
            }
        }
    };
    let torque_at = |stage: Stage, t: f64, w: f64| -> f64 {
        let te = electrical(stage, t, w).map_or(0.0, |p| p.torque);
        te - load.torque(w, ws)
    };
    for step in 0..=steps {
        let t = step as f64 * dt;
        // Stage logic.
        if stage == Stage::First {
            let (changeover, gap) = match starter {
                Starter::StarDelta { changeover, open_s } => (Some(changeover), open_s),
                Starter::AutoTransformer { changeover, .. } => (Some(changeover), 0.0),
                _ => (None, 0.0),
            };
            if let Some(c) = changeover {
                let go = match c {
                    Changeover::AtTime(x) => t >= x,
                    Changeover::AtSpeed(f) => w >= f * ws,
                };
                if go && gap > 0.0 {
                    stage = Stage::Open;
                    open_until = t + gap;
                } else if go {
                    stage = Stage::Run;
                }
            }
        } else if stage == Stage::Open && t >= open_until {
            stage = Stage::Run;
        }
        let point = electrical(stage, t, w);
        let (i_line, vw, te) = point.map_or((0.0, 0.0, 0.0), |p| {
            (p.line_current, p.winding_voltage, p.torque)
        });
        peak = peak.max(i_line);
        i2t += i_line * i_line * dt;
        samples.push(Sample {
            t,
            rpm: w * 60.0 / (2.0 * PI),
            slip: 1.0 - w / ws,
            motor_torque: te,
            load_torque: load.torque(w, ws),
            line_current: i_line,
            winding_voltage: vw,
            stage,
        });
        // RK4 on the speed; the torque depends on speed (and time, for the
        // ramp), so the four slopes use the intermediate points.
        let f = |t: f64, w: f64| torque_at(stage, t, w.max(0.0)) / inertia;
        let k1 = f(t, w);
        let k2 = f(t + dt / 2.0, w + dt / 2.0 * k1);
        let k3 = f(t + dt / 2.0, w + dt / 2.0 * k2);
        let k4 = f(t + dt, w + dt * k3);
        w = (w + dt / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4)).clamp(0.0, ws);
    }
    let final_rpm = samples.last().map_or(0.0, |s| s.rpm);
    let two_stage = matches!(
        starter,
        Starter::StarDelta { .. } | Starter::AutoTransformer { .. }
    );
    let start_time = samples
        .iter()
        .find(|s| {
            final_rpm > 0.0 && s.rpm >= 0.98 * final_rpm && (s.stage == Stage::Run || !two_stage)
        })
        .map(|s| s.t);
    StartResult {
        stalled: final_rpm < 1.0,
        samples,
        start_time,
        final_rpm,
        peak_line_current: peak,
        i2t,
    }
}
