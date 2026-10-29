//! Two closed-loop winches on one load (lane L2c): shared load, rope stretch, speed mismatch,
//! synchronisation and one drive tripping.
//!
//! The load is a rigid beam of mass `M` and length `L` hanging on two vertical ropes, one at
//! each end; the centre of gravity sits `e` from the middle towards winch A. Each rope is a
//! spring-damper: it pulls only when taut, `F = k (x - y)` with `x` the payout position of the
//! drum (hook equivalent, positive up) and `y` the pick-up point of the beam. The beam is
//! either free to tilt or guided (tilt locked, as between guide rails).
//!
//! Exact by definition (statics of the stated model): with a free beam and vertical ropes the
//! rope forces are `F_A = M g (1/2 + e/L)` and `F_B = M g (1/2 - e/L)` whatever the drums did,
//! and the tilt is `(x_A - x_B) / L` for small angles. With a guided beam and equal rope
//! stiffness `k` the forces differ by `k (x_A - x_B)`. Model: rope stiffness and damping,
//! slack ropes, small-tilt geometry with vertical ropes (valid only for a few degrees; the
//! snapshot says when it is not), the synchronisation controllers.

use std::f64::consts::TAU;

use novo_drive::InputError;
use novo_drive::closed_loop::{
    ClFault, ClInputs, ClOutputs, ClState, ClosedLoopWinch, RunMode, WinchSnapshot,
};

const G: f64 = novo_drive::GRAVITY;

/// Largest substep, as a fraction of the shortest natural period of the rope-mass system.
const STEP_FRACTION: f64 = 0.08;
const MAX_SUBSTEPS: usize = 400;

fn positive(field: &'static str, v: f64) -> Result<f64, InputError> {
    if !v.is_finite() {
        Err(InputError::NotFinite { field })
    } else if v <= 0.0 {
        Err(InputError::NotPositive { field, value: v })
    } else {
        Ok(v)
    }
}

/// A wire rope as a spring with damping, the stiffness measured at the hook.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rope {
    stiffness_n_m: f64,
    /// Free rope length at the start (0 = constant stiffness).
    length_m: f64,
    damping_ratio: f64,
}

impl Rope {
    /// Constant stiffness `k` in N/m. `damping_ratio` is 0 (none) to 1 (critical for half the beam).
    pub fn with_stiffness(stiffness_n_m: f64, damping_ratio: f64) -> Result<Self, InputError> {
        positive("rope stiffness", stiffness_n_m)?;
        if !(damping_ratio.is_finite() && (0.0..=2.0).contains(&damping_ratio)) {
            return Err(InputError::OutOfRange {
                field: "damping_ratio",
                value: damping_ratio,
                min: 0.0,
                max: 2.0,
            });
        }
        Ok(Self {
            stiffness_n_m,
            length_m: 0.0,
            damping_ratio,
        })
    }

    /// From the metal cross-section (mm2), the free length (m) and an effective modulus (GPa):
    /// `k = E A / L`, growing as the free length shortens when the hook rises. The modulus of a
    /// wire rope is well below that of steel and depends on make and load; it is an entered value.
    pub fn from_steel(
        area_mm2: f64,
        length_m: f64,
        modulus_gpa: f64,
        damping_ratio: f64,
    ) -> Result<Self, InputError> {
        positive("rope area", area_mm2)?;
        positive("rope length", length_m)?;
        positive("rope modulus", modulus_gpa)?;
        let mut r = Self::with_stiffness(
            modulus_gpa * 1e9 * area_mm2 * 1e-6 / length_m,
            damping_ratio,
        )?;
        r.length_m = length_m;
        Ok(r)
    }

    pub fn stiffness_n_m(&self) -> f64 {
        self.stiffness_n_m
    }

    /// Stiffness when the drum has paid `x_m` metres (negative = lowered).
    fn stiffness_at(&self, x_m: f64) -> f64 {
        if self.length_m > 0.0 {
            let l = (self.length_m - x_m).max(0.25 * self.length_m).max(0.1);
            self.stiffness_n_m * self.length_m / l
        } else {
            self.stiffness_n_m
        }
    }
}

