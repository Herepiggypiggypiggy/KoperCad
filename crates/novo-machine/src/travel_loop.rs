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
//! The drive's brake logic output (R2) is written to a bound relay contact in the drawing, like a
//! limit switch; the brake coil follows the bound brake feed device (or the relay directly when
//! none is bound). A spring brake on the motor shaft holds the axis, stretches the stop and
//! turns the energy it absorbs into heat; its force is capped by the grip of the braked wheels.
//!
//! Exact by definition: the order of the tick, the limit switch rule, the arithmetic of
//! reflected inertia, the quadratic wind force, the stop distance bookkeeping. Model:
//! everything inside `novo_drive::drive` and `novo_drive::induction`, the brake timing (numbers
//! you enter), the rolling resistance (one constant, no stiction), the wind (a uniform speed
//! along the track with one drag coefficient) and the grip (one coefficient, no slip dynamics:
//! the force is capped, the wheels do not spin or slide as separate bodies).

use std::f64::consts::TAU;

use novo_drive::InputError;
use novo_drive::brake::{BrakeState, SpringBrake};
use novo_drive::drive::{Drive, DriveInputs, DriveOutputs};
use novo_drive::induction::{InductionMotor, OperatingPoint, Supply};
use novo_drive::motor::Direction;

use crate::cosim::StepConfig;
use crate::hoist_loop::{BindingFinding, LimitSwitch};
use crate::port::CircuitPort;

/// Largest derived load number accepted (inertia, torque, speed factor): far beyond any crane,
/// far below overflow.
const MAX_DERIVED: f64 = 1.0e12;

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
    wind: Wind,
}

/// Air density used for the wind force, kg/m3.
const AIR_DENSITY: f64 = 1.2;

/// One gust on top of the steady wind: a trapezoid in time (linear rise, hold, linear fall).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Gust {
    /// Extra wind speed at the top of the gust, m/s (forward positive).
    pub peak_m_s: f64,
    pub start_s: f64,
    pub rise_s: f64,
    pub hold_s: f64,
    pub fall_s: f64,
}

impl Gust {
    /// The extra wind speed at time `t` (0 before the gust, 0 after it).
    pub fn speed_at(&self, t: f64) -> f64 {
        let t = t - self.start_s;
        if t <= 0.0 {
            0.0
        } else if t < self.rise_s {
            self.peak_m_s * t / self.rise_s
        } else if t <= self.rise_s + self.hold_s {
            self.peak_m_s
        } else if t < self.rise_s + self.hold_s + self.fall_s {
            self.peak_m_s * (1.0 - (t - self.rise_s - self.hold_s) / self.fall_s)
        } else {
            0.0
        }
    }
}

/// Wind on the axis: a steady speed along the track plus an optional gust, acting on a drag
/// area. The force follows the speed of the air relative to the axis, `1/2 rho cd A (w - v)|w - v|`,
/// so a crane that runs with the wind feels less of it and one that runs against it feels more.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Wind {
    /// Steady wind speed along the track, m/s (forward positive).
    pub speed_m_s: f64,
    pub gust: Option<Gust>,
    /// Drag area `cd * A` is `drag_coeff * area_m2`; both zero means no wind force from speed.
    pub area_m2: f64,
    pub drag_coeff: f64,
}

impl Wind {
    /// Wind speed at time `t`, gust included.
    pub fn speed_at(&self, t: f64) -> f64 {
        self.speed_m_s + self.gust.map_or(0.0, |g| g.speed_at(t))
    }

    /// Force on the axis in N (forward positive) at time `t` while the axis moves at `v` m/s.
    pub fn force_n(&self, t: f64, v: f64) -> f64 {
        let rel = self.speed_at(t) - v;
        0.5 * AIR_DENSITY * self.drag_coeff * self.area_m2 * rel * rel.abs()
    }

