//! The fixed-step loop over travel modules: a drive, its motor and a wheeled load (lane L5a part 3).
//!
//! One tick, in this order, always the same, so a run is deterministic:
//! 1. limit switches are written from the positions of the previous tick,
//! 2. the circuit settles by one step,
//! 3. each module reads the drive's digital inputs and mains from the circuit,
//! 4. `Drive::step` runs with the motor current and power measured in the previous tick,
//! 5. the motor model (`novo_drive::induction`) runs the shaft under the drive's frequency and
//!    voltage and the wheel load, and its current and power become the next tick's feedback.
//!
//! The drive's output terminals are not traced through the circuit: the drive replaces the
//! contactors, so what the circuit gives it is its control inputs and its mains.
//!
//! Exact by definition: the order of the tick, the limit switch rule, the arithmetic of
//! reflected inertia. Model: everything inside `novo_drive::drive` and `novo_drive::induction`,
//! and the rolling resistance (one constant, no stiction, no brake).

use std::f64::consts::TAU;

use novo_drive::InputError;
use novo_drive::drive::{Drive, DriveInputs, DriveOutputs};
use novo_drive::induction::{InductionMotor, OperatingPoint, Supply};
use novo_drive::motor::Direction;

use crate::cosim::StepConfig;
use crate::hoist_loop::{BindingFinding, LimitSwitch};
use crate::port::CircuitPort;

/// Standard gravity, m/s2.
const G: f64 = novo_drive::GRAVITY;

/// The wheeled load of a travel axis (trolley, bridge) as seen from the motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TravelLoad {
    mass_kg: f64,
    wheel_radius_m: f64,
    ratio: f64,
    efficiency: f64,
    rolling_coeff: f64,
    wind_force_n: f64,
    motor_inertia_kg_m2: f64,
}

impl TravelLoad {
    /// `mass_kg` is everything that moves, `ratio` the gearbox ratio motor to wheel,
    /// `efficiency` the drivetrain efficiency (0 to 1), `rolling_coeff` the rolling resistance
    /// force over the weight (typically 0.005 to 0.02), `wind_force_n` a constant force in the
    /// forward direction (negative against it).
    pub fn new(
        mass_kg: f64,
        wheel_radius_m: f64,
        ratio: f64,
        efficiency: f64,
        rolling_coeff: f64,
        wind_force_n: f64,
        motor_inertia_kg_m2: f64,
    ) -> Result<Self, InputError> {
        pos("mass_kg", mass_kg)?;
        pos("wheel_radius_m", wheel_radius_m)?;
        pos("ratio", ratio)?;
        if !(efficiency.is_finite() && efficiency > 0.0 && efficiency <= 1.0) {
            return Err(InputError::OutOfRange {
                field: "efficiency",
                value: efficiency,
                min: 0.0,
                max: 1.0,
            });
        }
        nonneg("rolling_coeff", rolling_coeff)?;
        nonneg("motor_inertia_kg_m2", motor_inertia_kg_m2)?;
        if !wind_force_n.is_finite() {
            return Err(InputError::NotFinite {
                field: "wind_force_n",
            });
        }
        Ok(Self {
            mass_kg,
            wheel_radius_m,
            ratio,
            efficiency,
            rolling_coeff,
            wind_force_n,
            motor_inertia_kg_m2,
        })
    }

    /// Metres per second per rad/s of motor speed: `r / i`.
    pub fn metres_per_rad(&self) -> f64 {
        self.wheel_radius_m / self.ratio
    }

    /// Total inertia on the motor shaft: rotor plus the mass reflected, `m (r / i)^2`.
    pub fn inertia_kg_m2(&self) -> f64 {
        self.motor_inertia_kg_m2 + self.mass_kg * self.metres_per_rad().powi(2)
    }

    /// Rolling resistance on the shaft in N m while moving: `m g c r / (i eta)`.
    pub fn rolling_torque_nm(&self) -> f64 {
        self.mass_kg * G * self.rolling_coeff * self.metres_per_rad() / self.efficiency
    }

    /// Torque the load applies to the shaft at motor speed `omega` (positive = forward): rolling
    /// resistance against the motion (smoothed through standstill) and the wind.
    pub fn torque_nm(&self, omega: f64) -> f64 {
        -self.rolling_torque_nm() * (omega / 0.5).clamp(-1.0, 1.0)
            + self.wind_force_n * self.metres_per_rad()
    }
}

fn pos(field: &'static str, v: f64) -> Result<(), InputError> {
    if !v.is_finite() {
        Err(InputError::NotFinite { field })
    } else if v <= 0.0 {
        Err(InputError::NotPositive { field, value: v })
    } else {
        Ok(())
    }
}

fn nonneg(field: &'static str, v: f64) -> Result<(), InputError> {
    if !v.is_finite() {
        Err(InputError::NotFinite { field })
    } else if v < 0.0 {
        Err(InputError::Negative { field, value: v })
    } else {
        Ok(())
    }
}

/// Where a travel module is attached to the schematic.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TravelBinding {
    /// Device tags whose energised state feeds the drive's digital inputs, in the order of the
    /// drive's terminal macro (`None` = the input is not wired).
    pub inputs: Vec<Option<String>>,
    /// Device tag of the main contactor coil that feeds the drive. `None` means mains is always on.
    pub mains: Option<String>,
    /// Limit switches on the travel: `Upper` protects the forward end, `Lower` the reverse end.
    pub limits: Vec<LimitSwitch>,
}

/// What a travel module did in the last tick.
#[derive(Debug, Clone, PartialEq)]
pub struct TravelSnapshot {
    pub drive: DriveOutputs,
    pub motor: OperatingPoint,
    /// Position in metres, 0 at the start (positive is forward).
    pub position_m: f64,
    /// Speed in metres per second.
    pub speed_m_s: f64,
}