/// The load.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Beam {
    pub mass_kg: f64,
    /// Distance between the two pick-up points, m.
    pub length_m: f64,
    /// Centre of gravity from the middle, towards winch A, m.
    pub cog_offset_m: f64,
    /// Tilt locked (guide rails): the stretch of the ropes then carries the mismatch.
    pub guided: bool,
}

impl Beam {
    pub fn new(
        mass_kg: f64,
        length_m: f64,
        cog_offset_m: f64,
        guided: bool,
    ) -> Result<Self, InputError> {
        positive("beam mass", mass_kg)?;
        positive("beam length", length_m)?;
        if !cog_offset_m.is_finite() || cog_offset_m.abs() >= 0.5 * length_m {
            return Err(InputError::OutOfRange {
                field: "cog_offset_m",
                value: cog_offset_m,
                min: -0.5 * length_m,
                max: 0.5 * length_m,
            });
        }
        Ok(Self {
            mass_kg,
            length_m,
            cog_offset_m,
            guided,
        })
    }

    /// Static rope forces `(F_A, F_B)` in N.
    pub fn static_forces_n(&self) -> (f64, f64) {
        let w = self.mass_kg * G;
        (
            w * (0.5 + self.cog_offset_m / self.length_m),
            w * (0.5 - self.cog_offset_m / self.length_m),
        )
    }

    fn inertia_kg_m2(&self) -> f64 {
        self.mass_kg
            * (self.length_m * self.length_m / 12.0 + self.cog_offset_m * self.cog_offset_m)
    }
}

/// How winch B follows winch A.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SyncMode {
    /// Both get the same hook speed reference and nothing corrects a drift.
    Independent,
    /// B adds `gain` m/s of speed per metre of position difference to its reference (position
    /// measured at the encoders), limited to `limit_m_s`.
    PositionLock { gain_per_s: f64, limit_m_s: f64 },
    /// B runs in torque control with `share` times the torque of A and its speed held at the
    /// reference plus `margin` (fraction), like a master-follower pair.
    TorqueFollower { share: f64, margin: f64 },
}

/// What happens to the healthy winch when the other drive faults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartnerAction {
    /// A hard-wired or bus link: the partner gets a fast stop.
    StopBoth,
    /// No link: the partner keeps following the command.
    KeepRunning,
}

/// Supervision of the pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Supervision {
    /// Both stop when the encoder positions differ by more than this, m (0 = off).
    pub max_position_error_m: f64,
    pub on_partner_fault: PartnerAction,
    /// Rated rope load, N, for the overload flag of the snapshot.
    pub rope_swl_n: f64,
    /// Largest tilt, degrees, for which the vertical-rope model is trusted.
    pub model_tilt_limit_deg: f64,
}

impl Supervision {
    pub fn new(rope_swl_n: f64) -> Self {
        Self {
            max_position_error_m: 0.0,
            on_partner_fault: PartnerAction::StopBoth,
            rope_swl_n,
            model_tilt_limit_deg: 5.0,
        }
    }
}

/// The command to the pair for one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairCommand {
    pub run: bool,
    /// Hook speed in m/s, positive raises.
    pub hook_speed_m_s: f64,
    /// STO channels per winch.
    pub sto: [(bool, bool); 2],
    pub external_fault: [bool; 2],
    pub fault_reset: bool,
}

impl PairCommand {
    pub fn run(hook_speed_m_s: f64) -> Self {
        Self {
            run: true,
            hook_speed_m_s,
            ..Self::default()
        }
    }
}

impl Default for PairCommand {
    fn default() -> Self {
        Self {
            run: false,
            hook_speed_m_s: 0.0,
            sto: [(true, true); 2],
            external_fault: [false; 2],
            fault_reset: false,
        }
    }
}

