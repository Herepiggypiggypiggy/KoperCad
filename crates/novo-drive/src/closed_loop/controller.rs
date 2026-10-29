//! The closed-loop (encoder) hoist drive: field-oriented torque control in an averaged model.
//!
//! One call to [`ClosedLoopDrive::step`] advances the drive by one fixed step. It takes the
//! shaft as the encoder sees it and gives the motor torque back; the shaft, the brake and the
//! load live in [`super::unit`]. Inside: speed loop (PI) around a torque loop (first order),
//! torque limit from the current limit and field weakening, the hoist sequence (flux, torque
//! proof, brake release, run, zero-speed hold, brake close), monitoring (overspeed, speed
//! deviation, rollback, encoder plausibility, brake feedback, STO) and the DC bus.

use std::f64::consts::TAU;

use super::encoder::Encoder;
use super::sto::StoMonitor;
use crate::drive::{BusConfig, DcBus, I2tOverload, MotorNameplate};

/// Where the torque at brake release comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PreTorque {
    /// None: the speed loop has to wind up against the load after the brake opens.
    None,
    /// A fixed holding torque in N m (positive holds a hanging load up).
    Fixed(f64),
    /// From a load cell on the hook ([`ClInputs::load_cell_nm`]).
    LoadCell,
    /// The torque the drive held when the brake last closed.
    Memory,
}

/// Faults of the closed-loop drive (neutral codes; a profile maps them to a maker's codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClFault {
    Overspeed,
    /// Speed differs from the ramped reference for too long (stall, overload, dragging brake).
    SpeedDeviation,
    /// The shaft moved too far while the drive held it at zero speed.
    Rollback,
    /// The encoder disagrees with the shaft speed from the motor model.
    EncoderFeedback,
    /// The drive could not build the torque it needs before releasing the brake.
    TorqueProofTimeout,
    /// The brake did not confirm open in time, or closed while running.
    BrakeFeedback,
    MotorOverload,
    BusOverVoltage,
    BusUnderVoltage,
    /// The STO channels disagreed longer than the limit.
    StoDiscrepancy,
    External,
}

impl ClFault {
    pub fn code(self) -> &'static str {
        match self {
            Self::Overspeed => "OSP",
            Self::SpeedDeviation => "SDV",
            Self::Rollback => "RBK",
            Self::EncoderFeedback => "ENC",
            Self::TorqueProofTimeout => "TPR",
            Self::BrakeFeedback => "BFB",
            Self::MotorOverload => "MOL",
            Self::BusOverVoltage => "OBV",
            Self::BusUnderVoltage => "UBV",
            Self::StoDiscrepancy => "STD",
            Self::External => "EXT",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Overspeed => "Speed above the overspeed level",
            Self::SpeedDeviation => {
                "Speed deviates from the reference (stall, overload or dragging brake)"
            }
            Self::Rollback => "Shaft moved while held at zero speed",
            Self::EncoderFeedback => "Encoder disagrees with the shaft speed",
            Self::TorqueProofTimeout => "Torque not built in time before brake release",
            Self::BrakeFeedback => "Brake feedback does not match the brake command",
            Self::MotorOverload => "Motor overload (thermal model)",
            Self::BusOverVoltage => "DC bus overvoltage (regenerating without enough braking)",
            Self::BusUnderVoltage => "DC bus undervoltage",
            Self::StoDiscrepancy => "Safe torque off channels disagree",
            Self::External => "External fault input",
        }
    }
}

/// Sequence state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClState {
    /// Ready, no torque, brake closed.
    Off,
    /// Run requested, the motor flux builds.
    Magnetising,
    /// Pre-torque is built against the closed brake until the drive proves it.
    TorqueProof,
    /// Brake relay on, zero speed held while the brake opens.
    Releasing,
    Running,
    /// Run removed, ramping to zero speed.
    Stopping,
    /// Brake relay off, zero speed still held until the brake has taken the load.
    BrakeClosing,
    /// STO requested: no torque, brake relay off.
    StoActive,
    Faulted,
}

