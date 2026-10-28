//! The drive: terminals and reference in, output frequency and voltage out (averaged model).
//!
//! One call to [`Drive::step`] advances the drive by one fixed time step. The drive does not
//! simulate the motor: it takes the motor current and electrical power measured by the motor
//! model (lane L2) as input and gives frequency, voltage and a torque limit back. That keeps
//! the engine pure and lets the co-simulation loop (L5a) join drive and motor.

use super::{
    bus::{BusConfig, BusStep, DcBus},
    protection::{Fault, I2tOverload},
    ramp::{Ramp, SRamp},
    terminals::{Commands, ReferenceSource, TerminalMacro},
    vf::VfCurve,
};
use std::f64::consts::PI;

/// Motor data the drive needs (nameplate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotorNameplate {
    pub voltage_v: f64,
    pub current_a: f64,
    pub frequency_hz: f64,
    pub rpm: f64,
    pub power_kw: f64,
    pub power_factor: f64,
}

impl MotorNameplate {
    /// Rated shaft torque `P / w` in N m.
    pub fn rated_torque_nm(&self) -> f64 {
        let w = 2.0 * PI * self.rpm / 60.0;
        if w > 0.0 {
            self.power_kw * 1000.0 / w
        } else {
            0.0
        }
    }

    /// Magnetising current estimate `I_n sin(phi)` in A (model).
    pub fn magnetising_current_a(&self) -> f64 {
        let pf = self.power_factor.clamp(0.0, 1.0);
        self.current_a * (1.0 - pf * pf).sqrt()
    }
}

/// How the output voltage and torque are controlled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlMode {
    /// Voltage follows the V/f curve; the current limit holds the ramp (open loop).
    VoltsPerHertz,
    /// Sensorless vector, steady-state model: the torque-producing current is limited by the
    /// current limit and the flux current, so the drive reports a torque limit. The dynamic
    /// current loops are not simulated.
    SensorlessVector,
}

/// Drive configuration. Parameter names are neutral; a profile maps them to a maker's codes.
#[derive(Debug, Clone, PartialEq)]
pub struct DriveConfig {
    pub motor: MotorNameplate,
    pub mode: ControlMode,
    pub vf: VfCurve,
    /// Rated output current of the drive in A (for the drive overload model).
    pub rated_current_a: f64,
    /// Current limit in A (setting; V/f holds the ramp, vector limits torque).
    pub current_limit_a: f64,
    /// Output current at which the drive trips at once (hardware limit), A.
    pub hard_trip_a: f64,
    pub min_hz: f64,
    pub max_hz: f64,
    /// Frequency bands (low, high) the output does not rest in.
    pub skip_bands: Vec<(f64, f64)>,
    pub accel_time_s: f64,
    pub decel_time_s: f64,
    /// Frequency the ramp times refer to.
    pub ramp_reference_hz: f64,
    /// Fraction of the ramp time on each acceleration edge (0 = linear).
    pub s_ramp_fraction: f64,
    pub fast_stop_time_s: f64,
    /// Below this output frequency a stop command ends in the stopped state.
    pub stop_threshold_hz: f64,
    pub reference: ReferenceSource,
    /// Preset frequencies; index 0 is unused (preset 0 means the main reference).
    pub presets_hz: [f64; 4],
    pub jog_hz: f64,
    /// Motor potentiometer rate in Hz/s.
    pub pot_rate_hz_s: f64,
    pub macro_: TerminalMacro,
    /// Hold the decel ramp when the bus is above this voltage (0 = off).
    pub ramp_stretch_v: f64,
    pub bus: BusConfig,
    pub motor_overload: (f64, f64),
    pub drive_overload: (f64, f64),
    /// Output frequency from which the brake-release logic output is on.
    pub brake_release_hz: f64,
    /// Generic assumed power factor on the mains side (stated model value).
    pub mains_power_factor: f64,
}