/// Why the pair stopped itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncFault {
    PositionError,
    PartnerFault,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairSnapshot {
    pub time_s: f64,
    pub winches: [WinchSnapshot; 2],
    /// Rope forces (A, B) in N.
    pub force_n: [f64; 2],
    /// Share of the weight carried by A.
    pub share_a: f64,
    /// Beam centre height in m.
    pub height_m: f64,
    pub tilt_deg: f64,
    /// Encoder position of each drum (hook equivalent), m.
    pub position_m: [f64; 2],
    /// A minus B, m.
    pub position_error_m: f64,
    /// Where the drums have really taken the hook ropes to (differs from `position_m` by a drum error), m.
    pub true_position_m: [f64; 2],
    pub overload: [bool; 2],
    /// A rope is slack.
    pub slack: [bool; 2],
    /// The tilt is beyond what the vertical-rope model covers.
    pub beyond_model: bool,
    pub sync_fault: Option<SyncFault>,
}

#[derive(Debug, Clone)]
pub struct WinchPair {
    pub a: ClosedLoopWinch,
    pub b: ClosedLoopWinch,
    ropes: [Rope; 2],
    pub beam: Beam,
    pub mode: SyncMode,
    pub supervision: Supervision,
    /// Peak rope forces seen so far, N.
    pub peak_force_n: [f64; 2],
    // Beam state.
    h: f64,
    vh: f64,
    phi: f64,
    vphi: f64,
    /// Rope pre-stretch at the start (static equilibrium), m.
    s0: [f64; 2],
    /// Hook metres per radian of each drum as built (used by the mechanics).
    mpr: [f64; 2],
    /// The same as the drives were commissioned with (used for references and encoder positions).
    mpr_nominal: [f64; 2],
    eta: [f64; 2],
    time_s: f64,
    last_torque: [f64; 2],
    sync_fault: Option<SyncFault>,
    last_force: [f64; 2],
}

impl WinchPair {
    /// The winches start with the ropes loaded for the static forces of the beam.
    pub fn new(
        a: ClosedLoopWinch,
        b: ClosedLoopWinch,
        ropes: [Rope; 2],
        beam: Beam,
        mode: SyncMode,
        supervision: Supervision,
    ) -> Self {
        let (fa, fb) = beam.static_forces_n();
        let mpr = [a.hoist().metres_per_rad(), b.hoist().metres_per_rad()];
        let eta = [
            a.hoist().lowering_torque_nm() / a.hoist().static_torque_nm(),
            b.hoist().lowering_torque_nm() / b.hoist().static_torque_nm(),
        ];
        Self {
            s0: [fa / ropes[0].stiffness_n_m(), fb / ropes[1].stiffness_n_m()],
            a,
            b,
            ropes,
            beam,
            mode,
            supervision,
            peak_force_n: [fa, fb],
            h: 0.0,
            vh: 0.0,
            phi: 0.0,
            vphi: 0.0,
            mpr,
            mpr_nominal: mpr,
            eta,
            time_s: 0.0,
            last_torque: [0.0; 2],
            sync_fault: None,
            last_force: [fa, fb],
        }
    }

    /// The drums as built differ from what the drives were commissioned with: the real hook
    /// travel per radian of winch A and B is the nominal one times these factors (1.01 = a
    /// drum 1 % larger than entered). The drives and their encoders keep using the nominal
    /// values, so a position lock between them does not see this error.
    pub fn with_drum_error(mut self, scale: [f64; 2]) -> Self {
        for ((real, nominal), s) in self.mpr.iter_mut().zip(self.mpr_nominal).zip(scale) {
            *real = nominal * s;
        }
        self
    }

    pub fn winch(&self, i: usize) -> &ClosedLoopWinch {
        if i == 0 { &self.a } else { &self.b }
    }

    fn encoder_positions(&self) -> [f64; 2] {
        [
            self.a.drive.encoder.position_rad() * self.mpr_nominal[0],
            self.b.drive.encoder.position_rad() * self.mpr_nominal[1],
        ]
    }