/// Drive configuration. Speeds are in rpm at the motor shaft, positive raises.
#[derive(Debug, Clone, PartialEq)]
pub struct ClosedLoopConfig {
    pub motor: MotorNameplate,
    pub current_limit_a: f64,
    pub overspeed_rpm: f64,
    /// Speed loop gains: proportional in N m per rad/s, integral in N m per rad.
    pub kp: f64,
    pub ki: f64,
    /// Torque loop time constant (first order lag), s.
    pub torque_tau_s: f64,
    pub accel_rpm_s: f64,
    pub decel_rpm_s: f64,
    /// Limit on the rate of change of acceleration in rpm/s2 (0 = linear ramp).
    pub jerk_rpm_s2: f64,
    pub fast_stop_rpm_s: f64,
    pub flux_time_s: f64,
    pub pre_torque: PreTorque,
    /// The drive releases the brake once its torque is this fraction of the pre-torque.
    pub proof_fraction: f64,
    /// And at least this torque, N m.
    pub min_release_torque_nm: f64,
    pub proof_timeout_s: f64,
    /// Zero speed is held this long after the relay closes (used without brake feedback).
    pub release_wait_s: f64,
    /// Whether a brake feedback contact is wired to the drive.
    pub brake_feedback: bool,
    pub feedback_timeout_s: f64,
    pub zero_speed_rpm: f64,
    pub zero_hold_s: f64,
    /// Zero speed is held this long after the relay opens, before torque is removed.
    pub brake_close_wait_s: f64,
    pub speed_deviation_rpm: f64,
    pub deviation_time_s: f64,
    pub rollback_limit_rad: f64,
    pub encoder_ppr: u32,
    pub encoder_window_s: f64,
    /// Compare the encoder with the shaft speed from the motor model.
    pub encoder_monitor: bool,
    pub observer_tolerance_rpm: f64,
    /// The model based speed is only trusted above this speed.
    pub observer_min_rpm: f64,
    pub observer_time_s: f64,
    pub sto_discrepancy_s: f64,
    pub bus: BusConfig,
    pub motor_overload: (f64, f64),
    /// Motor efficiency at rated load, for the electrical power (model).
    pub efficiency: f64,
}

impl ClosedLoopConfig {
    /// A generic 400 V closed-loop hoist drive for the motor. A starting point, not a product:
    /// every number is meant to be edited.
    pub fn generic_hoist(motor: MotorNameplate) -> Self {
        let t_rated = motor.rated_torque_nm();
        let w_rated = TAU * motor.rpm / 60.0;
        Self {
            motor,
            current_limit_a: 1.5 * motor.current_a,
            overspeed_rpm: 1.2 * motor.rpm,
            kp: 0.8 * t_rated / (0.05 * w_rated).max(1e-9),
            ki: 0.8 * t_rated / (0.05 * w_rated).max(1e-9) / 0.1,
            torque_tau_s: 0.004,
            accel_rpm_s: motor.rpm / 2.0,
            decel_rpm_s: motor.rpm / 2.0,
            jerk_rpm_s2: 0.0,
            fast_stop_rpm_s: motor.rpm * 2.0,
            flux_time_s: 0.3,
            pre_torque: PreTorque::None,
            proof_fraction: 0.9,
            min_release_torque_nm: 0.1 * t_rated,
            proof_timeout_s: 1.0,
            release_wait_s: 0.3,
            brake_feedback: false,
            feedback_timeout_s: 1.0,
            zero_speed_rpm: 2.0,
            zero_hold_s: 0.2,
            brake_close_wait_s: 0.4,
            speed_deviation_rpm: 0.2 * motor.rpm,
            deviation_time_s: 0.5,
            rollback_limit_rad: TAU / 4.0,
            encoder_ppr: 1024,
            encoder_window_s: 0.005,
            encoder_monitor: true,
            observer_tolerance_rpm: 0.1 * motor.rpm,
            observer_min_rpm: 0.1 * motor.rpm,
            observer_time_s: 0.1,
            sto_discrepancy_s: 1.0,
            bus: BusConfig::generic_400v(),
            motor_overload: (1.5, 60.0),
            efficiency: 0.9,
        }
    }