    fn check(&self) -> Result<(), InputError> {
        finite("wind_speed_m_s", self.speed_m_s)?;
        nonneg("wind_area_m2", self.area_m2)?;
        nonneg("wind_drag_coeff", self.drag_coeff)?;
        if let Some(g) = self.gust {
            finite("gust_peak_m_s", g.peak_m_s)?;
            nonneg("gust_start_s", g.start_s)?;
            nonneg("gust_rise_s", g.rise_s)?;
            nonneg("gust_hold_s", g.hold_s)?;
            nonneg("gust_fall_s", g.fall_s)?;
            if g.rise_s == 0.0 || g.fall_s == 0.0 {
                // A step would divide by zero in the ramp and is no gust.
                return Err(InputError::NotPositive {
                    field: if g.rise_s == 0.0 {
                        "gust_rise_s"
                    } else {
                        "gust_fall_s"
                    },
                    value: 0.0,
                });
            }
        }
        for (field, value) in [
            ("wind_speed_m_s", self.speed_m_s),
            ("gust_peak_m_s", self.gust.map_or(0.0, |g| g.peak_m_s.abs())),
        ] {
            if value.abs() > MAX_WIND_M_S {
                return Err(InputError::OutOfRange {
                    field,
                    value,
                    min: -MAX_WIND_M_S,
                    max: MAX_WIND_M_S,
                });
            }
        }
        Ok(())
    }
}

/// Largest wind speed accepted, m/s: well above any storm (a category 5 hurricane is about 80).
const MAX_WIND_M_S: f64 = 150.0;

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
        let load = Self {
            mass_kg,
            wheel_radius_m,
            ratio,
            efficiency,
            rolling_coeff,
            wind_force_n,
            motor_inertia_kg_m2,
            wind: Wind::default(),
        };
        // Each value is finite, but a product of extreme ones can still overflow.
        for (field, value) in [
            ("metres_per_rad", load.metres_per_rad()),
            ("inertia_kg_m2", load.inertia_kg_m2()),
            ("rolling_torque_nm", load.rolling_torque_nm()),
            ("wind_torque_nm", load.wind_force_n * load.metres_per_rad()),
        ] {
            if !value.is_finite() || value.abs() > MAX_DERIVED {
                return Err(InputError::OutOfRange {
                    field,
                    value,
                    min: -MAX_DERIVED,
                    max: MAX_DERIVED,
                });
            }
        }
        if load.metres_per_rad() <= 0.0 || load.inertia_kg_m2() <= 0.0 {
            return Err(InputError::NotPositive {
                field: "inertia_kg_m2",
                value: load.inertia_kg_m2(),
            });
        }
        Ok(load)
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

    /// Adds the wind of the site: steady speed, gust and drag area (see [`Wind`]).
    pub fn with_wind(mut self, wind: Wind) -> Result<Self, InputError> {
        wind.check()?;
        self.wind = wind;
        // The same overflow guard as in `new`: the strongest relative air speed is the wind at
        // its peak plus the fastest the axis can run, taken as 50 m/s.
        let peak = wind.speed_m_s.abs() + wind.gust.map_or(0.0, |g| g.peak_m_s.abs()) + 50.0;
        let torque = 0.5
            * AIR_DENSITY
            * wind.drag_coeff
            * wind.area_m2
            * peak
            * peak
            * self.metres_per_rad();
        if !torque.is_finite() || torque > MAX_DERIVED {
            return Err(InputError::OutOfRange {
                field: "wind_torque_nm",
                value: torque,
                min: -MAX_DERIVED,
                max: MAX_DERIVED,
            });
        }
        Ok(self)
    }

    pub fn wind(&self) -> &Wind {
        &self.wind
    }

    /// Total wind force on the axis in N (forward positive) at time `t` and axis speed `v`: the
    /// constant force of the property sheet plus the force of the air flow.
    pub fn wind_force_n(&self, t: f64, v: f64) -> f64 {
        self.wind_force_n + self.wind.force_n(t, v)
    }

    /// A load force at the wheel, as torque on the shaft: the drivetrain loses `eta` whichever
    /// way the power flows. A load that opposes the motion costs the motor `1/eta`, one that
    /// pushes the axis (back-drives the gearbox) gives back `eta`. The two blend smoothly
    /// through standstill, over 0.5 rad/s, like the rolling resistance.
    fn through_gearbox(&self, ideal_nm: f64, omega: f64) -> f64 {
        if ideal_nm == 0.0 {
            return 0.0;
        }
        let a = (omega * ideal_nm.signum() / 0.5).clamp(-1.0, 1.0);
        ideal_nm * ((1.0 + a) * 0.5 * self.efficiency + (1.0 - a) * 0.5 / self.efficiency)
    }

    /// Torque the load applies to the shaft at motor speed `omega` (positive = forward) at time
    /// `t`: rolling resistance against the motion (smoothed through standstill) and the wind.
    pub fn torque_at(&self, omega: f64, t: f64) -> f64 {
        let mpr = self.metres_per_rad();
        let rolling = -self.mass_kg * G * self.rolling_coeff * mpr * (omega / 0.5).clamp(-1.0, 1.0);
        let wind = self.wind_force_n(t, omega * mpr) * mpr;
        self.through_gearbox(rolling, omega) + self.through_gearbox(wind, omega)
    }

    /// Torque the load applies to the shaft with the constant wind force only (no air flow, no
    /// gust).
    pub fn torque_nm(&self, omega: f64) -> f64 {
        let mpr = self.metres_per_rad();
        let rolling = -self.mass_kg * G * self.rolling_coeff * mpr * (omega / 0.5).clamp(-1.0, 1.0);
        self.through_gearbox(rolling, omega) + self.through_gearbox(self.wind_force_n * mpr, omega)
    }

    /// Grip of the wheels in N: the weight on the driven (or braked) wheels times the
    /// coefficient of friction between wheel and rail.
    pub fn grip_n(&self, grip: &Grip) -> f64 {
        grip.coeff * grip.share * self.mass_kg * G
    }

    pub fn mass_kg(&self) -> f64 {
        self.mass_kg
    }

    pub fn efficiency(&self) -> f64 {
        self.efficiency
    }
}