    fn pick_up(&self, i: usize) -> (f64, f64) {
        let sign = if i == 0 { 1.0 } else { -1.0 };
        let half = 0.5 * self.beam.length_m;
        (
            self.h + sign * half * self.phi.sin(),
            self.vh + sign * half * self.phi.cos() * self.vphi,
        )
    }

    fn rope_force(&self, i: usize) -> f64 {
        let w = self.winch(i);
        let x = w.shaft().angle_rad * self.mpr[i];
        let vx = w.shaft().omega_rad_s * self.mpr[i];
        let (y, vy) = self.pick_up(i);
        let k = self.ropes[i].stiffness_at(x);
        let m_half = 0.5 * self.beam.mass_kg;
        let c = 2.0 * self.ropes[i].damping_ratio * (k * m_half).sqrt();
        (k * (x - y + self.s0[i]) + c * (vx - vy)).max(0.0)
    }

    pub fn step(&mut self, cmd: &PairCommand, dt_s: f64) -> PairSnapshot {
        // Supervision from the state at the start of the tick.
        let pos = self.encoder_positions();
        let err = pos[0] - pos[1];
        let faulted = [
            self.a.drive.fault().is_some(),
            self.b.drive.fault().is_some(),
        ];
        let any_fault = faulted[0] || faulted[1];
        if self.sync_fault.is_none() {
            if self.supervision.max_position_error_m > 0.0
                && err.abs() > self.supervision.max_position_error_m
                && cmd.run
            {
                self.sync_fault = Some(SyncFault::PositionError);
            } else if any_fault && self.supervision.on_partner_fault == PartnerAction::StopBoth {
                self.sync_fault = Some(SyncFault::PartnerFault);
            }
        }
        if !cmd.run && !any_fault && cmd.fault_reset {
            self.sync_fault = None;
        }
        let stop = self.sync_fault.is_some();
        let run = cmd.run && !stop;

        let ref_rpm = |i: usize, v: f64| v / self.mpr_nominal[i] * 60.0 / TAU;
        let mut ia = ClInputs {
            run,
            speed_ref_rpm: ref_rpm(0, cmd.hook_speed_m_s),
            fast_stop: stop,
            sto: cmd.sto[0],
            external_fault: cmd.external_fault[0],
            fault_reset: cmd.fault_reset,
            ..ClInputs::default()
        };
        let mut ib = ClInputs {
            run,
            speed_ref_rpm: ref_rpm(1, cmd.hook_speed_m_s),
            fast_stop: stop,
            sto: cmd.sto[1],
            external_fault: cmd.external_fault[1],
            fault_reset: cmd.fault_reset,
            ..ClInputs::default()
        };
        match self.mode {
            SyncMode::Independent => {}
            SyncMode::PositionLock {
                gain_per_s,
                limit_m_s,
            } => {
                let corr = (gain_per_s * err).clamp(-limit_m_s, limit_m_s);
                ib.speed_ref_rpm = ref_rpm(1, cmd.hook_speed_m_s + corr);
            }
            SyncMode::TorqueFollower { share, margin } => {
                let lim = ref_rpm(1, cmd.hook_speed_m_s).abs() * (1.0 + margin) + 5.0;
                ib.mode = RunMode::Torque {
                    torque_ref_nm: share * self.last_torque[0],
                    speed_limit_rpm: lim,
                };
            }
        }
        if let Some(fb) = brake_feedback(&self.a) {
            ia.brake_feedback = Some(fb);
        }
        if let Some(fb) = brake_feedback(&self.b) {
            ib.brake_feedback = Some(fb);
        }
        let (oa, ba) = self.a.control_step(&ia, dt_s);
        let (ob, bb) = self.b.control_step(&ib, dt_s);
        self.last_torque = [oa.torque_nm, ob.torque_nm];
        self.integrate(&[oa, ob], [ba, bb], dt_s);
        self.time_s += dt_s;
        let f = self.last_force;
        for (peak, f) in self.peak_force_n.iter_mut().zip(f) {
            *peak = peak.max(f);
        }
        self.snapshot(oa, ob, ba, bb)
    }