    pub fn rated_torque_nm(&self) -> f64 {
        self.motor.rated_torque_nm()
    }

    fn base_rad_s(&self) -> f64 {
        TAU * self.motor.rpm / 60.0
    }

    /// Torque available at motor speed `omega_rad_s`, N m: the current limit, then constant
    /// power above rated speed (model; stator resistance neglected).
    pub fn torque_limit_nm(&self, omega_rad_s: f64) -> f64 {
        let m = &self.motor;
        let i_mag = m.magnetising_current_a();
        let it_rated = (m.current_a * m.current_a - i_mag * i_mag).max(0.0).sqrt();
        let it_max = (self.current_limit_a.powi(2) - i_mag * i_mag)
            .max(0.0)
            .sqrt();
        let base = if it_rated > 0.0 {
            m.rated_torque_nm() * it_max / it_rated
        } else {
            0.0
        };
        let w = omega_rad_s.abs();
        let weak = if w > self.base_rad_s() && self.base_rad_s() > 0.0 {
            self.base_rad_s() / w
        } else {
            1.0
        };
        base * weak
    }
}

/// Mode of the speed/torque path while running.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RunMode {
    /// Speed control from [`ClInputs::speed_ref_rpm`].
    Speed,
    /// Torque control (a follower): the torque reference, with the speed held under a limit.
    Torque {
        torque_ref_nm: f64,
        speed_limit_rpm: f64,
    },
}

/// What the world gives the drive in one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClInputs {
    /// Run command (level).
    pub run: bool,
    /// Signed speed reference in rpm, positive raises.
    pub speed_ref_rpm: f64,
    pub mode: RunMode,
    /// Stop on the fast-stop ramp.
    pub fast_stop: bool,
    /// STO channels, `true` = energised.
    pub sto: (bool, bool),
    /// Brake feedback contact, `Some(true)` = brake confirms open (only read when wired).
    pub brake_feedback: Option<bool>,
    /// Torque the hanging load asks for, from a load cell, N m (used by [`PreTorque::LoadCell`]).
    pub load_cell_nm: Option<f64>,
    pub mains_ok: bool,
    pub fault_reset: bool,
    pub external_fault: bool,
    /// Whether the brake coil is fed. `None`: the coil follows the drive's brake relay directly
    /// (ideal wiring). `Some`: the circuit decides (a brake contactor and rectifier in the
    /// drawing), so a contactor that does not pull in leaves the brake closed.
    pub brake_coil: Option<bool>,
}

impl Default for ClInputs {
    fn default() -> Self {
        Self {
            run: false,
            speed_ref_rpm: 0.0,
            mode: RunMode::Speed,
            fast_stop: false,
            sto: (true, true),
            brake_feedback: None,
            load_cell_nm: None,
            mains_ok: true,
            fault_reset: false,
            external_fault: false,
            brake_coil: None,
        }
    }
}

/// The shaft as the drive's sensors see it (true values; the encoder turns them into counts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShaftSense {
    pub angle_rad: f64,
    pub omega_rad_s: f64,
}

