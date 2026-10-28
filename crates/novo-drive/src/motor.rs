//! Induction motors: synchronous speed, torque-speed curve and the two-speed pole-change
//! motor of a contactor hoist (lane L2a).
//!
//! Exact by definition: synchronous speed. Model: the torque-speed curve is the Kloss
//! formula fitted to the nameplate (rated power, rated speed, breakdown ratio).

use crate::Fidelity;
use crate::error::{self, InputError};

/// Synchronous speed in rpm for a supply frequency and pole count: `n_sync = 120 f / p`.
///
/// `poles` must be an even number of at least 2 and `frequency_hz` finite and not negative
/// (the sense of rotation is a [`Direction`], not a negative frequency).
pub fn sync_rpm(frequency_hz: f64, poles: u32) -> Result<f64, InputError> {
    error::non_negative("frequency_hz", frequency_hz)?;
    error::poles(poles)?;
    Ok(120.0 * frequency_hz / f64::from(poles))
}

/// Fidelity of [`sync_rpm`].
pub const SYNC_RPM_FIDELITY: Fidelity = Fidelity::Exact;

/// Fidelity of the torque-speed curve ([`Winding::torque_nm`]).
pub const TORQUE_CURVE_FIDELITY: Fidelity = Fidelity::Model;

/// Sense of rotation. For a hoist, positive is raising.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Positive,
    Negative,
}

impl Direction {
    pub fn sign(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }
}

/// One winding of an induction motor on a fixed supply, described by its nameplate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Winding {
    poles: u32,
    frequency_hz: f64,
    rated_power_w: f64,
    rated_speed_rpm: f64,
    breakdown_ratio: f64,
}

impl Winding {
    /// `breakdown_ratio` is breakdown torque over rated torque (greater than 1).
    /// The rated speed must be below the synchronous speed (the rated slip is positive).
    pub fn new(
        poles: u32,
        frequency_hz: f64,
        rated_power_w: f64,
        rated_speed_rpm: f64,
        breakdown_ratio: f64,
    ) -> Result<Self, InputError> {
        error::positive("frequency_hz", frequency_hz)?;
        error::poles(poles)?;
        error::positive("rated_power_w", rated_power_w)?;
        error::positive("rated_speed_rpm", rated_speed_rpm)?;
        error::finite("breakdown_ratio", breakdown_ratio)?;
        let sync = sync_rpm(frequency_hz, poles)?;
        if rated_speed_rpm >= sync {
            return Err(InputError::OutOfRange {
                field: "rated_speed_rpm",
                value: rated_speed_rpm,
                min: 0.0,
                max: sync,
            });
        }
        if breakdown_ratio <= 1.0 {
            return Err(InputError::OutOfRange {
                field: "breakdown_ratio",
                value: breakdown_ratio,
                min: 1.0,
                max: f64::INFINITY,
            });
        }
        Ok(Self {
            poles,
            frequency_hz,
            rated_power_w,
            rated_speed_rpm,
            breakdown_ratio,
        })
    }

    pub fn poles(&self) -> u32 {
        self.poles
    }

    pub fn sync_rpm(&self) -> f64 {
        120.0 * self.frequency_hz / f64::from(self.poles)
    }

    pub fn sync_rad_s(&self) -> f64 {
        self.sync_rpm() * std::f64::consts::TAU / 60.0
    }

    pub fn rated_slip(&self) -> f64 {
        (self.sync_rpm() - self.rated_speed_rpm) / self.sync_rpm()
    }

    /// Rated torque in N m: `P / omega_rated`.
    pub fn rated_torque_nm(&self) -> f64 {
        self.rated_power_w / (self.rated_speed_rpm * std::f64::consts::TAU / 60.0)
    }

    pub fn breakdown_torque_nm(&self) -> f64 {
        self.breakdown_ratio * self.rated_torque_nm()
    }

    /// Slip at breakdown torque from the Kloss fit: `s_b = s_n (k + sqrt(k^2 - 1))`, `k = T_b / T_n`.
    pub fn breakdown_slip(&self) -> f64 {
        let k = self.breakdown_ratio;
        self.rated_slip() * (k + (k * k - 1.0).sqrt())
    }