impl DriveConfig {
    /// Generic 400 V, 50 Hz travel drive for the motor, 2-wire with presets. A starting point,
    /// not a product: every number is meant to be edited.
    pub fn generic_travel(motor: MotorNameplate, rated_current_a: f64) -> Self {
        Self {
            motor,
            mode: ControlMode::VoltsPerHertz,
            vf: VfCurve::new(motor.voltage_v, motor.frequency_hz),
            rated_current_a,
            current_limit_a: 1.5 * rated_current_a,
            hard_trip_a: 2.2 * rated_current_a,
            min_hz: 0.0,
            max_hz: 50.0,
            skip_bands: Vec::new(),
            accel_time_s: 5.0,
            decel_time_s: 5.0,
            ramp_reference_hz: motor.frequency_hz,
            s_ramp_fraction: 0.0,
            fast_stop_time_s: 1.0,
            stop_threshold_hz: 0.1,
            reference: ReferenceSource::Panel(motor.frequency_hz),
            presets_hz: [0.0, 10.0, 50.0, 25.0],
            jog_hz: 5.0,
            pot_rate_hz_s: 2.0,
            macro_: TerminalMacro::two_wire_presets(),
            ramp_stretch_v: 0.0,
            bus: BusConfig::generic_400v(),
            motor_overload: (1.5, 60.0),
            drive_overload: (1.5, 60.0),
            brake_release_hz: 2.0,
            mains_power_factor: 0.95,
        }
    }
}

/// Run state of the drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveState {
    /// No mains, the bus is not charged.
    NotReady,
    /// Ready, output off.
    Stopped,
    /// Output on and following the reference.
    Running,
    /// Stop commanded, ramping down.
    Stopping,
    /// Tripped; output off until reset.
    Faulted,
}

/// What the world gives the drive in one step.
#[derive(Debug, Clone, PartialEq)]
pub struct DriveInputs {
    /// Digital input terminals, `true` = energised, in the order of the macro.
    pub di: Vec<bool>,
    /// Analog input value: volts or mA depending on the configured reference source.
    pub analog: f64,
    /// Frequency word from a PLC or fieldbus, Hz.
    pub word_hz: f64,
    pub mains_ok: bool,
    /// Output current measured by the motor model, A.
    pub motor_current_a: f64,
    /// Electrical power at the motor terminals, W (negative when regenerating).
    pub motor_power_w: f64,
    /// Short circuit flag from the load side.
    pub output_short: bool,
}

impl Default for DriveInputs {
    fn default() -> Self {
        Self {
            di: Vec::new(),
            analog: 0.0,
            word_hz: 0.0,
            mains_ok: true,
            motor_current_a: 0.0,
            motor_power_w: 0.0,
            output_short: false,
        }
    }
}

/// What the drive gives back in one step.
#[derive(Debug, Clone, PartialEq)]
pub struct DriveOutputs {
    pub state: DriveState,
    /// Signed output frequency in Hz (negative = reverse).
    pub frequency_hz: f64,
    /// Line-to-line output voltage (rms).
    pub voltage_v: f64,
    /// Target the ramp is heading for (after limits), Hz.
    pub target_hz: f64,
    /// Torque limit in N m (model). Vector mode: current limit and flux current. V/f mode: the
    /// same limit scaled by the flux-squared capability, which neglects the stator resistance.
    pub torque_limit_nm: f64,
    pub current_limit_active: bool,
    pub bus_v: f64,
    pub chopper_on: bool,
    pub resistor_w: f64,
    /// Mains input current estimate in A (model: assumed power factor).
    pub mains_current_a: f64,
    pub motor_overload_used: f64,
    pub drive_overload_used: f64,
    pub fault: Option<Fault>,
    /// Fault relay: energised while the drive is healthy.
    pub fault_relay_ok: bool,
    /// Logic output: output is on.
    pub running: bool,
    /// Logic output: output frequency has reached the target.
    pub at_reference: bool,
    /// Logic output: brake release (output frequency above the entered threshold, no fault).
    pub brake_release: bool,
}

/// The drive.
#[derive(Debug, Clone)]
pub struct Drive {
    pub cfg: DriveConfig,
    ramp: Ramp,
    sramp: SRamp,
    pub bus: DcBus,
    motor_i2t: I2tOverload,
    drive_i2t: I2tOverload,
    state: DriveState,
    fault: Option<Fault>,
    pot_hz: f64,
    last_reset: bool,
    last_dir: f64,
    current_limit_active: bool,
}