/// What the drive gives back in one step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClOutputs {
    pub state: ClState,
    pub fault: Option<ClFault>,
    /// Motor torque on the shaft, N m, positive raises.
    pub torque_nm: f64,
    pub torque_limit_nm: f64,
    pub current_limit_active: bool,
    /// Ramped speed reference, rpm.
    pub ref_rpm: f64,
    /// Speed as the encoder gives it, rpm.
    pub measured_rpm: f64,
    pub encoder_counts: i64,
    pub current_a: f64,
    pub power_w: f64,
    pub bus_v: f64,
    pub chopper_on: bool,
    /// The drive's brake relay: on = release the brake.
    pub brake_release_cmd: bool,
    /// The drive has proven its torque this cycle.
    pub torque_proven: bool,
    pub zero_speed: bool,
    pub sto_requested: bool,
    pub motor_overload_used: f64,
}

#[derive(Debug, Clone)]
pub struct ClosedLoopDrive {
    pub cfg: ClosedLoopConfig,
    pub encoder: Encoder,
    sto: StoMonitor,
    pub bus: DcBus,
    i2t: I2tOverload,
    state: ClState,
    fault: Option<ClFault>,
    t_in_state: f64,
    integ: f64,
    torque_nm: f64,
    ref_rad_s: f64,
    accel_rad_s2: f64,
    memory_torque_nm: f64,
    proven: bool,
    hold_angle_rad: f64,
    last_power_w: f64,
    dev_t: f64,
    obs_t: f64,
    fb_t: f64,
    zero_t: f64,
    last_reset: bool,
}

impl ClosedLoopDrive {
    pub fn new(cfg: ClosedLoopConfig) -> Result<Self, crate::InputError> {
        let encoder = Encoder::new(cfg.encoder_ppr, cfg.encoder_window_s)?;
        let i2t = I2tOverload::new(
            cfg.motor.current_a,
            cfg.motor_overload.0,
            cfg.motor_overload.1,
        );
        Ok(Self {
            sto: StoMonitor::new(cfg.sto_discrepancy_s),
            bus: DcBus::new(cfg.bus),
            encoder,
            i2t,
            cfg,
            state: ClState::Off,
            fault: None,
            t_in_state: 0.0,
            integ: 0.0,
            torque_nm: 0.0,
            ref_rad_s: 0.0,
            accel_rad_s2: 0.0,
            memory_torque_nm: 0.0,
            proven: false,
            hold_angle_rad: 0.0,
            last_power_w: 0.0,
            dev_t: 0.0,
            obs_t: 0.0,
            fb_t: 0.0,
            zero_t: 0.0,
            last_reset: false,
        })
    }

    pub fn state(&self) -> ClState {
        self.state
    }

    pub fn fault(&self) -> Option<ClFault> {
        self.fault
    }

    /// The torque held when the brake last closed, N m (the [`PreTorque::Memory`] value).
    pub fn memory_torque_nm(&self) -> f64 {
        self.memory_torque_nm
    }

    fn enter(&mut self, s: ClState) {
        self.state = s;
        self.t_in_state = 0.0;
    }

    fn trip(&mut self, f: ClFault) {
        if self.fault.is_none() {
            self.fault = Some(f);
        }
        self.enter(ClState::Faulted);
        self.integ = 0.0;
        self.torque_nm = 0.0;
        self.ref_rad_s = 0.0;
        self.accel_rad_s2 = 0.0;
        self.proven = false;
    }

    fn pre_torque(&self, inp: &ClInputs) -> f64 {
        match self.cfg.pre_torque {
            PreTorque::None => 0.0,
            PreTorque::Fixed(t) => t,
            PreTorque::LoadCell => inp.load_cell_nm.unwrap_or(0.0),
            PreTorque::Memory => self.memory_torque_nm,
        }
    }