/// How well the wheels hold on the rail: the share of the weight on the driven and braked
/// wheels and the coefficient of friction (steel on dry steel about 0.15 to 0.2, wet or oily
/// 0.08 to 0.12). Model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grip {
    pub share: f64,
    pub coeff: f64,
}

impl Default for Grip {
    fn default() -> Self {
        Self {
            share: 0.5,
            coeff: 0.15,
        }
    }
}

impl Grip {
    pub fn new(share: f64, coeff: f64) -> Result<Self, InputError> {
        if !(share.is_finite() && share > 0.0 && share <= 1.0) {
            return Err(InputError::OutOfRange {
                field: "driven_share",
                value: share,
                min: 0.0,
                max: 1.0,
            });
        }
        pos("adhesion", coeff)?;
        Ok(Self { share, coeff })
    }
}

fn finite(field: &'static str, v: f64) -> Result<(), InputError> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(InputError::NotFinite { field })
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
    /// Device tag of the drive's brake relay contact (logic output R2). The module writes its
    /// state; the drawing connects it to the brake contactor.
    pub brake_relay: Option<String>,
    /// Device tag whose energised state feeds the brake coil (brake contactor, rectifier).
    /// `None`: the brake follows the drive's brake output directly (an ideal wiring).
    pub brake_feed: Option<String>,
}

/// The spring brake on the motor shaft of a travel axis and the grip of the wheels it brakes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TravelBrake {
    pub brake: SpringBrake,
    pub grip: Grip,
}

/// Distance the axis travelled between letting go of the direction command and standstill,
/// the number to compare with the clearance in front of the buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StopRecord {
    pub from_speed_m_s: f64,
    pub distance_m: f64,
    pub time_s: f64,
}

/// What a travel run found worth saying.
#[derive(Debug, Clone, PartialEq)]
pub enum TravelFindingKind {
    /// The axis stopped after the command was released.
    Stopped(StopRecord),
    /// The brake wanted more force than the wheels can pass to the rail: the stop is capped at
    /// the grip (`decel_m_s2`), longer than the brake alone.
    BrakeSkid {
        decel_m_s2: f64,
        brake_force_n: f64,
        grip_n: f64,
    },
    /// The motor asked for more force at the wheels than they hold.
    DriveSlip { force_n: f64, grip_n: f64 },
    /// The axis moved with the command off and the brake not holding (open, late, or worn).
    RollAway { distance_m: f64, peak_m_s: f64 },
    /// The motor dragged against a closed brake (the brake output came after the drive did).
    BrakeDrag { energy_j: f64 },
}