/// A travel module: drive, motor, load and bindings.
#[derive(Debug)]
pub struct TravelModule {
    pub id: String,
    pub drive: Drive,
    pub motor: InductionMotor,
    pub load: TravelLoad,
    pub binding: TravelBinding,
    omega: f64,
    position_m: f64,
    feedback: (f64, f64),
    snapshot: Option<TravelSnapshot>,
}

impl TravelModule {
    pub fn new(
        id: impl Into<String>,
        drive: Drive,
        motor: InductionMotor,
        load: TravelLoad,
        binding: TravelBinding,
    ) -> Self {
        Self {
            id: id.into(),
            drive,
            motor,
            load,
            binding,
            omega: 0.0,
            position_m: 0.0,
            feedback: (0.0, 0.0),
            snapshot: None,
        }
    }

    pub fn position_m(&self) -> f64 {
        self.position_m
    }

    /// Motor speed in rpm (signed).
    pub fn rpm(&self) -> f64 {
        self.omega * 60.0 / TAU
    }

    /// The last tick's numbers, `None` before the first tick.
    pub fn snapshot(&self) -> Option<&TravelSnapshot> {
        self.snapshot.as_ref()
    }
}

/// The loop over any number of travel modules.
pub struct TravelLoop {
    pub config: StepConfig,
    pub modules: Vec<TravelModule>,
    time_s: f64,
}

impl TravelLoop {
    pub fn new(config: StepConfig) -> Result<Self, InputError> {
        config.validate()?;
        Ok(Self {
            config,
            modules: Vec::new(),
            time_s: 0.0,
        })
    }

    pub fn time_s(&self) -> f64 {
        self.time_s
    }

    pub fn module(&self, id: &str) -> Option<&TravelModule> {
        self.modules.iter().find(|m| m.id == id)
    }

    /// Binding problems to show before the run: a device the circuit does not have, and drive
    /// configuration problems that would silently disable a protection.
    pub fn check_bindings(&self, circuit: &impl CircuitPort) -> Vec<BindingFinding> {
        let mut out = Vec::new();
        let finding = |module: &str, message: String| BindingFinding {
            module: module.to_owned(),
            message,
        };
        for m in &self.modules {
            for tag in m.binding.inputs.iter().flatten() {
                if !circuit.knows_device(tag) {
                    out.push(finding(
                        &m.id,
                        format!("drive input {tag} is not in the circuit"),
                    ));
                }
            }
            if let Some(tag) = &m.binding.mains
                && !circuit.knows_device(tag)
            {
                out.push(finding(
                    &m.id,
                    format!("main contactor {tag} is not in the circuit"),
                ));
            }
            for limit in &m.binding.limits {
                if !circuit.knows_device(&limit.binding.device_tag) {
                    out.push(finding(
                        &m.id,
                        format!(
                            "limit switch {} is not in the circuit",
                            limit.binding.device_tag
                        ),
                    ));
                }
            }
            let wired = m.binding.inputs.len();
            let needed = m.drive.cfg.macro_.inputs.len();
            if wired < needed {
                out.push(finding(
                    &m.id,
                    format!(
                        "the drive macro \"{}\" has {needed} inputs and only {wired} are bound; the rest stay off",
                        m.drive.cfg.macro_.name
                    ),
                ));
            }
            for problem in m.drive.config_problems() {
                out.push(finding(&m.id, problem.to_owned()));
            }
        }
        out
    }

    /// One tick.
    pub fn step(&mut self, circuit: &mut impl CircuitPort) {
        let dt = self.config.dt_s;
        for m in &self.modules {
            for limit in &m.binding.limits {
                circuit.set_contact(&limit.binding.device_tag, limit.closed_at(m.position_m));
            }
        }
        circuit.settle(dt);
        for m in &mut self.modules {
            let di: Vec<bool> = m
                .binding
                .inputs
                .iter()
                .map(|t| t.as_deref().is_some_and(|t| circuit.is_energised(t)))
                .collect();
            let mains_ok = m
                .binding
                .mains
                .as_deref()
                .is_none_or(|tag| circuit.is_energised(tag));
            let inputs = DriveInputs {
                di,
                mains_ok,
                motor_current_a: m.feedback.0,
                motor_power_w: m.feedback.1,
                ..DriveInputs::default()
            };
            let out = m.drive.step(&inputs, dt);
            let direction = if out.frequency_hz < 0.0 {
                Direction::Negative
            } else {
                Direction::Positive
            };
            let volts = if out.running { out.voltage_v } else { 0.0 };
            let supply = Supply::new(out.frequency_hz.abs(), volts.max(0.0), direction)
                .expect("drive outputs are finite and not negative");
            let inertia = m.load.inertia_kg_m2();
            let load = m.load;
            let step = m
                .motor
                .step_shaft(&mut m.omega, &supply, inertia, dt, |w| load.torque_nm(w))
                .expect("inertia and step are validated positive");
            m.feedback = (step.point.stator_current_a, step.point.input_power_w);
            let v = m.omega * m.load.metres_per_rad();
            m.position_m += v * dt;
            m.snapshot = Some(TravelSnapshot {
                drive: out,
                motor: step.point,
                position_m: m.position_m,
                speed_m_s: v,
            });
        }
        self.time_s += dt;
    }

    /// Runs `ticks` ticks.
    pub fn run(&mut self, circuit: &mut impl CircuitPort, ticks: usize) {
        for _ in 0..ticks {
            self.step(circuit);
        }
    }
}