impl Drive {
    pub fn new(cfg: DriveConfig) -> Self {
        let ramp = Ramp {
            frequency_hz: 0.0,
            accel_time_s: cfg.accel_time_s,
            decel_time_s: cfg.decel_time_s,
            reference_hz: cfg.ramp_reference_hz,
        };
        let sramp = SRamp::new(
            cfg.accel_time_s,
            cfg.decel_time_s,
            cfg.ramp_reference_hz,
            cfg.s_ramp_fraction,
        );
        let bus = DcBus::new(cfg.bus);
        let motor_i2t = I2tOverload::new(
            cfg.motor.current_a,
            cfg.motor_overload.0,
            cfg.motor_overload.1,
        );
        let drive_i2t = I2tOverload::new(
            cfg.rated_current_a,
            cfg.drive_overload.0,
            cfg.drive_overload.1,
        );
        Self {
            cfg,
            ramp,
            sramp,
            bus,
            motor_i2t,
            drive_i2t,
            state: DriveState::Stopped,
            fault: None,
            pot_hz: 0.0,
            last_reset: false,
            last_dir: 1.0,
            current_limit_active: false,
        }
    }

    /// Problems in the configuration that would silently disable a protection; empty when fine.
    pub fn config_problems(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        if !self.motor_i2t.is_valid() {
            v.push("Motor overload point is not valid (needs a rated current, a ratio above 1 and a time above 0): the motor model never trips");
        }
        if !self.drive_i2t.is_valid() {
            v.push("Drive overload point is not valid (needs a rated current, a ratio above 1 and a time above 0): the drive model never trips");
        }
        v
    }

    pub fn state(&self) -> DriveState {
        self.state
    }

    pub fn fault(&self) -> Option<Fault> {
        self.fault
    }

    fn frequency(&self) -> f64 {
        if self.cfg.s_ramp_fraction > 0.0 {
            self.sramp.frequency_hz
        } else {
            self.ramp.frequency_hz
        }
    }

    fn set_frequency(&mut self, f: f64) {
        self.ramp.frequency_hz = f;
        self.sramp.frequency_hz = f;
        self.sramp.accel_hz_s = 0.0;
    }

    fn trip(&mut self, f: Fault) {
        if self.fault.is_none() {
            self.fault = Some(f);
        }
        self.state = DriveState::Faulted;
        self.set_frequency(0.0);
    }

    /// Main reference magnitude in Hz before presets, plus a loss-of-signal flag.
    fn main_reference_hz(&mut self, inp: &DriveInputs, cmd: &Commands, dt: f64) -> (f64, bool) {
        match self.cfg.reference {
            ReferenceSource::Panel(f) => (f, false),
            ReferenceSource::AnalogVoltage {
                min_v,
                max_v,
                min_hz,
                max_hz,
            } => (scale(inp.analog, min_v, max_v, min_hz, max_hz), false),
            ReferenceSource::AnalogCurrent {
                min_ma,
                max_ma,
                min_hz,
                max_hz,
                loss_ma,
            } => {
                let lost = loss_ma.is_some_and(|l| inp.analog < l);
                (scale(inp.analog, min_ma, max_ma, min_hz, max_hz), lost)
            }
            ReferenceSource::Word => (inp.word_hz, false),
            ReferenceSource::MotorPot => {
                let rate = self.cfg.pot_rate_hz_s * dt;
                if cmd.pot_up {
                    self.pot_hz += rate;
                }
                if cmd.pot_down {
                    self.pot_hz -= rate;
                }
                self.pot_hz = self.pot_hz.clamp(self.cfg.min_hz, self.cfg.max_hz);
                (self.pot_hz, false)
            }
        }
    }

    /// Push a frequency out of any skip band to the nearer edge.
    fn apply_skip(&self, f: f64) -> f64 {
        let mut f = f;
        for &(lo, hi) in &self.cfg.skip_bands {
            if f > lo && f < hi {
                f = if f - lo < hi - f { lo } else { hi };
            }
        }
        f
    }

    /// Torque limit in N m at an output frequency (model).
    fn torque_limit(&self, f: f64) -> f64 {
        let m = &self.cfg.motor;
        let i_mag = m.magnetising_current_a();
        let it_rated = (m.current_a * m.current_a - i_mag * i_mag).max(0.0).sqrt();
        let it_max = (self.cfg.current_limit_a.powi(2) - i_mag * i_mag)
            .max(0.0)
            .sqrt();
        let base = if it_rated > 0.0 {
            m.rated_torque_nm() * it_max / it_rated
        } else {
            0.0
        };
        let weak = if f.abs() > self.cfg.vf.base_hz && self.cfg.vf.base_hz > 0.0 {
            self.cfg.vf.base_hz / f.abs()
        } else {
            1.0
        };
        match self.cfg.mode {
            ControlMode::SensorlessVector => base * weak,
            ControlMode::VoltsPerHertz => base * self.cfg.vf.torque_capability_ratio(f).min(1.0),
        }
    }