    fn ramp(&mut self, target_rad_s: f64, rate_rpm_s: f64, dt: f64) {
        let rate = rate_rpm_s * TAU / 60.0;
        let jerk = self.cfg.jerk_rpm_s2 * TAU / 60.0;
        let err = target_rad_s - self.ref_rad_s;
        if jerk <= 0.0 {
            let d = rate * dt;
            self.ref_rad_s = if err.abs() <= d {
                target_rad_s
            } else {
                self.ref_rad_s + d * err.signum()
            };
            self.accel_rad_s2 = 0.0;
            return;
        }
        // Jerk limited: the acceleration follows the largest value from which the target can
        // still be reached without overshoot, slewed at the jerk limit.
        let ahead = (err.abs() - self.accel_rad_s2.abs() * dt).max(0.0);
        let a_goal = err.signum() * rate.min((2.0 * jerk * ahead).sqrt());
        let da = jerk * dt;
        self.accel_rad_s2 += (a_goal - self.accel_rad_s2).clamp(-da, da);
        let next = self.ref_rad_s + self.accel_rad_s2 * dt;
        if (next - target_rad_s) * err.signum() >= 0.0 {
            self.ref_rad_s = target_rad_s;
            self.accel_rad_s2 = 0.0;
        } else {
            self.ref_rad_s = next;
        }
    }

    fn speed_loop(&mut self, ref_rad_s: f64, meas: f64, tlim: f64, dt: f64) -> f64 {
        let e = ref_rad_s - meas;
        let unsat = self.cfg.kp * e + self.integ;
        let t = unsat.clamp(-tlim, tlim);
        let pushing_out = (unsat > tlim && e > 0.0) || (unsat < -tlim && e < 0.0);
        if !pushing_out {
            self.integ = (self.integ + self.cfg.ki * e * dt).clamp(-tlim, tlim);
        }
        t
    }