impl TravelFindingKind {
    pub fn text(&self) -> String {
        match self {
            Self::Stopped(r) => format!(
                "Stopped from {:.2} m/s in {:.2} m ({:.1} s) after the command was released.",
                r.from_speed_m_s.abs(),
                r.distance_m,
                r.time_s
            ),
            Self::BrakeSkid {
                decel_m_s2,
                brake_force_n,
                grip_n,
            } => format!(
                "The brake asks {brake_force_n:.0} N at the wheels, the rail grip is {grip_n:.0} N: \
                 the wheels skid and the stop is capped at {decel_m_s2:.2} m/s2."
            ),
            Self::DriveSlip { force_n, grip_n } => format!(
                "The drive asks {force_n:.0} N at the wheels, the rail grip is {grip_n:.0} N: \
                 the driven wheels would spin."
            ),
            Self::RollAway {
                distance_m,
                peak_m_s,
            } => format!(
                "The axis moved {distance_m:.2} m (up to {peak_m_s:.2} m/s) with no command and \
                 the brake not holding."
            ),
            Self::BrakeDrag { energy_j } => format!(
                "The motor dragged against the closed brake and put {energy_j:.0} J into it."
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TravelFinding {
    pub time_s: f64,
    pub kind: TravelFindingKind,
}

/// Most findings kept per module; a long run says how many it dropped.
const MAX_FINDINGS: usize = 40;

/// What a travel module did in the last tick.
#[derive(Debug, Clone, PartialEq)]
pub struct TravelSnapshot {
    pub drive: DriveOutputs,
    pub motor: OperatingPoint,
    /// Position in metres, 0 at the start (positive is forward).
    pub position_m: f64,
    /// Speed in metres per second.
    pub speed_m_s: f64,
    /// Wind force on the axis in N (forward positive) and the speed of the air.
    pub wind_force_n: f64,
    pub wind_speed_m_s: f64,
    /// Brake: `None` when the axis has none, else its holding torque on the motor shaft in N m.
    pub brake_torque_nm: Option<f64>,
    pub brake_state: Option<BrakeState>,
    /// The last stop, if the axis stopped at least once.
    pub last_stop: Option<StopRecord>,
}

/// Bookkeeping of a stop that is under way.
#[derive(Debug, Clone, Copy)]
struct StopWatch {
    from_speed_m_s: f64,
    start_position_m: f64,
    start_time_s: f64,
}

/// A travel module: drive, motor, load and bindings.
#[derive(Debug)]
pub struct TravelModule {
    pub id: String,
    pub drive: Drive,
    pub motor: InductionMotor,
    pub load: TravelLoad,
    pub binding: TravelBinding,
    /// Grip of the driven wheels (also used without a brake, for the slip finding).
    pub grip: Grip,
    /// The spring brake on the motor shaft, if the axis has one.
    pub brake: Option<SpringBrake>,
    omega: f64,
    position_m: f64,
    feedback: (f64, f64),
    snapshot: Option<TravelSnapshot>,
    fault: Option<String>,
    relay: bool,
    findings: Vec<TravelFinding>,
    findings_dropped: usize,
    stopwatch: Option<StopWatch>,
    command_was_on: bool,
    last_stop: Option<StopRecord>,
    brake_energy_j: f64,
    brake_drag_j: f64,
    drag_reported: bool,
    rolled: bool,
    skid_reported: bool,
    slip_reported: bool,
    drift: Option<(f64, f64)>,
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
            grip: Grip::default(),
            brake: None,
            omega: 0.0,
            position_m: 0.0,
            feedback: (0.0, 0.0),
            snapshot: None,
            fault: None,
            relay: false,
            findings: Vec::new(),
            findings_dropped: 0,
            stopwatch: None,
            command_was_on: false,
            last_stop: None,
            brake_energy_j: 0.0,
            brake_drag_j: 0.0,
            drag_reported: false,
            rolled: false,
            skid_reported: false,
            slip_reported: false,
            drift: None,
        }
    }

    /// Fits a spring brake on the motor shaft and the grip of the wheels.
    pub fn with_brake(mut self, brake: SpringBrake, grip: Grip) -> Self {
        self.brake = Some(brake);
        self.grip = grip;
        self
    }

    pub fn position_m(&self) -> f64 {
        self.position_m
    }

    /// Motor speed in rpm (signed).
    pub fn rpm(&self) -> f64 {
        self.omega * 60.0 / TAU
    }

    /// Why this module stopped, if it did: the engine refused a number in the run. A stopped
    /// module keeps its last snapshot and no longer moves; the rest of the loop runs on.
    pub fn fault(&self) -> Option<&str> {
        self.fault.as_deref()
    }

    /// The last tick's numbers, `None` before the first tick.
    pub fn snapshot(&self) -> Option<&TravelSnapshot> {
        self.snapshot.as_ref()
    }

    pub fn findings(&self) -> &[TravelFinding] {
        &self.findings
    }

    pub fn findings_dropped(&self) -> usize {
        self.findings_dropped
    }

    /// Heat the brake absorbed by slipping (against the motor or while stopping), J.
    pub fn brake_energy_j(&self) -> f64 {
        self.brake_energy_j
    }

    fn note(&mut self, time_s: f64, kind: TravelFindingKind) {
        if self.findings.len() < MAX_FINDINGS {
            self.findings.push(TravelFinding { time_s, kind });
        } else {
            self.findings_dropped += 1;
        }
    }

    /// Braking torque at the motor shaft: the spring brake, capped by what the braked wheels can
    /// pass to the rail. Returns the torque and, when the cap bit, the skid finding.
    fn brake_torque(&self) -> (f64, Option<TravelFindingKind>) {
        let Some(brake) = &self.brake else {
            return (0.0, None);
        };
        let wanted = brake.holding_torque_nm();
        if wanted <= 0.0 {
            return (0.0, None);
        }
        let mpr = self.load.metres_per_rad();
        let grip_n = self.load.grip_n(&self.grip);
        // Force at the wheels from the shaft torque; the gearbox gives back `eta` on the way.
        let force_n = wanted / mpr * self.load.efficiency();
        if force_n <= grip_n {
            return (wanted, None);
        }
        let capped = grip_n / self.load.efficiency() * mpr;
        (
            capped,
            Some(TravelFindingKind::BrakeSkid {
                decel_m_s2: capped / self.load.inertia_kg_m2() * mpr,
                brake_force_n: force_n,
                grip_n,
            }),
        )
    }
}

/// The loop over any number of travel modules.
pub struct TravelLoop {
    pub config: StepConfig,
    pub modules: Vec<TravelModule>,
    time_s: f64,
}

/// Below this speed (m/s) an axis counts as standing.
const STANDSTILL_M_S: f64 = 0.004;

/// Above this speed (m/s) a release of the command starts a stop record.
const MOVING_M_S: f64 = 0.02;

/// Distance (m) an axis may move without a command before it counts as rolling away.
const ROLL_AWAY_M: f64 = 0.05;

/// Heat (J) the brake may take from the motor dragging against it before it is reported.
const DRAG_REPORT_J: f64 = 100.0;

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
            for (what, tag) in [
                ("brake relay", &m.binding.brake_relay),
                ("brake contactor", &m.binding.brake_feed),
            ] {
                if let Some(tag) = tag
                    && !circuit.knows_device(tag)
                {
                    out.push(finding(
                        &m.id,
                        format!("{what} {tag} is not in the circuit"),
                    ));
                }
            }
            if m.brake.is_some()
                && m.binding.brake_feed.is_some()
                && m.binding.brake_relay.is_none()
            {
                out.push(finding(
                    &m.id,
                    "the brake has a contactor but no relay contact of the drive is bound: \
                     nothing opens it, so the brake stays closed"
                        .to_owned(),
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
        let t = self.time_s;
        for m in &self.modules {
            for limit in &m.binding.limits {
                circuit.set_contact(&limit.binding.device_tag, limit.closed_at(m.position_m));
            }
            if let Some(tag) = &m.binding.brake_relay {
                circuit.set_contact(tag, m.relay);
            }
        }
        circuit.settle(dt);
        for m in &mut self.modules {
            if m.fault.is_some() {
                continue;
            }
            if let Err(why) = step_module(m, circuit, t, dt) {
                m.fault = Some(format!("the run stopped: {why}"));
            }
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

/// One tick of one module: drive, brake, shaft, bookkeeping.
fn step_module(
    m: &mut TravelModule,
    circuit: &mut impl CircuitPort,
    t: f64,
    dt: f64,
) -> Result<(), InputError> {
    let di: Vec<bool> = m
        .binding
        .inputs
        .iter()
        .map(|tag| tag.as_deref().is_some_and(|tag| circuit.is_energised(tag)))
        .collect();
    // The first two inputs of the travel macro are forward and reverse.
    let command_on = di.iter().take(2).any(|on| *on);
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
    m.relay = out.brake_release;
    if let Some(brake) = m.brake.as_mut() {
        let fed = match &m.binding.brake_feed {
            Some(tag) => circuit.is_energised(tag),
            None => out.brake_release,
        };
        brake.step(fed, dt);
    }
    let (brake_nm, skid) = m.brake_torque();
    let direction = if out.frequency_hz < 0.0 {
        Direction::Negative
    } else {
        Direction::Positive
    };
    let volts = if out.running { out.voltage_v } else { 0.0 };
    let supply = Supply::new(out.frequency_hz.abs(), volts.max(0.0), direction)?;
    let inertia = m.load.inertia_kg_m2();
    let load = m.load;
    let mpr = load.metres_per_rad();
    let omega0 = m.omega;

    // At rest a closed brake holds the axis while the motor and the wind pull less than it.
    let held = if brake_nm > 0.0 && omega0 == 0.0 {
        let p0 = m.motor.evaluate(&supply, 0.0)?;
        let net = p0.torque_nm + load.torque_at(0.0, t);
        (net.abs() <= brake_nm).then_some(p0)
    } else {
        None
    };
    let (point, omega1) = match held {
        Some(p0) => (p0, 0.0),
        None => {
            let mut w = omega0;
            let step = m
                .motor
                .step_shaft(&mut w, &supply, inertia, dt, |w| load.torque_at(w, t))?;
            if brake_nm > 0.0 {
                // The brake acts as a constant retarding torque on the shaft that cannot reverse
                // it: it takes the speed down by `T dt / J` and stops the shaft at zero, unless
                // the motor and the wind still pull harder than the brake holds.
                let dw = brake_nm / inertia * dt;
                if w.abs() > dw {
                    w -= w.signum() * dw;
                } else {
                    let p = m.motor.evaluate(&supply, 0.0)?;
                    if (p.torque_nm + load.torque_at(0.0, t)).abs() <= brake_nm {
                        w = 0.0;
                    }
                }
            }
            (step.point, w)
        }
    };
    m.omega = omega1;
    m.feedback = (point.stator_current_a, point.input_power_w);
    let v = omega1 * mpr;
    m.position_m += v * dt;

    let slip_energy = brake_nm * (0.5 * (omega0 + omega1)).abs() * dt;
    m.brake_energy_j += slip_energy;
    if command_on && out.running {
        m.brake_drag_j += slip_energy;
        if !m.drag_reported && m.brake_drag_j >= DRAG_REPORT_J {
            m.drag_reported = true;
            let energy_j = m.brake_drag_j;
            m.note(t, TravelFindingKind::BrakeDrag { energy_j });
        }
    }
    if let Some(kind) = skid
        && !m.skid_reported
        && v.abs() > MOVING_M_S
    {
        m.skid_reported = true;
        m.note(t, kind);
    }
    let grip_n = load.grip_n(&m.grip);
    let drive_n = (point.torque_nm / mpr).abs() * load.efficiency();
    if !m.slip_reported && command_on && drive_n > grip_n {
        m.slip_reported = true;
        m.note(
            t,
            TravelFindingKind::DriveSlip {
                force_n: drive_n,
                grip_n,
            },
        );
    }
    track_stop(m, command_on, v, t);

    let wind_speed = load.wind().speed_at(t);
    m.snapshot = Some(TravelSnapshot {
        drive: out,
        motor: point,
        position_m: m.position_m,
        speed_m_s: v,
        wind_force_n: load.wind_force_n(t, v),
        wind_speed_m_s: wind_speed,
        brake_torque_nm: m.brake.as_ref().map(SpringBrake::holding_torque_nm),
        brake_state: m.brake.as_ref().map(SpringBrake::state),
        last_stop: m.last_stop,
    });
    Ok(())
}

/// Stop distance and roll-away bookkeeping.
fn track_stop(m: &mut TravelModule, command_on: bool, v: f64, t: f64) {
    if command_on {
        m.stopwatch = None;
        m.drift = None;
        m.rolled = false;
    } else if m.command_was_on && v.abs() > MOVING_M_S && m.stopwatch.is_none() {
        m.stopwatch = Some(StopWatch {
            from_speed_m_s: v,
            start_position_m: m.position_m,
            start_time_s: t,
        });
    }
    m.command_was_on = command_on;
    if let Some(sw) = m.stopwatch
        && v.abs() < STANDSTILL_M_S
    {
        let record = StopRecord {
            from_speed_m_s: sw.from_speed_m_s,
            distance_m: (m.position_m - sw.start_position_m).abs(),
            time_s: t - sw.start_time_s,
        };
        m.stopwatch = None;
        m.last_stop = Some(record);
        m.note(t, TravelFindingKind::Stopped(record));
    }
    if command_on || m.stopwatch.is_some() {
        return;
    }
    let (origin, peak) = m.drift.get_or_insert((m.position_m, 0.0));
    *peak = peak.max(v.abs());
    let (moved, peak_m_s) = ((m.position_m - *origin).abs(), *peak);
    if moved > ROLL_AWAY_M && !m.rolled {
        m.rolled = true;
        m.note(
            t,
            TravelFindingKind::RollAway {
                distance_m: moved,
                peak_m_s,
            },
        );
    }
}