    /// Advance one step of `dt_s` seconds.
    pub fn step(&mut self, inp: &DriveInputs, dt_s: f64) -> DriveOutputs {
        let dt = if dt_s.is_finite() && dt_s > 0.0 {
            dt_s
        } else {
            0.0
        };
        let cmd = Commands::decode(&self.cfg.macro_, &inp.di);
        let reset_edge = (cmd.fault_reset) && !self.last_reset;
        self.last_reset = cmd.fault_reset;

        // DC bus first: it is fed by the load power of the previous state.
        let load_w = if self.state == DriveState::NotReady {
            0.0
        } else {
            inp.motor_power_w
        };
        let bus: BusStep = if dt > 0.0 {
            self.bus.step(load_w, inp.mains_ok, dt)
        } else {
            BusStep {
                volts: self.bus.volts(),
                chopper_on: self.bus.chopper_on,
                resistor_w: 0.0,
                mains_w: 0.0,
            }
        };

        // Fault detection (latched).
        if self.fault.is_none() {
            let running = matches!(self.state, DriveState::Running | DriveState::Stopping);
            let i = inp.motor_current_a.abs();
            if inp.output_short {
                self.trip(Fault::OutputShort);
            } else if i > self.cfg.hard_trip_a && running {
                self.trip(Fault::OverCurrent);
            } else if bus.volts > self.cfg.bus.over_voltage_v {
                self.trip(Fault::BusOverVoltage);
            } else if running && bus.volts < self.cfg.bus.under_voltage_v {
                self.trip(Fault::BusUnderVoltage);
            } else if running && !inp.mains_ok {
                self.trip(Fault::MainsPhaseLoss);
            } else if cmd.external_fault {
                self.trip(Fault::External);
            } else if self.bus.resistor_overloaded() {
                self.trip(Fault::ResistorOverload);
            } else if running && dt > 0.0 {
                if self.drive_i2t.step(i, dt) {
                    self.trip(Fault::DriveOverload);
                } else if self.motor_i2t.step(i, dt) {
                    self.trip(Fault::MotorOverload);
                }
            } else if dt > 0.0 {
                // Cool while the output is off.
                self.drive_i2t.step(0.0, dt);
                self.motor_i2t.step(0.0, dt);
            }
        }

        // Reference and commands.
        let (main_ref, ref_lost) = self.main_reference_hz(inp, &cmd, dt);
        if ref_lost && self.fault.is_none() && self.state != DriveState::Stopped {
            self.trip(Fault::ReferenceLost);
        }
        let dir_cmd = if cmd.run {
            Some(if cmd.direction_reverse { -1.0 } else { 1.0 })
        } else if cmd.forward && !cmd.reverse {
            Some(1.0)
        } else if cmd.reverse && !cmd.forward {
            Some(-1.0)
        } else {
            None
        };
        let run_cmd = dir_cmd.is_some() && cmd.enable && !cmd.fast_stop;

        // Reset: needs the cause gone and the run command off (restart on level is blocked).
        if self.state == DriveState::Faulted && (reset_edge || (self.fault.is_none())) {
            let cause_gone =
                !cmd.external_fault && !inp.output_short && !self.bus.resistor_overloaded();
            if cause_gone && inp.mains_ok {
                self.fault = None;
                self.motor_i2t.reset();
                self.drive_i2t.reset();
                self.state = DriveState::Stopped;
            }
        }
        if self.state != DriveState::Faulted {
            if !inp.mains_ok
                && self.state == DriveState::Stopped
                && bus.volts < self.cfg.bus.under_voltage_v
            {
                self.state = DriveState::NotReady;
            } else if self.state == DriveState::NotReady
                && inp.mains_ok
                && bus.volts >= self.cfg.bus.under_voltage_v
            {
                self.state = DriveState::Stopped;
            }
        }

        // Target frequency.
        let mut target = 0.0;
        let mut ramp_time_decel = self.cfg.decel_time_s;
        if let (true, Some(dir)) = (run_cmd, dir_cmd) {
            if matches!(
                self.state,
                DriveState::Stopped | DriveState::Stopping | DriveState::Running
            ) {
                let mag = if cmd.jog {
                    self.cfg.jog_hz
                } else if cmd.preset > 0 {
                    self.cfg.presets_hz[cmd.preset.min(3)]
                } else {
                    main_ref
                };
                let mag = mag.abs().min(self.cfg.max_hz);
                let mag = if mag > 0.0 {
                    mag.max(self.cfg.min_hz)
                } else {
                    0.0
                };
                target = self.apply_skip(mag) * dir;
                self.last_dir = dir;
                if self.state != DriveState::Running {
                    self.state = DriveState::Running;
                }
            }
        } else if matches!(self.state, DriveState::Running) {
            self.state = DriveState::Stopping;
        }
        if cmd.fast_stop && !matches!(self.state, DriveState::Faulted | DriveState::NotReady) {
            ramp_time_decel = self.cfg.fast_stop_time_s;
            if self.frequency().abs() > self.cfg.stop_threshold_hz {
                self.state = DriveState::Stopping;
            }
        }

        // Ramp with current limit and bus voltage stretch.
        let f_before = self.frequency();
        let i = inp.motor_current_a.abs();
        self.current_limit_active = self.cfg.current_limit_a > 0.0 && i > self.cfg.current_limit_a;
        if matches!(self.state, DriveState::Running | DriveState::Stopping) && dt > 0.0 {
            let outward = target.abs() > f_before.abs() && target * f_before >= 0.0;
            let decelerating = !outward;
            let hold = (self.current_limit_active
                && outward
                && self.cfg.mode == ControlMode::VoltsPerHertz)
                || (decelerating
                    && self.cfg.ramp_stretch_v > 0.0
                    && bus.volts >= self.cfg.ramp_stretch_v);
            let (accel, decel) = (self.cfg.accel_time_s, ramp_time_decel);
            self.ramp.accel_time_s = accel;
            self.ramp.decel_time_s = decel;
            self.sramp.accel_time_s = accel;
            self.sramp.decel_time_s = decel;
            if !hold {
                if self.cfg.s_ramp_fraction > 0.0 {
                    self.sramp.step(target, dt);
                    self.ramp.frequency_hz = self.sramp.frequency_hz;
                } else {
                    self.ramp.step(target, dt);
                    self.sramp.frequency_hz = self.ramp.frequency_hz;
                }
            }
            // V/f with a heavy overcurrent (above 110 % of the limit) backs the frequency off.
            if self.current_limit_active
                && self.cfg.mode == ControlMode::VoltsPerHertz
                && i > 1.1 * self.cfg.current_limit_a
                && dt > 0.0
            {
                let back = f_before.signum()
                    * (self.cfg.ramp_reference_hz / self.cfg.decel_time_s.max(1e-9))
                    * dt;
                let f = self.frequency();
                let nf = if f.abs() > back.abs() { f - back } else { 0.0 };
                self.set_frequency(nf);
            }
            if self.state == DriveState::Stopping
                && self.frequency().abs() <= self.cfg.stop_threshold_hz
            {
                self.set_frequency(0.0);
                self.state = DriveState::Stopped;
            }
        }

        let f = self.frequency();
        let on = matches!(self.state, DriveState::Running | DriveState::Stopping);
        let voltage_v = if on {
            self.cfg.vf.volts_for_frequency(f)
        } else {
            0.0
        };
        let mains_current_a = if bus.mains_w > 0.0 && self.cfg.bus.mains_v > 0.0 {
            bus.mains_w
                / (3f64.sqrt() * self.cfg.bus.mains_v * self.cfg.mains_power_factor.max(0.1))
        } else {
            0.0
        };
        DriveOutputs {
            state: self.state,
            frequency_hz: if on { f } else { 0.0 },
            voltage_v,
            target_hz: target,
            torque_limit_nm: if on { self.torque_limit(f) } else { 0.0 },
            current_limit_active: self.current_limit_active && on,
            bus_v: bus.volts,
            chopper_on: bus.chopper_on,
            resistor_w: bus.resistor_w,
            mains_current_a,
            motor_overload_used: self.motor_i2t.used(),
            drive_overload_used: self.drive_i2t.used(),
            fault: self.fault,
            fault_relay_ok: self.fault.is_none(),
            running: on,
            at_reference: on && (f - target).abs() < 1e-9,
            brake_release: on && self.fault.is_none() && f.abs() >= self.cfg.brake_release_hz,
        }
    }
}

fn scale(x: f64, in_lo: f64, in_hi: f64, out_lo: f64, out_hi: f64) -> f64 {
    if (in_hi - in_lo).abs() < f64::EPSILON {
        return out_lo;
    }
    let t = ((x - in_lo) / (in_hi - in_lo)).clamp(0.0, 1.0);
    out_lo + t * (out_hi - out_lo)
}