    /// Advance one step of `dt_s` seconds. `shaft` is the true shaft state; the encoder turns
    /// it into the measured speed and position.
    pub fn step(&mut self, inp: &ClInputs, shaft: ShaftSense, dt_s: f64) -> ClOutputs {
        let dt = if dt_s.is_finite() && dt_s > 0.0 {
            dt_s
        } else {
            0.0
        };
        self.t_in_state += dt;
        let meas = self.encoder.update(shaft.angle_rad, dt);
        let meas_rpm = meas * 60.0 / TAU;
        let sto = self.sto.step(inp.sto.0, inp.sto.1, dt);
        let bus = self.bus.step(self.last_power_w, inp.mains_ok, dt);
        let reset_edge = inp.fault_reset && !self.last_reset;
        self.last_reset = inp.fault_reset;
        let energised = !matches!(
            self.state,
            ClState::Off | ClState::Faulted | ClState::StoActive
        );

        // Monitoring that applies in every energised state.
        if energised {
            let obs_valid = self.cfg.encoder_monitor
                && shaft.omega_rad_s.abs() * 60.0 / TAU > self.cfg.observer_min_rpm;
            let overspeed_rad = self.cfg.overspeed_rpm * TAU / 60.0;
            let mut seen = meas.abs();
            if obs_valid {
                seen = seen.max(shaft.omega_rad_s.abs());
            }
            if seen > overspeed_rad {
                self.trip(ClFault::Overspeed);
            } else if obs_valid
                && (shaft.omega_rad_s - meas).abs() > self.cfg.observer_tolerance_rpm * TAU / 60.0
            {
                self.obs_t += dt;
                if self.obs_t >= self.cfg.observer_time_s {
                    self.trip(ClFault::EncoderFeedback);
                }
            } else {
                self.obs_t = 0.0;
            }
            if self
                .i2t
                .step(self.motor_current(self.torque_nm, shaft.omega_rad_s), dt)
            {
                self.trip(ClFault::MotorOverload);
            }
            if bus.volts > self.cfg.bus.over_voltage_v {
                self.trip(ClFault::BusOverVoltage);
            } else if bus.volts < self.cfg.bus.under_voltage_v {
                self.trip(ClFault::BusUnderVoltage);
            }
            if inp.external_fault {
                self.trip(ClFault::External);
            }
        }
        if sto.discrepancy && self.state != ClState::Faulted {
            self.trip(ClFault::StoDiscrepancy);
        }
        // STO request (or one channel low): torque off at once, brake relay off, no fault. A
        // channel mismatch that lasts latches the discrepancy fault above.
        if !sto.torque_allowed && !matches!(self.state, ClState::Faulted | ClState::StoActive) {
            self.enter(ClState::StoActive);
            self.integ = 0.0;
            self.torque_nm = 0.0;
            self.ref_rad_s = 0.0;
            self.accel_rad_s2 = 0.0;
            self.proven = false;
        }

        let tlim = self.cfg.torque_limit_nm(shaft.omega_rad_s);
        let ready = sto.torque_allowed && inp.mains_ok && !inp.external_fault;
        let mut t_cmd = 0.0;
        let mut brake_cmd = false;

        match self.state {
            ClState::Off => {
                self.ref_rad_s = 0.0;
                self.proven = false;
                if inp.run && ready {
                    self.enter(ClState::Magnetising);
                }
            }
            ClState::Magnetising => {
                if !inp.run {
                    self.enter(ClState::Off);
                } else if self.t_in_state >= self.cfg.flux_time_s {
                    self.enter(ClState::TorqueProof);
                }
            }
            ClState::TorqueProof => {
                let pre_raw = self.pre_torque(inp);
                let pre = pre_raw.clamp(-tlim, tlim);
                // Without a pre-torque the drive still builds the minimum release torque, in
                // the direction the load is about to be moved.
                let min = self.cfg.min_release_torque_nm;
                let dir = if pre != 0.0 {
                    pre.signum()
                } else if inp.speed_ref_rpm < 0.0 {
                    -1.0
                } else {
                    1.0
                };
                t_cmd = if pre.abs() >= min { pre } else { dir * min };
                // Proof is against what was asked for, not what the limit lets the drive make:
                // a load beyond the drive is never proven.
                let asked = if pre_raw.abs() >= min {
                    pre_raw.abs()
                } else {
                    t_cmd.abs()
                };
                let thr = (self.cfg.proof_fraction * asked).max(min);
                if !inp.run {
                    self.enter(ClState::Off);
                    t_cmd = 0.0;
                } else if self.torque_nm.abs() >= thr - 1e-9 && self.torque_nm * t_cmd >= 0.0 {
                    self.proven = true;
                    self.integ = t_cmd;
                    self.hold_angle_rad = self.encoder.position_rad();
                    self.fb_t = 0.0;
                    self.enter(ClState::Releasing);
                } else if self.t_in_state > self.cfg.proof_timeout_s {
                    self.trip(ClFault::TorqueProofTimeout);
                    t_cmd = 0.0;
                }
            }
            ClState::Releasing => {
                brake_cmd = true;
                t_cmd = self.speed_loop(0.0, meas, tlim, dt);
                let open = if self.cfg.brake_feedback {
                    inp.brake_feedback == Some(true)
                } else {
                    self.t_in_state >= self.cfg.release_wait_s
                };
                if self.cfg.brake_feedback && !open && self.t_in_state > self.cfg.feedback_timeout_s
                {
                    self.trip(ClFault::BrakeFeedback);
                    t_cmd = 0.0;
                    brake_cmd = false;
                } else if self.rolled_back() {
                    self.trip(ClFault::Rollback);
                    t_cmd = 0.0;
                    brake_cmd = false;
                } else if open {
                    self.enter(ClState::Running);
                }
                if !inp.run && self.state == ClState::Releasing && open {
                    self.enter(ClState::Stopping);
                }
            }
            ClState::Running => {
                brake_cmd = true;
                self.check_feedback(inp, dt);
                if self.fault.is_some() {
                    brake_cmd = false;
                } else if !inp.run {
                    self.enter(ClState::Stopping);
                } else {
                    t_cmd = self.run_torque(inp, meas, tlim, dt);
                    self.check_deviation(meas, dt);
                    self.check_hold(meas);
                }
                if self.fault.is_some() {
                    t_cmd = 0.0;
                    brake_cmd = false;
                }
            }
            ClState::Stopping => {
                brake_cmd = true;
                let rate = if inp.fast_stop {
                    self.cfg.fast_stop_rpm_s
                } else {
                    self.cfg.decel_rpm_s
                };
                self.ramp(0.0, rate, dt);
                t_cmd = self.speed_loop(self.ref_rad_s, meas, tlim, dt);
                self.check_deviation(meas, dt);
                self.check_feedback(inp, dt);
                let slow = meas_rpm.abs() < self.cfg.zero_speed_rpm && self.ref_rad_s == 0.0;
                self.zero_t = if slow { self.zero_t + dt } else { 0.0 };
                if self.fault.is_some() {
                    t_cmd = 0.0;
                    brake_cmd = false;
                } else if inp.run && ready {
                    self.enter(ClState::Running);
                } else if self.zero_t >= self.cfg.zero_hold_s {
                    self.memory_torque_nm = self.torque_nm;
                    self.hold_angle_rad = self.encoder.position_rad();
                    self.enter(ClState::BrakeClosing);
                    brake_cmd = false;
                }
            }
            ClState::BrakeClosing => {
                t_cmd = self.speed_loop(0.0, meas, tlim, dt);
                if self.rolled_back() {
                    self.trip(ClFault::Rollback);
                    t_cmd = 0.0;
                } else if inp.run && ready {
                    self.enter(ClState::Releasing);
                    brake_cmd = true;
                } else if self.t_in_state >= self.cfg.brake_close_wait_s {
                    self.integ = 0.0;
                    self.proven = false;
                    self.enter(ClState::Off);
                    t_cmd = 0.0;
                }
            }
            ClState::StoActive => {
                if !sto.requested && sto.torque_allowed && !inp.run {
                    self.enter(ClState::Off);
                }
            }
            ClState::Faulted => {
                if reset_edge && !inp.run && !sto.discrepancy {
                    self.fault = None;
                    self.enter(ClState::Off);
                }
            }
        }

        // Torque loop: first order lag towards the command, never above the limit. STO and
        // faults remove torque at once.
        let t_cmd = t_cmd.clamp(-tlim, tlim);
        let limit_active = t_cmd.abs() >= tlim - 1e-9 && tlim > 0.0;
        if matches!(
            self.state,
            ClState::Off | ClState::Faulted | ClState::StoActive | ClState::Magnetising
        ) {
            self.torque_nm = 0.0;
        } else {
            let a = 1.0 - (-dt / self.cfg.torque_tau_s.max(1e-9)).exp();
            self.torque_nm += (t_cmd - self.torque_nm) * a;
            self.torque_nm = self.torque_nm.clamp(-tlim, tlim);
        }
        let live = !matches!(
            self.state,
            ClState::Off | ClState::Faulted | ClState::StoActive
        );
        let current = if live {
            self.motor_current(self.torque_nm, shaft.omega_rad_s)
        } else {
            0.0
        };
        let p_mech = self.torque_nm * shaft.omega_rad_s;
        let p_rated = self.cfg.motor.power_kw * 1000.0;
        let loss_rated = p_rated * (1.0 / self.cfg.efficiency.clamp(0.05, 1.0) - 1.0);
        let power = if live {
            let i_n = self.cfg.motor.current_a.max(1e-9);
            p_mech + 0.4 * loss_rated + 0.6 * loss_rated * (current / i_n).powi(2)
        } else {
            0.0
        };
        self.last_power_w = power;
        ClOutputs {
            state: self.state,
            fault: self.fault,
            torque_nm: self.torque_nm,
            torque_limit_nm: tlim,
            current_limit_active: limit_active,
            ref_rpm: self.ref_rad_s * 60.0 / TAU,
            measured_rpm: meas_rpm,
            encoder_counts: self.encoder.counts(),
            current_a: current,
            power_w: power,
            bus_v: bus.volts,
            chopper_on: bus.chopper_on,
            brake_release_cmd: brake_cmd,
            torque_proven: self.proven,
            zero_speed: meas_rpm.abs() < self.cfg.zero_speed_rpm,
            sto_requested: sto.requested,
            motor_overload_used: self.i2t.used(),
        }
    }