    /// Torque in N m on the shaft at shaft speed `omega_rad_s` (signed, positive = rising
    /// direction of the shaft) when the winding is fed with the field turning in `direction`.
    /// Beyond synchronous speed in the field direction the torque is negative in the field
    /// sense (generator braking, an overhauling load); against the field it is plugging.
    ///
    /// Kloss: `T = 2 T_b / (s/s_b + s_b/s)`. Model, see [`TORQUE_CURVE_FIDELITY`].
    pub fn torque_nm(&self, direction: Direction, omega_rad_s: f64) -> f64 {
        let sign = direction.sign();
        let slip = (self.sync_rad_s() - sign * omega_rad_s) / self.sync_rad_s();
        if slip == 0.0 {
            return 0.0;
        }
        let sb = self.breakdown_slip();
        sign * 2.0 * self.breakdown_torque_nm() / (slip / sb + sb / slip)
    }

    /// Slip at which the stable part of the curve delivers `torque_nm` (field sense, signed);
    /// `None` when the torque exceeds breakdown. Closed form of the Kloss curve:
    /// `s = s_b (T_b - sqrt(T_b^2 - T^2)) / T`.
    pub fn slip_for_torque(&self, torque_nm: f64) -> Option<f64> {
        let tb = self.breakdown_torque_nm();
        if !torque_nm.is_finite() || torque_nm.abs() > tb {
            return None;
        }
        if torque_nm == 0.0 {
            return Some(0.0);
        }
        Some(self.breakdown_slip() * (tb - (tb * tb - torque_nm * torque_nm).sqrt()) / torque_nm)
    }

    /// Steady shaft speed in rad/s (signed) at which the winding balances `load_torque_nm`
    /// (the torque the motor must deliver in the field sense, negative when the load drives it).
    pub fn steady_omega_rad_s(&self, direction: Direction, torque_nm: f64) -> Option<f64> {
        let s = self.slip_for_torque(torque_nm)?;
        Some(direction.sign() * self.sync_rad_s() * (1.0 - s))
    }
}

/// Which winding of a pole-change motor is fed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindingSel {
    /// High pole count, slow, small torque (fine).
    Fine,
    /// Low pole count, fast (coarse).
    Coarse,
}

/// Two-speed pole-change motor (for example 12/2 poles). One shaft, one rotor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleChangeMotor {
    fine: Winding,
    coarse: Winding,
    rotor_inertia_kg_m2: f64,
}

impl PoleChangeMotor {
    pub fn new(
        fine: Winding,
        coarse: Winding,
        rotor_inertia_kg_m2: f64,
    ) -> Result<Self, InputError> {
        error::non_negative("rotor_inertia_kg_m2", rotor_inertia_kg_m2)?;
        if fine.poles() <= coarse.poles() {
            return Err(InputError::BadPoles {
                poles: fine.poles(),
            });
        }
        Ok(Self {
            fine,
            coarse,
            rotor_inertia_kg_m2,
        })
    }

    pub fn winding(&self, sel: WindingSel) -> &Winding {
        match sel {
            WindingSel::Fine => &self.fine,
            WindingSel::Coarse => &self.coarse,
        }
    }

    pub fn rotor_inertia_kg_m2(&self) -> f64 {
        self.rotor_inertia_kg_m2
    }

    pub fn torque_nm(&self, sel: WindingSel, direction: Direction, omega_rad_s: f64) -> f64 {
        self.winding(sel).torque_nm(direction, omega_rad_s)
    }
}

/// What the motor model leaves out, shown next to the numbers in the UI.
pub const MOTOR_MODEL_LEAVES_OUT: &[&str] = &[
    "Torque-speed curve is a Kloss fit to the nameplate, not a measured curve",
    "No saturation, deep-bar effects, supply voltage dip or line impedance",
    "No stator current, power factor or winding temperature",
    "Changeover transients (flux and torque spikes) are not modelled; a dead time is",
];