    fn integrate(&mut self, out: &[ClOutputs; 2], brake: [f64; 2], dt: f64) {
        let m = self.beam.mass_kg;
        let k_max = self.ropes[0]
            .stiffness_at(0.0)
            .max(self.ropes[1].stiffness_at(0.0))
            * 4.0;
        let j_over = [
            self.a.shaft().inertia_kg_m2() / (self.mpr[0] * self.mpr[0]),
            self.b.shaft().inertia_kg_m2() / (self.mpr[1] * self.mpr[1]),
        ];
        let m_min = (0.5 * m).min(j_over[0]).min(j_over[1]);
        let omega_max = (k_max / m_min).sqrt();
        let h_max = STEP_FRACTION / omega_max;
        let n = ((dt / h_max).ceil() as usize).clamp(1, MAX_SUBSTEPS);
        let h = dt / n as f64;
        let inertia = self.beam.inertia_kg_m2();
        let l = self.beam.length_m;
        let e = self.beam.cog_offset_m;
        for _ in 0..n {
            let f = [self.rope_force(0), self.rope_force(1)];
            self.last_force = f;
            // Drums: motor torque against the rope; losses scale with the rope force.
            for (i, w) in [&mut self.a, &mut self.b].into_iter().enumerate() {
                let tau_rope = f[i] * self.mpr[i];
                let eta = self.eta[i];
                w.shaft_mut().step(
                    h,
                    out[i].torque_nm - tau_rope,
                    tau_rope * (1.0 / eta - 1.0) + brake[i],
                    tau_rope * (1.0 - eta) + brake[i],
                );
            }
            // Beam.
            let ah = (f[0] + f[1]) / m - G;
            self.vh += ah * h;
            self.h += self.vh * h;
            if self.beam.guided {
                self.phi = 0.0;
                self.vphi = 0.0;
            } else {
                let c = self.phi.cos();
                let aphi = (0.5 * l * c * (f[0] - f[1]) - m * G * e * c) / inertia;
                self.vphi += aphi * h;
                self.phi += self.vphi * h;
            }
        }
    }

    fn snapshot(&self, oa: ClOutputs, ob: ClOutputs, ba: f64, bb: f64) -> PairSnapshot {
        let f = self.last_force;
        let pos = self.encoder_positions();
        let swl = self.supervision.rope_swl_n;
        let w = (f[0] + f[1]).max(1e-9);
        let avg = 0.5 * w;
        let tilt = self.phi.to_degrees();
        PairSnapshot {
            time_s: self.time_s,
            winches: [self.a.snapshot(oa, ba), self.b.snapshot(ob, bb)],
            force_n: f,
            share_a: f[0] / w,
            height_m: self.h,
            tilt_deg: tilt,
            position_m: pos,
            position_error_m: pos[0] - pos[1],
            true_position_m: [
                self.a.shaft().angle_rad * self.mpr[0],
                self.b.shaft().angle_rad * self.mpr[1],
            ],
            overload: [f[0] > swl, f[1] > swl],
            slack: [f[0] < 0.05 * avg, f[1] < 0.05 * avg],
            beyond_model: tilt.abs() > self.supervision.model_tilt_limit_deg,
            sync_fault: self.sync_fault,
        }
    }

    pub fn sync_fault(&self) -> Option<SyncFault> {
        self.sync_fault
    }

    pub fn fault(&self, i: usize) -> Option<ClFault> {
        self.winch(i).drive.fault()
    }

    pub fn state(&self, i: usize) -> ClState {
        self.winch(i).drive.state()
    }
}

/// The brake feedback contact of a winch whose drive has it wired.
fn brake_feedback(w: &ClosedLoopWinch) -> Option<bool> {
    w.drive
        .cfg
        .brake_feedback
        .then(|| w.brake().holding_torque_nm() == 0.0)
}