    fn motor_current(&self, torque_nm: f64, omega: f64) -> f64 {
        let m = &self.cfg.motor;
        let i_mag = m.magnetising_current_a();
        let it_rated = (m.current_a * m.current_a - i_mag * i_mag).max(0.0).sqrt();
        let id = {
            let w = omega.abs();
            if w > self.cfg.base_rad_s() && self.cfg.base_rad_s() > 0.0 {
                i_mag * self.cfg.base_rad_s() / w
            } else {
                i_mag
            }
        };
        let iq = if m.rated_torque_nm() > 0.0 {
            torque_nm.abs() / m.rated_torque_nm() * it_rated
        } else {
            0.0
        };
        (id * id + iq * iq).sqrt()
    }

    fn rolled_back(&self) -> bool {
        (self.encoder.position_rad() - self.hold_angle_rad).abs() > self.cfg.rollback_limit_rad
    }

    fn check_feedback(&mut self, inp: &ClInputs, dt: f64) {
        if self.cfg.brake_feedback && inp.brake_feedback == Some(false) {
            self.fb_t += dt;
            if self.fb_t > 0.1 {
                self.trip(ClFault::BrakeFeedback);
            }
        } else {
            self.fb_t = 0.0;
        }
    }

    fn check_deviation(&mut self, meas: f64, dt: f64) {
        let dev = (self.ref_rad_s - meas).abs() * 60.0 / TAU;
        if dev > self.cfg.speed_deviation_rpm {
            self.dev_t += dt;
            if self.dev_t >= self.cfg.deviation_time_s {
                self.trip(ClFault::SpeedDeviation);
            }
        } else {
            self.dev_t = 0.0;
        }
    }

    /// While the reference is zero the position is watched; otherwise it follows the shaft.
    fn check_hold(&mut self, meas: f64) {
        if self.ref_rad_s != 0.0 || meas.abs() * 60.0 / TAU > self.cfg.zero_speed_rpm * 5.0 {
            self.hold_angle_rad = self.encoder.position_rad();
        } else if self.rolled_back() {
            self.trip(ClFault::Rollback);
        }
    }

    fn run_torque(&mut self, inp: &ClInputs, meas: f64, tlim: f64, dt: f64) -> f64 {
        match inp.mode {
            RunMode::Speed => {
                let target = if inp.fast_stop {
                    0.0
                } else {
                    inp.speed_ref_rpm * TAU / 60.0
                };
                let max = self.cfg.overspeed_rpm * TAU / 60.0 * 0.95;
                let target = target.clamp(-max, max);
                let rate = if inp.fast_stop {
                    self.cfg.fast_stop_rpm_s
                } else if target.abs() < self.ref_rad_s.abs() || target * self.ref_rad_s < 0.0 {
                    self.cfg.decel_rpm_s
                } else {
                    self.cfg.accel_rpm_s
                };
                self.ramp(target, rate, dt);
                self.speed_loop(self.ref_rad_s, meas, tlim, dt)
            }
            RunMode::Torque {
                torque_ref_nm,
                speed_limit_rpm,
            } => {
                let lim = speed_limit_rpm.abs() * TAU / 60.0;
                self.ref_rad_s = meas;
                // The speed limiter is a stiff proportional stop at the limit (20 x the speed
                // loop gain) so it does not eat into the torque reference below the limit.
                let upper = 20.0 * self.cfg.kp * (lim - meas);
                let lower = 20.0 * self.cfg.kp * (-lim - meas);
                let t = torque_ref_nm.clamp(lower.min(upper), upper.max(lower));
                self.integ = t.clamp(-tlim, tlim);
                t
            }
        }
    }
}
